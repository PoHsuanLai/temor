//! The ⌘T bar's rows and where they come from (§7). Rows are ranked here and handed to quire's
//! palette machine as they are; the palette never re-ranks.

use crate::vocab::ids::{FolderId, TabId};
use crate::vocab::text::{CommandText, Cwd, HostAlias, PluginId};
use ds_core::word::Word;
use ds_style::space::list::SpaceId;

/// Finds a program on the person's `PATH`. The Run row exists only when this says the first
/// word is there: a lookup, not a guess. The real one lives with the process code; tests pass
/// a fixed table.
pub trait PathResolver {
    /// Whether `program` is a command the shell would find.
    fn resolves(&self, program: &str) -> bool;
}

/// How often and how lately a folder was used: larger comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Frecency(pub u32);

/// A saved window layout's name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayoutName(pub String);

/// Something local the bar can match (rule 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Candidate {
    /// What the row says and what the query matches.
    pub title: String,
    /// What picking it does.
    pub target: Target,
}

/// A local thing to go to, in the order the kinds are listed when they match equally well.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// Switch to a Space.
    Space(SpaceId),
    /// Go to a tab.
    Tab(TabId),
    /// Open a folder of tabs.
    Folder(FolderId),
    /// Connect to a host.
    Host(HostAlias),
    /// Open a saved layout.
    Layout(LayoutName),
    /// Run a recent command again.
    RecentCommand(CommandText),
    /// Open a new tab in a folder.
    Directory {
        /// The folder.
        cwd: Cwd,
        /// How much it is used.
        frecency: Frecency,
    },
}

/// A row an `open.provider` plugin offered for the current query (rule 3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginRow {
    /// The plugin.
    pub plugin: PluginId,
    /// The plugin's own id for the item.
    pub item: String,
    /// What the row says.
    pub title: String,
}

/// What picking a row does.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BarAction {
    /// Go to a local thing.
    Go(Target),
    /// Run the query as a command in a new tab.
    Run(CommandText),
    /// Hand the item back to its plugin.
    Plugin {
        /// The plugin.
        plugin: PluginId,
        /// Its item.
        item: String,
    },
    /// Ask the model for a command (§8.3).
    Suggest(String),
    /// Open the agent view in a new pane with this request.
    AskAgent(String),
    /// Hand the request to the companion with this tab as context.
    AskAbout(String),
}

/// Which rule placed a row; rows come in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Word)]
pub enum RowGroup {
    /// The Run row (rule 2), on top.
    Run,
    /// Local matches (rule 1).
    Match,
    /// Plugin rows (rule 3).
    Plugin,
    /// AI rows (rule 4), always last.
    Ai,
}

/// A row's identity, stable while the bar is open, so the highlight is a key and not a position.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowKey(pub String);

/// One ranked row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BarRow {
    /// Its identity.
    pub key: RowKey,
    /// The rule that placed it.
    pub group: RowGroup,
    /// What it says.
    pub title: String,
    /// What picking it does.
    pub action: BarAction,
}
