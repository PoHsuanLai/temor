use super::wrapper::{node_script, program_at};

fn words(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

#[test]
fn a_wrapper_is_read_by_its_own_options_to_the_program_word() {
    #[rustfmt::skip]
    let cases = [
        ("no wrapper", "zsh -l", Some(0)),
        ("a path", "/usr/bin/zsh -l", Some(0)),
        ("env plain", "env zsh", Some(1)),
        ("env flags and assignments", "env -i -0 A=b C=d zsh", Some(5)),
        ("env unset", "env -u A --unset B --unset=C zsh", Some(6)),
        ("env chdir", "env -C /x --chdir /y --chdir=/z zsh", Some(6)),
        ("env double dash", "env A=b -- zsh", Some(3)),
        ("env split string", "env -S a_b zsh", None),
        ("env unknown option", "env --frob zsh", None),
        ("env with no program", "env -i A=b", None),
        ("bwrap flags", "bwrap --unshare-all --die-with-parent zsh", Some(3)),
        ("bwrap one argument", "bwrap --chdir /x --hostname h zsh", Some(5)),
        ("bwrap two arguments", "bwrap --ro-bind /a /b --setenv K V zsh", Some(7)),
        ("bwrap three arguments", "bwrap --overlay a b c zsh", Some(5)),
        ("bwrap double dash", "bwrap --unshare-net -- zsh", Some(3)),
        ("bwrap unknown option", "bwrap --frob zsh", None),
        ("bwrap short option", "bwrap -x zsh", None),
        ("bwrap missing arguments", "bwrap --ro-bind /a", None),
        ("nice -n", "nice -n 5 zsh", Some(3)),
        ("nice attached", "nice -n5 zsh", Some(2)),
        ("nice digits", "nice -10 zsh", Some(2)),
        ("nice long", "nice --adjustment=5 zsh", Some(2)),
        ("nice long separate", "nice --adjustment 5 zsh", Some(3)),
        ("nohup", "nohup zsh", Some(1)),
        ("nohup unknown option", "nohup -x zsh", None),
        ("setsid flags", "setsid -f --wait zsh", Some(3)),
        ("setsid unknown option", "setsid -x zsh", None),
        ("stdbuf separate", "stdbuf -i L -o0 --error=L zsh", Some(5)),
        ("stdbuf long separate", "stdbuf --output L zsh", Some(3)),
        ("timeout duration", "timeout 5 zsh", Some(2)),
        ("timeout options", "timeout -s KILL -k 1 --foreground --preserve-status 5 zsh", Some(8)),
        ("timeout long values", "timeout --signal=KILL --kill-after=1 5 zsh", Some(4)),
        ("timeout double dash", "timeout -- 5 zsh", Some(3)),
        ("timeout with no program", "timeout 5", None),
        ("wrappers nest", "nice env -i bwrap --unshare-all -- timeout 9 zsh", Some(8)),
        ("a wrapper with nothing", "env", None),
    ];
    for (name, line, want) in cases {
        assert_eq!(program_at(&words(line)), want, "{name}");
    }
}

#[test]
fn node_runs_the_first_argument_that_is_not_an_option() {
    #[rustfmt::skip]
    let cases = [
        ("a script", "node /x/cli.js --a", Some("/x/cli.js")),
        ("after options", "node --no-warnings -r pre /x/cli.js", Some("/x/cli.js")),
        ("after a double dash", "node -- /x/cli.js", Some("/x/cli.js")),
        ("code given", "node -e code", None),
        ("a repl", "node", None),
    ];
    for (name, line, want) in cases {
        assert_eq!(node_script(&words(line), 0), want, "{name}");
    }
}
