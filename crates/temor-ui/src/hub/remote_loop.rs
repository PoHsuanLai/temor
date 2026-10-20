//! Hearing the helper jobs and connections for as long as the window lives.

use dioxus::prelude::*;

use super::desk::Desk;

/// Reads what the helper's jobs and connections report, and shows what it means.
pub fn spawn_remote_loop(desk: Desk) {
    let Some(mut inbox) = desk.hub.0.borrow_mut().remote.inbox.take() else {
        return;
    };
    spawn(async move {
        while let Some(event) = inbox.recv().await {
            let notes = desk.hub.0.borrow_mut().remote_event(event);
            desk.tell(notes);
        }
    });
}
