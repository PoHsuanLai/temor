//! The hub's state: the workspace, one slot per pane, the scrollback of recently closed panes,
//! and the channel every pane's events arrive on.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use temor_core::quick::QuickState;
use temor_core::split::SplitTree;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::{ClosedState, KeepClosed, Workspace};
use temor_pane::PaneEvent;
use temor_render::Palette;
use temor_vt::Snapshot;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::agents::Agents;
use super::bar::BarSlot;
use super::facts::FactsHub;
use super::offer::OfferHub;
use super::peek::PeekHub;
use super::remote::RemoteHub;
use super::run::RunHub;
use super::shelf::ShelfHub;
use super::spaces::SpaceBook;
use super::ssh::SshHub;
use super::viewers::ViewerHub;
use crate::edge::SavePace;
use crate::plugins::PluginHub;
use crate::view::launch::Launch;
use crate::view::slot::Slot;

/// The longest the session file waits for a change to be written.
pub(super) const SAVE_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// Everything the window keeps about its panes. Lives on the window's thread.
pub struct Hub {
    /// The workspace of the Space on screen; the other Spaces' are in `book`.
    pub(super) ws: Workspace,
    pub(super) book: SpaceBook,
    pub(super) slots: BTreeMap<PaneId, Rc<RefCell<Slot>>>,
    /// The scrollback of panes in recently closed tabs, kept until the tab is forgotten.
    pub(super) recent: BTreeMap<PaneId, Snapshot>,
    /// The kept shell state of panes about to start again (after a restore, or a reopened tab):
    /// the new shell is given its variables.
    pub(super) carried: BTreeMap<PaneId, temor_core::session::Checkpoint>,
    /// What each pane's checkpoint file holds now, so only a change is written.
    pub(super) on_disk: BTreeMap<PaneId, temor_core::session::Checkpoint>,
    pub(super) events: UnboundedSender<(PaneId, PaneEvent)>,
    pub(super) inbox: Option<UnboundedReceiver<(PaneId, PaneEvent)>>,
    pub(super) launch: Launch,
    pub(super) state_dir: Option<PathBuf>,
    /// The viewer's store (shared with anyview), for a run that keeps its state.
    pub(super) viewer_store: Option<PathBuf>,
    pub(super) palette: Palette,
    pub(super) pace: SavePace,
    pub(super) notices: Vec<String>,
    /// Actions picked in the command bar, which the desk runs once the pick has settled: the
    /// window runs all of them one way (`Desk::run_action`).
    pub(super) bar_actions: Vec<temor_pane::keys::Action>,
    pub(super) bar: BarSlot,
    /// The quick terminal: never part of the workspace, so never saved with the session.
    pub(super) quick: QuickState,
    /// The downloads shelf and the questions programs have put to the person.
    pub(super) shelf: ShelfHub,
    pub(super) agents: Agents,
    pub(super) ssh: SshHub,
    /// What the panes' folders have in them (branch, changes, environments).
    pub(super) facts: FactsHub,
    /// What plugins gave the window and the way to ask them (PLAN P11).
    pub(super) plugins: PluginHub,
    /// The shell offer's state for every pane.
    pub(super) offer: OfferHub,
    /// Sessions on hosts' helpers (Tier B).
    pub(super) remote: RemoteHub,
    /// The panes that show a file (PLAN §10).
    pub(super) viewers: ViewerHub,
    pub(super) stages: super::stage::Stages,
    /// The peek card's worker (PLAN §10).
    pub(super) peek: PeekHub,
    /// Programs run in tabs of their own in place of the shell (`temor -e`).
    pub(super) run: RunHub,
    /// Running programs moved to terminals of their own, shown as live cards.
    pub(super) live_cards: super::live_cards::LiveCards,
    /// The apps programs belong to, found as tabs run them (PLAN P22).
    pub(super) app_icons: RefCell<crate::edge::app_icons::AppIcons>,
    pub(super) icon_pack: crate::edge::icon_pack::IconPack,
    pub(super) history: super::history::History,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub")
            .field("panes", &self.slots.len())
            .finish_non_exhaustive()
    }
}

impl Hub {
    /// A hub with no panes yet; [`Hub::start`] restores the last session or opens a first tab.
    pub fn new(launch: Launch) -> Hub {
        let (events, inbox) = unbounded_channel();
        let (shelf, trouble) = ShelfHub::open(&launch);
        let ambient = &launch.ambient;
        let (icon_pack, pack_trouble) =
            crate::edge::icon_pack::IconPack::load(&ambient.config_dir, &ambient.home);
        let notices = launch
            .first_words
            .take()
            .into_iter()
            .chain(trouble)
            .chain(pack_trouble)
            .collect();
        let agents = Agents::new(&launch, events.clone());
        let ssh = SshHub::new(&launch.ambient);
        let facts = FactsHub::new();
        let plugins = PluginHub::new(&launch);
        let offer = OfferHub::new(&launch);
        let remote = RemoteHub::new(&launch.ambient, launch.state_dir.clone());
        Hub {
            ws: Workspace::new(ClosedState::new(KeepClosed::DEFAULT)),
            book: SpaceBook::new(),
            slots: BTreeMap::new(),
            recent: BTreeMap::new(),
            carried: BTreeMap::new(),
            on_disk: BTreeMap::new(),
            events,
            inbox: Some(inbox),
            bar: BarSlot::new(&launch),
            quick: QuickState::Closed,
            state_dir: launch.state_dir.clone(),
            viewer_store: launch
                .state_dir
                .as_ref()
                .map(|_| launch.ambient.viewer_store()),
            launch,
            palette: Palette::default(),
            pace: SavePace::new(SAVE_EVERY),
            notices,
            bar_actions: Vec::new(),
            shelf,
            agents,
            ssh,
            facts,
            plugins,
            offer,
            remote,
            viewers: ViewerHub::default(),
            stages: super::stage::Stages::default(),
            peek: PeekHub::default(),

            run: RunHub::default(),
            live_cards: Default::default(),
            app_icons: RefCell::default(),
            icon_pack,
            history: Default::default(),
        }
    }

    /// The slot of `pane`, if it has one.
    pub fn slot(&self, pane: PaneId) -> Option<Rc<RefCell<Slot>>> {
        self.slots.get(&pane).cloned()
    }

    /// The workspace, for reading.
    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    /// The pane that has the keyboard: the quick terminal's while it is shown, else the focused
    /// pane of the tab in front.
    pub fn keyboard_pane(&self) -> Option<PaneId> {
        self.quick_shown()
            .or_else(|| self.ws.front().and_then(|tab| self.ws.focus_of(tab)))
    }

    /// The next message for the person, if one is waiting; taking it clears it.
    pub fn take_notice(&mut self) -> Option<String> {
        match self.notices.is_empty() {
            true => None,
            false => Some(self.notices.remove(0)),
        }
    }
}

/// A shared hub, as the views hold it. Two are equal when they are the same hub.
#[derive(Clone)]
pub struct HubRef(pub Rc<RefCell<Hub>>);

impl PartialEq for HubRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for HubRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HubRef")
    }
}

impl HubRef {
    /// The slot of `pane`, if it has one.
    pub fn slot(&self, pane: PaneId) -> Option<Rc<RefCell<Slot>>> {
        self.0.borrow().slot(pane)
    }

    /// The pane that has the keyboard.
    pub fn keyboard_pane(&self) -> Option<PaneId> {
        self.0.borrow().keyboard_pane()
    }

    /// The tab in front with its panes and the one that has the keys.
    pub fn front(&self) -> Option<(TabId, SplitTree, PaneId)> {
        let hub = self.0.borrow();
        let tab = hub.ws.front()?;
        let tree = hub.ws.tabs.tab(tab)?.tree.clone();
        Some((tab, tree, hub.ws.focus_of(tab)?))
    }

    /// The pane with the keys in `tab`.
    pub fn focus_pane_of(&self, tab: TabId) -> Option<PaneId> {
        self.0.borrow().ws.focus_of(tab)
    }
}
