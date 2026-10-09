//! What a chord can ask the pane to do (PLAN §6). Closed set, so every match is checked.

use ds_core::word::Word;

/// A pane action bound to a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum Action {
    /// Copy the selection to the clipboard.
    Copy,
    /// Paste the clipboard into the shell (bracketed when the program asked for it).
    Paste,
    /// Open the find bar over the scrollback.
    Find,
    /// Jump to the next match.
    FindNext,
    /// Jump to the previous match.
    FindPrevious,
    /// Close the find bar.
    CloseFind,
    /// Bring the previous command's prompt into view (⌘↑).
    PreviousPrompt,
    /// Bring the next command's prompt into view (⌘↓).
    NextPrompt,
}
