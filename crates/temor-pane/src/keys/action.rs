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
    /// Open a new tab (⌘T).
    NewTab,
    /// Close the focused pane; the last pane closes its tab (⌘W).
    ClosePane,
    /// Split the focused pane, the new one on its right (⌘D).
    SplitRight,
    /// Split the focused pane, the new one below it (⌘⇧D).
    SplitDown,
    /// Move focus to the pane on the left (⌘⌥←).
    FocusLeft,
    /// Move focus to the pane on the right (⌘⌥→).
    FocusRight,
    /// Move focus to the pane above (⌘⌥↑).
    FocusUp,
    /// Move focus to the pane below (⌘⌥↓).
    FocusDown,
    /// Show the focused pane over its whole tab, or back to the split (⌘⇧↩).
    ToggleZoom,
    /// Even out the splits of the tab (⌃⌘=).
    Equalize,
    /// Bring back the tab closed last (⌘⇧T).
    ReopenTab,
    /// Select the tab at this sidebar position, 1 to 9 (⌘1 to ⌘9). Not in [`Action::ALL`]: the
    /// digits are fixed defaults, not settings.
    SelectTab(u8),
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
        Self::NewTab,
        Self::ClosePane,
        Self::SplitRight,
        Self::SplitDown,
        Self::FocusLeft,
        Self::FocusRight,
        Self::FocusUp,
        Self::FocusDown,
        Self::ToggleZoom,
        Self::Equalize,
        Self::ReopenTab,
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
            Self::NewTab => "new_tab",
            Self::ClosePane => "close_pane",
            Self::SplitRight => "split_right",
            Self::SplitDown => "split_down",
            Self::FocusLeft => "focus_left",
            Self::FocusRight => "focus_right",
            Self::FocusUp => "focus_up",
            Self::FocusDown => "focus_down",
            Self::ToggleZoom => "toggle_zoom",
            Self::Equalize => "equalize",
            Self::ReopenTab => "reopen_tab",
            Self::SelectTab(_) => "select_tab",
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
            Self::NewTab => "New tab",
            Self::ClosePane => "Close pane",
            Self::SplitRight => "Split right",
            Self::SplitDown => "Split down",
            Self::FocusLeft => "Focus left",
            Self::FocusRight => "Focus right",
            Self::FocusUp => "Focus up",
            Self::FocusDown => "Focus down",
            Self::ToggleZoom => "Zoom",
            Self::Equalize => "Equalize",
            Self::ReopenTab => "Reopen tab",
            Self::SelectTab(_) => "Select tab",
        }
    }
}
