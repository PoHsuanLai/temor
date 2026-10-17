//! The ⌘T bar's rows and where they come from (§7). Rows are ranked here and handed to quire's
//! palette machine as they are; the palette never re-ranks.

use crate::vocab::ids::{FolderId, Seq, TabId};
use crate::vocab::text::{CommandText, Cwd, HostAlias, PluginId};
use ds_core::word::Word;
use ds_style::space::list::SpaceId;
use temor_pane::keys::Action;

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
    /// Bring back the tab closed last (recently closed, with its scrollback). `Seq` is the order
    /// it was closed in, so the row's identity changes when another tab is closed after it.
    Closed(Seq),
    /// Open a folder of tabs.
    Folder(FolderId),
    /// Connect to a host.
    Host(HostAlias),
    /// Open a saved layout.
    Layout(LayoutName),
    /// Run a recent command again.
    RecentCommand(CommandText),
    /// Do one of the window's declared actions (Split right, Zoom, ...).
    Action(Action),
    /// Open a new tab with a shell on this computer, while the pane in front is on a host.
    LocalTab,
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
    /// The plugin's name from its manifest: the header its rows sit under, and what orders the
    /// plugins.
    pub name: String,
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
    /// Open a plain tab with one shell. The bar offers it on an empty query, so ⌘T then Enter
    /// is a new tab.
    NewTab,
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
    /// The Run row (rule 2), on top; on an empty query, the "New tab" row.
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
