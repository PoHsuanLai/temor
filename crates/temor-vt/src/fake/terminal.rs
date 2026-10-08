//! The fake terminal: parser actions applied to a [`Screen`], events to the sink.

use super::parse::{Action, Parser, Private};
use super::screen::Screen;
use crate::scan::{InlineFileScanner, Scanned};
use crate::{
    CellPx, EventSink, ExitStatus, GridSize, ModeState, PromptEvent, PromptKind, TerminalConfig,
    TerminalMode, Truncation, UnknownKind, ViewportScroll, Vt, VtError, VtEvent,
};

/// A stand-in for `Terminal` that needs no Zig: see the module docs for what it understands.
pub struct FakeVt {
    screen: Screen,
    parser: Parser,
    scanner: InlineFileScanner,
    modes: Vec<TerminalMode>,
    title: String,
    pwd: String,
    sink: Box<dyn EventSink>,
}

impl std::fmt::Debug for FakeVt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeVt")
            .field("size", &self.screen.size)
            .finish_non_exhaustive()
    }
}

impl FakeVt {
    /// A blank fake of `config.size` sending its events to `sink`. Mode 2027 starts on, as in
    /// the real terminal.
    pub fn new(config: TerminalConfig, sink: Box<dyn EventSink>) -> Self {
        Self {
            screen: Screen::new(config.size),
            parser: Parser::default(),
            scanner: InlineFileScanner::new(config.inline_file_max),
            modes: vec![
                TerminalMode::GraphemeCluster,
                TerminalMode::Wraparound,
                TerminalMode::CursorVisible,
            ],
            title: String::new(),
            pwd: String::new(),
            sink,
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Print(c) => self.screen.print(c),
            Action::Control(0x07) => self.sink.send(VtEvent::Bell),
            Action::Control(0x08) => self.screen.backspace(),
            Action::Control(0x09) => self.screen.tab(),
            Action::Control(0x0a..=0x0c) => self.screen.linefeed(),
            Action::Control(0x0d) => self.screen.carriage_return(),
            Action::Control(_) => {}
            Action::Csi {
                private,
                params,
                last,
            } => self.csi(private, &params, last),
            Action::Osc(body) => self.osc(body),
        }
    }

    fn csi(&mut self, private: Private, params: &[u16], last: u8) {
        let first = params.first().copied().unwrap_or(0);
        match (private, last) {
            (Private::Ansi, b'H' | b'f') => {
                let row = first.max(1) - 1;
                let col = params.get(1).copied().unwrap_or(1).max(1) - 1;
                self.screen.goto(row, col);
            }
            (Private::Ansi, b'J') if first == 2 => self.screen.erase_display(),
            (Private::Ansi, b'c') if first == 0 => {
                self.sink.send(VtEvent::Reply(b"\x1b[?62;22c".to_vec()))
            }
            (Private::Dec, b'h' | b'l') => {
                let state = if last == b'h' {
                    ModeState::Set
                } else {
                    ModeState::Reset
                };
                for p in params {
                    if let Some(mode) = dec_mode(*p) {
                        self.set_mode(mode, state);
                    }
                }
            }
            _ => {}
        }
    }

    fn set_mode(&mut self, mode: TerminalMode, state: ModeState) {
        self.modes.retain(|m| *m != mode);
        if state == ModeState::Set {
            self.modes.push(mode);
        }
    }

    fn osc(&mut self, body: Vec<u8>) {
        let text = String::from_utf8_lossy(&body).into_owned();
        let (number, rest) = text.split_once(';').unwrap_or((text.as_str(), ""));
        match number {
            "0" | "2" => {
                self.title = rest.to_owned();
                self.sink.send(VtEvent::Title(self.title.clone()));
            }
            "7" => {
                self.pwd = rest.to_owned();
                self.sink.send(VtEvent::Cwd(self.pwd.clone()));
            }
            "133" => {
                if let Some(event) = prompt(rest) {
                    self.sink.send(VtEvent::Prompt(event));
                }
            }
            _ => self.sink.send(VtEvent::Unknown {
                kind: UnknownKind::Osc,
                payload: body,
                truncation: Truncation::Complete,
            }),
        }
    }
}

fn dec_mode(n: u16) -> Option<TerminalMode> {
    const MODES: &[TerminalMode] = &[
        TerminalMode::CursorKeys,
        TerminalMode::Wraparound,
        TerminalMode::CursorVisible,
        TerminalMode::MouseNormal,
        TerminalMode::MouseButton,
        TerminalMode::MouseAny,
        TerminalMode::MouseSgr,
        TerminalMode::AltScreen,
        TerminalMode::BracketedPaste,
        TerminalMode::SyncOutput,
        TerminalMode::GraphemeCluster,
    ];
    MODES.iter().copied().find(|m| m.number().0 == n)
}

/// The OSC 133 mark after `133;`.
fn prompt(rest: &str) -> Option<PromptEvent> {
    let mut parts = rest.split(';');
    Some(match parts.next()? {
        "A" => PromptEvent::PromptStart(PromptKind::Primary),
        "B" => PromptEvent::InputStart,
        "C" => PromptEvent::OutputStart {
            command: String::new(),
        },
        "D" => PromptEvent::CommandEnd {
            exit: parts
                .next()
                .and_then(|c| c.parse().ok())
                .map_or(ExitStatus::Unreported, ExitStatus::Code),
            error: String::new(),
        },
        _ => return None,
    })
}

impl Vt for FakeVt {
    fn write(&mut self, bytes: &[u8]) {
        let mut actions = Vec::new();
        let mut files = Vec::new();
        let parser = &mut self.parser;
        self.scanner.feed(bytes, |seg| match seg {
            Scanned::Bytes(b) => actions.extend(b.iter().filter_map(|&byte| parser.step(byte))),
            Scanned::File {
                payload,
                truncation,
            } => {
                files.push((
                    actions.len(),
                    VtEvent::Unknown {
                        kind: UnknownKind::Osc1337File,
                        payload,
                        truncation,
                    },
                ));
            }
        });
        let mut files = files.into_iter().peekable();
        for (i, action) in actions.into_iter().enumerate() {
            while let Some((_, event)) = files.next_if(|(at, _)| *at == i) {
                self.sink.send(event);
            }
            self.apply(action);
        }
        for (_, event) in files {
            self.sink.send(event);
        }
    }

    fn resize(&mut self, size: GridSize, _cell: CellPx) -> Result<(), VtError> {
        if size.cols.0 == 0 || size.rows.0 == 0 {
            return Err(VtError::InvalidValue("size"));
        }
        self.screen.resize(size);
        Ok(())
    }

    /// The fake has no viewport; scrolling does nothing.
    fn scroll_viewport(&mut self, _to: ViewportScroll) {}

    fn mode(&self, mode: TerminalMode) -> Result<ModeState, VtError> {
        Ok(if self.modes.contains(&mode) {
            ModeState::Set
        } else {
            ModeState::Reset
        })
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn pwd(&self) -> String {
        self.pwd.clone()
    }

    fn plain_text(&self) -> Result<String, VtError> {
        Ok(self.screen.text())
    }
}
