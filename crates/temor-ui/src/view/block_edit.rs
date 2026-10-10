//! The block toolbar's "Edit here" (PLAN §9b), shown on panes on another computer: the selected
//! text, or the word under the pointer, names a file there; it is fetched and opened in an
//! editor on this computer, and saved back when the person saves.

use std::cell::RefCell;
use std::rc::Rc;

use temor_core::vocab::ids::{BlockId, PaneId};

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

/// The toolbar's press: hands the pane's file to the host-file editor.
pub fn bar_edit(
    slot: Rc<RefCell<Slot>>,
    desk: Desk,
    pane: PaneId,
) -> impl FnMut(BlockId) + 'static {
    move |_: BlockId| {
        let text = slot
            .borrow()
            .session
            .as_ref()
            .and_then(|session| session.text_to_edit());
        match file_to_edit(text) {
            Ok(text) => desk.host_edit(pane, &text),
            Err(words) => desk.say(words.to_owned()),
        }
        desk.bump();
    }
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
