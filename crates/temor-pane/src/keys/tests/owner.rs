//! Who owns each action, and the words a menu uses for it.

use super::super::{Action, Owner, bindable};

#[test]
fn every_action_has_one_owner_and_the_numbered_and_unbound_ones_have_it_too() {
    use Owner::{Pane, Window, Workspace};
    #[rustfmt::skip]
    let rows: &[(Action, Owner)] = &[
        (Action::Copy, Pane),
        (Action::FindNext, Pane),
        (Action::CloseFind, Pane),
        (Action::ReinputCommands, Pane),
        (Action::NewTab, Workspace),
        (Action::ClosePane, Workspace),
        (Action::ReopenTab, Workspace),
        (Action::SelectTab(3), Workspace),
        (Action::OpenBar, Window),
        (Action::ToggleQuick, Window),
        (Action::PinTab, Window),
        (Action::JumpToWaiting, Window),
        (Action::SelectSpace(2), Window),
    ];
    for (action, owner) in rows {
        assert_eq!(action.owner(), *owner, "{action:?}");
    }
    // The tabs are the workspace's whatever their number.
    assert!(
        bindable()
            .into_iter()
            .filter(|action| matches!(action, Action::SelectTab(_)))
            .all(|action| action.owner() == Workspace)
    );
}

#[test]
fn a_menu_says_what_the_bar_says_in_title_case_except_where_the_menu_is_plainer() {
    #[rustfmt::skip]
    let rows: &[(Action, &str)] = &[
        (Action::NewTab, "New Tab"),
        (Action::ReopenTab, "Reopen Closed Tab"),
        (Action::ToggleSidebar, "Hide Sidebar"),
        (Action::ReinputCommands, "Put in Prompt"),
        (Action::CopyOutputs, "Copy Outputs"),
        (Action::PinTab, "Pin Tab"),
        (Action::UnpinTab, "Unpin Tab"),
        (Action::ClosePane, "Close Pane"),
        (Action::SelectAllBlocks, "Select All Commands"),
    ];
    for (action, title) in rows {
        assert_eq!(action.menu_title(), *title, "{action:?}");
    }
}
