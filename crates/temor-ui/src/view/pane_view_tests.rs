//! A pane's view hands its texture over when the pane moves: a split mounts the new view before
//! the old one is dropped.

use std::cell::RefCell;

use dioxus::prelude::*;
use ds_blitz::use_gpu;

use super::super::slot::Slot;
use super::{hide, show};

thread_local! {
    static OUTCOME: RefCell<Option<(bool, bool)>> = const { RefCell::new(None) };
}

/// Two views of one pane: the new one shows, then the old one is dropped.
#[allow(non_snake_case)]
fn MovedPane() -> Element {
    let gpu = use_gpu();
    let (old, new) = (gpu.handle(), gpu.handle());
    let slot = RefCell::new(Slot::default());
    show(&slot, old.clone());
    show(&slot, new.clone());
    hide(&slot, &old);
    let s = slot.borrow();
    OUTCOME.with(|o| *o.borrow_mut() = Some((s.texture.as_ref() == Some(&new), s.shown)));
    rsx! {}
}

/// The view that is going away again: now the pane is off the screen.
#[allow(non_snake_case)]
fn ClosedPane() -> Element {
    let gpu = use_gpu();
    let only = gpu.handle();
    let slot = RefCell::new(Slot::default());
    show(&slot, only.clone());
    hide(&slot, &only);
    let s = slot.borrow();
    OUTCOME.with(|o| *o.borrow_mut() = Some((s.texture.is_none(), s.shown)));
    rsx! {}
}

fn run(app: fn() -> Element) -> (bool, bool) {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    OUTCOME
        .with(|o| o.borrow_mut().take())
        .expect("the view ran")
}

#[test]
fn the_old_view_going_away_leaves_the_new_views_texture() {
    assert_eq!(run(MovedPane), (true, true));
}

#[test]
fn the_last_view_going_away_takes_the_pane_off_the_screen() {
    assert_eq!(run(ClosedPane), (true, false));
}
