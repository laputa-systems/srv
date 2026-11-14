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
        assert!(String::from_utf8(output.stderr).unwrap().ends_with(FOOTER));
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
