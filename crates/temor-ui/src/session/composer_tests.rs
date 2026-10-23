//! The composer's history from plain lists: no session, no shell.

use super::*;

#[test]
fn the_history_is_newest_first_each_command_once_where_it_last_ran() {
    let commands = ["ls", "git status", "ls", "", "make  ", "make"];
    assert_eq!(
        newest_first(commands.into_iter()),
        ["make", "ls", "git status"]
    );
}

#[test]
fn stepping_goes_back_to_the_oldest_and_forward_past_the_newest_to_what_was_typed() {
    use HistoryStep::{Newer, Older};
    assert_eq!(stepped(None, Older, 3), Some(0));
    assert_eq!(stepped(Some(0), Older, 3), Some(1));
    assert_eq!(stepped(Some(2), Older, 3), Some(2), "stays at the oldest");
    assert_eq!(stepped(Some(1), Newer, 3), Some(0));
    assert_eq!(stepped(Some(0), Newer, 3), None, "back to what was typed");
    assert_eq!(stepped(None, Newer, 3), None);
    assert_eq!(stepped(None, Older, 0), None, "nothing to step through");
}
