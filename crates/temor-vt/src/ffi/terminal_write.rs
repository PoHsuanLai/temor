//! Feeding program output: inline files taken out, shell marks cut and anchored, the rest to
//! libghostty (PLAN §8.1, §9).

use temor_vt_sys as sys;

use super::callbacks::TitleChange;
use super::terminal::Terminal;
use crate::scan::Scanned;
use crate::{UnknownKind, VtEvent};

impl Terminal {
    /// Feeds program output. Inline files (OSC 1337 `File=`) are taken out first and reported as
    /// [`UnknownKind::Osc1337File`]; everything else goes to libghostty, whose callbacks send
    /// their events before this returns.
    pub fn write(&mut self, bytes: &[u8]) {
        let mut segments = Vec::new();
        self.scanner.feed(bytes, |seg| segments.push(seg));
        for seg in segments {
            match seg {
                Scanned::Bytes(b) => self.write_marked(b),
                Scanned::File {
                    payload,
                    truncation,
                } => self.callbacks().send(VtEvent::Unknown {
                    kind: UnknownKind::Osc1337File,
                    payload,
                    truncation,
                }),
            }
        }
        if self.callbacks().title.replace(TitleChange::Unchanged) == TitleChange::Changed {
            let title = self.title();
            self.callbacks().sink.send(VtEvent::Title(title));
        }
    }

    /// Hands bytes to libghostty, cutting each OSC 133 mark out as its own piece. Each piece is
    /// announced by `MarkBegin` with its number, so the mark's own events sit between that and
    /// its `Anchor` (the cursor where the mark stood). A missed anchor leaves no `Anchor` behind,
    /// so the numbers keep the next mark's events from pairing with the missed one.
    fn write_marked(&mut self, bytes: &[u8]) {
        let mut start = 0;
        for end in self.marks.feed(bytes) {
            self.mark_seq += 1;
            let seq = self.mark_seq;
            self.callbacks().send(VtEvent::MarkBegin { seq });
            self.vt_write(&bytes[start..end]);
            if let Some(anchor) = self.anchor_cursor() {
                self.callbacks().send(VtEvent::Anchor { seq, anchor });
            }
            start = end;
        }
        self.vt_write(&bytes[start..]);
    }

    fn vt_write(&mut self, bytes: &[u8]) {
        // SAFETY: the terminal is live and `bytes` is readable for the call. Callbacks may run;
        // they only use `Callbacks` through shared refs.
        unsafe { sys::ghostty_terminal_vt_write(self.raw(), bytes.as_ptr(), bytes.len()) };
    }
}
