//! Tailscale SSH's browser check (PLAN §9b, D22). Before the shell starts, a host that wants the
//! person to confirm in a browser prints a link. The pane's screen is read for it until the
//! shell starts; the window shows a row with an Open button, and nothing opens the link but
//! that click.

use std::collections::BTreeSet;

use temor_core::vocab::ids::PaneId;
use temor_pane::text::{LinkVerdict, check_link};
use temor_ssh::PreShellEvent;

use super::model::Hub;
use super::ssh::Check;
use crate::session::Session;

impl Hub {
    /// Reads the screens of host panes that have not started their shell yet.
    ///
    /// The scan is fed what the pane's terminal shows (the pane engine does not hand out the raw
    /// bytes it reads); a link on screen is the same link, and a redraw of it is reported once.
    pub(super) fn ssh_scan(&mut self, painted: &BTreeSet<PaneId>) {
        for pane in painted {
            let Some(host) = self.ssh.panes.get_mut(pane) else {
                continue;
            };
            let Some(scan) = host.scan.as_mut() else {
                continue;
            };
            let screen = self
                .slots
                .get(pane)
                .and_then(|slot| slot.borrow().session.as_ref().map(Session::screen_text));
            let Some(screen) = screen else { continue };
            for event in scan.feed(format!("{screen}\n").as_bytes()) {
                if let PreShellEvent::ConfirmInBrowser(url) = event {
                    host.check = Check::Shown(url);
                }
            }
        }
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
