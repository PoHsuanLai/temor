//! Resizing reaches the program (`TIOCSWINSZ`).

use temor_pty::{Cols, GridSize, Px, Rows};

use crate::support::{Scratch, Term, bash_interactive, setsid};

#[test]
fn stty_sees_the_new_size() {
    let scratch = Scratch::new();
    let mut term = Term::spawn(&bash_interactive(&scratch), &setsid());
    assert!(term.read_until_has(0, "$ "), "no prompt: {:?}", term.text());
    let start = term.out.len();
    term.send("stty size\r");
    assert!(term.read_until_has(start, "24 80"), "{:?}", term.text());
    term.pty
        .resize(GridSize {
            cols: Cols(101),
            rows: Rows(33),
            width: Px(909),
            height: Px(594),
        })
        .expect("resize");
    let start = term.out.len();
    term.send("stty size\r");
    assert!(term.read_until_has(start, "33 101"), "{:?}", term.text());
}
