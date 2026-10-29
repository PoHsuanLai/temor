//! Hearing the SSH jobs, and the timer that polls what has no event of its own: the local copies
//! of edited files, and a connection that is still being made.

use temor_client::model::host::forward::ForwardIn;
use temor_client::model::host::master::Master;
use temor_client::ssh::{HostAlias, MasterProbe};

use super::model::Hub;
use temor_client::ssh::work::SshEvent;

/// A connection being made is asked about every this many ticks.
const PROBE_EVERY: u32 = 3;

impl Hub {
    /// Carries out what a job found, and returns the messages it left for the person.
    pub fn ssh_event(&mut self, event: SshEvent) -> Vec<String> {
        match event {
            SshEvent::Probed { host, probe } => self.on_probed(host, probe),
            SshEvent::Identified { host, fingerprint } => self.on_identified(host, fingerprint),
            SshEvent::Edit { id, input } => self.on_edit(id, input),
            SshEvent::EditSeen { id, stamp } => self.on_edit_seen(id, stamp),
            SshEvent::Upload { id, result } => self.on_upload(id, result),
            SshEvent::UploadProgress { id, sent } => self.on_upload_progress(id, sent),
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

    /// One tick: polls the local copies, and asks after a connection still being made. Returns
    /// whether the window must be drawn again, and what the person should be told.
    pub(super) fn ssh_tick(&mut self) -> (bool, Vec<String>) {
        self.ssh.ticks = self.ssh.ticks.wrapping_add(1);
        self.ssh_start_shown();
        self.poll_waiting_edits();
        // Both run, whatever the first says.
        let redraw = self.poll_edits() | self.remote_tick();
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
