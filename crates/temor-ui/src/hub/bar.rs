//! The ⌘T bar in the hub (PLAN §7): the model that walks the ranked rows, the candidates the
//! window offers it, and the requests the views send (open, type, move, pick, close). Carrying a
//! picked row out is `bar_run`.

use std::collections::BTreeMap;
use std::rc::Rc;

use ds::window::vocab::PageZoom;
use temor_core::bar::{BarContext, BarRow, Candidate, RowKey, Sources, Visits, candidates};
use temor_core::vocab::text::{CommandText, HostAlias};
use temor_pane::keys::{Action, terminal_shortcut};

use super::desk::Desk;
use super::dispatch::Origin;
use super::hosts::HostRows;
use super::model::{Hub, HubRef};
use crate::bar::{
    BAR_ACTIONS, BarModel, BarSeams, SpaceRows, closed_candidate, directory_uses, recent_commands,
};
use crate::view::launch::Launch;

/// What the hub keeps for the bar.
pub(super) struct BarSlot {
    pub(super) model: BarModel,
    pub(super) seams: BarSeams,
    /// How often each folder was opened from the bar, by path.
    pub(super) visits: BTreeMap<String, Visits>,
}

impl BarSlot {
    pub(super) fn new(launch: &Launch) -> BarSlot {
        BarSlot {
            model: BarModel::default(),
            seams: BarSeams::new(launch),
            visits: BTreeMap::new(),
        }
    }
}

/// What the bar's view draws.
#[derive(Debug, Clone, PartialEq)]
pub struct BarData {
    pub query: String,
    pub rows: Vec<BarRow>,
    /// The highlighted row's place, the palette's.
    pub selected: usize,
}

impl Hub {
    /// Replaces what the bar reads from outside (the Spaces, the model).
    pub fn set_bar_seams(&mut self, seams: BarSeams) {
        self.bar.seams = seams;
    }

    /// Lists these Spaces in the bar and switches through them.
    pub fn set_bar_spaces(&mut self, spaces: Rc<dyn SpaceRows>) {
        self.bar.seams.spaces = spaces;
    }

    /// Everything the bar can match right now.
    fn bar_candidates(&self) -> Vec<Candidate> {
        let seams = &self.bar.seams;
        let spaces = seams.spaces.spaces();
        // The hub's own host list: ssh config hosts and tailnet machines, each once.
        let hosts: Vec<HostAlias> = self
            .host_rows()
            .iter()
            .filter_map(|row| row.destination())
            .map(|alias| HostAlias(alias.as_str().to_owned()))
            .collect();
        let recent = recent_commands(self.block_commands());
        let now = (seams.now)();
        let directories = directory_uses(&self.bar.visits, self.ws.cwds.values(), now);
        // Pin Tab or Unpin Tab, whichever the tab in front can take.
        let pinned = self.front_pinned();
        let actions: Vec<Action> = BAR_ACTIONS
            .iter()
            .copied()
            .filter(|action| match action {
                Action::PinTab => pinned == Some(false),
                Action::UnpinTab => pinned == Some(true),
                _ => true,
            })
            .collect();
        let sources = Sources {
            actions: &actions,
            spaces: &spaces,
            tabs: &self.ws.tabs.place.tabs,
            folders: &self.ws.tabs.place.folders,
            hosts: &hosts,
            layouts: &[],
            recent: &recent,
            directories: &directories,
            now,
        };
        let mut all = candidates(&sources);
        all.extend(closed_candidate(&self.ws.closed));
        all.extend(self.local_tab_candidate());
        all
    }

    /// The commands every pane's blocks ran, newest first within a pane.
    fn block_commands(&self) -> Vec<CommandText> {
        let mut out = Vec::new();
        for slot in self.slots.values() {
            let slot = slot.borrow();
            let Some(session) = slot.session.as_ref() else {
                continue;
            };
            for (id, _) in session.block_outcomes().into_iter().rev() {
                if let Some(command) = session.block_command(id) {
                    out.push(CommandText(command.to_owned()));
                }
            }
        }
        out
    }

    /// Runs `step` on the bar with the context of now.
    pub(super) fn bar_apply(&mut self, step: impl FnOnce(BarModel, BarContext<'_>) -> BarModel) {
        let candidates = self.bar_candidates();
        let cx = BarContext {
            candidates: &candidates,
            resolver: &*self.bar.seams.paths,
            ai: self.bar.seams.model.available(),
        };
        let model = std::mem::take(&mut self.bar.model);
        self.bar.model = step(model, cx);
    }

    /// ⌘T.
    pub fn bar_toggle(&mut self) {
        self.plugins_stop();
        self.bar_apply(|model, cx| model.toggled(cx));
    }

    /// Escape, or a click outside the bar.
    pub fn bar_close(&mut self) {
        self.plugins_stop();
        self.bar_apply(|model, cx| model.closed(cx));
    }

    /// The field changed.
    pub fn bar_typed(&mut self, text: String) {
        let asked = text.clone();
        self.bar_apply(|model, cx| model.typed(text, cx));
        self.plugins_ask(&asked);
    }

    /// The highlight moved to the row at `index`.
    pub fn bar_select(&mut self, index: usize) {
        let model = std::mem::take(&mut self.bar.model);
        self.bar.model = model.selecting(index);
    }

    /// The highlight moved to `index` among the bar's rows and `extra` rows listed after them.
    pub fn bar_select_among(&mut self, index: usize, extra: usize) {
        let model = std::mem::take(&mut self.bar.model);
        self.bar.model = model.selecting_among(index, extra);
    }

    /// The row `key` was run (Enter, or a click): the bar closes and the row is carried out.
    pub fn bar_pick(&mut self, key: &RowKey) {
        let candidates = self.bar_candidates();
        let cx = BarContext {
            candidates: &candidates,
            resolver: &*self.bar.seams.paths,
            ai: self.bar.seams.model.available(),
        };
        let model = std::mem::take(&mut self.bar.model);
        let (model, row) = model.picked(key, cx);
        self.bar.model = model;
        if let Some(row) = row {
            self.run_bar_row(row);
        }
        self.plugins_stop();
    }

    /// What the bar's view draws, while it is open.
    pub fn bar_data(&self) -> Option<BarData> {
        self.bar.model.is_open().then(|| BarData {
            query: self.bar.model.query().to_owned(),
            rows: self.bar.model.rows().to_vec(),
            selected: self.bar.model.selected(),
        })
    }
}

impl HubRef {
    /// What the bar's view draws, while it is open.
    pub fn bar_data(&self) -> Option<BarData> {
        self.0.borrow().bar_data()
    }

    /// Whether the bar is open: its field has the keyboard, and no pane takes it back.
    pub fn bar_is_open(&self) -> bool {
        self.0.borrow().bar.model.is_open()
    }

    /// The first-launch hint for `pane`, in the person's own chords: only the first pane of a new
    /// window (never a restored one) has one.
    pub fn first_hint(&self, pane: temor_core::vocab::ids::PaneId) -> Option<String> {
        let hub = self.0.borrow();
        if hub.first_hint != Some(pane) {
            return None;
        }
        let keymap = &hub.launch.keymap;
        let bar = terminal_shortcut(keymap, Action::OpenBar)?;
        let split = terminal_shortcut(keymap, Action::SplitRight)?;
        Some(format!("{bar} for anything \u{b7} {split} to split"))
    }

    /// Lists these Spaces in the bar and switches through them.
    pub fn set_bar_spaces(&self, spaces: Rc<dyn SpaceRows>) {
        self.0.borrow_mut().set_bar_spaces(spaces);
    }

    /// The Spaces a quick terminal can be moved to.
    pub fn bar_spaces(&self) -> Vec<temor_core::bar::SpaceEntry> {
        self.0.borrow().bar.seams.spaces.spaces()
    }
}

impl Desk {
    /// Runs `change` on the hub, shows the notices it left, and draws the window again.
    pub(super) fn settle(&self, change: impl FnOnce(&mut Hub)) {
        let notes = {
            let mut hub = self.hub.0.borrow_mut();
            change(&mut hub);
            std::mem::take(&mut hub.notices)
        };
        for text in notes {
            self.say(text);
        }
        self.bump();
    }

    /// ⌘T.
    pub fn bar_toggle(&self) {
        self.settle(Hub::bar_toggle);
    }

    /// Escape, or a click outside the bar.
    pub fn bar_close(&self) {
        self.settle(Hub::bar_close);
    }

    /// The bar's field changed.
    pub fn bar_typed(&self, text: String) {
        self.settle(|hub| hub.bar_typed(text));
    }

    /// The bar's highlight moved.
    pub fn bar_select(&self, index: usize) {
        self.settle(|hub| hub.bar_select(index));
    }

    /// The bar's highlight moved, with `extra` rows listed after the bar's own.
    pub fn bar_select_among(&self, index: usize, extra: usize) {
        self.settle(|hub| hub.bar_select_among(index, extra));
    }

    /// A bar row was run.
    pub fn bar_pick(&self, key: &RowKey) {
        self.settle(|hub| hub.bar_pick(key));
        self.switch_to_picked_space();
        let asked = std::mem::take(&mut self.hub.0.borrow_mut().bar_actions);
        for action in asked {
            self.run_action(action, Origin::Bar);
        }
    }

    /// An action the window itself takes (`Owner::Window`), not a pane and not the workspace: the
    /// bar, the quick terminal, the sidebar, Downloads, Settings, the page's zoom, pinning the
    /// tab in front, the radar's jump and the Space switch.
    pub fn window_action(&self, action: Action) -> WindowAction {
        match action {
            Action::OpenBar => self.bar_toggle(),
            Action::ToggleQuick => self.quick_toggle(),
            Action::ToggleSidebar => self.sidebar_toggle(),
            Action::ShowDownloads => self.shelf_toggle(),
            Action::Settings => crate::settings_window::open(),
            Action::SettingsAt(pane) => crate::settings_window::open_at(pane),
            Action::ZoomIn => self.window.page_zoom(PageZoom::Bigger),
            Action::ZoomOut => self.window.page_zoom(PageZoom::Smaller),
            Action::ActualSize => self.window.page_zoom(PageZoom::ActualSize),
            Action::JumpToWaiting => self.jump_to_waiting(),
            Action::SelectSpace(number) => self.switch_to_number(number),
            Action::PinTab | Action::UnpinTab => {
                let input = self.hub.0.borrow().pin_input(action);
                if let Some(input) = input {
                    self.act(input);
                }
            }
            _ => return WindowAction::NotMine,
        }
        WindowAction::Taken
    }
}

/// Whether the window took an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    /// The window carried it out.
    Taken,
    /// It is the pane's or the workspace's.
    NotMine,
}
