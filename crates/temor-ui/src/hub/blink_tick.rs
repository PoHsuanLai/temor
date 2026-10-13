//! The cursor's blink timer: a pane whose cursor blinks is repainted when its half turns (see
//! `session::blink`). Only the pane's own texture is drawn again, not the window.

use std::time::Duration;

use dioxus::prelude::spawn;
use ds::base::time::clock::sleep;

use super::desk::Desk;
use crate::view::slot::repaint;

/// How often the panes are looked at: well under a half, so a turn is drawn within this.
const TICK: Duration = Duration::from_millis(50);

/// Repaints each pane whose cursor's blink has turned, for as long as the window lives.
pub fn spawn_blink_loop(desk: Desk) {
    spawn(async move {
        loop {
            sleep(TICK).await;
            let due: Vec<_> = desk
                .hub
                .0
                .borrow()
                .slots
                .values()
                .filter(|slot| {
                    slot.borrow()
                        .session
                        .as_ref()
                        .is_some_and(|session| session.blink_due())
                })
                .cloned()
                .collect();
            for slot in due {
                repaint(&slot);
            }
        }
    });
}
