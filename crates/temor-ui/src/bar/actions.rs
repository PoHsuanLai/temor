//! The window's declared actions the command bar lists as rows (PLAN §7): everything a chord
//! does that makes sense to run from a list. "Reopen tab" is not here: the bar's own row for
//! the tab closed last ("Reopen <name>") is that action, and it only exists while there is one.
//! The ones that need the keyboard for themselves (open the bar, Copy, Paste, the find stepping
//! and the numbered tab and Space jumps) are left out.

use temor_pane::keys::{Action, SettingsPane};

/// The actions the bar lists, in the order they show when they match equally.
pub const BAR_ACTIONS: &[Action] = &[
    Action::NewTab,
    Action::SplitRight,
    Action::SplitDown,
    Action::ToggleZoom,
    Action::Equalize,
    Action::ClosePane,
    Action::FocusLeft,
    Action::FocusRight,
    Action::FocusUp,
    Action::FocusDown,
    Action::Find,
    Action::PreviousPrompt,
    Action::NextPrompt,
    Action::SelectAllBlocks,
    Action::PreviousBookmark,
    Action::NextBookmark,
    Action::ToggleQuick,
    Action::ToggleSidebar,
    Action::PinTab,
    Action::UnpinTab,
    Action::ShowDownloads,
    Action::Back,
    Action::Forward,
    Action::ShowStage,
    Action::ShowConsole,
    Action::ShowStageAndConsole,
    Action::DetachProgram,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::ActualSize,
    Action::JumpToWaiting,
    Action::Settings,
    Action::SettingsAt(SettingsPane::General),
    Action::SettingsAt(SettingsPane::Appearance),
    Action::SettingsAt(SettingsPane::Keyboard),
    Action::SettingsAt(SettingsPane::Spaces),
];
