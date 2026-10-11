//! The first-launch hint and the actions the bar runs: pure parts of the hub.

use std::cell::RefCell;
use std::rc::Rc;

use temor_core::vocab::ids::PaneId;
use temor_pane::keys::{Action, terminal_shortcut};

use super::model::HubRef;
use super::tests::hub_after;

#[test]
fn the_hint_is_the_first_pane_of_a_new_window_in_the_persons_own_chords() {
    let hub = HubRef(Rc::new(RefCell::new(hub_after(Vec::new()))));
    let keymap = hub.0.borrow().launch.keymap.clone();
    let bar = terminal_shortcut(&keymap, Action::OpenBar).expect("the bar has a chord");
    let split = terminal_shortcut(&keymap, Action::SplitRight).expect("split has a chord");
    let line = format!("{bar} for anything \u{b7} {split} to split");

    #[rustfmt::skip]
    let rows: &[(&str, Option<PaneId>, PaneId, Option<&str>)] = &[
        ("a new window's first pane", Some(PaneId(1)), PaneId(1), Some(&line)),
        ("another pane", Some(PaneId(1)), PaneId(2), None),
        ("a restored session", None, PaneId(1), None),
    ];
    for (name, first, asked, want) in rows {
        hub.0.borrow_mut().first_hint = *first;
        assert_eq!(hub.first_hint(*asked).as_deref(), *want, "{name}");
    }
}
