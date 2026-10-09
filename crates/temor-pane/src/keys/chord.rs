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

    /// These modifiers without `other`.
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
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

    /// The label the person sees (PLAN §6): a ⌘ chord is the Ctrl+Shift form the desktop
    /// delivers for ⌘ or the Super form that arrives unremapped; both show ⌘. Other chords use
    /// the Mac symbols in ⌃⌥⇧⌘ order.
    pub fn label(&self) -> String {
        // The Ctrl+Shift table form stands for ⌘ alone; a Super chord keeps its real Shift.
        let (command, mods) = match self.is_ctrl_shift_form() {
            true => (true, self.mods.without(Mods::CTRL).without(Mods::SHIFT)),
            false => (
                self.mods.contains(Mods::SUPER),
                self.mods.without(Mods::SUPER),
            ),
        };
        let mut out = String::new();
        for (flag, symbol) in [(Mods::CTRL, "⌃"), (Mods::ALT, "⌥"), (Mods::SHIFT, "⇧")] {
            if mods.contains(flag) {
                out.push_str(symbol);
            }
        }
        if command {
            out.push('⌘');
        }
        out.push_str(&key_label(self.key));
        out
    }

    /// A ⌘ chord as the Super key sends it: Super without Ctrl (PLAN §6, the terminal rule).
    /// Never quire's `is_command`, which is true for a plain Ctrl chord too.
    pub fn is_command_form(&self) -> bool {
        self.mods.contains(Mods::SUPER) && !self.mods.contains(Mods::CTRL)
    }

    /// The Ctrl+Shift form of a ⌘ chord, the one keycap sends for ⌘ (PLAN §6).
    pub fn is_ctrl_shift_form(&self) -> bool {
        self.mods.contains(Mods::CTRL.with(Mods::SHIFT))
    }

    /// The refusal for a bare Ctrl chord (Ctrl with a letter, digit or punctuation, and no Shift or
    /// Alt): it belongs to the program, and the refusal names the ⌘ forms to bind instead.
    pub fn bare_ctrl_refusal(&self) -> Option<SettingsError> {
        let bare = self.mods == Mods::CTRL;
        match self.key {
            ChordKey::Char(c) if bare => {
                let key = c.to_uppercase().collect::<String>();
                Some(SettingsError::ProgramChord {
                    chord: format!("Ctrl+{key}"),
                    ctrl_shift: format!("Ctrl+Shift+{key}"),
                    command: format!("⌘{key}"),
                })
            }
            _ => None,
        }
    }

    /// Chords the desktop keeps (PLAN §6): Super with Space, Tab or a digit (the shell's and
    /// compositor's), and Ctrl+Space (the input method's). Other Super chords are the app's ⌘
    /// chords, and a person's keymap may bind them.
    pub fn is_reserved(&self) -> bool {
        let ctrl_space = self.mods.contains(Mods::CTRL)
            && matches!(
                self.key,
                ChordKey::Named(NamedKey::Space) | ChordKey::Char(' ')
            );
        let shell_super = self.mods.contains(Mods::SUPER)
            && matches!(
                self.key,
                ChordKey::Named(NamedKey::Space | NamedKey::Tab) | ChordKey::Char('0'..='9')
            );
        ctrl_space || shell_super
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
