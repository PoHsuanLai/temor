//! Where a program starts a synchronized update (`CSI ? 2026 h`), across chunk boundaries.
//! libghostty asks the host to capture the frame the instant a hold begins: "nothing after the
//! start of the hold has been processed yet". The terminal cuts its input right after the `h`
//! so the capture happens there, whatever else the chunk holds.
//!
//! Only the plain form `CSI ? 2026 h` is found. A hold set together with other modes is not cut;
//! the terminal then reports it without a frame, and the window falls back to the last one.

const START: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";
const ERASE_HISTORY: &[u8] = b"\x1b[3J";
const ESC: u8 = 0x1b;

/// Where a fixed sequence ends, across chunks: how much of it has matched so far. The sequence
/// has one ESC, at its front: the start of a hold, or the erase of the scrollback (`CSI 3 J`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SeqCuts {
    seq: &'static [u8],
    matched: usize,
}

impl SeqCuts {
    /// Finds the start of a synchronized update.
    pub(crate) fn hold_start() -> Self {
        Self::of(START)
    }

    /// Finds the erase of the scrollback, `CSI 3 J`.
    pub(crate) fn erase_history() -> Self {
        Self::of(ERASE_HISTORY)
    }

    fn of(seq: &'static [u8]) -> Self {
        Self { seq, matched: 0 }
    }

    /// The offsets in `input` just after each whole sequence, in order.
    pub(crate) fn feed(&mut self, input: &[u8]) -> Vec<usize> {
        let mut cuts = Vec::new();
        let mut i = 0;
        while i < input.len() {
            if self.matched == 0 {
                // Fast path: nothing changes until an ESC.
                match input[i..].iter().position(|&b| b == ESC) {
                    Some(at) => i += at,
                    None => break,
                }
            }
            self.matched = self.next(input[i]);
            i += 1;
            if self.matched == self.seq.len() {
                cuts.push(i);
                self.matched = 0;
            }
        }
        cuts
    }

    /// The matched length after `b`. The sequence has one ESC, at its front, so a mismatch
    /// falls back to 1 on an ESC and to 0 otherwise.
    fn next(&self, b: u8) -> usize {
        match b == self.seq[self.matched] {
            true => self.matched + 1,
            false => usize::from(b == ESC),
        }
    }
}

/// Whether `rest` ends a synchronized update (`CSI ? 2026 l`).
pub(crate) fn ends_within(rest: &[u8]) -> bool {
    rest.windows(END.len()).any(|w| w == END)
}
