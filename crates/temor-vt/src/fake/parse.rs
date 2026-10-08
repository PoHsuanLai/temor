//! A byte-at-a-time VT parser just big enough for the fake: one step per byte, one action out.

/// What a byte completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    Print(char),
    /// A C0 control (BEL, BS, TAB, LF, CR, ...).
    Control(u8),
    /// `CSI [?] params final`.
    Csi {
        private: Private,
        params: Vec<u16>,
        last: u8,
    },
    /// Everything between `ESC ]` and BEL/ST.
    Osc(Vec<u8>),
}

/// Whether a CSI carried the `?` private marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Private {
    Dec,
    Ansi,
}

#[derive(Debug, Default)]
pub(super) enum Parser {
    #[default]
    Ground,
    Utf8 {
        buf: Vec<u8>,
        need: usize,
    },
    Esc,
    Csi(Vec<u8>),
    Osc(Vec<u8>),
    OscEsc(Vec<u8>),
}

impl Parser {
    /// Advances by one byte.
    pub(super) fn step(&mut self, b: u8) -> Option<Action> {
        match std::mem::take(self) {
            Parser::Ground => self.ground(b),
            Parser::Utf8 { mut buf, need } => {
                buf.push(b);
                if buf.len() < need {
                    *self = Parser::Utf8 { buf, need };
                    return None;
                }
                let c = std::str::from_utf8(&buf)
                    .ok()
                    .and_then(|s| s.chars().next());
                Some(Action::Print(c.unwrap_or('\u{fffd}')))
            }
            Parser::Esc => match b {
                b'[' => {
                    *self = Parser::Csi(Vec::new());
                    None
                }
                b']' => {
                    *self = Parser::Osc(Vec::new());
                    None
                }
                _ => None,
            },
            Parser::Csi(mut body) => {
                if (0x40..=0x7e).contains(&b) {
                    return Some(csi(&body, b));
                }
                body.push(b);
                *self = Parser::Csi(body);
                None
            }
            Parser::Osc(mut body) => match b {
                0x07 => Some(Action::Osc(body)),
                0x1b => {
                    *self = Parser::OscEsc(body);
                    None
                }
                _ => {
                    body.push(b);
                    *self = Parser::Osc(body);
                    None
                }
            },
            Parser::OscEsc(body) => (b == b'\\').then_some(Action::Osc(body)),
        }
    }

    fn ground(&mut self, b: u8) -> Option<Action> {
        match b {
            0x1b => {
                *self = Parser::Esc;
                None
            }
            0x00..=0x1f | 0x7f => Some(Action::Control(b)),
            0x20..=0x7e => Some(Action::Print(char::from(b))),
            _ => {
                let need = match b {
                    0xc0..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf7 => 4,
                    _ => return Some(Action::Print('\u{fffd}')),
                };
                *self = Parser::Utf8 { buf: vec![b], need };
                None
            }
        }
    }
}

fn csi(body: &[u8], last: u8) -> Action {
    let (private, body) = match body.split_first() {
        Some((b'?', rest)) => (Private::Dec, rest),
        _ => (Private::Ansi, body),
    };
    let params = body
        .split(|&c| c == b';')
        .filter(|p| !p.is_empty())
        .map(|p| {
            std::str::from_utf8(p)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        })
        .collect();
    Action::Csi {
        private,
        params,
        last,
    }
}
