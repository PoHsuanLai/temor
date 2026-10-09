//! Key chords: a key and the modifiers held with it, parsed from settings text ("ctrl+shift+c")
//! and labelled for the person (the desktop key rule, PLAN §6: keycap delivers ⌘ as Ctrl+Shift).

use crate::error::SettingsError;

/// Held modifiers as a bit set. Not a struct of bools (CONVENTIONS §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods(u8);

impl Mods {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);
    pub const SUPER: Self = Self(8);

    /// These modifiers plus `other`.
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every modifier in `other` is held.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// The named keys a chord may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Enter,
    Tab,
    Escape,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

/// The key of a chord. Letters are lowercase; a named key is never a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChordKey {
    Char(char),
    Named(NamedKey),
}

/// A key with the modifiers held down with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: ChordKey,
    pub mods: Mods,
}

impl Chord {
    /// A chord from a key event: a character is folded to lowercase, so Shift+C and c match
    /// the same binding once the Shift is in `mods`.
    pub fn of_char(c: char, mods: Mods) -> Self {
        Self {
            key: ChordKey::Char(fold(c)),
            mods,
        }
    }

    /// Parses `ctrl+shift+c` style text. Modifier names: ctrl, shift, alt (option), super
    /// (cmd, meta). The last part is the key. Names are case-insensitive.
    pub fn parse(text: &str) -> Result<Self, SettingsError> {
        let bad = || SettingsError::BadChord(text.to_owned());
        let parts: Vec<String> = text.split('+').map(|p| p.trim().to_lowercase()).collect();
        let (key_part, mod_parts) = parts.split_last().ok_or_else(bad)?;
        let mut mods = Mods::NONE;
        for part in mod_parts {
            mods = mods.with(match part.as_str() {
                "ctrl" | "control" => Mods::CTRL,
                "shift" => Mods::SHIFT,
                "alt" | "option" => Mods::ALT,
                "super" | "cmd" | "meta" => Mods::SUPER,
                _ => return Err(bad()),
            });
        }
        let key = parse_key(key_part).ok_or_else(bad)?;
        Ok(Self { key, mods })
    }

    /// The label the person sees (PLAN §6): Ctrl+Shift+letter is shown as ⌘ + letter, the
    /// chord the desktop delivers for ⌘; other chords use the Mac symbols in ⌃⌥⇧⌘ order.
    pub fn label(&self) -> String {
        let key = key_label(self.key);
        if self.mods == Mods::CTRL.with(Mods::SHIFT) {
            return format!("⌘{key}");
        }
        let mut out = String::new();
        for (flag, symbol) in [
            (Mods::CTRL, "⌃"),
            (Mods::ALT, "⌥"),
            (Mods::SHIFT, "⇧"),
            (Mods::SUPER, "⌘"),
        ] {
            if self.mods.contains(flag) {
                out.push_str(symbol);
            }
        }
        out.push_str(&key);
        out
    }

    /// Chords the desktop keeps (PLAN §6): anything with Super (the shell and compositor's),
    /// and Ctrl+Space (the input method's). A person's keymap may not bind them.
    pub fn is_reserved(&self) -> bool {
        self.mods.contains(Mods::SUPER)
            || (self.mods.contains(Mods::CTRL) && self.key == ChordKey::Named(NamedKey::Space))
            || (self.mods.contains(Mods::CTRL) && self.key == ChordKey::Char(' '))
    }
}

/// Lowercases a character, keeping it when it has no single lowercase form.
pub fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

fn parse_key(part: &str) -> Option<ChordKey> {
    let mut chars = part.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() => return Some(ChordKey::Char(fold(c))),
        (None, _) => return None,
        _ => {}
    }
    let named = match part {
        "enter" | "return" => NamedKey::Enter,
        "tab" => NamedKey::Tab,
        "escape" | "esc" => NamedKey::Escape,
        "space" => NamedKey::Space,
        "backspace" => NamedKey::Backspace,
        "delete" => NamedKey::Delete,
        "insert" => NamedKey::Insert,
        "home" => NamedKey::Home,
        "end" => NamedKey::End,
        "pageup" => NamedKey::PageUp,
        "pagedown" => NamedKey::PageDown,
        "up" => NamedKey::ArrowUp,
        "down" => NamedKey::ArrowDown,
        "left" => NamedKey::ArrowLeft,
        "right" => NamedKey::ArrowRight,
        _ => return None,
    };
    Some(ChordKey::Named(named))
}

fn key_label(key: ChordKey) -> String {
    match key {
        ChordKey::Char(c) => c.to_uppercase().collect(),
        ChordKey::Named(named) => match named {
            NamedKey::Enter => "↩".to_owned(),
            NamedKey::Tab => "⇥".to_owned(),
            NamedKey::Escape => "⎋".to_owned(),
            NamedKey::Space => "Space".to_owned(),
            NamedKey::Backspace => "⌫".to_owned(),
            NamedKey::Delete => "⌦".to_owned(),
            NamedKey::Insert => "Insert".to_owned(),
            NamedKey::Home => "Home".to_owned(),
            NamedKey::End => "End".to_owned(),
            NamedKey::PageUp => "Page Up".to_owned(),
            NamedKey::PageDown => "Page Down".to_owned(),
            NamedKey::ArrowUp => "↑".to_owned(),
            NamedKey::ArrowDown => "↓".to_owned(),
            NamedKey::ArrowLeft => "←".to_owned(),
            NamedKey::ArrowRight => "→".to_owned(),
        },
    }
}
