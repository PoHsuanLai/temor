//! The Settings window's changes to a Space's project, applied in the main window (PLAN §7): the
//! kit's handle belongs to this window, so a change comes here through the board and goes in the
//! way a Space menu's edit does, and the kit then writes `spaces.json`.

use dioxus::prelude::spawn;

use super::desk::Desk;
use crate::settings_window::SpaceBoard;

/// Applies each change the Settings window sends, for as long as this window lives.
pub fn spawn_space_edits(desk: Desk, board: SpaceBoard) {
    let mut heard = board.heard_edits();
    spawn(async move {
        loop {
            for edit in board.take_edits() {
                desk.spaces
                    .apply(edit.space, |space| edit.change.apply_to(&mut space.payload));
            }
            desk.bump();
            if heard.changed().await.is_err() {
                break;
            }
        }
    });
}
