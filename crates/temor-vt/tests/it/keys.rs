//! Key encoding: legacy sequences, application cursor keys, and the kitty keyboard protocol
//! once the program pushes `CSI > 1 u`.

use temor_vt::{KeyAction, KeyCode, KeyEncoder, KeyInput, Modifiers};

use crate::support::term;

fn key(code: KeyCode, mods: Modifiers, text: &str) -> KeyInput {
    KeyInput::press(code, mods, text)
}

/// Alt+B. On macOS Ghostty's encoder types Option+letter as the letter, the system's way, unless
/// option-as-alt is set (`KeyEncoder::set_option_as_alt`; see `option_as_alt_makes_option_an_alt`).
#[cfg(not(target_os = "macos"))]
const ALT_B: &[u8] = b"\x1bb";
#[cfg(target_os = "macos")]
const ALT_B: &[u8] = b"b";

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
        ("alt letter", key(KeyCode::B, Modifiers::ALT, "b"), ALT_B),
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

/// On macOS, Option as Alt sends the escape prefix, and the choice survives `sync`, which resets it.
#[cfg(target_os = "macos")]
#[test]
fn option_as_alt_makes_option_an_alt_and_survives_sync() {
    use temor_vt::OptionAsAlt;
    let (t, _rx) = term(80, 24);
    let mut enc = KeyEncoder::new().expect("encoder");
    let alt_b = key(KeyCode::B, Modifiers::ALT, "b");
    enc.sync(&t);
    assert_eq!(enc.encode(&alt_b).expect("encode"), b"b");
    enc.set_option_as_alt(OptionAsAlt::Both);
    assert_eq!(enc.encode(&alt_b).expect("encode"), b"\x1bb");
    enc.sync(&t);
    assert_eq!(enc.encode(&alt_b).expect("encode"), b"\x1bb");
    enc.set_option_as_alt(OptionAsAlt::Off);
    assert_eq!(enc.encode(&alt_b).expect("encode"), b"b");
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
