//! Paste rules: what is unsafe to paste, and how the bytes are framed.

use super::{PasteCheck, PasteMode, UnsafeReason, check, encode};

#[test]
fn a_newline_is_unsafe_only_when_the_paste_is_not_bracketed() {
    assert_eq!(
        check("ls\n", PasteMode::Plain, false),
        PasteCheck::NeedsConfirmation(UnsafeReason::Newline)
    );
    assert_eq!(check("ls\n", PasteMode::Bracketed, false), PasteCheck::Safe);
}

#[test]
fn a_carriage_return_is_a_line_break_in_either_mode() {
    for mode in [PasteMode::Plain, PasteMode::Bracketed] {
        assert_eq!(
            check("rm -rf x\r", mode, false),
            PasteCheck::NeedsConfirmation(UnsafeReason::Newline),
            "{mode:?}"
        );
        assert_eq!(
            check("a\r\nb", mode, false),
            PasteCheck::NeedsConfirmation(UnsafeReason::Newline),
            "{mode:?}"
        );
    }
}

#[test]
fn what_libghostty_flags_is_confirmed_in_every_mode() {
    let flagged = check("a\x1b[201~b", PasteMode::Bracketed, true);
    assert_eq!(
        flagged,
        PasteCheck::NeedsConfirmation(UnsafeReason::EndOfFrame)
    );
    assert_eq!(check("echo hi", PasteMode::Plain, false), PasteCheck::Safe);
}

#[test]
fn paste_frames_and_cleans_bytes() {
    assert_eq!(
        encode("a\x03b", PasteMode::Bracketed),
        b"\x1b[200~a b\x1b[201~".to_vec()
    );
    assert_eq!(encode("a\nb", PasteMode::Plain), b"a\rb".to_vec());
}
