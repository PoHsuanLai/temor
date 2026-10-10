//! What a program on a host may do on this computer (the clipboard, files, notifications): asked
//! once, kept in the consent book per machine beside the bootstrap's answers (PLAN §9b,
//! "Consent per host"). Local panes need no answer.

use temor_core::host::consent::{ConsentIn, Permit};
use temor_core::remote::Grant;
use temor_ssh::HostAlias;

use super::model::Hub;

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
