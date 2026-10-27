//! The pane's marks as the window hands them on. The commands are the pane's (`PaneCore` in
//! `temor-client`), which hears the shell's marks itself; the window tells it of Enter and of a
//! kept screen.

use super::super::Session;

impl Session {
    /// The person pressed Enter at the shell.
    pub(in crate::session) fn note_enter(&mut self) {
        self.core().note_enter();
    }

    /// The pane began as a kept screen: the commands it ran before are `history`, and the blocks
    /// on that screen are found again.
    pub fn begin_restored(&mut self, history: Vec<temor_core::session::CommandRecord>) {
        self.core().begin_restored(history);
    }
}
