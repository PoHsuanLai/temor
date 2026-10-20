//! What the screen state sends its events and replies to.

use std::sync::{Arc, Mutex};

use temor_shell::cwd_from_url;
use temor_vt::{EventSink, ReplySink, VtEvent};

use super::input::Input;
use crate::sync::lock;

/// Keeps the latest working directory the program reported.
pub(super) struct CwdSink(pub(super) Arc<Mutex<String>>);

impl EventSink for CwdSink {
    fn send(&self, event: VtEvent) {
        if let VtEvent::Cwd(url) = event
            && let Some(path) = cwd_from_url(&url)
        {
            *lock(&self.0) = path;
        }
    }
}

/// Sends the screen state's replies (device reports and the like) to the program.
pub(super) struct Replies(pub(super) Arc<Input>);

impl ReplySink for Replies {
    fn reply(&self, bytes: &[u8]) {
        self.0.reply(bytes);
    }
}
