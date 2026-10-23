//! A card's "more" menu (PLAN §9 blocks): copy the command, save the output to Downloads, show
//! the files it listed, and on a pane on another computer, edit it locally.

use ds::prelude::*;
use ds_core::vocab::Shortcut;

use crate::view::menu_key::with_key_opt;

/// What the "more" menu can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum More {
    CopyCommand,
    Save,
    View,
    Edit,
}

/// The "more" menu's items: each with a tip saying what it does. `output` and `view` say whether
/// the output can be saved and searched, `edit` is there on a pane on another computer, and
/// `copy_command_key` is the keymap's chord for "Copy Command".
pub(crate) fn more_menu(
    output: Availability,
    view: Availability,
    edit: Option<Availability>,
    copy_command_key: Option<Shortcut>,
) -> Vec<MenuItem<More>> {
    let copy = MenuItem::new(More::CopyCommand, "Copy Command")
        .with_tip("Copy this command to the clipboard");
    let mut items = vec![
        with_key_opt(copy, copy_command_key),
        MenuItem::new(More::Save, "Save Output")
            .with_tip("Save this command's output to Downloads")
            .with_availability(output),
        MenuItem::new(More::View, "View Files")
            .with_tip("Show the files this command listed")
            .with_availability(view),
    ];
    if let Some(availability) = edit {
        items.push(
            MenuItem::new(More::Edit, "Edit Locally")
                .with_tip(
                    "Open the selected file, or the last file in the command, in an editor here",
                )
                .with_availability(availability),
        );
    }
    items
}
