//! The headers the bar's rows sit under (§7): Actions, Tabs, Spaces and so on. A row's section
//! follows from what it does, so the ranking and the view agree without a second field to keep.

use super::model::{BarAction, BarRow, Target};
use temor_core::vocab::text::PluginId;

/// Which header a row sits under, in the order the headers are listed when matches fit equally.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// What one plugin offered, under a header with the plugin's name. The sections of plugins
    /// sit after the built-in ones, in the order `rank` lists the plugins (by name).
    Plugin(PluginId),
    /// The AI rows, always last.
    Ask,
}

impl Section {
    /// The header's words, for every section but a plugin's (which takes its plugin's name).
    pub fn title(&self) -> Option<&'static str> {
        match self {
            Section::Actions => Some("Actions"),
            Section::Tabs => Some("Tabs"),
            Section::Spaces => Some("Spaces"),
            Section::Computers => Some("Computers"),
            Section::RecentCommands => Some("Recent commands"),
            Section::Folders => Some("Folders"),
            Section::Run => Some("Run"),
            Section::Plugin(_) => None,
            Section::Ask => Some("Ask"),
        }
    }

    /// The section `row` sits under.
    pub fn of(row: &BarRow) -> Section {
        match &row.action {
            BarAction::NewTab => Section::Actions,
            BarAction::Run(_) => Section::Run,
            BarAction::Go(target) => section_of_target(target),
            BarAction::Plugin { plugin, .. } => Section::Plugin(plugin.clone()),
            BarAction::Suggest(_) | BarAction::AskAgent(_) | BarAction::AskAbout(_) => Section::Ask,
        }
    }
}

/// The section a local target sits under.
pub(super) fn section_of_target(target: &Target) -> Section {
    match target {
        Target::Action(_) | Target::Closed(_) | Target::LocalTab => Section::Actions,
        Target::Tab(_) | Target::Folder(_) => Section::Tabs,
        Target::Space(_) | Target::Layout(_) => Section::Spaces,
        Target::Host(_) => Section::Computers,
        Target::RecentCommand(_) => Section::RecentCommands,
        Target::Directory { .. } => Section::Folders,
    }
}
