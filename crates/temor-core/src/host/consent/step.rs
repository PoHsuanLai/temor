//! `gate`: what to do for the next connection to a host. `consent_step`: remember an answer, or
//! tie an alias to the machine a connection proved it reaches.

use super::model::{BootstrapAction, ConsentBook, Who};
use crate::remote::Grant;
use temor_ssh::TerminfoPolicy;

/// How to start a session on a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunMode {
    /// A plain `ssh` session: no setup runs on the host.
    Plain,
    /// Send the bootstrap, with this terminfo policy.
    Bootstrap(TerminfoPolicy),
}

/// What the next connection to a host needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gate {
    /// Ask the person this first (one question at a time), then call [`gate`] again.
    Ask(BootstrapAction),
    /// Every question is answered: start the session this way.
    Run(RunMode),
}

/// The next step for a connection to `who`: the first open question, else how to run. A host
/// that refuses the setup is never asked about the terminfo file.
pub fn gate(book: &ConsentBook, who: &Who) -> Gate {
    let grants = book.grants(who);
    match (grants.integrate, grants.terminfo) {
        (Grant::Ask, _) => Gate::Ask(BootstrapAction::Integrate),
        (Grant::Refused, _) => Gate::Run(RunMode::Plain),
        (Grant::Allowed, Grant::Ask) => Gate::Ask(BootstrapAction::Terminfo),
        (Grant::Allowed, Grant::Allowed) => Gate::Run(RunMode::Bootstrap(TerminfoPolicy::Install)),
        (Grant::Allowed, Grant::Refused) => Gate::Run(RunMode::Bootstrap(TerminfoPolicy::Skip)),
    }
}

/// What moves the book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentIn {
    /// The person answered a question about the host (`Grant::Ask` forgets the answer).
    Answered {
        who: Who,
        action: BootstrapAction,
        grant: Grant,
    },
    /// A connection verified the host's key: `who.key` is that identity and `who.alias` the
    /// name it was reached by. Records kept under the alias alone join the machine's record.
    Identified { who: Who },
}

/// What the book wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsentOut {
    /// The book changed: write it to the session file.
    Persist,
}

/// The book after `input`.
pub fn consent_step(book: ConsentBook, input: ConsentIn) -> (ConsentBook, Vec<ConsentOut>) {
    let mut next = book.clone();
    match input {
        ConsentIn::Answered { who, action, grant } => {
            let at = next.matching(&who);
            let index = next.fold(&at, &who);
            next.set_grant(index, action, grant);
        }
        ConsentIn::Identified { who } => {
            let at = next.matching(&who);
            if !at.is_empty() {
                next.fold(&at, &who);
            }
        }
    }
    match next == book {
        true => (book, Vec::new()),
        false => (next, vec![ConsentOut::Persist]),
    }
}
