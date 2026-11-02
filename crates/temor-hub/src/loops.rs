//! The window's loops: each hears one inbox, or wakes on one tick, for as long as the window
//! lives, hands what it got to the hub and shows what the hub says back. The loops with more to
//! do between (the panes' events, the keys, the settings file, the cursor's blink, the desktop)
//! have their own.

use std::time::{Duration, Instant};

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use tokio::sync::mpsc::UnboundedReceiver;

use super::desk::Desk;
use super::model::Hub;

/// Whether the hub asks for the window to be drawn again after a message or a tick.
pub trait Redraw {
    fn redraw(self) -> bool;
}

/// Not unless something was said.
impl Redraw for () {
    fn redraw(self) -> bool {
        false
    }
}

impl Redraw for bool {
    fn redraw(self) -> bool {
        self
    }
}

/// Runs `heard` on the hub and shows what it said; words for the person draw the window again
/// too.
fn settle<R: Redraw>(desk: &Desk, heard: impl FnOnce(&mut Hub) -> R) {
    let (redraw, notes) = desk.hub.0.borrow_mut().said(heard);
    if redraw.redraw() || !notes.is_empty() {
        desk.tell(notes);
    }
}

impl Desk {
    /// Hands each message `inbox` brings to `heard`, while the window lives. No inbox (it was
    /// taken already) starts nothing.
    fn hear<E: 'static, R: Redraw>(
        &self,
        inbox: Option<UnboundedReceiver<E>>,
        mut heard: impl FnMut(&mut Hub, E) -> R + 'static,
    ) {
        let Some(mut inbox) = inbox else { return };
        let desk = self.clone();
        spawn(async move {
            while let Some(message) = inbox.recv().await {
                settle(&desk, |hub| heard(hub, message));
            }
        });
    }

    /// Runs `run` every `tick`, while the window lives.
    fn every<R: Redraw>(&self, tick: Duration, mut run: impl FnMut(&mut Hub) -> R + 'static) {
        let desk = self.clone();
        spawn(async move {
            loop {
                sleep(tick).await;
                settle(&desk, &mut run);
            }
        });
    }
}

/// Starts the window's plain loops. Call once, as the window starts.
pub fn spawn_loops(desk: &Desk) {
    let inboxes = {
        let mut hub = desk.hub.0.borrow_mut();
        (
            hub.ssh.inbox.take(),
            hub.remote.inbox.take(),
            hub.viewers.host.inbox.take(),
            hub.facts.take_inbox(),
            hub.plugins.host.take_inbox(),
            hub.icons.done.take(),
        )
    };
    let (ssh, remote, viewers, facts, plugins, icons) = inboxes;
    desk.hear(ssh, Hub::ssh_event);
    desk.hear(remote, Hub::remote_event);
    desk.hear(viewers, Hub::host_copy_done);
    desk.hear(facts, Hub::facts_ready);
    desk.hear(plugins, Hub::plugin_done);
    // A look-up is done: the rows draw again with its icon.
    desk.hear(icons, |_, ()| true);
    desk.every(Duration::from_millis(300), Hub::ssh_tick);
    desk.every(Duration::from_millis(250), Hub::offer_tick);
    // The radar settles about once a second.
    desk.every(Duration::from_secs(1), Hub::agent_tick);
    // Whether a change waits to be written; never in the render path.
    desk.every(Duration::from_millis(500), |hub| {
        hub.poll_save(Instant::now());
    });
}
