//! The exact remote command run through each outer login shell a person might use:
//! `<shell> -c "<remote>"`. Every shell must start the bootstrap the same way, and a payload
//! with backslashes and quotes must reach `sh` byte for byte.

use std::path::PathBuf;

use temor_ssh::wrap_script;

use crate::harness::{Launch, contains, host_name, require, scratch};

/// Outer shells to try. A missing one is a printed skip, or a failure under TEMOR_REQUIRE_TOOLS=1.
const OUTER: [&str; 6] = ["sh", "bash", "dash", "zsh", "fish", "nu"];

/// Single quotes, double quotes with escapes, and a backslash-quote run, all in one script.
const PAYLOAD: &str = r#"printf '%s|%s|%s\n' 'b\s"l' "d\"q\\" 'x'"'"'y'; printf 'end\n'"#;
const PAYLOAD_OUT: &[u8] = b"b\\s\"l|d\"q\\|x'y\nend\n";

fn outer_shells() -> Vec<(&'static str, PathBuf)> {
    OUTER
        .iter()
        .filter_map(|name| require(name).map(|path| (*name, path)))
        .collect()
}

#[test]
fn every_outer_login_shell_starts_the_bootstrap_the_same_way() {
    let host = host_name();
    let mut seen: Vec<(&str, String)> = Vec::new();
    for (name, outer) in outer_shells() {
        let s = scratch();
        let mut launch = Launch::new(&s);
        launch.outer = outer;
        launch.input = "printf 'T=%s\\n' \"$TERM\"\nexit\n".to_owned();
        let ran = launch.run();
        let text = String::from_utf8_lossy(&ran.text).into_owned();
        assert!(ran.status.success(), "{name}: {:?}: {text}", ran.status);
        let osc = format!("\x1b]7;file://{host}{}\x1b\\", s.work.display());
        assert!(
            contains(&ran.text, &osc),
            "{name}: no exact OSC 7 in {text:?}"
        );
        assert!(
            contains(&ran.text, "T=xterm-"),
            "{name}: TERM not set: {text:?}"
        );
        assert_eq!(
            std::fs::read_dir(&s.tmp).expect("tmp").count(),
            0,
            "{name}: temp dir left"
        );
        // Each run has its own scratch directory; compare with that path masked out.
        let masked = text.replace(&s.work.display().to_string(), "<work>");
        seen.push((name, masked));
    }
    if let Some((first_name, first)) = seen.first() {
        for (name, text) in &seen[1..] {
            assert_eq!(text, first, "{name} printed differently from {first_name}");
        }
    }
}

#[test]
fn a_payload_with_backslashes_and_quotes_survives_every_outer_shell() {
    let wrapped = wrap_script(PAYLOAD).expect("payload fits");
    for (name, outer) in outer_shells() {
        let s = scratch();
        let mut launch = Launch::new(&s);
        launch.outer = outer;
        launch.command = wrapped.as_str().to_owned();
        launch.input = String::new();
        let ran = launch.run();
        assert!(ran.status.success(), "{name}: {:?}", ran.status);
        assert_eq!(ran.text, PAYLOAD_OUT, "{name} altered the payload");
    }
}
