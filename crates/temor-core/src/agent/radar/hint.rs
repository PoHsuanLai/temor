//! Title hints: the weakest source. Agents that animate their window title show a spinner
//! glyph while they work; this reads only that, never the words.

/// What a title says about the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleHint {
    /// It starts with a spinner.
    Busy,
    /// It does not.
    Quiet,
}

/// The hint in `title`. A title is busy when its first character is a Braille pattern
/// (U+2800 to U+28FF), the spinner Claude Code and similar tools put there. Assumed, not
/// verified against every agent; an unknown agent just reads as quiet and the stronger
/// sources carry it.
pub fn title_hint(title: &str) -> TitleHint {
    match title.chars().next() {
        Some('\u{2800}'..='\u{28FF}') => TitleHint::Busy,
        _ => TitleHint::Quiet,
    }
}
