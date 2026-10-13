//! What a key can ask the pane to do (PLAN §6). A closed set, so every match is checked, and
//! the two-way map to chordkit's actions: temor owns no chords, only these names.

use chordkit::{Action as Named, AppAction, SpaceNumber, StandardAction};

/// The app id temor registers its own actions under.
pub(super) const APP: &str = "temor";

/// A pane action a key can run.
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
    /// Close the find bar. Not a keymap entry: Escape closes it only while it is open, and the
    /// system's Cancel does the same (see `route`).
    CloseFind,
    /// Pick the command above the picked one (the newest, with none picked) and bring it into
    /// view.
    PreviousPrompt,
    /// Pick the command below the picked one and bring it into view.
    NextPrompt,
    /// Open a plain tab with one shell (the standard "new", ⌘N). ⌘T is the command bar, whose
    /// Enter on an empty bar also opens one.
    NewTab,
    /// Open the command bar (⌘T), or close it while it is open.
    OpenBar,
    /// Show the quick terminal over the window, or hide it; its shell keeps running while hidden.
    ToggleQuick,
    /// Close the focused pane; the last pane closes its tab (the standard "close").
    ClosePane,
    /// Split the focused pane, the new one on its right.
    SplitRight,
    /// Split the focused pane, the new one below it.
    SplitDown,
    /// Move focus to the pane on the left.
    FocusLeft,
    /// Move focus to the pane on the right.
    FocusRight,
    /// Move focus to the pane above.
    FocusUp,
    /// Move focus to the pane below.
    FocusDown,
    /// Show the focused pane over its whole tab, or back to the split.
    ToggleZoom,
    /// Even out the splits of the tab.
    Equalize,
    /// Bring back the tab closed last.
    ReopenTab,
    /// Show the sidebar, or hide it. At every window width; a narrow window starts without it.
    ToggleSidebar,
    /// Focus the agent pane that is waiting for the person, the next one after this pane.
    JumpToWaiting,
    /// Select the tab at this sidebar position, 1 to 9.
    SelectTab(u8),
    /// Switch to the Space at this position, 1 to 9 (the standard "switch to Space n"). It has
    /// no default of temor's: the desktop owns that key.
    SelectSpace(u8),
}

/// The app actions' ids after `temor.`, for the actions that are not standard.
const APP_SLUGS: &[(Action, &str)] = &[
    (Action::NewTab, "new-tab"),
    (Action::OpenBar, "open-bar"),
    (Action::ToggleQuick, "toggle-quick"),
    (Action::PreviousPrompt, "previous-prompt"),
    (Action::NextPrompt, "next-prompt"),
    (Action::SplitRight, "split-right"),
    (Action::SplitDown, "split-down"),
    (Action::FocusLeft, "focus-left"),
    (Action::FocusRight, "focus-right"),
    (Action::FocusUp, "focus-up"),
    (Action::FocusDown, "focus-down"),
    (Action::ToggleZoom, "zoom"),
    (Action::Equalize, "equalize"),
    (Action::ReopenTab, "reopen-tab"),
    (Action::JumpToWaiting, "jump-to-waiting"),
    (Action::ToggleSidebar, "toggle-sidebar"),
];

/// The actions that are chordkit's standard ones.
const STANDARD: &[(Action, StandardAction)] = &[
    (Action::Copy, StandardAction::Copy),
    (Action::Paste, StandardAction::Paste),
    (Action::Find, StandardAction::Find),
    (Action::FindNext, StandardAction::FindNext),
    (Action::FindPrevious, StandardAction::FindPrevious),
    (Action::ClosePane, StandardAction::Close),
];

impl Action {
    /// What the person reads for this action, in messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Find => "Find",
            Self::FindNext => "Find next",
            Self::FindPrevious => "Find previous",
            Self::CloseFind => "Close find",
            Self::PreviousPrompt => "Select previous command",
            Self::NextPrompt => "Select next command",
            Self::NewTab => "New tab",
            Self::OpenBar => "Command bar",
            Self::ToggleQuick => "Quick terminal",
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
            Self::JumpToWaiting => "Jump to waiting agent",
            Self::ToggleSidebar => "Sidebar",
            Self::SelectTab(_) => "Select tab",
            Self::SelectSpace(_) => "Select Space",
        }
    }

    /// The chordkit action this one is bound as, or `None` for an action with no keymap entry
    /// ([`Action::CloseFind`], or a tab or Space number outside 1 to 9).
    pub fn to_chordkit(self) -> Option<Named> {
        if let Some((_, standard)) = STANDARD.iter().find(|(ours, _)| *ours == self) {
            return Some(Named::Standard(*standard));
        }
        if let Some((_, slug)) = APP_SLUGS.iter().find(|(ours, _)| *ours == self) {
            return AppAction::new(&format!("{APP}.{slug}"))
                .ok()
                .map(Named::App);
        }
        match self {
            Self::SelectTab(n) if (1..=9).contains(&n) => AppAction::new(&format!("{APP}.tab-{n}"))
                .ok()
                .map(Named::App),
            Self::SelectSpace(n) => {
                SpaceNumber::new(n).map(|n| Named::Standard(StandardAction::SwitchToSpace(n)))
            }
            _ => None,
        }
    }

    /// The pane action a chordkit action means, or `None` for one temor does not act on (the
    /// keymap also carries the system's actions, such as Select all). The standard Cancel is
    /// [`Action::CloseFind`]: it closes the find bar when one is open.
    pub fn from_chordkit(action: &Named) -> Option<Self> {
        match action {
            Named::Standard(StandardAction::Cancel) => Some(Self::CloseFind),
            Named::Standard(StandardAction::SwitchToSpace(n)) => Some(Self::SelectSpace(n.get())),
            Named::Standard(standard) => STANDARD
                .iter()
                .find(|(_, candidate)| candidate == standard)
                .map(|(ours, _)| *ours),
            Named::App(app) => {
                let slug = app.id().strip_prefix(APP)?.strip_prefix('.')?;
                if let Some(digits) = slug.strip_prefix("tab-") {
                    let n: u8 = digits.parse().ok()?;
                    return (1..=9).contains(&n).then_some(Self::SelectTab(n));
                }
                APP_SLUGS
                    .iter()
                    .find(|(_, candidate)| *candidate == slug)
                    .map(|(ours, _)| *ours)
            }
            _ => None,
        }
    }
}
