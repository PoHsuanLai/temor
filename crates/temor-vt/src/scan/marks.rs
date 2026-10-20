//! Where OSC 133 shell marks end, across chunk boundaries. The terminal cuts its input at each
//! mark's terminator, so the cursor can be anchored where the mark stood: an OSC moves nothing.
//! The rules follow libghostty's parse table (PLAN §8.1, §9): an OSC ends at BEL, ST (ESC \),
//! CAN, SUB or any ESC. C1 bytes are payload inside an OSC or a DCS and are not controls in text
//! (libghostty is UTF-8 only), so none of them ends or starts a mark. A mark is cut right after
//! the byte that ends it, which is when libghostty reports it. SOS, PM and APC payloads are
//! ignored up to their end (C1 ST ends one), and an ESC inside any of them aborts it.

use super::controls::{CAN, ESC, SUB, ends_osc};

/// Introducer text after `ESC ]` that makes an OSC a shell mark.
const MARK: &[u8] = b"133;";
const C1_ST: u8 = 0x9C;
const C1_OSC: u8 = 0x9D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Ordinary text.
    Text,
    /// After an ESC.
    Esc,
    /// After the OSC introducer, with this many bytes of `133;` matched.
    Osc(usize),
    /// Inside an OSC that is not a mark.
    Other,
    /// Inside a mark.
    Mark,
    /// Inside a DCS payload (ignored until ESC, CAN or SUB).
    DcsPayload,
    /// Inside an SOS, PM or APC payload (ignored until ST, CAN, SUB or ESC).
    StringPayload,
}

/// A mark ended at this byte: the cut falls right after it, which is where libghostty reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cut;

/// The cut-point scanner's state between chunks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MarkCuts {
    state: State,
}

impl MarkCuts {
    pub(crate) fn new() -> Self {
        Self { state: State::Text }
    }

    /// The offsets in `input` at which a mark's bytes end, in order: the piece before each offset
    /// holds one whole mark. A mark split across chunks ends in the chunk holding its terminator.
    pub(crate) fn feed(&mut self, input: &[u8]) -> Vec<usize> {
        let mut cuts = Vec::new();
        let mut i = 0;
        while i < input.len() {
            if self.state == State::Text {
                // Fast path: nothing changes in text until one of these bytes.
                match input[i..].iter().position(|&b| starts_sequence(b)) {
                    Some(at) => i += at,
                    None => break,
                }
            }
            self.byte(i, input[i], &mut cuts);
            i += 1;
        }
        cuts
    }

    fn byte(&mut self, at: usize, b: u8, cuts: &mut Vec<usize>) {
        let (next, cut) = step(self.state, b);
        if cut.is_some() {
            cuts.push(at + 1);
        }
        self.state = next;
    }
}

/// Bytes that leave the text state.
fn starts_sequence(b: u8) -> bool {
    b == ESC
}

/// One byte: the state after it, and a cut if a mark ended.
fn step(state: State, b: u8) -> (State, Option<Cut>) {
    use State::*;
    let plain = |s| (s, None);
    match state {
        Text => match b {
            ESC => plain(Esc),
            _ => plain(Text),
        },
        Esc => match b {
            b']' => plain(Osc(0)),
            b'P' => plain(DcsPayload),
            b'X' | b'^' | b'_' => plain(StringPayload),
            ESC => plain(Esc),
            _ => plain(Text),
        },
        Osc(n) if n < MARK.len() && b == MARK[n] => plain(if n + 1 == MARK.len() {
            Mark
        } else {
            Osc(n + 1)
        }),
        Osc(_) | Other => osc_byte(b),
        Mark => match b {
            // libghostty reports the mark when it reads the ESC (of ST, or of whatever follows),
            // so the ESC goes with the mark and the rest of the sequence with the next piece.
            ESC => (Esc, Some(Cut)),
            _ if ends_osc(b) => (Text, Some(Cut)),
            _ => plain(Mark),
        },
        DcsPayload => match b {
            ESC => plain(Esc),
            CAN | SUB => plain(Text),
            _ => plain(DcsPayload),
        },
        StringPayload => match b {
            ESC => plain(Esc),
            C1_ST | CAN | SUB => plain(Text),
            C1_OSC => plain(Osc(0)),
            _ => plain(StringPayload),
        },
    }
}

/// A byte inside an OSC that is not a mark.
fn osc_byte(b: u8) -> (State, Option<Cut>) {
    match b {
        ESC => (State::Esc, None),
        _ if ends_osc(b) => (State::Text, None),
        _ => (State::Other, None),
    }
}
