//! What the host views ask of the window: each call runs on the hub, shows the messages it left,
//! and draws the window again.

use temor_core::host::edit::Resolution;
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;
use temor_ssh::{HostRow, Port};

use super::desk::Desk;
use super::model::Hub;
use crate::edge::ssh_work::EditId;

impl Desk {
    fn with_hub(&self, run: impl FnOnce(&mut Hub)) {
        let notes = {
            let mut hub = self.hub.0.borrow_mut();
            run(&mut hub);
            std::mem::take(&mut hub.notices)
        };
        for note in notes {
            self.say(note);
        }
        self.bump();
    }

    /// Opens a new tab on the host `row` names.
    pub fn open_host(&self, row: &HostRow) {
        self.with_hub(|hub| hub.open_host(row));
    }

    /// The person answered the question the host pane asks.
    pub fn host_answer(&self, pane: PaneId, grant: Grant) {
        self.with_hub(|hub| hub.answer_host(pane, grant));
    }

    /// The person pressed "Try anyway" on a computer shown offline.
    pub fn host_try_anyway(&self, pane: PaneId) {
        self.with_hub(|hub| hub.try_offline(pane));
    }

    /// The person pressed Open on the browser check row.
    pub fn host_open_check(&self, pane: PaneId) {
        self.with_hub(|hub| hub.open_check(pane));
    }

    /// Forwards the host's port `text`.
    pub fn host_forward(&self, pane: PaneId, text: &str) {
        self.with_hub(|hub| hub.forward_port(pane, text));
    }

    /// Closes the forward of the host's `remote` port.
    pub fn host_cancel_forward(&self, pane: PaneId, remote: Port) {
        self.with_hub(|hub| hub.cancel_forward(pane, remote));
    }

    /// Opens a forwarded port in the browser.
    pub fn host_open_forwarded(&self, pane: PaneId, remote: Port) {
        self.with_hub(|hub| hub.open_forwarded(pane, remote));
    }

    /// Opens the host's port over the tailnet in the browser.
    pub fn host_open_direct(&self, pane: PaneId, remote: Port) {
        self.with_hub(|hub| hub.open_direct(pane, remote));
    }

    /// Opens the host's file `typed` in a local editor.
    pub fn host_edit(&self, pane: PaneId, typed: &str) {
        self.with_hub(|hub| hub.edit_remote(Some(pane), typed));
    }

    /// The person chose what to do about a changed file.
    pub fn host_resolve(&self, id: EditId, choice: Resolution) {
        self.with_hub(|hub| hub.resolve_edit(id, choice));
    }

    /// The person is done editing the file.
    pub fn host_close_edit(&self, id: EditId) {
        self.with_hub(|hub| hub.close_edit(id));
    }
}
