//! What a key can ask the pane to do (PLAN §6). A closed set, so every match is checked, and
//! the two-way map to chordkit's actions: temor owns no chords, only these names.

use chordkit::{Action as Named, AppAction, SpaceNumber, StandardAction};

use super::settings_pane::SettingsPane;

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
    /// Copy the picked commands only, one per line.
    CopyCommands,
    /// Copy the picked commands' outputs only, a blank line between them.
    CopyOutputs,
    /// Pick every command on the scrollback.
    SelectAllBlocks,
    /// Bookmark the picked commands, or take their bookmark away.
    ToggleBookmark,
    /// Fold the picked commands' output to just their header row, or unfold it.
    ToggleFold,
    /// Pick the nearest bookmarked command above the picked one.
    PreviousBookmark,
    /// Pick the nearest bookmarked command below the picked one.
    NextBookmark,
    /// Put the picked commands in the prompt to run again, without running them.
    ReinputCommands,
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
    /// Make the window's text and controls bigger (the standard "bigger").
    ZoomIn,
    /// Make the window's text and controls smaller (the standard "smaller").
    ZoomOut,
    /// Bring the window back to its own size (the standard "actual size").
    ActualSize,
    /// Bring back the tab closed last.
    ReopenTab,
    /// Show the sidebar, or hide it. At every window width; a narrow window starts without it.
    ToggleSidebar,
    /// Focus the agent pane that is waiting for the person, the next one after this pane.
    JumpToWaiting,
    /// Pin the tab in front to the sidebar's tiles. No default chord: the command bar lists it.
    PinTab,
    /// Take the tab in front off the sidebar's tiles. No default chord: the command bar lists it.
    UnpinTab,
    /// Open the list of downloaded files, or close it. No default chord: the command bar lists
    /// it, and the sidebar's head has its button.
    ShowDownloads,
    /// Open the Settings window, or raise it (the standard "settings", ⌘,).
    Settings,
    /// Open the Settings window on this page. No chord: the command bar lists one row for each.
    SettingsAt(SettingsPane),
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
    (Action::CopyCommands, "copy-commands"),
    (Action::CopyOutputs, "copy-outputs"),
    (Action::ToggleBookmark, "toggle-bookmark"),
    (Action::ToggleFold, "toggle-fold"),
    (Action::PreviousBookmark, "previous-bookmark"),
    (Action::NextBookmark, "next-bookmark"),
    (Action::ReinputCommands, "reinput-commands"),
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
    (Action::PinTab, "pin-tab"),
    (Action::UnpinTab, "unpin-tab"),
    (Action::ShowDownloads, "show-downloads"),
];

/// The actions that are chordkit's standard ones.
const STANDARD: &[(Action, StandardAction)] = &[
    (Action::Copy, StandardAction::Copy),
    (Action::Paste, StandardAction::Paste),
    (Action::Find, StandardAction::Find),
    (Action::FindNext, StandardAction::FindNext),
    (Action::FindPrevious, StandardAction::FindPrevious),
    (Action::ClosePane, StandardAction::Close),
    // A terminal has no text field to select all of: the platform's select all picks every
    // command, as Warp's does.
    (Action::SelectAllBlocks, StandardAction::SelectAll),
    (Action::ZoomIn, StandardAction::Bigger),
    (Action::ZoomOut, StandardAction::Smaller),
    (Action::ActualSize, StandardAction::ActualSize),
    (Action::Settings, StandardAction::Settings),
    // As mailo has it: ⌃⌘S on a Mac, F9 elsewhere.
    (Action::ToggleSidebar, StandardAction::ToggleSidebar),
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
            Self::CopyCommands => "Copy commands",
            Self::CopyOutputs => "Copy outputs",
            Self::SelectAllBlocks => "Select all commands",
            Self::ToggleBookmark => "Bookmark",
            Self::ToggleFold => "Fold output",
            Self::PreviousBookmark => "Previous bookmark",
            Self::NextBookmark => "Next bookmark",
            Self::ReinputCommands => "Put commands in prompt",
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
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ActualSize => "Actual size",
            Self::ReopenTab => "Reopen tab",
            Self::JumpToWaiting => "Jump to waiting agent",
            Self::ToggleSidebar => "Sidebar",
            Self::PinTab => "Pin tab",
            Self::UnpinTab => "Unpin tab",
            Self::ShowDownloads => "Downloads",
            Self::Settings => "Settings",
            Self::SettingsAt(pane) => pane.row_label(),
            Self::SelectTab(_) => "Select tab",
            Self::SelectSpace(_) => "Select Space",
        }
    }

    /// The chordkit action this one is bound as, or `None` for an action with no keymap entry
    /// ([`Action::CloseFind`], [`Action::SettingsAt`], or a tab or Space number outside 1 to 9).
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
    /// keymap also carries the system's actions, such as Undo). The standard Cancel is
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

/// Every action a person can give a key of their own, in the order the Keyboard page lists them:
/// the standard ones temor runs, temor's own, then the tabs 1 to 9. The Space switch is the
/// desktop's, and the Settings pages and "close find" have no entry.
pub fn bindable() -> Vec<Action> {
    STANDARD
        .iter()
        .map(|(ours, _)| *ours)
        .chain(APP_SLUGS.iter().map(|(ours, _)| *ours))
        .chain((1..=9).map(Action::SelectTab))
        .collect()
}
