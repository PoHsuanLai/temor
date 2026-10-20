use super::{CommandLine, agent_of, is_known_agent};

fn line(tokens: &[&str]) -> CommandLine {
    let bytes: Vec<u8> = tokens.iter().flat_map(|t| t.bytes().chain([0])).collect();
    CommandLine::from_cmdline(&bytes).expect("a command line")
}

#[test]
fn an_agent_is_known_by_its_name_or_by_a_program_in_its_command_line() {
    let wrapped = line(&[
        "/usr/bin/env",
        "-i",
        "A=b",
        "/usr/bin/bwrap",
        "--",
        "claude",
        "--resume",
        "x",
    ]);
    let by_path = line(&["node", "/opt/x/codex"]);
    let plain = line(&["zsh", "-l"]);
    let cases: Vec<(&str, &str, Option<&CommandLine>, Option<&str>)> = vec![
        ("by name", "claude", None, Some("claude")),
        ("by name, any case", "Claude", None, Some("claude")),
        ("a sandbox wrapper", "bwrap", Some(&wrapped), Some("claude")),
        (
            "a path in the arguments",
            "node",
            Some(&by_path),
            Some("codex"),
        ),
        ("a shell", "zsh", Some(&plain), None),
        ("a shell with no command line", "zsh", None, None),
        ("a partial name is not a match", "claude-code", None, None),
    ];
    for (name, comm, command, want) in cases {
        let got = agent_of(comm, command);
        assert_eq!(got.as_ref().map(|a| a.as_str()), want, "{name}");
    }
}

#[test]
fn a_report_names_a_known_agent_only_by_its_exact_name() {
    for (name, want) in [
        ("claude", true),
        ("Codex", true),
        ("cursor-agent", true),
        ("claude-code", false),
        ("make", false),
        ("", false),
    ] {
        assert_eq!(is_known_agent(name), want, "{name:?}");
    }
}
