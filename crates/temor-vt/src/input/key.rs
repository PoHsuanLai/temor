//! Keys: physical key codes (layout independent), modifiers and the text a key produced.

/// A physical key, named after its position on a US layout (W3C UI Events `code`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Equal,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    IntlBackslash,
    Enter,
    Tab,
    Backspace,
    Space,
    Escape,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    MetaLeft,
    MetaRight,
    CapsLock,
    NumLock,
    ContextMenu,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    NumpadEnter,
    NumpadEqual,
}

/// Press, auto-repeat or release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyAction {
    #[default]
    Press,
    Repeat,
    Release,
}

/// Held modifiers, as a bit set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u16);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    pub const CAPS_LOCK: Self = Self(1 << 4);
    pub const NUM_LOCK: Self = Self(1 << 5);

    /// These modifiers plus `other`.
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every modifier in `other` is held.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The bits, laid out as libghostty's `GhosttyMods`.
    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    pub(crate) const fn bits(self) -> u16 {
        self.0
    }
}

/// Kitty keyboard protocol flags (`CSI > flags u`), as a bit set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KittyKeyFlags(u8);

impl KittyKeyFlags {
    pub const DISABLED: Self = Self(0);
    pub const DISAMBIGUATE: Self = Self(1 << 0);
    pub const REPORT_EVENTS: Self = Self(1 << 1);
    pub const REPORT_ALTERNATES: Self = Self(1 << 2);
    pub const REPORT_ALL: Self = Self(1 << 3);
    pub const REPORT_ASSOCIATED: Self = Self(1 << 4);

    /// These flags plus `other`.
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The flags from their protocol bits.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x1f)
    }

    /// The protocol bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// Whether macOS's Option key acts as Alt (sends an escape prefix, so Option+B moves back a word)
/// or types what the layout makes of it (`∫`). Other systems have no such key and ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OptionAsAlt {
    /// Option types what the layout makes of it.
    #[default]
    Off,
    /// Both Option keys are Alt.
    Both,
    /// Only the left Option key is Alt.
    Left,
    /// Only the right Option key is Alt.
    Right,
}

/// One key event for the program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyInput {
    pub code: KeyCode,
    pub action: KeyAction,
    pub mods: Modifiers,
    /// The text the layout produced, before Ctrl/Alt changed it; empty for keys without text
    /// (arrows, function keys) and never a control character.
    pub text: String,
    /// The key's character with no modifiers on the current layout, when it has one.
    pub unshifted: Option<char>,
    /// An input method is composing (pre-edit) while the key arrives.
    pub composing: bool,
}

impl KeyInput {
    /// A press of `code` with `mods` and the layout's `text`.
    pub fn press(code: KeyCode, mods: Modifiers, text: &str) -> Self {
        Self {
            code,
            action: KeyAction::Press,
            mods,
            text: text.to_owned(),
            unshifted: text.chars().next().filter(|_| text.chars().count() == 1),
            composing: false,
        }
    }
}
