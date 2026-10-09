//! quire's raw key events as a [`Stroke`]: the physical key (from `Code`, falling back to the
//! logical key on a US layout), the held modifiers, and the text the layout produced.

use dioxus::html::{Code, Key, Modifiers as HeldMods};
use ds::edit::raw_key::{KeyPhase, RawKey};
use temor_pane::keys::{Chord, ChordKey, Mods, NamedKey, fold};
use temor_vt::{Ime, KeyAction, KeyCode, KeyInput, Modifiers};

use super::route::Stroke;

/// Builds the stroke for one key event from quire's raw key (the window's `RawKeySurface`),
/// with the input method's state for it. `None` for a key with no physical or known logical
/// identity (a dead or unidentified key): it is ignored rather than sent to the program as
/// something else.
pub fn stroke(raw: &RawKey, ime: Ime) -> Option<Stroke> {
    let mods = core_mods(raw.modifiers);
    let code = code_key(&raw.code).or_else(|| logical_key(&raw.key))?;
    let action = match raw.phase {
        KeyPhase::Press => KeyAction::Press,
        KeyPhase::Repeat => KeyAction::Repeat,
        KeyPhase::Release => KeyAction::Release,
    };
    let unshifted = raw
        .unshifted
        .as_ref()
        .or(Some(&raw.key))
        .and_then(logical_char)
        .map(fold);
    let key = KeyInput {
        code,
        action,
        mods: vt_mods(raw.modifiers),
        text: raw.text.clone().unwrap_or_default(),
        unshifted,
        ime,
    };
    Some(Stroke {
        chord: chord_of(&raw.key, mods),
        key,
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

/// The held modifiers as the keymap sees them. Meta counts as Super (PLAN §6).
pub fn core_mods(held: HeldMods) -> Mods {
    let mut out = Mods::NONE;
    for (flag, mods) in [
        (HeldMods::CONTROL, Mods::CTRL),
        (HeldMods::SHIFT, Mods::SHIFT),
        (HeldMods::ALT, Mods::ALT),
        (HeldMods::SUPER, Mods::SUPER),
        (HeldMods::META, Mods::SUPER),
    ] {
        if held.contains(flag) {
            out = out.with(mods);
        }
    }
    out
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

/// The chord a logical key makes with the held modifiers: a character (folded to lowercase) or
/// a named key.
pub fn chord_of(key: &Key, mods: Mods) -> Option<Chord> {
    let key = match key {
        Key::Character(s) => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => ChordKey::Char(fold(c)),
                _ => return None,
            }
        }
        other => ChordKey::Named(named(other)?),
    };
    Some(Chord { key, mods })
}

fn named(key: &Key) -> Option<NamedKey> {
    Some(match key {
        Key::Enter => NamedKey::Enter,
        Key::Tab => NamedKey::Tab,
        Key::Escape => NamedKey::Escape,
        Key::Backspace => NamedKey::Backspace,
        Key::Delete => NamedKey::Delete,
        Key::Insert => NamedKey::Insert,
        Key::Home => NamedKey::Home,
        Key::End => NamedKey::End,
        Key::PageUp => NamedKey::PageUp,
        Key::PageDown => NamedKey::PageDown,
        Key::ArrowUp => NamedKey::ArrowUp,
        Key::ArrowDown => NamedKey::ArrowDown,
        Key::ArrowLeft => NamedKey::ArrowLeft,
        Key::ArrowRight => NamedKey::ArrowRight,
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
