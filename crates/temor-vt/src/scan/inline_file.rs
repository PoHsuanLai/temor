//! A byte-at-a-time recogniser for `ESC ] 1337 ; File= ... BEL|ST` and iTerm2's multipart forms
//! (`MultipartFile=`, `FilePart=`, `FileEnd`), safe across chunk splits. Ghostty parses OSC 1337
//! itself and drops all four as unimplemented, so all four are taken out here.

use super::controls::{BEL, ESC, ends_osc};
use crate::ByteCount;

/// The introducers the scanner looks for. None is a prefix of another.
const INTRODUCERS: &[&[u8]] = &[
    b"\x1b]1337;File=",
    b"\x1b]1337;MultipartFile=",
    b"\x1b]1337;FilePart=",
    b"\x1b]1337;FileEnd",
];
/// `ESC ]`: a payload is everything after it, as libghostty reports unknown OSCs.
const OSC_HEAD: usize = 2;

/// What one chunk turns into, in order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Scanned<'a> {
    /// Bytes for libghostty.
    Bytes(&'a [u8]),
    /// One whole inline-file sequence: `1337;File=...` without introducer or terminator.
    File { payload: Vec<u8>, truncated: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Passing bytes through.
    Ground,
    /// The bytes in `held` start an introducer and are held back, possibly from an earlier chunk.
    Prefix,
    /// Inside the payload.
    Payload,
    /// Inside the payload, right after an ESC (the start of ST, or an abort).
    PayloadEsc,
}

/// The scanner's state between chunks.
#[derive(Debug)]
pub(crate) struct InlineFileScanner {
    state: State,
    /// The introducer bytes held back while in [`State::Prefix`].
    held: Vec<u8>,
    payload: Vec<u8>,
    /// Payload bytes kept past the introducer's own length.
    head: usize,
    max: ByteCount,
    truncated: bool,
}

impl InlineFileScanner {
    /// A scanner that keeps at most `max` payload bytes per sequence.
    pub(crate) fn new(max: ByteCount) -> Self {
        Self {
            state: State::Ground,
            held: Vec::new(),
            payload: Vec::new(),
            head: 0,
            max,
            truncated: false,
        }
    }

    /// Splits `input` into pass-through bytes and complete inline files, in order.
    ///
    /// Bytes that may start a sequence are held back, not passed on: a chunk ending in a lone
    /// ESC, or in any prefix of an introducer such as `ESC ] 1337 ; File=`, reaches libghostty only when a later chunk
    /// shows it is not an inline file. A program that stops right there (an ESC as the very last
    /// byte it ever writes) leaves those few bytes unprocessed until more output arrives.
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
                        self.begin_prefix();
                        i = esc + 1;
                    }
                },
                State::Prefix => {
                    self.held.push(input[i]);
                    if INTRODUCERS.iter().any(|k| k.starts_with(&self.held)) {
                        i += 1;
                        if INTRODUCERS.contains(&self.held.as_slice()) {
                            self.start_payload();
                            self.state = State::Payload;
                        }
                    } else {
                        // Not ours: release what was held and look at this byte afresh.
                        self.held.pop();
                        out(Scanned::Bytes(release(&self.held)));
                        self.held.clear();
                        self.state = State::Ground;
                        run_start = i;
                    }
                }
                State::Payload => {
                    let rest = &input[i..];
                    let stop = rest.iter().position(|&b| ends_osc(b));
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
                                self.cancel();
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
                        self.cancel();
                        self.begin_prefix();
                    }
                }
            }
        }
        if self.state == State::Ground && run_start < input.len() {
            out(Scanned::Bytes(&input[run_start..]));
        }
    }

    /// Drops a cancelled payload, releasing its memory (it can be large).
    fn cancel(&mut self) {
        self.payload = Vec::new();
    }

    /// How many payload bytes are held.
    #[cfg(test)]
    pub(crate) fn held_payload(&self) -> usize {
        self.payload.len()
    }

    /// An ESC was seen: it may start an introducer.
    fn begin_prefix(&mut self) {
        self.held.clear();
        self.held.push(ESC);
        self.state = State::Prefix;
    }

    fn start_payload(&mut self) {
        self.payload.clear();
        self.payload.extend_from_slice(&self.held[OSC_HEAD..]);
        self.head = self.payload.len();
        self.held.clear();
        self.truncated = false;
    }

    fn push(&mut self, bytes: &[u8]) {
        let room = (self.max.0 + self.head).saturating_sub(self.payload.len());
        if bytes.len() > room {
            self.truncated = true;
        }
        self.payload
            .extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    fn finish(&mut self) -> Scanned<'static> {
        Scanned::File {
            payload: std::mem::take(&mut self.payload),
            truncated: self.truncated,
        }
    }
}

/// The held introducer bytes as a slice with the lifetime of the static introducers, so they can
/// be handed on as pass-through bytes: they are always a prefix of one of [`INTRODUCERS`].
fn release(held: &[u8]) -> &'static [u8] {
    INTRODUCERS
        .iter()
        .find(|k| k.starts_with(held))
        .map_or(&[][..], |k| &k[..held.len()])
}
