//! A chord the terminal does not take goes the way every action goes (`Desk::run_action`): in
//! ds-harness (event dispatch only, never OS input), in scratch folders.

use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds::components::app::spaces::{SpaceId, Spaces};
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_core::space::Project;
use temor_ui::edge::save_spaces;
use temor_ui::view::probe::Probe;

use super::drive::wait_until;
use super::support::scratch;
use super::window::harness_in;

fn head(h: &Harness) -> String {
    h.text_of(".ds-space-head")
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// ⌃2 typed in a terminal switches to the second Space, as the bar's Space row does.
#[test]
fn a_space_chord_typed_in_a_terminal_switches_the_space() {
    if !super::support::tools_present() {
        return;
    }
    let state = scratch("actions-state").join("state");
    let mut spaces = Spaces::first_run([Project::default(), Project::default()], Project::default);
    spaces.rename(SpaceId(0), "Home");
    spaces.rename(SpaceId(1), "Elsewhere");
    save_spaces(&state, &spaces).expect("spaces saved");
    let probe = Probe::new();
    let mut h = harness_in("actions-space-chord", &probe, Some(state));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_eq!(head(&h), "Home");

    h.send(Input::click(Point {
        x: Px(500.0),
        y: Px(250.0),
    }));
    h.advance(Duration::from_millis(200));
    h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('2')));
    h.advance(Duration::from_millis(400));
    assert_eq!(head(&h), "Elsewhere", "{}", h.html());
}
