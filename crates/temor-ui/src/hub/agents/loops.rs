//! The loops that keep agents current while the window lives: finished git jobs are carried on
//! as they arrive, and about once a second the radar settles and worktrees due a look get one.

use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;

use super::radar::Changed;
use crate::hub::Desk;

/// How often the radar is ticked.
const TICK: Duration = Duration::from_secs(1);

/// Starts the two loops. Call once, next to the pane event loop.
pub fn spawn_agent_loops(desk: Desk) {
    let inbox = desk.hub.0.borrow_mut().agents.jobs.take_inbox();
    if let Some(mut inbox) = inbox {
        let desk = desk.clone();
        spawn(async move {
            while let Some(done) = inbox.recv().await {
                let notices = {
                    let mut hub = desk.hub.0.borrow_mut();
                    hub.agent_done(done);
                    std::mem::take(&mut hub.notices)
                };
                for text in notices {
                    desk.say(text);
                }
                desk.bump();
            }
        });
    }
    spawn(async move {
        loop {
            sleep(TICK).await;
            let changed = desk.hub.0.borrow_mut().agent_tick();
            if changed == Changed::Yes {
                desk.bump();
            }
        }
    });
}
