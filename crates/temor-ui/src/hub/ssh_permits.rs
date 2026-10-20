//! What a program on a host may do on this computer (the clipboard, files, notifications): asked
//! once, kept in the consent book per machine beside the bootstrap's answers (PLAN §9b,
//! "Consent per host"). Local panes need no answer.

use temor_core::host::consent::{ConsentIn, Permit};
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;
use temor_ssh::HostAlias;

use super::model::{Hub, HubRef};

impl Hub {
    /// What the person answered about `permit` for the host `alias` (open when never asked).
    pub fn permit_grant(&self, alias: &HostAlias, permit: Permit) -> Grant {
        self.ssh.book.grants(&self.who_of(alias)).permit(permit)
    }

    /// Remembers the person's answer about `permit` for the host `alias`.
    pub fn remember_permit(&mut self, alias: &HostAlias, permit: Permit, grant: Grant) {
        let who = self.who_of(alias);
        self.step_book(ConsentIn::PermitAnswered { who, permit, grant });
    }
}

impl HubRef {
    /// The kept answer about a host pane's programs writing to the clipboard; `None` for a local
    /// pane (local programs may write) or a host never asked about.
    pub fn clipboard_consent(&self, pane: PaneId) -> Option<Grant> {
        let hub = self.0.borrow();
        let alias = hub.alias_of(pane)?;
        Some(hub.permit_grant(&alias, Permit::Clipboard)).filter(|grant| *grant != Grant::Ask)
    }

    /// Keeps the person's answer to a host pane's clipboard write, for the host's other tabs and
    /// the next launch. Local panes keep nothing.
    pub fn clipboard_answered(&self, pane: PaneId, grant: Grant) {
        let mut hub = self.0.borrow_mut();
        let Some(alias) = hub.alias_of(pane) else {
            return;
        };
        hub.remember_permit(&alias, Permit::Clipboard, grant);
    }
}
