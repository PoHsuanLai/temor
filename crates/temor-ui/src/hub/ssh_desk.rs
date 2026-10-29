//! What the host views ask of the window: each call runs on the hub, shows the messages it left,
//! and draws the window again.

use temor_client::model::host::edit::Resolution;
use temor_client::model::remote::Grant;
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::{HostRow, Port};

use super::desk::Desk;
use temor_client::ssh::work::EditId;

impl Desk {
    /// Opens a new tab on the host `row` names.
    pub fn open_host(&self, row: &HostRow) {
        self.settle(|hub| hub.open_host(row));
    }

    /// The person answered the question the host pane asks.
    pub fn host_answer(&self, pane: PaneId, grant: Grant) {
        self.settle(|hub| hub.answer_host(pane, grant));
    }

    /// The person pressed "Try anyway" on a computer shown offline.
    pub fn host_try_anyway(&self, pane: PaneId) {
        self.settle(|hub| hub.try_offline(pane));
    }

    /// The person pressed Open on the browser check row.
    pub fn host_open_check(&self, pane: PaneId) {
        self.settle(|hub| hub.open_check(pane));
    }

    /// Forwards the host's port `text`.
    pub fn host_forward(&self, pane: PaneId, text: &str) {
        self.settle(|hub| hub.forward_port(pane, text));
    }

    /// Closes the forward of the host's `remote` port.
    pub fn host_cancel_forward(&self, pane: PaneId, remote: Port) {
        self.settle(|hub| hub.cancel_forward(pane, remote));
    }

    /// Opens a forwarded port in the browser.
    pub fn host_open_forwarded(&self, pane: PaneId, remote: Port) {
        self.settle(|hub| hub.open_forwarded(pane, remote));
    }

    /// Opens the host's port over the tailnet in the browser.
    pub fn host_open_direct(&self, pane: PaneId, remote: Port) {
        self.settle(|hub| hub.open_direct(pane, remote));
    }

    /// Opens the host's file `typed` in a local editor.
    pub fn host_edit(&self, pane: PaneId, typed: &str) {
        self.settle(|hub| hub.edit_remote(Some(pane), typed));
    }

    /// The person chose what to do about a changed file.
    pub fn host_resolve(&self, id: EditId, choice: Resolution) {
        self.settle(|hub| hub.resolve_edit(id, choice));
    }

    /// The person is done editing the file.
    pub fn host_close_edit(&self, id: EditId) {
        self.settle(|hub| hub.close_edit(id));
    }
}
