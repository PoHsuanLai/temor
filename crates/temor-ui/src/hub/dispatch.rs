//! The one way an action runs (PLAN §6, §7): a terminal's key, a viewer's key, the ⌘T bar, a menu
//! and the window's own controls all end in [`Desk::run_action`]. `Action::owner` says whose it
//! is: the pane's own, the workspace's, or the window's. A route differs only in what it knows
//! about where the action came from, which is [`Origin`].

use temor_pane::keys::{Action, Owner};

use super::desk::Desk;
use super::keys::workspace_input;
use super::model::Hub;

/// Where an action came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A key in a terminal. The terminal has run the pane's own actions by now.
    Terminal,
    /// A key in the quick terminal, which has run its own actions too, and has no tabs or splits.
    Quick,
    /// A key in a file viewer, which has no pane actions of its own.
    Viewer,
    /// A row of the ⌘T bar, which has closed: the pane that has the keyboard is the one it is for.
    Bar,
    /// A menu item or a button of the window.
    Menu,
}

impl Origin {
    /// Whether the person acts on the quick terminal: they typed at it, or the bar they opened
    /// over it picked the row. A key in a pane behind it is that pane's.
    pub(super) fn on_quick(self, quick_shown: bool) -> bool {
        match self {
            Origin::Quick => true,
            Origin::Bar => quick_shown,
            Origin::Terminal | Origin::Viewer | Origin::Menu => false,
        }
    }
}

impl Desk {
    /// Carries out `action`, from `from`. Shows what it has to say and draws the window again.
    pub fn run_action(&self, action: Action, from: Origin) {
        match (action, from) {
            // The viewer's own find lives in the bar, as its "In This File" group.
            (Action::Find, Origin::Viewer) => self.find_in_viewer(),
            (Action::Find, Origin::Bar | Origin::Menu) if self.hub.keyboard_viewer().is_some() => {
                self.find_in_viewer();
            }
            _ => match action.owner() {
                Owner::Window => {
                    let _ = self.window_action(action);
                }
                Owner::Workspace => self.run_workspace_action(action, from),
                Owner::Pane => match from {
                    Origin::Bar | Origin::Menu => {
                        self.settle(|hub| hub.run_in_keyboard_pane(action))
                    }
                    // A terminal ran its own already; a viewer has none.
                    Origin::Terminal | Origin::Quick | Origin::Viewer => {}
                },
            },
        }
    }

    fn find_in_viewer(&self) {
        self.hub.want_find_in_viewer();
        self.bar_toggle();
    }

    fn run_workspace_action(&self, action: Action, from: Origin) {
        // The quick terminal has no tabs or splits of its own.
        if from.on_quick(self.hub.quick_shown().is_some()) {
            if from == Origin::Bar {
                self.say("The quick terminal has no tabs or splits of its own.".to_owned());
            }
            return;
        }
        if let Some(input) = workspace_input(action) {
            self.act(input);
        }
    }
}

impl Hub {
    /// Has the pane with the keyboard run its own `action`. Whatever it says, or fails with,
    /// is left for the person in the notices, as a key would show it.
    pub(super) fn run_in_keyboard_pane(&mut self, action: Action) {
        let Some(slot) = self.keyboard_pane().and_then(|pane| self.slots.get(&pane)) else {
            return;
        };
        let mut said = Vec::new();
        if let Some(session) = slot.borrow_mut().session.as_mut() {
            if let Err(err) = session.run(action) {
                said.push(err.to_string());
            }
            said.extend(session.take_notice());
        }
        self.notices.extend(said);
    }
}
