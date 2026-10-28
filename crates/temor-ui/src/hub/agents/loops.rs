//! The loop that keeps agents current while the window lives: about once a second the radar
//! settles.

use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;

use crate::hub::Desk;

/// How often the radar is ticked.
const TICK: Duration = Duration::from_secs(1);

/// Starts the loop. Call once, next to the pane event loop.
pub fn spawn_agent_loops(desk: Desk) {
    spawn(async move {
        loop {
            sleep(TICK).await;
            if desk.hub.0.borrow_mut().agent_tick() {
                desk.bump();
            }
        }
    });
}
