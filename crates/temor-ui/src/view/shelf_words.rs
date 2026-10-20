//! What the Downloads list says about each thing a person can do with a file: one label for the
//! button, the menu item and the hover on its own, and the hover hint.

use temor_core::shelf::RowAction as Act;

use crate::hub::PaneTarget;

/// What a row's X says and does for assistive technology, on a download and on an upload alike.
pub const REMOVE_FROM_LIST: &str = "Remove from List";

/// The words for one action of a file: its label (a button's, a menu item's) and its hover hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActWords {
    pub label: String,
    pub tip: String,
}

impl ActWords {
    fn new(label: impl Into<String>, tip: impl Into<String>) -> Option<Self> {
        Some(Self {
            label: label.into(),
            tip: tip.into(),
        })
    }
}

/// The words for `act`, with `target` the terminal that has the keyboard. `None` where the action
/// has nothing to say: Use in pane, when no terminal has the keyboard.
pub fn act_words(act: Act, target: &PaneTarget) -> Option<ActWords> {
    match act {
        Act::Open => ActWords::new("Open", "Open the file"),
        Act::Reveal => ActWords::new("Show in Folder", "Show the file in your files"),
        Act::CopyPath => ActWords::new("Copy Path", "Copy the file's path"),
        Act::ShowInTab(_) => ActWords::new("Show Tab", "Go back to the tab it came from"),
        Act::Clear => ActWords::new(REMOVE_FROM_LIST, "Take it off the list; the file stays"),
        Act::UseInPane => match target {
            PaneTarget::Nowhere => None,
            PaneTarget::Here => ActWords::new(
                "Paste Path",
                "Type the file's path into the terminal you were using",
            ),
            PaneTarget::Host(host) => ActWords::new(
                format!("Send to {host}"),
                format!("Copy the file to {host}, into the folder of the terminal you were using"),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_one_label_for_its_button_and_its_menu_and_use_in_pane_needs_a_terminal() {
        let label = |act, target: &PaneTarget| act_words(act, target).map(|w| w.label);
        let here = PaneTarget::Here;
        let table = [
            ("open", label(Act::Open, &here), Some("Open")),
            ("reveal", label(Act::Reveal, &here), Some("Show in Folder")),
            ("clear", label(Act::Clear, &here), Some("Remove from List")),
            (
                "paste here",
                label(Act::UseInPane, &here),
                Some("Paste Path"),
            ),
            (
                "no terminal",
                label(Act::UseInPane, &PaneTarget::Nowhere),
                None,
            ),
        ];
        for (name, got, want) in table {
            assert_eq!(got.as_deref(), want, "{name}");
        }
    }
}
