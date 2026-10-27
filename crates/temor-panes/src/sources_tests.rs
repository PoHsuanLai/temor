//! The sources that stand on their own, against a program that stands in for nushell.

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::mpsc::channel;

use super::*;

/// A program that runs `body`, which stands in for `nu`.
fn fake_nu(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("nu");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

fn asked(source: &NushellSource, line: &str) -> Answered {
    let (tx, rx) = channel();
    let deliver: Deliver = Arc::new(move |answered| {
        let _ = tx.send(answered);
    });
    let ask = Ask {
        line: line.to_owned(),
        folder: None,
    };
    source.ask(ask, 7, deliver);
    rx.recv_timeout(Duration::from_secs(10)).expect("an answer")
}

fn source(program: PathBuf, patience: Duration) -> NushellSource {
    NushellSource { program, patience }
}

#[test]
fn nushells_output_becomes_the_suggestions_for_the_request() {
    let dir = tempfile::tempdir().expect("dir");
    // The script it is given is the line, at the offset it is given.
    let nu = fake_nu(
        dir.path(),
        r#"test "$1" = --no-config-file && test "$2" = --ide-complete && test "$3" = 6 && test "$(cat "$4")" = "ls /us" && echo '{"completions": ["/usr/"]}'"#,
    );
    let got = asked(&source(nu, Duration::from_secs(5)), "ls /us");
    assert_eq!((got.request, got.source), (7, NUSHELL));
    assert_eq!(
        got.suggestions,
        vec![Suggestion {
            word: "/usr/".to_owned(),
            from: 3,
            to: 6,
            space: false,
            detail: None
        }]
    );
}

#[test]
fn a_nushell_that_fails_is_late_or_says_nonsense_answers_with_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    // (what, the program's body)
    let cases = [
        ("fails", "exit 3"),
        ("says nonsense", "echo not json"),
        ("is late", "sleep 30"),
    ];
    for (what, body) in cases {
        let nu = fake_nu(dir.path(), body);
        let got = asked(&source(nu, Duration::from_millis(300)), "ls");
        assert!(got.suggestions.is_empty(), "{what}");
    }
    let missing = source(dir.path().join("absent"), Duration::from_secs(1));
    assert!(asked(&missing, "ls").suggestions.is_empty(), "is not there");
}

#[test]
fn only_a_pane_running_nushell_has_nushells_source() {
    let deliver: Deliver = Arc::new(|_| {});
    // (the program, whether nushell's source is there)
    for (program, has) in [
        ("/usr/bin/nu", true),
        ("/opt/homebrew/bin/nu", true),
        ("/bin/bash", false),
        ("/usr/bin/fish", false),
    ] {
        let sources = Sources::for_program(Path::new(program), deliver.clone());
        assert_eq!(sources.list.len(), usize::from(has), "{program}");
        assert_eq!(sources.any_ready(), has, "{program}");
    }
}
