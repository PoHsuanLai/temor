//! The terminal's line discipline settings.

use crate::support::{Scratch, Term, bash_interactive, setsid};

#[test]
fn input_is_marked_utf8() {
    if crate::support::pty_unsupported("input_is_marked_utf8") {
        return;
    }
    let scratch = Scratch::new();
    let mut term = Term::spawn(&bash_interactive(&scratch), &setsid());
    assert!(term.read_until_has(0, "$ "), "no prompt: {:?}", term.text());
    let start = term.out.len();
    term.send("stty -a; echo stty-$((1+1))\r");
    assert!(term.read_until_has(start, "stty-2"), "{:?}", term.text());
    let text = String::from_utf8_lossy(&term.out[start..]).into_owned();
    assert!(
        text.contains(" iutf8") && !text.contains("-iutf8"),
        "IUTF8 is off: {text:?}"
    );
}
