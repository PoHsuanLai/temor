//! The window's one-second tick: it hangs up the live cards whose grace is over, and redraws the
//! window when it did.

use std::time::Duration;

use dioxus::prelude::spawn;
use ds::base::time::clock::sleep;

use super::desk::Desk;

/// How often the live cards are looked at.
const TICK: Duration = Duration::from_secs(1);

/// Hangs up the live cards whose grace is over, for as long as the window lives.
pub fn spawn_running_loop(desk: Desk) {
    spawn(async move {
        loop {
            sleep(TICK).await;
            let hung = desk.hub.0.borrow_mut().live_tick(std::time::Instant::now());
            if hung {
                desk.bump();
            }
        }
    });
}
