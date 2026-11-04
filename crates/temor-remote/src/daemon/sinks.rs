//! What the screen state sends its events and replies to: its session's own commands, which the
//! pump takes once the screen has read what the program wrote.

use std::sync::mpsc::Sender;

use temor_shell::cwd_from_url;
use temor_vt::{EventSink, ReplySink, VtEvent};

use super::session::Command;

/// The working directory the program reports.
pub(super) struct CwdSink(pub(super) Sender<Command>);

impl EventSink for CwdSink {
    fn send(&self, event: VtEvent) {
        if let VtEvent::Cwd(url) = event
            && let Some(path) = cwd_from_url(&url)
        {
            // The pump is the only reader; it has not gone while its screen writes.
            let _ = self.0.send(Command::Cwd(path));
        }
    }
}

/// The screen state's replies (device reports and the like) to the program.
pub(super) struct Replies(pub(super) Sender<Command>);

impl ReplySink for Replies {
    fn reply(&self, bytes: &[u8]) {
        let _ = self.0.send(Command::Reply(bytes.to_vec()));
    }
}
