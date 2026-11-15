use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn srv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_srv"))
}

/// Runs a command that must exit on its own; a binary that starts serving
/// instead is killed after the deadline and fails the test.
fn finished(command: &mut Command) -> Output {
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("{command:?} did not exit within 5s");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

struct Running {
    child: Child,
    port: u16,
    stderr: BufReader<std::process::ChildStderr>,
}

impl Running {
    fn start(args: &[&str]) -> Running {
        let mut child = srv()
            .args(["-p", "0", "-b", "127.0.0.1"])
            .args(args)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = BufReader::new(child.stderr.take().unwrap());
        let mut running = Running { child, port: 0, stderr };
        let mut banner = String::new();
        running.stderr.read_line(&mut banner).unwrap();
        running.port = banner
            .trim_end()
            .rsplit_once(" over HTTP on 127.0.0.1:")
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .filter(|&port| port != 0)
            .unwrap_or_else(|| panic!("banner does not name the bound port: {banner}"));
        running
    }

    fn get(&self, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn stop(mut self) -> String {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        let mut rest = String::new();
        self.stderr.read_to_string(&mut rest).unwrap();
        rest
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn failure(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn serves_the_named_directory_and_logs_requests_unless_quiet() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "alpha").unwrap();
    let root = dir.path().to_str().unwrap();

    let logged = Running::start(&[root]);
    let response = logged.get("/a.txt");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.ends_with("\r\n\r\nalpha"), "{response}");
    assert!(logged.stop().contains("GET /a.txt"));

    let quiet = Running::start(&["-q", root]);
    assert!(quiet.get("/a.txt").ends_with("alpha"));
    assert!(!quiet.stop().contains("GET /a.txt"));
}

#[test]
fn a_file_argument_is_served_for_every_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("only.txt");
    std::fs::write(&file, "single").unwrap();
    std::fs::write(dir.path().join("other.txt"), "other").unwrap();

    let running = Running::start(&["-q", file.to_str().unwrap()]);
    assert!(running.get("/other.txt").ends_with("\r\n\r\nsingle"));
    assert!(running.get("/").ends_with("\r\n\r\nsingle"));
    running.stop();
}

#[test]
fn usage_errors_exit_1_with_a_message_and_serve_nothing() {
    for (args, message) in [
        (&["-p"][..], "-p requires a value"),
        (&["-b"][..], "-b requires a value"),
        (&["/nonexistent/srv-cli-test"][..], "/nonexistent/srv-cli-test: "),
        (&["/dev/null"][..], "/dev/null is not a file or directory"),
    ] {
        let stderr = failure(&finished(srv().args(args)));
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn an_occupied_port_exits_1() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let dir = tempfile::tempdir().unwrap();

    let output = finished(srv().args(["-p", &port, dir.path().to_str().unwrap()]));
    assert!(failure(&output).contains(&format!("failed to bind 127.0.0.1:{port}")));
}

#[test]
fn version_prints_the_package_version_and_exits_0() {
    for flag in ["-V", "--version"] {
        let output = finished(srv().arg(flag));
        assert!(output.status.success(), "{flag}: {output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("srv {}\n", env!("CARGO_PKG_VERSION")));
    }
}

#[test]
fn docs_flag_serves_the_docs_browser() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# A").unwrap();

    let running = Running::start(&["-q", "--docs", dir.path().to_str().unwrap()]);
    assert!(running.get("/anything").contains("<title>Docs</title>"));
    assert!(running.get("/__docs__/tree").ends_with(r#"[{"name":"a.md","path":"a.md","dir":false}]}"#));
    running.stop();
}
