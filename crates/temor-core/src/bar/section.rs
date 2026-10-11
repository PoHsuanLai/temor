//! The headers the bar's rows sit under (§7): Actions, Tabs, Spaces and so on. A row's section
//! follows from what it does, so the ranking and the view agree without a second field to keep.

use super::model::{BarAction, BarRow, Target};

/// Which header a row sits under, in the order the headers are listed when matches fit equally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Section {
    /// Window actions (Split right, Zoom, New tab, Reopen tab).
    Actions,
    /// Tabs and tab groups.
    Tabs,
    /// Spaces and saved layouts.
    Spaces,
    /// Computers to connect to.
    Computers,
    /// Commands run lately.
    RecentCommands,
    /// Folders to open a tab in.
    Folders,
    /// The command typed, to run in a new tab.
    Run,
    /// What plugins offered.
    Plugins,
    /// The AI rows, always last.
    Ask,
}

impl Section {
    /// The section `row` sits under.
    pub fn of(row: &BarRow) -> Section {
        match &row.action {
            BarAction::NewTab => Section::Actions,
            BarAction::Run(_) => Section::Run,
            BarAction::Go(target) => section_of_target(target),
            BarAction::Plugin { .. } => Section::Plugins,
            BarAction::Suggest(_) | BarAction::AskAgent(_) | BarAction::AskAbout(_) => Section::Ask,
        }
    }
}

/// The section a local target sits under.
pub(super) fn section_of_target(target: &Target) -> Section {
    match target {
        Target::Action(_) | Target::Closed(_) => Section::Actions,
        Target::Tab(_) | Target::Folder(_) => Section::Tabs,
        Target::Space(_) | Target::Layout(_) => Section::Spaces,
        Target::Host(_) => Section::Computers,
        Target::RecentCommand(_) => Section::RecentCommands,
        Target::Directory { .. } => Section::Folders,
    }
}
