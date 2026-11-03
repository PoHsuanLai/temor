//! What a view holds to ask the window for things: the hub, the signal that redraws the window,
//! and the signal the message bar reads. Cloned into every pane view and the sidebar.

use dioxus::prelude::*;
use ds::base::vocab::Shown;
use ds::window::host::WindowHost;
use ds::window::vocab::PageZoom;
use temor_client::model::vocab::ids::{FolderId, PaneId, TabId};
use temor_client::model::vocab::text::TabTitle;
use temor_client::model::workspace::{TabsIn, WorkspaceIn};

use super::model::HubRef;
use super::space_rows::PendingSwitch;
use temor_ui_base::space_handle::SpacesOf;

/// What a folder is called until it is named.
pub const NEW_FOLDER: &str = "New Folder";

/// The window's desk. Two desks are equal when they are the same window's.
#[derive(Clone, PartialEq)]
pub struct Desk {
    pub hub: HubRef,
    /// Bumped to draw the window again, as the hub changes (it is told of each change) and as a
    /// pane's own view state does; views read it to follow the hub.
    pub ui: Signal<u32>,
    /// The message the person sees above the panes.
    pub notice: temor_ui_base::notices::Notices,
    /// The Spaces: the kit's handle, which switches, edits and writes them.
    pub spaces: SpacesOf,
    /// The Space the ⌘T bar asked for, until the desk switches to it.
    pub picked_space: PendingSwitch,
    /// Whether the sidebar is shown: the window's, which the show/hide action flips.
    pub sidebar: Signal<Shown>,
    /// The window itself, which zooms the page; none where nothing hosts the window.
    pub window: HostOf,
    /// The folder of the sidebar being named where it stands (one just made, or Rename Folder).
    pub naming: Signal<Option<FolderId>>,
    /// Bumped to give the keyboard back to the pane that has it: a pick in a menu leaves it
    /// nowhere (Blitz ends a click on a plain element by clearing the focus).
    pub regain: Signal<u32>,
}

/// The window's host, if there is one. Always equal: a desk holds its own window's.
#[derive(Debug, Clone, Default)]
pub struct HostOf(pub Option<WindowHost>);

impl PartialEq for HostOf {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl HostOf {
    /// Zooms the page one step, or back to its own size.
    pub fn page_zoom(&self, step: PageZoom) {
        if let Some(host) = &self.0 {
            host.page_zoom(step);
        }
    }
}

impl std::fmt::Debug for Desk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desk").finish_non_exhaustive()
    }
}

/// The part of the hub `read` takes for `given` (the props it depends on), as a memo: it follows
/// each change to the hub and to `given`, and what reads it draws again only when that part
/// changed.
pub fn use_hub<D: PartialEq + Clone + 'static, T: PartialEq + 'static>(
    desk: &Desk,
    given: D,
    read: impl Fn(&HubRef, &D) -> T + 'static,
) -> Memo<T> {
    let desk = desk.clone();
    use_memo(use_reactive((&given,), move |(given,)| {
        let _ = (desk.ui)();
        read(&desk.hub, &given)
    }))
}

impl Desk {
    /// Draws the window again.
    pub fn bump(&self) {
        let mut ui = self.ui;
        ui += 1;
    }

    /// Gives the keyboard back to the pane that has it, as a menu's pick ends.
    pub fn keyboard_back(&self) {
        let mut regain = self.regain;
        regain += 1;
    }

    /// Shows `text` to the person.
    pub fn say(&self, text: String) {
        self.notice.say(text);
    }

    /// Makes a folder, at the top or in `parent`, with `tab` in it when one is given, and names
    /// it where it stands, as Finder names a new folder.
    pub fn new_folder(&self, parent: Option<FolderId>, tab: Option<TabId>) {
        let folder = self.hub.next_folder();
        let title = TabTitle(NEW_FOLDER.to_owned());
        self.act(WorkspaceIn::Tabs(TabsIn::NewFolder { title, parent }));
        if !self.hub.has_folder(folder) {
            return;
        }
        if let Some(tab) = tab {
            let folder = Some(folder);
            self.act(WorkspaceIn::Tabs(TabsIn::MoveToFolder { tab, folder }));
        }
        let mut naming = self.naming;
        naming.set(Some(folder));
    }

    /// Shows `notes` to the person.
    pub fn tell(&self, notes: impl IntoIterator<Item = String>) {
        for text in notes {
            self.say(text);
        }
    }

    /// Runs a workspace request and shows what it has to say; the hub draws the window again.
    pub fn act(&self, input: WorkspaceIn) {
        self.tell(self.hub.act(input));
    }

    /// Starts a new shell in a pane whose program ended.
    pub fn restart(&self, pane: PaneId) {
        self.tell(self.hub.restart(pane));
    }
}
