//! What a Space recalls (the `R` of `Spaces<P, R>`): its tabs, their folders and trees, and
//! which tab was in front.

use crate::split::SplitTree;
use crate::vocab::ids::{FolderId, TabId};
use crate::vocab::lenient::lenient_list;
use crate::vocab::text::{Relaunch, TabTitle};
use serde::{Deserialize, Serialize};

/// A Space's tabs as it was left. The default (a Space never opened) has none.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Place {
    /// The tabs, in sidebar order. A tab whose stored form is damaged is dropped alone.
    #[serde(default, deserialize_with = "lenient_list")]
    pub tabs: Vec<Tab>,
    /// The sidebar's folders.
    #[serde(default, deserialize_with = "lenient_list")]
    pub folders: Vec<Folder>,
    /// The tab in front; none when there are no tabs.
    #[serde(default)]
    pub focused: Option<TabId>,
}

/// A tab: it stays until it is closed (D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    /// Its identity.
    pub id: TabId,
    /// What the sidebar row says.
    pub title: TabTitle,
    /// Whether it is pinned.
    #[serde(default)]
    pub pin: Pin,
    /// The folder it is grouped in.
    #[serde(default)]
    pub folder: Option<FolderId>,
    /// Its panes.
    pub tree: SplitTree,
}

/// A named group of tabs in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Its identity.
    pub id: FolderId,
    /// What the sidebar calls it.
    pub title: TabTitle,
}

/// Whether a tab is pinned at the top of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Pin {
    /// Pinned: its command runs again when temor starts.
    Pinned {
        /// The command it relaunches.
        relaunch: Relaunch,
    },
    /// An ordinary tab.
    #[default]
    Loose,
}
