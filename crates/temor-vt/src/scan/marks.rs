//! Where OSC 133 shell marks end, across chunk boundaries. The terminal cuts its input right
//! after each whole mark so that the cursor can be anchored where the mark stood: an OSC moves
//! nothing, so the cursor after the mark is the cursor at the mark (PLAN §8.1, §9 blocks).

/// The introducer's text after `ESC ]`: a mark is `133;` and then its payload.
const MARK: &[u8] = b"133;";
const BEL: u8 = 0x07;
const ESC: u8 = 0x1b;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Ordinary text.
    Text,
    /// After an ESC: `]` opens an OSC.
    Esc,
    /// After `ESC ]`, with this many bytes of `133;` matched so far.
    Osc(usize),
    /// Inside an OSC that is not a mark. `esc` is set right after an ESC (the start of ST).
    Other { esc: bool },
    /// Inside a mark. `esc` is set right after an ESC (the start of ST).
    Mark { esc: bool },
}

/// The cut-point scanner's state between chunks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MarkCuts {
    state: State,
}

impl MarkCuts {
    pub(crate) fn new() -> Self {
        Self { state: State::Text }
    }

    /// The exclusive end offsets in `input` of each OSC 133 sequence that finishes in it, in
    /// order. A sequence split across chunks finishes in the chunk holding its terminator.
    pub(crate) fn feed(&mut self, input: &[u8]) -> Vec<usize> {
        let mut cuts = Vec::new();
        for (i, &b) in input.iter().enumerate() {
            self.state = match self.state {
                State::Text if b == ESC => State::Esc,
                State::Text => State::Text,
                State::Esc if b == b']' => State::Osc(0),
                State::Esc if b == ESC => State::Esc,
                State::Esc => State::Text,
                State::Osc(n) if n < MARK.len() && b == MARK[n] => {
                    if n + 1 == MARK.len() {
                        State::Mark { esc: false }
                    } else {
                        State::Osc(n + 1)
                    }
                }
                State::Osc(_) if b == BEL => State::Text,
                State::Osc(_) if b == ESC => State::Other { esc: true },
                State::Osc(_) => State::Other { esc: false },
                State::Other { esc: true } if b == b'\\' => State::Text,
                State::Other { .. } if b == BEL => State::Text,
                State::Other { .. } if b == ESC => State::Other { esc: true },
                State::Other { .. } => State::Other { esc: false },
                State::Mark { esc: true } if b == b'\\' => {
                    cuts.push(i + 1);
                    State::Text
                }
                State::Mark { .. } if b == BEL => {
                    cuts.push(i + 1);
                    State::Text
                }
                State::Mark { .. } if b == ESC => State::Mark { esc: true },
                State::Mark { .. } => State::Mark { esc: false },
            };
        }
        cuts
    }
}
