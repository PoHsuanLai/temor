//! The one way an action runs (PLAN §6, §7): a terminal's key, a viewer's key, the ⌘T bar, a menu
//! and the window's own controls all end in [`Desk::run_action`]. `Action::owner` says whose it
//! is: the pane's own, the workspace's, or the window's. A route differs only in what it knows
//! about where the action came from, which is [`Origin`].

use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::ids::PaneId;
use temor_pane_view::session::Left;
use temor_panes::pane::keys::{Action, Owner};

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
                        let mut ran = None;
                        self.settle(|hub| ran = hub.run_in_keyboard_pane(action));
                        if let Some((pane, left)) = ran {
                            self.carry_out(pane, left);
                        }
                    }
                    // A terminal ran its own already; a viewer has none.
                    Origin::Terminal | Origin::Quick | Origin::Viewer => {}
                },
            },
        }
    }

    /// Says the message, opens the file and runs the action a call on `pane`'s session left.
    /// Whether an action ran: the workspace draws what it changes, and the keyboard may have
    /// left the pane.
    pub fn carry_out(&self, pane: PaneId, left: Left) -> bool {
        let Left {
            notice,
            asked,
            view,
        } = left;
        if let Some(text) = notice {
            self.say(text);
        }
        if let Some(ask) = view {
            // A file on another computer is fetched first; the viewer opens when it has arrived.
            let notes = match ask.site {
                PaneSite::Local => self.hub.open_viewer(pane, ask),
                PaneSite::Remote => self.hub.open_host_viewer(pane, &ask.file.to_string_lossy()),
            };
            self.tell(notes);
            self.bump();
        }
        // The quick terminal has no tabs or splits of its own.
        let Some(action) = asked else {
            return false;
        };
        let from = match self.hub.is_quick(pane) {
            true => Origin::Quick,
            false => Origin::Terminal,
        };
        self.run_action(action, from);
        true
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
    /// Has the pane with the keyboard run its own `action`: what it fails with is left for the
    /// person in the notices, and what its session left is the pane's, to be carried out.
    pub(super) fn run_in_keyboard_pane(&mut self, action: Action) -> Option<(PaneId, Left)> {
        let pane = self.keyboard_pane()?;
        let slot = self.panes.slot(pane)?;
        let mut slot = slot.borrow_mut();
        let (ran, left) = slot.session.as_mut()?.said(|session| session.run(action));
        self.notices.extend(ran.err().map(|err| err.to_string()));
        Some((pane, left))
    }
}
