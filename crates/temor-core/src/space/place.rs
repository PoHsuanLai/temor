//! What a Space recalls (the `R` of `Spaces<P, R>`): its tabs, their folders and trees, and
//! which tab was in front.

use crate::split::SplitTree;
use crate::vocab::ids::{FolderId, PaneId, TabId};
use crate::vocab::lenient::{lenient_list, lenient_or_default};
use crate::vocab::text::{Relaunch, TabTitle, ViewerRef};
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
    /// The files the viewer panes among the trees show. Only the Spaces file keeps these (a
    /// session file has its own list), so that a Space recalled from its place alone brings
    /// them back as viewers rather than shells. Empty in a live workspace.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub viewers: Vec<Viewing>,
}

/// A viewer pane and the file it shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewing {
    pub pane: PaneId,
    pub file: ViewerRef,
}

impl Place {
    /// This place with `viewers` (pane, file) kept for the panes its trees have, in pane order.
    #[must_use]
    pub fn with_viewers<'a>(
        mut self,
        viewers: impl IntoIterator<Item = (&'a PaneId, &'a ViewerRef)>,
    ) -> Place {
        let panes: Vec<PaneId> = self.tabs.iter().flat_map(|tab| tab.tree.panes()).collect();
        self.viewers = viewers
            .into_iter()
            .filter(|(pane, _)| panes.contains(pane))
            .map(|(pane, file)| Viewing {
                pane: *pane,
                file: file.clone(),
            })
            .collect();
        self
    }
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
    #[serde(default, deserialize_with = "lenient_list")]
    viewers: Vec<Viewing>,
}

impl<'de> Deserialize<'de> for Place {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Place, D::Error> {
        let read: PlaceRead = lenient_or_default(deserializer)?;
        Ok(Place {
            tabs: read.tabs,
            folders: read.folders,
            focused: read.focused,
            viewers: read.viewers,
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
