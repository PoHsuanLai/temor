use super::{CommandLine, agent_of, is_known_agent};

fn line(tokens: &[&str]) -> CommandLine {
    let bytes: Vec<u8> = tokens.iter().flat_map(|t| t.bytes().chain([0])).collect();
    CommandLine::from_cmdline(&bytes).expect("a command line")
}

/// A row: its name, the process name, the command line, and the agent and its program word wanted.
type Case<'a> = (
    &'a str,
    &'a str,
    Option<&'a CommandLine>,
    Option<(&'a str, usize)>,
);

#[test]
fn an_agent_is_known_by_its_name_or_by_the_program_its_command_line_runs() {
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
    let handed_over = line(&[
        "/usr/bin/env",
        "-i",
        "A=b",
        "/usr/bin/bwrap",
        "--die-with-parent",
        "--ro-bind",
        "/usr",
        "/usr",
        "--setenv",
        "HOME",
        "/h",
        "--",
        "/usr/bin/claude",
        "--resume",
        "x",
    ]);
    let by_script = line(&["node", "/opt/x/codex"]);
    let package = line(&[
        "node",
        "/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js",
    ]);
    let evaluated = line(&["node", "-e", "claude"]);
    let plain = line(&["zsh", "-l"]);
    let search = line(&["rg", "claude"]);
    let editor = line(&["vim", "amp"]);
    let manual = line(&["man", "codex"]);
    let which = line(&["which", "goose"]);
    let git = line(&["git", "checkout", "crush"]);
    let unset = line(&["env", "-u", "claude", "zsh"]);
    let chdir = line(&["bwrap", "--chdir", "codex", "--", "zsh"]);
    let bound = line(&["bwrap", "--ro-bind", "/a", "/claude", "--", "zsh"]);
    #[rustfmt::skip]
    let cases: Vec<Case> = vec![
        ("by name", "claude", None, Some(("claude", 0))),
        ("by name, any case", "Claude", None, Some(("claude", 0))),
        ("a sandbox wrapper", "bwrap", Some(&wrapped), Some(("claude", 5))),
        ("the handed-over line", "bwrap", Some(&handed_over), Some(("claude", 12))),
        ("a node script", "node", Some(&by_script), Some(("codex", 0))),
        ("the Anthropic package", "node", Some(&package), Some(("claude", 0))),
        ("node running code", "node", Some(&evaluated), None),
        ("a shell", "zsh", Some(&plain), None),
        ("a shell with no command line", "zsh", None, None),
        ("a partial name is not a match", "claude-code", None, None),
        ("an agent name searched for", "rg", Some(&search), None),
        ("an agent name edited", "vim", Some(&editor), None),
        ("an agent name looked up", "man", Some(&manual), None),
        ("an agent name located", "which", Some(&which), None),
        ("an agent name in git", "git", Some(&git), None),
        ("an env option argument", "env", Some(&unset), None),
        ("a bwrap option argument", "bwrap", Some(&chdir), None),
        ("a bwrap bind target", "bwrap", Some(&bound), None),
    ];
    for (name, comm, command, want) in cases {
        let got = agent_of(comm, command);
        let got = got.as_ref().map(|s| (s.agent.as_str(), s.program_at));
        assert_eq!(got, want, "{name}");
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
