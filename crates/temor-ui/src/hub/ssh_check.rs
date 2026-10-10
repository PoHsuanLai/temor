//! Tailscale SSH's browser check (PLAN §9b, D22). Before the shell starts, a host that wants the
//! person to confirm in a browser prints a link. What the host prints is read for it until the
//! shell starts; the window shows a row with an Open button, and nothing opens the link but
//! that click.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, Weak};

use temor_core::vocab::ids::PaneId;
use temor_pane::ByteTap;
use temor_pane::text::{LinkVerdict, check_link};
use temor_ssh::{CheckScan, CheckUrl, PreShellEvent};

use super::model::Hub;
use super::ssh::Check;

/// What the pane's output has said about a check, read from the bytes `ssh` wrote (a link that
/// wraps on a narrow screen is still one link there).
#[derive(Debug)]
pub(super) struct RawCheck {
    state: Mutex<RawState>,
}

#[derive(Debug)]
struct RawState {
    /// `None` once the shell started.
    scan: Option<CheckScan>,
    /// The newest link not yet taken.
    found: Option<CheckUrl>,
}

impl RawCheck {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(RawState {
                scan: Some(CheckScan::new()),
                found: None,
            }),
        }
    }

    /// The listener to give the pane, which reads for this check while the hub keeps it.
    pub(super) fn listener(self: &Arc<Self>) -> Arc<dyn ByteTap> {
        Arc::new(Listener(Arc::downgrade(self)))
    }

    /// The link found since the last call, if any.
    pub(super) fn take(&self) -> Option<CheckUrl> {
        self.lock().found.take()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RawState> {
        // A poisoned lock only means a panic elsewhere; the scan is still valid text state.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What the pane's thread calls with each chunk. It holds the check weakly: when the hub lets
/// go of the check (the shell started), the pane's output is no longer read at all.
#[derive(Debug)]
struct Listener(Weak<RawCheck>);

impl ByteTap for Listener {
    fn bytes(&self, data: &[u8]) {
        let Some(check) = self.0.upgrade() else {
            return;
        };
        let mut state = check.lock();
        let RawState { scan, found } = &mut *state;
        let Some(scan) = scan.as_mut() else { return };
        for event in scan.feed(data) {
            if let PreShellEvent::ConfirmInBrowser(url) = event {
                *found = Some(url);
            }
        }
    }
}

impl Hub {
    /// Reads what host panes that have not started their shell yet have printed.
    ///
    /// The scan is fed the raw bytes `ssh` wrote, on the pane's thread; here the newest link it
    /// found is shown, and a redraw of it is reported once.
    pub(super) fn ssh_scan(&mut self, painted: &BTreeSet<PaneId>) {
        let mut news = Vec::new();
        for pane in painted {
            let Some(host) = self.ssh.panes.get_mut(pane) else {
                continue;
            };
            let Some(url) = host.scan.as_ref().and_then(|raw| raw.take()) else {
                continue;
            };
            if host.check == Check::Nothing {
                news.push(*pane);
            }
            host.check = Check::Shown(url);
        }
        self.host_attention(news);
    }

    /// The shell started (its first folder report): the check, if any, is over.
    pub(super) fn ssh_shell_started(&mut self, pane: PaneId) {
        if let Some(host) = self.ssh.panes.get_mut(&pane) {
            host.scan = None;
            host.check = Check::Nothing;
        }
    }

    /// The person pressed Open on the check row.
    pub fn open_check(&mut self, pane: PaneId) {
        let Some(Check::Shown(url)) = self.ssh.panes.get(&pane).map(|host| host.check.clone())
        else {
            return;
        };
        match check_link(url.as_str()) {
            LinkVerdict::Open(target) => {
                if let Err(err) = self.ssh.opener.open(&target) {
                    self.notices.push(err.to_string());
                }
            }
            _ => self
                .notices
                .push("That confirmation page can't be opened".to_owned()),
        }
    }
}
