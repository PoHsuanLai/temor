//! A program's clipboard reads (OSC 52 with `?`) are answered from the pane's copy, and only
//! once the person has allowed them.

use temor_pane::{ReadPolicy, Term};
use temor_render::Palette;
use temor_vt::{CellPx, CursorShape, GridSize, LineCount, VtEvent};

fn term() -> Term {
    Term::new(
        GridSize::new(40, 5),
        CellPx::new(8, 16),
        LineCount(10),
        CursorShape::Block,
        &Palette::default(),
    )
    .expect("terminal")
}

#[test]
fn a_read_waits_for_the_person_then_is_served_from_the_copy() {
    // (policy set before the read, copy held, the reply carries "Hello", the window is asked)
    let table = [
        (ReadPolicy::Ask, Some("Hello"), false, true),
        (ReadPolicy::Refused, Some("Hello"), false, false),
        (ReadPolicy::Allowed, Some("Hello"), true, false),
        (ReadPolicy::Allowed, None, false, false),
    ];
    for (policy, copy, replied, asked) in table {
        let mut term = term();
        let mirror = term.clipboard();
        mirror.set_policy(policy);
        mirror.set_text(copy.map(str::to_owned));
        let out = term.write(b"\x1b]52;c;?\x07");
        let got = out
            .replies
            .iter()
            .any(|r| r.windows(8).any(|w| w == b"SGVsbG8="));
        assert_eq!(got, replied, "{policy:?} {copy:?}");
        let ask = out
            .events
            .iter()
            .any(|e| matches!(e, VtEvent::ClipboardRead { .. }));
        assert_eq!(ask, asked, "{policy:?} {copy:?}");
    }
}
