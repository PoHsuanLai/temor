//! The workspace step's inputs, outputs and refusals.

use super::tabs::{TabsIn, TabsOut};
use crate::split::{Columns, Direction, Goto, Share};
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Cwd, Relaunch, ViewerRef};

/// Where a new pane's program starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// A fresh shell (or the tab's pinned command).
    Fresh,
    /// Its stored scrollback, from a pane that was closed.
    Scrollback(PaneId),
}

/// A pane the window must start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    /// The pane.
    pub pane: PaneId,
    /// The tab it is in.
    pub tab: TabId,
    /// The folder to start in.
    pub cwd: Option<Cwd>,
    /// The command a pinned tab runs again (its first pane only).
    pub relaunch: Option<Relaunch>,
    /// Where its program starts from.
    pub start: Start,
}

/// A viewer pane the window must show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerOpen {
    /// The pane.
    pub pane: PaneId,
    /// The tab it is in.
    pub tab: TabId,
    /// The file it shows.
    pub file: ViewerRef,
    /// The pane it was opened from, when it was opened from one.
    pub from: Option<PaneId>,
}

/// What the workspace is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkspaceIn {
    /// Open a plain tab with one shell, in `cwd` when given.
    NewTab { cwd: Option<Cwd> },
    /// Split the focused pane of the tab in front, and focus the new pane.
    Split { towards: Direction },
    /// [`WorkspaceIn::Split`], told how wide the focused pane is: refused when a half would be
    /// narrower than `LEAST_COLUMNS`.
    SplitIn {
        /// Where the new pane goes.
        towards: Direction,
        /// The focused pane's width now.
        width: Columns,
    },
    /// Close a pane (the focused one when none is named). The last pane closes the tab.
    ClosePane(Option<PaneId>),
    /// Close a tab with all its panes; it is kept as recently closed.
    CloseTab(TabId),
    /// Move focus within the tab in front.
    Goto(Goto),
    /// Show the focused pane over the whole tab, or back to the split (toggles).
    ToggleZoom,
    /// Even out the splits of the tab in front.
    Equalize,
    /// Give the split of the tab in front between two neighbouring panes this share: a
    /// divider the person dragged.
    SetShare {
        /// The pane before the divider.
        before: PaneId,
        /// The pane after it.
        after: PaneId,
        /// The first child's part of the split.
        share: Share,
    },
    /// Focus a pane wherever it is, bringing its tab to the front.
    Focus(PaneId),
    /// Drop a tab onto a pane of another tab, splitting that pane towards `towards`.
    DropTab {
        /// The tab dragged.
        from: TabId,
        /// The pane it is dropped on.
        onto: PaneId,
        /// The side of `onto` it lands on.
        towards: Direction,
    },
    /// [`WorkspaceIn::DropTab`], told how wide the pane dropped on is: refused when a half would
    /// be narrower than `LEAST_COLUMNS`, as a split is.
    DropTabIn {
        from: TabId,
        onto: PaneId,
        towards: Direction,
        /// The width of `onto` now.
        width: Columns,
    },
    /// Tear a pane out of its tab into a tab of its own.
    Tear(PaneId),
    /// A command ended in a pane: its tab is marked unread unless it is in front.
    Activity(PaneId),
    /// The pane's working folder changed (OSC 7).
    Cwd { pane: PaneId, cwd: Cwd },
    /// Show a file in a viewer pane to the right of `from` (the focused pane of the tab in
    /// front when none is named). A tab that already has a viewer shows the file there instead
    /// of splitting again.
    OpenViewer {
        /// The file.
        file: ViewerRef,
        /// The pane it comes from.
        from: Option<PaneId>,
    },
    /// A viewer pane now shows `file` (the person walked to the next one): the session file
    /// keeps it. The pane itself is not told anything.
    ViewerShows {
        /// The viewer pane.
        pane: PaneId,
        /// The file it shows now.
        file: ViewerRef,
    },
    /// Take back the newest recently closed tab.
    Reopen,
    /// A tab-list request (select, pin, folders, rename). Removing tabs goes through `CloseTab`.
    Tabs(TabsIn),
}

/// What the workspace tells the window.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkspaceOut {
    /// Start this pane.
    Spawn(Spawn),
    /// Show this viewer pane (a file, not a program).
    Viewer(ViewerOpen),
    /// Take this pane's scrollback now, before it ends.
    Snapshot(PaneId),
    /// End this pane's program and drop the pane.
    End(PaneId),
    /// These panes left the recently closed list: drop their stored scrollback.
    Forgotten(Vec<PaneId>),
    /// This pane has keys now.
    Focused(PaneId),
    /// A tab-list answer that the window does not act on itself.
    Tab(TabsOut),
    /// The request was not carried out.
    Refused(RefuseWhy),
    /// The state changed: the window saves the session (debounced).
    Changed,
}

/// Why a workspace request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RefuseWhy {
    /// The tab already holds the most panes it can.
    TabFull,
    /// A split would leave a pane too narrow to work in.
    TooNarrow,
    /// The pane is not in any tab.
    NoSuchPane(PaneId),
    /// The tab is not in the list.
    NoSuchTab(TabId),
    /// A tab cannot be dropped onto its own pane.
    SameTab,
    /// A pinned tab keeps its pin; it is not merged into another tab.
    PinnedTab(TabId),
    /// A tab's only pane cannot be torn off; closing the tab is the way.
    OnlyPane(PaneId),
    /// No tab has been closed to bring back.
    NothingClosed,
    /// The request needs a tab-list step that does not exist here (removal).
    UseCloseTab,
    /// There is no tab in front.
    NoFront,
    /// The pane or tab counter has no id left to hand out.
    IdsExhausted,
    /// The file to show has no name.
    NoFile,
}
