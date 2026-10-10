//! Hearing the SSH jobs, and the timer that polls what has no event of its own: the local copies
//! of edited files, and a connection that is still being made.

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use std::time::Duration;
use temor_core::host::forward::ForwardIn;
use temor_core::host::master::Master;
use temor_ssh::{HostAlias, MasterProbe};

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
            SshEvent::EditSeen { id, stamp } => self.on_edit_seen(id, stamp),
            SshEvent::Upload { id, result } => self.on_upload(id, result),
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
        self.ssh_start_shown();
        self.poll_waiting_edits();
        let redraw = match (self.poll_edits(), self.remote_tick()) {
            (Redraw::No, Redraw::No) => Redraw::No,
            _ => Redraw::Yes,
        };
        let connecting: Vec<HostAlias> = self
            .ssh
            .lines
            .iter()
            .filter(|(_, line)| matches!(line.master, Master::Connecting { .. }))
            .map(|(alias, _)| alias.clone())
            .collect();
        self.ssh
            .baselines
            .retain(|alias, _| connecting.contains(alias));
        // The connection's socket appearing says it is up; `ssh -O check` is asked only when
        // that cannot say (several connections at once, or one joined without a new socket).
        let mut unsure = Vec::new();
        for alias in connecting {
            match connecting_alone(&self.ssh.lines) && self.socket_up(&alias) {
                true => self.on_probed(alias, MasterProbe::Running),
                false => unsure.push(alias),
            }
        }
        if self.ssh.ticks.is_multiple_of(PROBE_EVERY) {
            for alias in unsure {
                self.ssh.work.probe(alias);
            }
        }
        (redraw, std::mem::take(&mut self.notices))
    }
}

/// Whether only one connection is being made.
fn connecting_alone(lines: &std::collections::BTreeMap<HostAlias, super::ssh::HostLine>) -> bool {
    lines
        .values()
        .filter(|line| matches!(line.master, Master::Connecting { .. }))
        .count()
        == 1
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
