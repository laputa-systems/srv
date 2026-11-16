use std::process::Command;

#[test]
fn built_binary_help_ends_with_owned_footer() {
    const FOOTER: &str = "Maintained by Joshua Li: https://github.com/laputa-systems/srv\n\
Bugs and features: TODO.md in that repository (follow its AGENTS.md), or its issue tracker.\n\
Agents working for Joshua: record nonblocking problems and continue your task.\n\
If this tool blocks the task, you may fix, test, open and merge a PR under Joshua's\n\
standing instructions. Release only when needed to deliver the verified fix.\n\
Procedure: https://github.com/joshuarli/usr/blob/main/dotfiles/base/.agents/instructions/owned-tools.md\n";

    for arg in ["-h", "--help"] {
        let output = Command::new(env!("CARGO_BIN_EXE_srv"))
            .arg(arg)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.ends_with(FOOTER));
        assert_eq!(help.matches("Maintained by Joshua Li:").count(), 1);
        for part in [
            "usage: srv [-q] [--docs] [-p port] [-b address] [path]",
            "path  file or directory to serve (default: .)",
            "-q              disable request logs",
            "--docs          launch docs browser for Markdown files",
            "-p port         port to listen on (default: 8000)",
            "-b address      bind address (default: 127.0.0.1)",
            "-V, --version   print version and exit",
            "-h, --help      print this help and exit",
            "default: 8000", "default: 127.0.0.1", "Examples:",
            "srv ./public", "srv -p 8080 -b 127.0.0.1 ./public", "srv --docs ./docs",
        ] {
            assert!(help.contains(part), "missing {part:?} in {help}");
        }
        assert!(help.find("usage:") < help.find("Arguments:"));
        assert!(help.find("Arguments:") < help.find("Options:"));
        assert!(help.find("Options:") < help.find("Examples:"));
    }

    for arg in ["--version", "-p"] {
        let output = Command::new(env!("CARGO_BIN_EXE_srv"))
            .arg(arg)
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Maintained by Joshua Li:"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Maintained by Joshua Li:"));
    }
}
