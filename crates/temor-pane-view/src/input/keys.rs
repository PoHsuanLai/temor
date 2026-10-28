//! quire's raw key events as a [`Stroke`]: the physical key (from `Code`, falling back to the
//! logical key on a US layout), the held modifiers, and the text the layout produced, plus the
//! same press as chordkit reads it (physical modifiers and the logical key without Shift).

use dioxus::html::{Code, Key, Modifiers as HeldMods};
use ds::edit::raw_key::{KeyPhase, RawKey};
use temor_panes::vt::{KeyAction, KeyCode, KeyInput, Modifiers};

use super::route::Stroke;

/// Builds the stroke for one key event from quire's raw key (the window's `RawKeySurface`),
/// with the input method's state for it. `None` for a key with no physical or known logical
/// identity (a dead or unidentified key): it is ignored rather than sent to the program as
/// something else.
pub fn stroke(raw: &RawKey, composing: bool) -> Option<Stroke> {
    let code = code_key(&raw.code).or_else(|| logical_key(&raw.key))?;
    let action = match raw.phase {
        KeyPhase::Press => KeyAction::Press,
        KeyPhase::Repeat => KeyAction::Repeat,
        KeyPhase::Release => KeyAction::Release,
    };
    // The key without Shift: Shift+1 types `!`, but the key is `1` (the kitty protocol's code for it).
    let unshifted = match (
        raw.modifiers.contains(HeldMods::SHIFT),
        &raw.key,
        code_char(&raw.code),
    ) {
        (true, Key::Character(_), Some(base)) => Some(base),
        _ => logical_char(&raw.key).map(fold),
    };
    let key = KeyInput {
        code,
        action,
        mods: vt_mods(raw.modifiers),
        text: raw.text.clone().unwrap_or_default(),
        unshifted,
        composing,
    };
    Some(Stroke {
        input: pressed(&chord_key(raw), raw.modifiers, &key),
        key,
    })
}

/// A character lowercased, kept as it is when it has no single lowercase form.
fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// The key a chord is made from: the layout's key with no modifier held, so Ctrl+Shift+1 is the
/// chord `1` with Shift in the modifiers, not `!`. A digit or punctuation key under Shift takes
/// its US base character by physical code.
fn chord_key(raw: &RawKey) -> Key {
    let shifted = raw.modifiers.contains(HeldMods::SHIFT);
    match (&raw.key, shifted, code_char(&raw.code)) {
        (Key::Character(_), true, Some(base)) => Key::Character(base.to_string()),
        _ => raw.key.clone(),
    }
}

/// The unshifted US character of a digit or punctuation key.
fn code_char(code: &Code) -> Option<char> {
    Some(match code {
        Code::Digit0 => '0',
        Code::Digit1 => '1',
        Code::Digit2 => '2',
        Code::Digit3 => '3',
        Code::Digit4 => '4',
        Code::Digit5 => '5',
        Code::Digit6 => '6',
        Code::Digit7 => '7',
        Code::Digit8 => '8',
        Code::Digit9 => '9',
        Code::Minus => '-',
        Code::Equal => '=',
        Code::BracketLeft => '[',
        Code::BracketRight => ']',
        Code::Backslash => '\\',
        Code::Semicolon => ';',
        Code::Quote => '\'',
        Code::Comma => ',',
        Code::Period => '.',
        Code::Slash => '/',
        Code::Backquote => '`',
        _ => return None,
    })
}

/// Whether a key is a typed character from AltGr: a press with Ctrl and Alt both held that the
/// layout typed text for. The program gets that text; encoding Ctrl+Alt+key would send a control
/// character instead (AltGr's "@" would be lost).
pub fn is_altgr_text(key: &KeyInput) -> bool {
    matches!(key.action, KeyAction::Press | KeyAction::Repeat)
        && key.mods.contains(Modifiers::CTRL.with(Modifiers::ALT))
        && !key.text.is_empty()
}

/// The held modifiers as the terminal encodes them.
pub fn vt_mods(held: HeldMods) -> Modifiers {
    let mut out = Modifiers::NONE;
    for (flag, mods) in [
        (HeldMods::SHIFT, Modifiers::SHIFT),
        (HeldMods::CONTROL, Modifiers::CTRL),
        (HeldMods::ALT, Modifiers::ALT),
        (HeldMods::SUPER, Modifiers::SUPER),
        (HeldMods::META, Modifiers::SUPER),
        (HeldMods::CAPS_LOCK, Modifiers::CAPS_LOCK),
        (HeldMods::NUM_LOCK, Modifiers::NUM_LOCK),
    ] {
        if held.contains(flag) {
            out = out.with(mods);
        }
    }
    out
}

/// The press as chordkit reads it: the physical modifiers as held (the Meta flag stays Meta; the
/// keymap folds it to the platform's key), the logical key without Shift, and whether AltGr is
/// held, which is the toolkit's `ALT_GRAPH` flag or, where the platform reports AltGr as Ctrl and
/// Alt alone, a press with both held that typed text. `None` for a key chordkit has no name for
/// (a dead key, a keypad key): it goes to the program as it always did.
pub fn pressed(key: &Key, held: HeldMods, typed: &KeyInput) -> Option<chordkit::KeyInput> {
    let mut mods = chordkit::Modifiers::NONE;
    for (flag, modifier) in [
        (HeldMods::CONTROL, chordkit::Modifier::Ctrl),
        (HeldMods::ALT, chordkit::Modifier::Alt),
        (HeldMods::SHIFT, chordkit::Modifier::Shift),
        (HeldMods::SUPER, chordkit::Modifier::Super),
        (HeldMods::META, chordkit::Modifier::Meta),
    ] {
        if held.contains(flag) {
            mods = mods.with(modifier);
        }
    }
    let alt_gr = match held.contains(HeldMods::ALT_GRAPH) || is_altgr_text(typed) {
        true => chordkit::AltGr::Held,
        false => chordkit::AltGr::Released,
    };
    Some(chordkit::KeyInput::new(mods, chordkit_key(key)?).with_alt_gr(alt_gr))
}

/// The key chordkit names for a logical key.
fn chordkit_key(key: &Key) -> Option<chordkit::Key> {
    use chordkit::NamedKey as N;
    let named = |key: N| Some(chordkit::Key::Named(key));
    match key {
        Key::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(' '), None) => named(N::Space),
                (Some(c), None) => Some(chordkit::Key::character(c)),
                _ => None,
            }
        }
        Key::Enter => named(N::Enter),
        Key::Tab => named(N::Tab),
        Key::Escape => named(N::Escape),
        Key::Backspace => named(N::Backspace),
        Key::Delete => named(N::Delete),
        Key::Insert => named(N::Insert),
        Key::Home => named(N::Home),
        Key::End => named(N::End),
        Key::PageUp => named(N::PageUp),
        Key::PageDown => named(N::PageDown),
        Key::ArrowUp => named(N::Up),
        Key::ArrowDown => named(N::Down),
        Key::ArrowLeft => named(N::Left),
        Key::ArrowRight => named(N::Right),
        Key::PrintScreen => named(N::PrintScreen),
        other => function_number(other).and_then(chordkit::Key::function),
    }
}

/// The number of an F1 to F12 key.
fn function_number(key: &Key) -> Option<u8> {
    Some(match key {
        Key::F1 => 1,
        Key::F2 => 2,
        Key::F3 => 3,
        Key::F4 => 4,
        Key::F5 => 5,
        Key::F6 => 6,
        Key::F7 => 7,
        Key::F8 => 8,
        Key::F9 => 9,
        Key::F10 => 10,
        Key::F11 => 11,
        Key::F12 => 12,
        _ => return None,
    })
}

fn logical_char(key: &Key) -> Option<char> {
    match key {
        Key::Character(s) => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(c),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The physical key for a US layout's character, for layouts that report no physical code.
fn logical_key(key: &Key) -> Option<KeyCode> {
    match key {
        Key::Enter => Some(KeyCode::Enter),
        Key::Tab => Some(KeyCode::Tab),
        Key::Escape => Some(KeyCode::Escape),
        Key::Backspace => Some(KeyCode::Backspace),
        Key::Delete => Some(KeyCode::Delete),
        Key::Insert => Some(KeyCode::Insert),
        Key::Home => Some(KeyCode::Home),
        Key::End => Some(KeyCode::End),
        Key::PageUp => Some(KeyCode::PageUp),
        Key::PageDown => Some(KeyCode::PageDown),
        Key::ArrowUp => Some(KeyCode::ArrowUp),
        Key::ArrowDown => Some(KeyCode::ArrowDown),
        Key::ArrowLeft => Some(KeyCode::ArrowLeft),
        Key::ArrowRight => Some(KeyCode::ArrowRight),
        Key::Character(s) => us_char_key(s.chars().next()?),
        _ => None,
    }
}

/// The key a US layout types `c` with.
fn us_char_key(c: char) -> Option<KeyCode> {
    use KeyCode as K;
    let lower = c.to_ascii_lowercase();
    Some(match lower {
        'a' => K::A,
        'b' => K::B,
        'c' => K::C,
        'd' => K::D,
        'e' => K::E,
        'f' => K::F,
        'g' => K::G,
        'h' => K::H,
        'i' => K::I,
        'j' => K::J,
        'k' => K::K,
        'l' => K::L,
        'm' => K::M,
        'n' => K::N,
        'o' => K::O,
        'p' => K::P,
        'q' => K::Q,
        'r' => K::R,
        's' => K::S,
        't' => K::T,
        'u' => K::U,
        'v' => K::V,
        'w' => K::W,
        'x' => K::X,
        'y' => K::Y,
        'z' => K::Z,
        '0' | ')' => K::Digit0,
        '1' | '!' => K::Digit1,
        '2' | '@' => K::Digit2,
        '3' | '#' => K::Digit3,
        '4' | '$' => K::Digit4,
        '5' | '%' => K::Digit5,
        '6' | '^' => K::Digit6,
        '7' | '&' => K::Digit7,
        '8' | '*' => K::Digit8,
        '9' | '(' => K::Digit9,
        '`' | '~' => K::Backquote,
        '-' | '_' => K::Minus,
        '=' | '+' => K::Equal,
        '[' | '{' => K::BracketLeft,
        ']' | '}' => K::BracketRight,
        '\\' | '|' => K::Backslash,
        ';' | ':' => K::Semicolon,
        '\'' | '"' => K::Quote,
        ',' | '<' => K::Comma,
        '.' | '>' => K::Period,
        '/' | '?' => K::Slash,
        ' ' => K::Space,
        _ => return None,
    })
}

/// The physical key for a W3C `code`.
pub fn code_key(code: &Code) -> Option<KeyCode> {
    use KeyCode as K;
    Some(match code {
        Code::KeyA => K::A,
        Code::KeyB => K::B,
        Code::KeyC => K::C,
        Code::KeyD => K::D,
        Code::KeyE => K::E,
        Code::KeyF => K::F,
        Code::KeyG => K::G,
        Code::KeyH => K::H,
        Code::KeyI => K::I,
        Code::KeyJ => K::J,
        Code::KeyK => K::K,
        Code::KeyL => K::L,
        Code::KeyM => K::M,
        Code::KeyN => K::N,
        Code::KeyO => K::O,
        Code::KeyP => K::P,
        Code::KeyQ => K::Q,
        Code::KeyR => K::R,
        Code::KeyS => K::S,
        Code::KeyT => K::T,
        Code::KeyU => K::U,
        Code::KeyV => K::V,
        Code::KeyW => K::W,
        Code::KeyX => K::X,
        Code::KeyY => K::Y,
        Code::KeyZ => K::Z,
        Code::Digit0 => K::Digit0,
        Code::Digit1 => K::Digit1,
        Code::Digit2 => K::Digit2,
        Code::Digit3 => K::Digit3,
        Code::Digit4 => K::Digit4,
        Code::Digit5 => K::Digit5,
        Code::Digit6 => K::Digit6,
        Code::Digit7 => K::Digit7,
        Code::Digit8 => K::Digit8,
        Code::Digit9 => K::Digit9,
        Code::Backquote => K::Backquote,
        Code::Backslash => K::Backslash,
        Code::BracketLeft => K::BracketLeft,
        Code::BracketRight => K::BracketRight,
        Code::Comma => K::Comma,
        Code::Equal => K::Equal,
        Code::Minus => K::Minus,
        Code::Period => K::Period,
        Code::Quote => K::Quote,
        Code::Semicolon => K::Semicolon,
        Code::Slash => K::Slash,
        Code::IntlBackslash => K::IntlBackslash,
        Code::Enter => K::Enter,
        Code::Tab => K::Tab,
        Code::Backspace => K::Backspace,
        Code::Space => K::Space,
        Code::Escape => K::Escape,
        Code::Insert => K::Insert,
        Code::Delete => K::Delete,
        Code::Home => K::Home,
        Code::End => K::End,
        Code::PageUp => K::PageUp,
        Code::PageDown => K::PageDown,
        Code::ArrowUp => K::ArrowUp,
        Code::ArrowDown => K::ArrowDown,
        Code::ArrowLeft => K::ArrowLeft,
        Code::ArrowRight => K::ArrowRight,
        Code::F1 => K::F1,
        Code::F2 => K::F2,
        Code::F3 => K::F3,
        Code::F4 => K::F4,
        Code::F5 => K::F5,
        Code::F6 => K::F6,
        Code::F7 => K::F7,
        Code::F8 => K::F8,
        Code::F9 => K::F9,
        Code::F10 => K::F10,
        Code::F11 => K::F11,
        Code::F12 => K::F12,
        Code::ShiftLeft => K::ShiftLeft,
        Code::ShiftRight => K::ShiftRight,
        Code::ControlLeft => K::ControlLeft,
        Code::ControlRight => K::ControlRight,
        Code::AltLeft => K::AltLeft,
        Code::AltRight => K::AltRight,
        Code::MetaLeft => K::MetaLeft,
        Code::MetaRight => K::MetaRight,
        Code::CapsLock => K::CapsLock,
        Code::NumLock => K::NumLock,
        Code::ContextMenu => K::ContextMenu,
        Code::Numpad0 => K::Numpad0,
        Code::Numpad1 => K::Numpad1,
        Code::Numpad2 => K::Numpad2,
        Code::Numpad3 => K::Numpad3,
        Code::Numpad4 => K::Numpad4,
        Code::Numpad5 => K::Numpad5,
        Code::Numpad6 => K::Numpad6,
        Code::Numpad7 => K::Numpad7,
        Code::Numpad8 => K::Numpad8,
        Code::Numpad9 => K::Numpad9,
        Code::NumpadAdd => K::NumpadAdd,
        Code::NumpadSubtract => K::NumpadSubtract,
        Code::NumpadMultiply => K::NumpadMultiply,
        Code::NumpadDivide => K::NumpadDivide,
        Code::NumpadDecimal => K::NumpadDecimal,
        Code::NumpadEnter => K::NumpadEnter,
        Code::NumpadEqual => K::NumpadEqual,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::html::Location;

    fn raw(key: &str, code: Code, held: HeldMods, text: &str) -> RawKey {
        RawKey {
            phase: KeyPhase::Press,
            key: Key::Character(key.to_owned()),
            code,
            modifiers: held,
            location: Location::Standard,
            text: Some(text.to_owned()),
        }
    }

    #[test]
    fn the_unshifted_key_of_a_shifted_symbol_is_the_key_itself() {
        // (key as typed, code, held, text, unshifted)
        let rows = [
            ("!", Code::Digit1, HeldMods::SHIFT, "!", '1'),
            ("A", Code::KeyA, HeldMods::SHIFT, "A", 'a'),
            ("a", Code::KeyA, HeldMods::empty(), "a", 'a'),
            // An AZERTY `&` on the digit-1 key, no Shift: the layout's own character.
            ("&", Code::Digit1, HeldMods::empty(), "&", '&'),
        ];
        for (key, code, held, text, want) in rows {
            let got = stroke(&raw(key, code, held, text), false).expect("stroke");
            assert_eq!(got.key.unshifted, Some(want), "{key} {code:?}");
        }
    }
}
