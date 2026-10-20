//! The action table: every action the keymap can bind has its owner's route.

use temor_pane::keys::{Action, Owner, bindable};

use super::dispatch::Origin;
use super::keys::workspace_input;

#[test]
fn every_workspace_action_makes_a_request_and_no_other_owners_action_does() {
    let every = bindable()
        .into_iter()
        .chain([Action::CloseFind, Action::SettingsAt(Default::default())]);
    for action in every {
        let made = workspace_input(action);
        match action.owner() {
            Owner::Workspace => assert!(made.is_some(), "{action:?} makes no request"),
            Owner::Pane | Owner::Window => assert!(made.is_none(), "{action:?}"),
        }
    }
}

/// Only the quick terminal itself, or the bar opened over it, is refused tabs and splits. A key
/// in a pane behind the quick terminal still splits or closes that pane's tab.
#[test]
fn only_the_quick_terminal_and_the_bar_over_it_are_refused_tabs_and_splits() {
    #[rustfmt::skip]
    let cases = [
        (Origin::Quick,    true,  true),
        (Origin::Terminal, true,  false),
        (Origin::Terminal, false, false),
        (Origin::Bar,      true,  true),
        (Origin::Bar,      false, false),
        (Origin::Menu,     true,  false),
        (Origin::Viewer,   true,  false),
    ];
    for (from, shown, refused) in cases {
        assert_eq!(
            from.on_quick(shown),
            refused,
            "{from:?}, quick shown: {shown}"
        );
    }
}
