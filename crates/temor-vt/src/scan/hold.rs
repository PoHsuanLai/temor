//! Where a program starts a synchronized update (`CSI ? 2026 h`), across chunk boundaries.
//! libghostty asks the host to capture the frame the instant a hold begins: "nothing after the
//! start of the hold has been processed yet". The terminal cuts its input right after the `h`
//! so the capture happens there, whatever else the chunk holds.
//!
//! Only the plain form `CSI ? 2026 h` is found. A hold set together with other modes is not cut;
//! the terminal then reports it without a frame, and the window falls back to the last one.

const START: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";
const ESC: u8 = 0x1b;

/// The cut-point scanner's state between chunks: how much of the start sequence has matched.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HoldCuts {
    matched: usize,
}

impl HoldCuts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The offsets in `input` just after each start sequence, in order.
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
            self.matched = next(self.matched, input[i]);
            i += 1;
            if self.matched == START.len() {
                cuts.push(i);
                self.matched = 0;
            }
        }
        cuts
    }
}

/// The matched length after `b`. The start sequence has one ESC, at its front, so a mismatch
/// falls back to 1 on an ESC and to 0 otherwise.
fn next(matched: usize, b: u8) -> usize {
    match b == START[matched] {
        true => matched + 1,
        false => usize::from(b == ESC),
    }
}

/// Whether `rest` ends a synchronized update (`CSI ? 2026 l`).
pub(crate) fn ends_within(rest: &[u8]) -> bool {
    rest.windows(END.len()).any(|w| w == END)
}
