//! What a chord can ask the pane to do (PLAN §6). Closed set, so every match is checked.

/// A pane action bound to a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl Action {
    /// Every action, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Copy,
        Self::Paste,
        Self::Find,
        Self::FindNext,
        Self::FindPrevious,
        Self::CloseFind,
        Self::PreviousPrompt,
        Self::NextPrompt,
    ];

    /// The action written as `slug` in settings text (`find_next`), or `None`.
    pub fn parse(slug: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|action| action.slug() == slug)
    }

    /// The name settings text uses for this action.
    fn slug(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Paste => "paste",
            Self::Find => "find",
            Self::FindNext => "find_next",
            Self::FindPrevious => "find_previous",
            Self::CloseFind => "close_find",
            Self::PreviousPrompt => "previous_prompt",
            Self::NextPrompt => "next_prompt",
        }
    }

    /// What the person reads for this action, in messages such as "already used by Copy".
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Find => "Find",
            Self::FindNext => "Find next",
            Self::FindPrevious => "Find previous",
            Self::CloseFind => "Close find",
            Self::PreviousPrompt => "Previous prompt",
            Self::NextPrompt => "Next prompt",
        }
    }
}
