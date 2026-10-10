//! What the person is told when the workspace refuses a request. Plain words, no jargon.

use temor_core::workspace::RefuseWhy;

/// The words for a refusal.
pub fn refusal_text(why: &RefuseWhy) -> String {
    match why {
        RefuseWhy::TabFull => "This tab has no room for another pane".to_owned(),
        RefuseWhy::NoSuchPane(_) | RefuseWhy::NoSuchTab(_) | RefuseWhy::NoFront => {
            "That is no longer open".to_owned()
        }
        RefuseWhy::SameTab => "A tab cannot be dropped onto itself".to_owned(),
        RefuseWhy::PinnedTab(_) => "Unpin the tab before joining it to another".to_owned(),
        RefuseWhy::OnlyPane(_) => "This is the only pane in its tab".to_owned(),
        RefuseWhy::NothingClosed => "There is no closed tab to reopen".to_owned(),
        RefuseWhy::UseCloseTab => "Close the tab to remove it".to_owned(),
        RefuseWhy::NoFile => "There is no file to show".to_owned(),
        RefuseWhy::IdsExhausted => {
            "temor has opened too many panes; restart it to go on".to_owned()
        }
        _ => "temor can't do that here".to_owned(),
    }
}
