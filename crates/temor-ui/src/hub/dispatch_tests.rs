//! The action table: every action the keymap can bind has its owner's route.

use temor_pane::keys::{Action, Owner, bindable};

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
