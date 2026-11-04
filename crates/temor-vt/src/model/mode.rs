//! Terminal modes (ANSI SM/RM and DEC private DECSET/DECRST).

/// Whether a mode is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeState {
    Set,
    Reset,
}

impl ModeState {
    /// Whether the mode is on.
    pub fn is_set(self) -> bool {
        self == Self::Set
    }
}

/// ANSI modes (SM/RM) and DEC private modes (DECSET/DECRST) share numbers.
#[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeFamily {
    Ansi,
    Dec,
}

/// The modes temor reads or sets. Each maps to one ANSI or DEC private mode number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalMode {
    /// ANSI 4: insert mode.
    Insert,
    /// ANSI 20: line feed also returns the carriage.
    Linefeed,
    /// DEC 1: application cursor keys.
    CursorKeys,
    /// DEC 5: reverse video.
    ReverseColors,
    /// DEC 6: origin mode.
    Origin,
    /// DEC 7: auto-wrap.
    Wraparound,
    /// DEC 12: blinking cursor.
    CursorBlinking,
    /// DEC 25: cursor shown.
    CursorVisible,
    /// DEC 66: application keypad.
    KeypadKeys,
    /// DEC 9: X10 mouse reporting.
    MouseX10,
    /// DEC 1000: press and release mouse reporting.
    MouseNormal,
    /// DEC 1002: mouse reporting with drags.
    MouseButton,
    /// DEC 1003: mouse reporting with all motion.
    MouseAny,
    /// DEC 1004: focus in and out reports.
    FocusEvents,
    /// DEC 1006: SGR mouse format.
    MouseSgr,
    /// DEC 1016: SGR mouse format in pixels.
    MouseSgrPixels,
    /// DEC 1007: wheel scrolls send arrow keys on the alternate screen.
    AltScroll,
    /// DEC 1049: alternate screen, saving the cursor.
    AltScreen,
    /// DEC 2004: bracketed paste.
    BracketedPaste,
    /// DEC 2026: synchronized output.
    SyncOutput,
    /// DEC 2027: grapheme clusters take their full width. On by default in temor.
    GraphemeCluster,
    /// DEC 2031: report colour scheme changes.
    ColorSchemeReport,
    /// DEC 2048: in-band size reports.
    InBandResize,
}

impl TerminalMode {
    /// The mode number and its family.
    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    pub(crate) fn number(self) -> (u16, ModeFamily) {
        match self {
            Self::Insert => (4, ModeFamily::Ansi),
            Self::Linefeed => (20, ModeFamily::Ansi),
            Self::CursorKeys => (1, ModeFamily::Dec),
            Self::ReverseColors => (5, ModeFamily::Dec),
            Self::Origin => (6, ModeFamily::Dec),
            Self::Wraparound => (7, ModeFamily::Dec),
            Self::CursorBlinking => (12, ModeFamily::Dec),
            Self::CursorVisible => (25, ModeFamily::Dec),
            Self::KeypadKeys => (66, ModeFamily::Dec),
            Self::MouseX10 => (9, ModeFamily::Dec),
            Self::MouseNormal => (1000, ModeFamily::Dec),
            Self::MouseButton => (1002, ModeFamily::Dec),
            Self::MouseAny => (1003, ModeFamily::Dec),
            Self::FocusEvents => (1004, ModeFamily::Dec),
            Self::MouseSgr => (1006, ModeFamily::Dec),
            Self::MouseSgrPixels => (1016, ModeFamily::Dec),
            Self::AltScroll => (1007, ModeFamily::Dec),
            Self::AltScreen => (1049, ModeFamily::Dec),
            Self::BracketedPaste => (2004, ModeFamily::Dec),
            Self::SyncOutput => (2026, ModeFamily::Dec),
            Self::GraphemeCluster => (2027, ModeFamily::Dec),
            Self::ColorSchemeReport => (2031, ModeFamily::Dec),
            Self::InBandResize => (2048, ModeFamily::Dec),
        }
    }
}
