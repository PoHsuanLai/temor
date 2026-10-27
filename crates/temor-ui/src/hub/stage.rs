//! Each row's stage and console (PLAN §7, v14): the page a command opened beside its terminal,
//! what the row shows of the two, and Back and Forward through the pages it has shown. The page
//! is the tab's viewer pane; the rules for walking the pages are core's (`stage::Pages`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use temor_core::stage::{Layout, Pages};
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::ViewerRef;
use temor_core::workspace::WorkspaceIn;

use super::desk::Desk;
use super::model::{Hub, HubRef};

/// What the hub keeps of every row's stage.
#[derive(Default)]
pub(super) struct Stages {
    rows: BTreeMap<TabId, Stage>,
    /// Set while Back or Forward opens a page, so opening it is not a step of its own.
    walking: bool,
}

#[derive(Default)]
struct Stage {
    layout: Layout,
    pages: Pages<PathBuf>,
    /// The viewer pane the page is in, while there is one.
    viewer: Option<PaneId>,
}

/// What the window draws of a row's stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageView {
    pub layout: Layout,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

impl Hub {
    /// A page opened in `viewer`, on the stage of `tab`: a step Back can return from, and shown.
    pub(super) fn stage_opened(&mut self, tab: TabId, viewer: PaneId, file: PathBuf) {
        let walking = self.stages.walking;
        let stage = self.stages.rows.entry(tab).or_default();
        stage.viewer = Some(viewer);
        if !walking {
            stage.pages.visit(file);
        }
        // A page asked for is a page shown.
        if !stage.layout.shows_stage() {
            stage.layout = Layout::Both;
        }
    }

    /// The viewer pane `viewer` ended: its row's stage is empty, and Back opens its page again.
    pub(super) fn stage_closed(&mut self, viewer: PaneId) {
        if let Some(stage) = self
            .stages
            .rows
            .values_mut()
            .find(|stage| stage.viewer == Some(viewer))
        {
            stage.viewer = None;
            stage.pages.close();
            stage.layout = Layout::default();
        }
    }

    /// What the window draws of `tab`'s stage.
    pub fn stage_view(&self, tab: TabId) -> StageView {
        let Some(stage) = self.stages.rows.get(&tab) else {
            return StageView {
                layout: Layout::Console,
                ..StageView::default()
            };
        };
        StageView {
            layout: match stage.viewer {
                Some(_) => stage.layout,
                None => Layout::Console,
            },
            can_go_back: stage.pages.can_go_back(),
            can_go_forward: stage.pages.can_go_forward(),
        }
    }

    /// Show `layout` on the row in front. Without a page there is only the console.
    pub(super) fn set_layout(&mut self, layout: Layout) {
        let Some(tab) = self.ws.front() else { return };
        let Some(stage) = self.stages.rows.get_mut(&tab) else {
            return;
        };
        let Some(viewer) = stage.viewer else { return };
        stage.layout = layout;
        // The keys stay where they can be seen.
        let Some(keys) = self.keyboard_pane() else {
            return;
        };
        let to = match keys == viewer {
            true if !layout.shows_stage() => self.viewer_unfocus_to(viewer),
            false if !layout.shows_console() => Some(viewer),
            _ => None,
        };
        if let Some(pane) = to {
            self.act(WorkspaceIn::Focus(pane));
        }
    }

    /// The terminal the keyboard goes back to from `viewer`.
    fn viewer_unfocus_to(&self, viewer: PaneId) -> Option<PaneId> {
        match self.viewer_unfocus(viewer)? {
            WorkspaceIn::Focus(pane) => Some(pane),
            _ => None,
        }
    }

    /// Back (`back`) or Forward on the row in front: its page before or after this one opens.
    pub(super) fn walk_pages(&mut self, back: bool) {
        let Some(tab) = self.ws.front() else { return };
        let Some(stage) = self.stages.rows.get_mut(&tab) else {
            return;
        };
        let page = match back {
            true => stage.pages.back().cloned(),
            false => stage.pages.forward().cloned(),
        };
        let viewer = stage.viewer;
        let Some(page) = page else { return };
        // From the terminal the page came from, or the row's first.
        let from = viewer
            .and_then(|viewer| self.viewer_unfocus_to(viewer))
            .or_else(|| {
                let panes = self.ws.tabs.tab(tab)?.tree.panes();
                panes.into_iter().find(|pane| !self.is_viewer(*pane))
            });
        self.stages.walking = true;
        self.act(WorkspaceIn::OpenViewer {
            file: ViewerRef(page.to_string_lossy().into_owned()),
            from,
        });
        self.stages.walking = false;
    }
}

impl HubRef {
    /// See [`Hub::stage_view`].
    pub fn stage_view(&self, tab: TabId) -> StageView {
        self.0.borrow().stage_view(tab)
    }

    /// The file the page in the viewer `pane` is.
    pub fn page_file(&self, pane: PaneId) -> Option<String> {
        let hub = self.0.borrow();
        hub.ws
            .viewers
            .get(&pane)
            .map(|file| file.as_str().to_owned())
    }
}

impl Desk {
    /// Show `layout` on the row in front.
    pub fn set_layout(&self, layout: Layout) {
        self.settle(|hub| hub.set_layout(layout));
    }

    /// The page bar's Console button: the terminal beside the page, or hidden behind it.
    pub fn toggle_console(&self) {
        self.settle(|hub| {
            let now = hub
                .ws
                .front()
                .map(|tab| hub.stage_view(tab).layout)
                .unwrap_or_default();
            hub.set_layout(now.toggle_console());
        });
    }

    /// Back on the row in front.
    pub fn go_back(&self) {
        self.settle(|hub| hub.walk_pages(true));
    }

    /// Forward on the row in front.
    pub fn go_forward(&self) {
        self.settle(|hub| hub.walk_pages(false));
    }
}
