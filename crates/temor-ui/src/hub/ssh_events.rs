//! Hearing the SSH jobs, and the timer that polls what has no event of its own: the local copies
//! of edited files, and a connection that is still being made.

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use std::time::Duration;
use temor_core::host::forward::ForwardIn;
use temor_core::host::master::Master;

use super::desk::Desk;
use super::model::Hub;
use crate::edge::ssh_work::SshEvent;

/// How often the local copies are looked at (an editor's save shows within this).
const TICK: Duration = Duration::from_millis(300);
/// A connection being made is asked about every this many ticks.
const PROBE_EVERY: u32 = 3;

/// Whether the window must be drawn again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Redraw {
    Yes,
    No,
}

impl Hub {
    /// Carries out what a job found, and returns the messages it left for the person.
    pub fn ssh_event(&mut self, event: SshEvent) -> Vec<String> {
        match event {
            SshEvent::Probed { host, probe } => self.on_probed(host, probe),
            SshEvent::Identified { host, fingerprint } => self.on_identified(host, fingerprint),
            SshEvent::Edit { id, input } => self.on_edit(id, input),
            SshEvent::ForwardOpened { host, remote } => {
                self.feed_forwards(&host, ForwardIn::Opened { remote });
            }
            SshEvent::ForwardFailed {
                host,
                remote,
                fault,
            } => self.feed_forwards(&host, ForwardIn::Failed { remote, fault }),
        }
        std::mem::take(&mut self.notices)
    }

    /// One tick: polls the local copies, and asks after a connection still being made.
    pub(super) fn ssh_tick(&mut self) -> (Redraw, Vec<String>) {
        self.ssh.ticks = self.ssh.ticks.wrapping_add(1);
        let redraw = self.poll_edits();
        if self.ssh.ticks.is_multiple_of(PROBE_EVERY) {
            for (alias, line) in &self.ssh.lines {
                if matches!(line.master, Master::Connecting { .. }) {
                    self.ssh.work.probe(alias.clone());
                }
            }
        }
        (redraw, std::mem::take(&mut self.notices))
    }
}

/// Hears the SSH jobs and polls, for as long as the window lives.
pub fn spawn_ssh_loop(desk: Desk) {
    let Some(mut inbox) = desk.hub.0.borrow_mut().ssh.inbox.take() else {
        return;
    };
    let heard = desk.clone();
    spawn(async move {
        while let Some(event) = inbox.recv().await {
            let notes = heard.hub.0.borrow_mut().ssh_event(event);
            for note in notes {
                heard.say(note);
            }
            heard.bump();
        }
    });
    spawn(async move {
        loop {
            sleep(TICK).await;
            let (redraw, notes) = desk.hub.0.borrow_mut().ssh_tick();
            let shown = redraw == Redraw::Yes || !notes.is_empty();
            for note in notes {
                desk.say(note);
            }
            if shown {
                desk.bump();
            }
        }
    });
}
