//! Key encoding: legacy sequences, application cursor keys, and the kitty keyboard protocol
//! once the program pushes `CSI > 1 u`.

use temor_vt::{KeyAction, KeyCode, KeyEncoder, KeyInput, Modifiers};

use crate::support::term;

fn key(code: KeyCode, mods: Modifiers, text: &str) -> KeyInput {
    KeyInput::press(code, mods, text)
}

/// (case, key, bytes)
fn legacy_cases() -> Vec<(&'static str, KeyInput, &'static [u8])> {
    vec![
        ("letter", key(KeyCode::A, Modifiers::NONE, "a"), b"a"),
        ("ctrl c", key(KeyCode::C, Modifiers::CTRL, "c"), b"\x03"),
        ("enter", key(KeyCode::Enter, Modifiers::NONE, ""), b"\r"),
        (
            "arrow up",
            key(KeyCode::ArrowUp, Modifiers::NONE, ""),
            b"\x1b[A",
        ),
        ("escape", key(KeyCode::Escape, Modifiers::NONE, ""), b"\x1b"),
        (
            "shift arrow",
            key(KeyCode::ArrowRight, Modifiers::SHIFT, ""),
            b"\x1b[1;2C",
        ),
        ("alt letter", key(KeyCode::B, Modifiers::ALT, "b"), b"\x1bb"),
        ("f5", key(KeyCode::F5, Modifiers::NONE, ""), b"\x1b[15~"),
    ]
}

#[test]
fn legacy_keys_encode() {
    let (t, _rx) = term(80, 24);
    let mut enc = KeyEncoder::new().expect("encoder");
    enc.sync(&t);
    for (name, input, want) in legacy_cases() {
        assert_eq!(enc.encode(&input).expect("encode"), want, "case {name}");
    }
}

#[test]
fn application_cursor_keys_follow_the_terminal() {
    let (mut t, _rx) = term(80, 24);
    t.write(b"\x1b[?1h");
    let mut enc = KeyEncoder::new().expect("encoder");
    enc.sync(&t);
    assert_eq!(
        enc.encode(&key(KeyCode::ArrowUp, Modifiers::NONE, ""))
            .expect("encode"),
        b"\x1bOA"
    );
}

#[test]
fn kitty_keyboard_protocol_encodes_after_the_program_asks() {
    let (mut t, _rx) = term(80, 24);
    t.write(b"\x1b[>1u");
    assert_eq!(
        t.kitty_keyboard_flags(),
        temor_vt::KittyKeyFlags::DISAMBIGUATE
    );
    let mut enc = KeyEncoder::new().expect("encoder");
    enc.sync(&t);
    let cases: [(&str, KeyInput, &[u8]); 4] = [
        (
            "ctrl c",
            key(KeyCode::C, Modifiers::CTRL, "c"),
            b"\x1b[99;5u",
        ),
        (
            "escape",
            key(KeyCode::Escape, Modifiers::NONE, ""),
            b"\x1b[27u",
        ),
        ("plain letter", key(KeyCode::A, Modifiers::NONE, "a"), b"a"),
        (
            "ctrl shift a",
            key(KeyCode::A, Modifiers::CTRL.with(Modifiers::SHIFT), "a"),
            b"\x1b[97;6u",
        ),
    ];
    for (name, input, want) in cases {
        assert_eq!(enc.encode(&input).expect("encode"), want, "case {name}");
    }
    let release = KeyInput {
        action: KeyAction::Release,
        ..key(KeyCode::C, Modifiers::CTRL, "c")
    };
    assert_eq!(
        enc.encode(&release).expect("encode"),
        b"",
        "releases are not reported at flag 1"
    );
}
