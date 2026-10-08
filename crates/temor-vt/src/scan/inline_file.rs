//! A byte-at-a-time recogniser for `ESC ] 1337 ; File= ... BEL|ST`, safe across chunk splits.

use crate::{ByteCount, Truncation};

/// The introducer the scanner looks for.
const PREFIX: &[u8] = b"\x1b]1337;File=";
/// What a payload starts with: everything after `ESC ]`, as libghostty reports unknown OSCs.
const PAYLOAD_HEAD: &[u8] = b"1337;File=";

const BEL: u8 = 0x07;
const ESC: u8 = 0x1b;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;

/// What one chunk turns into, in order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Scanned<'a> {
    /// Bytes for libghostty.
    Bytes(&'a [u8]),
    /// One whole inline-file sequence: `1337;File=...` without introducer or terminator.
    File {
        payload: Vec<u8>,
        truncation: Truncation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Passing bytes through.
    Ground,
    /// The first `n` bytes of [`PREFIX`] are held back, possibly from an earlier chunk.
    Prefix(usize),
    /// Inside the payload.
    Payload,
    /// Inside the payload, right after an ESC (the start of ST, or an abort).
    PayloadEsc,
}

/// The scanner's state between chunks.
#[derive(Debug)]
pub(crate) struct InlineFileScanner {
    state: State,
    payload: Vec<u8>,
    max: ByteCount,
    truncation: Truncation,
}

impl InlineFileScanner {
    /// A scanner that keeps at most `max` payload bytes per sequence.
    pub(crate) fn new(max: ByteCount) -> Self {
        Self {
            state: State::Ground,
            payload: Vec::new(),
            max,
            truncation: Truncation::Complete,
        }
    }

    /// Splits `input` into pass-through bytes and complete inline files, in order. Bytes that
    /// may start a sequence are held until a later chunk decides.
    pub(crate) fn feed<'a>(&mut self, input: &'a [u8], mut out: impl FnMut(Scanned<'a>)) {
        let mut run_start = 0;
        let mut i = 0;
        while i < input.len() {
            match self.state {
                State::Ground => match input[i..].iter().position(|&b| b == ESC) {
                    None => i = input.len(),
                    Some(off) => {
                        let esc = i + off;
                        if esc > run_start {
                            out(Scanned::Bytes(&input[run_start..esc]));
                        }
                        self.state = State::Prefix(1);
                        i = esc + 1;
                    }
                },
                State::Prefix(n) => {
                    if input[i] == PREFIX[n] {
                        i += 1;
                        self.state = if n + 1 == PREFIX.len() {
                            self.start_payload();
                            State::Payload
                        } else {
                            State::Prefix(n + 1)
                        };
                    } else {
                        // Not ours: release what was held and look at this byte afresh.
                        out(Scanned::Bytes(&PREFIX[..n]));
                        self.state = State::Ground;
                        run_start = i;
                    }
                }
                State::Payload => {
                    let rest = &input[i..];
                    let stop = rest
                        .iter()
                        .position(|&b| matches!(b, BEL | ESC | CAN | SUB));
                    let body = &rest[..stop.unwrap_or(rest.len())];
                    self.push(body);
                    i += body.len();
                    if let Some(b) = stop.map(|s| rest[s]) {
                        i += 1;
                        match b {
                            BEL => {
                                out(self.finish());
                                self.state = State::Ground;
                                run_start = i;
                            }
                            ESC => self.state = State::PayloadEsc,
                            // CAN and SUB cancel the sequence; nothing is reported.
                            _ => {
                                self.state = State::Ground;
                                run_start = i;
                            }
                        }
                    }
                }
                State::PayloadEsc => {
                    if input[i] == b'\\' {
                        i += 1;
                        out(self.finish());
                        self.state = State::Ground;
                        run_start = i;
                    } else {
                        // ESC not followed by `\` cancels the sequence and starts a new one,
                        // which may itself be an inline file.
                        self.state = State::Prefix(1);
                    }
                }
            }
        }
        if self.state == State::Ground && run_start < input.len() {
            out(Scanned::Bytes(&input[run_start..]));
        }
    }

    fn start_payload(&mut self) {
        self.payload.clear();
        self.payload.extend_from_slice(PAYLOAD_HEAD);
        self.truncation = Truncation::Complete;
    }

    fn push(&mut self, bytes: &[u8]) {
        let room = (self.max.0 + PAYLOAD_HEAD.len()).saturating_sub(self.payload.len());
        if bytes.len() > room {
            self.truncation = Truncation::Truncated;
        }
        self.payload
            .extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    fn finish(&mut self) -> Scanned<'static> {
        Scanned::File {
            payload: std::mem::take(&mut self.payload),
            truncation: self.truncation,
        }
    }
}
