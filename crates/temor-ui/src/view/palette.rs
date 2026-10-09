//! The grid's colours follow the root's look and the environment (PLAN §5). The component sits
//! inside the `Ds` root, so it reads the Space look the root draws (`use_space_look`), and it
//! re-applies the palette whenever the look or the environment changes.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::ReadSignal;
use dioxus::prelude::*;
use ds::prelude::use_space_look;
use ds_settings::Environment;

use super::slot::{Slot, repaint};
use super::theme::resolved_palette;

/// The window's slot, shared with the palette component (a props value must compare).
#[derive(Clone, Debug)]
pub struct SlotRef(pub Rc<RefCell<Slot>>);

impl PartialEq for SlotRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The palette follower: a child of the `Ds` root that draws nothing.
#[component]
#[allow(non_snake_case)]
pub fn PaletteFollow(
    slot: SlotRef,
    env: ReadSignal<Environment>,
    notice: Signal<Option<String>>,
) -> Element {
    let look = use_space_look();
    let mut notice = notice;
    use_effect(use_reactive!(|look| {
        let now = env();
        let palette = resolved_palette(now.settings.appearance.appearance(), &look, now.system);
        if let Some(text) = slot.0.borrow_mut().set_palette(palette) {
            notice.set(Some(text));
        }
        repaint(&slot.0);
    }));
    rsx! {}
}
