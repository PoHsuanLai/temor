//! Title hints: the weakest source. Agents that animate their window title show a spinner
//! glyph while they work; this reads only that, never the words.

/// Whether `title` says its agent is busy: its first character is a Braille pattern
/// (U+2800 to U+28FF), the spinner Claude Code and similar tools put there. Assumed, not
/// verified against every agent; an unknown agent just reads as quiet and the stronger
/// sources carry it.
pub fn title_hint(title: &str) -> bool {
    matches!(title.chars().next(), Some('\u{2800}'..='\u{28FF}'))
}
