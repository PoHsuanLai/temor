//! Feeding program output: inline files taken out, shell marks cut and anchored, the rest to
//! libghostty (PLAN §8.1, §9).

use temor_vt_sys as sys;

use super::callbacks::TitleChange;
use super::terminal::Terminal;
use crate::scan::{Scanned, ends_within};
use crate::{RenderHold, UnknownKind, VtEvent};

/// Why the input is cut at an offset.
#[derive(Debug, Clone, Copy)]
enum Cut {
    /// An OSC 133 mark ended.
    Mark,
    /// A synchronized update started.
    Hold,
}

impl Terminal {
    /// Feeds program output. Inline files (OSC 1337 `File=`) are taken out first and reported as
    /// [`UnknownKind::Osc1337File`]; everything else goes to libghostty, whose callbacks send
    /// their events before this returns.
    pub fn write(&mut self, bytes: &[u8]) {
        let mut segments = Vec::new();
        self.scanner.feed(bytes, |seg| segments.push(seg));
        // A hold's end can straddle two pieces (the scanner hands a held-back ESC on as a piece of
        // its own), so the check after a hold's start reads the later pieces too.
        for (i, seg) in segments.iter().enumerate() {
            match seg {
                Scanned::Bytes(b) => self.write_marked(b, &segments[i + 1..]),
                Scanned::File {
                    payload,
                    truncation,
                } => self.callbacks().send(VtEvent::Unknown {
                    kind: UnknownKind::Osc1337File,
                    payload: payload.clone(),
                    truncation: *truncation,
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
    /// so the numbers keep the next mark's events from pairing with the missed one. The start of a
    /// synchronized update is cut too, so the frame can be captured where the hold began.
    fn write_marked(&mut self, bytes: &[u8], later: &[Scanned<'_>]) {
        let mut cuts: Vec<(usize, Cut)> = self
            .marks
            .feed(bytes)
            .into_iter()
            .map(|at| (at, Cut::Mark))
            .collect();
        cuts.extend(self.holds.feed(bytes).into_iter().map(|at| (at, Cut::Hold)));
        cuts.sort_by_key(|(at, _)| *at);
        let mut start = 0;
        for (end, cut) in cuts {
            match cut {
                Cut::Mark => {
                    self.mark_seq += 1;
                    let seq = self.mark_seq;
                    self.callbacks().send(VtEvent::MarkBegin { seq });
                    self.vt_write(&bytes[start..end]);
                    if let Some(anchor) = self.anchor_cursor() {
                        self.callbacks().send(VtEvent::Anchor { seq, anchor });
                    }
                }
                Cut::Hold => {
                    self.vt_write(&bytes[start..end]);
                    let rest: Vec<u8> = later
                        .iter()
                        .filter_map(|seg| match seg {
                            Scanned::Bytes(b) => Some(*b),
                            Scanned::File { .. } => None,
                        })
                        .fold(bytes[end..].to_vec(), |mut all, b| {
                            all.extend_from_slice(b);
                            all
                        });
                    self.hold_began(&rest);
                }
            }
            start = end;
        }
        self.vt_write(&bytes[start..]);
    }

    /// A start sequence was just written. When it began a hold that the rest of this write does
    /// not end, the window can paint mid-hold: let the hook capture the frame.
    fn hold_began(&mut self, rest: &[u8]) {
        if self.render_hold() != RenderHold::Held || ends_within(rest) {
            return;
        }
        if let Some(mut hook) = self.hold_hook.take() {
            hook.held(self);
            self.hold_hook = Some(hook);
        }
    }

    fn vt_write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // SAFETY: the terminal is live and `bytes` is readable for the call. Callbacks may run;
        // they only use `Callbacks` through shared refs.
        unsafe { sys::ghostty_terminal_vt_write(self.raw(), bytes.as_ptr(), bytes.len()) };
    }
}
