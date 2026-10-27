//! "Edit Locally" (PLAN §9b), on the block toolbar and the block menu of panes on another
//! computer: the selected text, or the last file the block's command names, names a file there;
//! it is fetched and opened in an editor on this computer, and saved back when the person saves.

use std::cell::RefCell;
use std::rc::Rc;

use temor_client::model::vocab::ids::{BlockId, PaneId};

use super::slot::Slot;
use crate::hub::Desk;

/// What to say when nothing names a file.
const NOTHING_NAMED: &str = "Select the file's name in the output first";

/// Which file to fetch for the pane's selection, or what to tell the person.
pub fn file_to_edit(text: Option<String>) -> Result<String, &'static str> {
    match text.map(|t| t.trim().to_owned()) {
        Some(text) if !text.is_empty() => Ok(text),
        _ => Err(NOTHING_NAMED),
    }
}

/// "Edit Locally", for the toolbar and the block menu alike: hands the file the selection or the
/// block's command names to the host-file editor. `block` is the block the toolbar is on; the menu
/// passes `None` and gets its picked block.
pub fn edit_on_this_computer(
    slot: &Rc<RefCell<Slot>>,
    desk: &Desk,
    pane: PaneId,
    block: Option<BlockId>,
) {
    let text = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|session| session.block_text_to_edit(block));
    match file_to_edit(text) {
        Ok(text) => desk.host_edit(pane, &text),
        Err(words) => desk.say(words.to_owned()),
    }
    desk.bump();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_some_text_names_a_file_to_fetch() {
        let cases = [
            (
                "a path",
                Some(" src/main.rs ".to_owned()),
                Ok("src/main.rs".to_owned()),
            ),
            ("blank", Some("  ".to_owned()), Err(NOTHING_NAMED)),
            ("nothing", None, Err(NOTHING_NAMED)),
        ];
        for (name, text, want) in cases {
            assert_eq!(file_to_edit(text), want, "{name}");
        }
    }
}
