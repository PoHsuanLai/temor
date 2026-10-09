//! Cuts kitty transfer sequences out of a byte stream, such as the replies a terminal writes to
//! a program's input. Anything else in the stream is ignored.

/// The longest payload kept; a longer sequence is dropped whole.
const MAX_PAYLOAD: usize = 64 * 1024;
const PREFIX: &[u8] = b"5113;";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Scan {
    Ground,
    Esc,
    Osc(Vec<u8>),
    OscEsc(Vec<u8>),
}

/// Feeds bytes in any split and returns each complete `ESC ] 5113 ; ... ST` payload (the bytes
/// between `ESC ]` and the terminator, including the `5113;`). `ST` is `ESC \` or BEL.
/// Not shared between threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameScanner {
    state: Scan,
}

impl Default for FrameScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameScanner {
    pub fn new() -> Self {
        Self {
            state: Scan::Ground,
        }
    }

    /// Reads `bytes` and returns the payloads that completed.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &byte in bytes {
            let state = std::mem::replace(&mut self.state, Scan::Ground);
            self.state = match (state, byte) {
                (Scan::Ground | Scan::Esc, 0x1b) => Scan::Esc,
                (Scan::Ground, _) => Scan::Ground,
                (Scan::Esc, b']') => Scan::Osc(Vec::new()),
                (Scan::Esc, _) => Scan::Ground,
                (Scan::Osc(buf), 0x07) => finish(buf, &mut out),
                (Scan::Osc(buf), 0x1b) => Scan::OscEsc(buf),
                (Scan::Osc(mut buf), byte) => {
                    if buf.len() >= MAX_PAYLOAD {
                        Scan::Ground
                    } else {
                        buf.push(byte);
                        Scan::Osc(buf)
                    }
                }
                (Scan::OscEsc(buf), b'\\') => finish(buf, &mut out),
                (Scan::OscEsc(_), b']') => Scan::Osc(Vec::new()),
                (Scan::OscEsc(_), 0x1b) => Scan::Esc,
                (Scan::OscEsc(_), _) => Scan::Ground,
            };
        }
        out
    }
}

fn finish(buf: Vec<u8>, out: &mut Vec<Vec<u8>>) -> Scan {
    if buf.starts_with(PREFIX) {
        out.push(buf);
    }
    Scan::Ground
}
