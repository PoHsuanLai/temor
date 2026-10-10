//! The ⌘T bar's Space rows (PLAN §7): the bar lists the kit's Spaces and asks for a switch. The
//! switch cannot happen inside the bar's pick (the hub is borrowed, and the kit calls back into
//! it), so the pick leaves the Space it wants in a [`PendingSwitch`] and the desk carries it out
//! once the hub is free.

use std::cell::Cell;
use std::rc::Rc;

use dioxus::prelude::ReadableExt;
use ds::components::app::spaces::SpaceId;
use temor_core::bar::{SpaceEntry, space_entries};

use super::desk::Desk;
use crate::bar::SpaceRows;
use crate::view::space_handle::SpacesOf;

/// The Space the bar asked for, until the desk switches to it. Shared by the rows and the desk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingSwitch(Rc<Cell<Option<SpaceId>>>);

impl PendingSwitch {
    fn ask(&self, space: SpaceId) {
        self.0.set(Some(space));
    }

    fn take(&self) -> Option<SpaceId> {
        self.0.take()
    }
}

/// The bar's view of the kit's Spaces.
pub struct KitSpaces {
    handle: SpacesOf,
    pending: PendingSwitch,
}

impl std::fmt::Debug for KitSpaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KitSpaces").finish_non_exhaustive()
    }
}

impl KitSpaces {
    pub fn new(handle: SpacesOf, pending: PendingSwitch) -> KitSpaces {
        KitSpaces { handle, pending }
    }
}

impl SpaceRows for KitSpaces {
    fn spaces(&self) -> Vec<SpaceEntry> {
        space_entries(self.handle.spaces().peek().list())
    }

    fn go_to(&self, space: SpaceId) -> Result<(), String> {
        self.pending.ask(space);
        Ok(())
    }
}

impl Desk {
    /// Switches to the Space the bar's last pick asked for, if it asked for one.
    pub(super) fn switch_to_picked_space(&self) {
        let Some(space) = self.picked_space.take() else {
            return;
        };
        if space != self.spaces.current() && self.spaces.switch(space).is_none() {
            self.say("Finish changing the Space first, then switch.".to_owned());
        }
        self.bump();
    }
}
