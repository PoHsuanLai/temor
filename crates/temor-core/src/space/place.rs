//! What a Space recalls (the `R` of `Spaces<P, R>`): its tabs, their folders and trees, and
//! which tab was in front.

use crate::split::SplitTree;
use crate::vocab::ids::{FolderId, TabId};
use crate::vocab::lenient::{lenient_list, lenient_or_default};
use crate::vocab::text::{Relaunch, TabTitle};
use serde::{Deserialize, Deserializer, Serialize};

/// A Space's tabs as it was left. The default (a Space never opened) has none.
///
/// Reading never fails: the kit reads every Space's place as one map, so one place that does not
/// parse would cost them all. A field that does not parse takes its default, and a place that is
/// not a map at all reads as the default place.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Place {
    /// The tabs, in sidebar order. A tab whose stored form is damaged is dropped alone.
    pub tabs: Vec<Tab>,
    /// The sidebar's folders.
    pub folders: Vec<Folder>,
    /// The tab in front; none when there are no tabs.
    pub focused: Option<TabId>,
}

/// [`Place`] as it is read, each field on its own.
#[derive(Default, Deserialize)]
struct PlaceRead {
    #[serde(default, deserialize_with = "lenient_list")]
    tabs: Vec<Tab>,
    #[serde(default, deserialize_with = "lenient_list")]
    folders: Vec<Folder>,
    #[serde(default, deserialize_with = "lenient_or_default")]
    focused: Option<TabId>,
}

impl<'de> Deserialize<'de> for Place {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Place, D::Error> {
        let read: PlaceRead = lenient_or_default(deserializer)?;
        Ok(Place {
            tabs: read.tabs,
            folders: read.folders,
            focused: read.focused,
        })
    }
}

/// A tab: it stays until it is closed (D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    /// Its identity.
    pub id: TabId,
    /// What the sidebar row says.
    pub title: TabTitle,
    /// Whether it is pinned.
    #[serde(default, deserialize_with = "lenient_or_default")]
    pub pin: Pin,
    /// The folder it is grouped in.
    #[serde(default, deserialize_with = "lenient_or_default")]
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
