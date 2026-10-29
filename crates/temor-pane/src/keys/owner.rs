//! Who carries out an action, and what a menu calls it. One table each, so a new action is
//! sorted once and every route (keys, the bar, menus) agrees.

use super::Action;

/// Who carries an action out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// The pane with the keyboard: copying, finding, picking commands.
    Pane,
    /// The workspace: tabs, splits and the focus among them.
    Workspace,
    /// The window itself: the bar, the quick terminal, the sidebar, Settings, the page's zoom,
    /// pinning, the radar's jump and the Space switch.
    Window,
}

impl Action {
    /// Who carries this action out.
    pub fn owner(self) -> Owner {
        match self {
            Self::Copy
            | Self::Paste
            | Self::Find
            | Self::FindNext
            | Self::FindPrevious
            | Self::CloseFind
            | Self::PreviousPrompt
            | Self::NextPrompt
            | Self::CopyCommands
            | Self::CopyOutputs
            | Self::SelectAllBlocks
            | Self::ToggleBookmark
            | Self::ToggleFold
            | Self::PreviousBookmark
            | Self::NextBookmark
            | Self::ReinputCommands => Owner::Pane,
            Self::NewTab
            | Self::ClosePane
            | Self::SplitRight
            | Self::SplitDown
            | Self::FocusLeft
            | Self::FocusRight
            | Self::FocusUp
            | Self::FocusDown
            | Self::ToggleZoom
            | Self::Equalize
            | Self::ReopenTab
            | Self::SelectTab(_) => Owner::Workspace,
            Self::OpenBar
            | Self::ToggleQuick
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ActualSize
            | Self::ToggleSidebar
            | Self::JumpToWaiting
            | Self::PinTab
            | Self::UnpinTab
            | Self::ShowDownloads
            | Self::Settings
            | Self::SettingsAt(_)
            | Self::Back
            | Self::Forward
            | Self::History
            | Self::PopOut
            | Self::ShowStage
            | Self::ShowConsole
            | Self::ShowStageAndConsole
            | Self::SelectSpace(_) => Owner::Window,
        }
    }

    /// What a menu item for this action reads, in Apple's title case. [`Action::label`] is the
    /// bar's sentence-case wording; the two are the same words.
    pub fn menu_title(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Find => "Find",
            Self::FindNext => "Find Next",
            Self::FindPrevious => "Find Previous",
            Self::CloseFind => "Close Find",
            Self::PreviousPrompt => "Select Previous Command",
            Self::NextPrompt => "Select Next Command",
            Self::CopyCommands => "Copy Commands",
            Self::CopyOutputs => "Copy Outputs",
            Self::SelectAllBlocks => "Select All Commands",
            Self::ToggleBookmark => "Bookmark",
            Self::ToggleFold => "Fold Output",
            Self::PreviousBookmark => "Previous Bookmark",
            Self::NextBookmark => "Next Bookmark",
            Self::ReinputCommands => "Put in Prompt",
            Self::NewTab => "New Tab",
            Self::OpenBar => "Command Bar",
            Self::ToggleQuick => "Quick Terminal",
            Self::ClosePane => "Close Pane",
            Self::SplitRight => "Split Right",
            Self::SplitDown => "Split Down",
            Self::FocusLeft => "Focus Left",
            Self::FocusRight => "Focus Right",
            Self::FocusUp => "Focus Up",
            Self::FocusDown => "Focus Down",
            Self::ToggleZoom => "Zoom",
            Self::Equalize => "Equalize",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ActualSize => "Actual Size",
            Self::ReopenTab => "Reopen Closed Tab",
            Self::JumpToWaiting => "Jump to Waiting Agent",
            // The menus that offer it are on the sidebar, which is shown.
            Self::ToggleSidebar => "Hide Sidebar",
            Self::PinTab => "Pin Tab",
            Self::UnpinTab => "Unpin Tab",
            Self::ShowDownloads => "Downloads",
            Self::Settings => "Settings",
            Self::SettingsAt(pane) => pane.row_label(),
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::History => "Show History",
            Self::PopOut => "Pop Out",
            Self::ShowStage => "Show Page Only",
            Self::ShowConsole => "Show Console Only",
            Self::ShowStageAndConsole => "Show Page and Console",
            Self::SelectTab(_) => "Select Tab",
            Self::SelectSpace(_) => "Select Space",
        }
    }
}
