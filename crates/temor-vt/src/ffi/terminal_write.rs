//! Feeding program output: inline files taken out, shell marks cut and anchored, the rest to
//! libghostty (PLAN §8.1, §9).

use temor_vt_sys as sys;

use super::callbacks::{MarkKind, TitleChange};
use super::terminal::Terminal;
use crate::scan::{Scanned, ends_within};
use crate::{AnchorId, PointSpace, RenderHold, UnknownKind, VtEvent};

/// Cancels the control sequence being parsed (DEC: CAN aborts it and does nothing else).
const CAN: u8 = 0x18;

/// How much of the scrollback a `CSI 3 J` erases.
enum Floor {
    /// All of it: no command is running, or the person cleared.
    None,
    /// None of it: the running command's output starts on the screen.
    Above,
    /// From the running command's output mark, at `row` of the screen space, down.
    From { anchor: AnchorId, row: u32 },
}

/// Why the input is cut at an offset.
#[derive(Debug, Clone, Copy)]
enum Cut {
    /// An OSC 133 mark ended.
    Mark,
    /// A synchronized update started.
    Hold,
    /// The scrollback is erased (`CSI 3 J`) once the offset's last byte is written.
    EraseHistory,
    /// The person's own `clear` is about to run: the screen's anchors end.
    PersonClears,
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
    /// synchronized update is cut too, so the frame can be captured where the hold began, and so
    /// is the erase of the scrollback: libghostty moves an erased row's anchor to the top-left
    /// cell rather than ending it, so the anchors in the scrollback are ended here instead, as the
    /// scrollback dropping their rows would.
    fn write_marked(&mut self, bytes: &[u8], later: &[Scanned<'_>]) {
        let mut cuts: Vec<(usize, Cut)> = self
            .marks
            .feed(bytes)
            .into_iter()
            .map(|at| (at, Cut::Mark))
            .collect();
        cuts.extend(self.holds.feed(bytes).into_iter().map(|at| (at, Cut::Hold)));
        cuts.extend(
            self.erases
                .feed(bytes)
                .into_iter()
                .map(|at| (at, Cut::EraseHistory)),
        );
        cuts.extend(
            self.person_clears
                .feed(bytes)
                .into_iter()
                .map(|at| (at, Cut::PersonClears)),
        );
        cuts.sort_by_key(|(at, _)| *at);
        let mut start = 0;
        for (end, cut) in cuts {
            match cut {
                Cut::Mark => {
                    self.mark_seq += 1;
                    let seq = self.mark_seq;
                    self.callbacks().send(VtEvent::MarkBegin { seq });
                    self.vt_write(&bytes[start..end]);
                    let anchor = self.anchor_cursor();
                    if let Some(anchor) = anchor {
                        self.callbacks().send(VtEvent::Anchor { seq, anchor });
                    }
                    match self.callbacks().mark.take() {
                        Some(MarkKind::Output) => self.floor = anchor,
                        Some(MarkKind::Prompt | MarkKind::End) => self.floor = None,
                        Some(MarkKind::Input) | None => {}
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
                Cut::EraseHistory => {
                    // Every byte but the final `J`: the rows to be erased are still there.
                    self.vt_write(&bytes[start..end - 1]);
                    match self.erase_floor() {
                        Floor::None => {
                            let erased = self.anchors_in_history();
                            self.vt_write(&bytes[end - 1..end]);
                            for anchor in erased {
                                self.release_anchor(anchor);
                            }
                        }
                        Floor::Above => self.vt_write(&[CAN]),
                        Floor::From { anchor, row } => {
                            self.vt_write(&[CAN]);
                            self.erase_history_from(row);
                            // The output now starts on the first row left, the screen's top.
                            self.move_anchor_to_top(anchor);
                        }
                    }
                }
                Cut::PersonClears => {
                    self.vt_write(&bytes[start..end]);
                    self.floor = None;
                    // libghostty clears a screen that does not end at a prompt in place, so the
                    // anchors there would stay on blank rows under the next prompt. The ones in
                    // the scrollback end with the erase that follows.
                    for anchor in self.anchors_on_screen() {
                        self.release_anchor(anchor);
                    }
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

    /// How much of the scrollback a `CSI 3 J` arriving now erases. While a command's output runs
    /// on the shell's screen, only its own rows: the ones from its output mark down.
    fn erase_floor(&self) -> Floor {
        let Some(anchor) = self.floor else {
            return Floor::None;
        };
        if self.on_alternate_screen() {
            return Floor::None;
        }
        match (
            self.anchor_point(anchor, PointSpace::Screen),
            self.anchor_point(anchor, PointSpace::Active),
        ) {
            (Some(_), Some(_)) => Floor::Above,
            (Some(at), None) => Floor::From {
                anchor,
                row: at.row,
            },
            (None, _) => Floor::None,
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
