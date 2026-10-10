//! The line protocol, version 1. One request per LF-terminated line of UTF-8, at most
//! [`MAX_LINE`] bytes; one reply line per request.
//!
//! ```text
//! client: hello 1
//! server: ok 1
//! client: status 7 waiting wait=permission app=claude note=Run%20cargo%20test%3F
//! server: ok 7
//! client: ping
//! server: pong
//! ```
//!
//! `status <seq> <state> [wait=<kind>] [app=<name>] [note=<text>]`: `seq` is a counter that only
//! grows within a pane; `state` is idle, working, waiting, done or failed; `wait` is permission,
//! question or sign_in and only means something for waiting; `app` and `note` are percent-encoded UTF-8
//! (`%XX` for every byte that is not a letter, digit, `-`, `_`, `.` or `~`). Unknown keys are
//! ignored so a later version can add some. A refusal is `err <code>`; a bad version or a line
//! over the limit also ends the connection.

use std::fmt;

/// The protocol version this library speaks.
pub const VERSION: u32 = 1;
/// The longest request line, in bytes, without its LF.
pub const MAX_LINE: usize = 4096;
/// The longest note, in bytes once decoded.
const MAX_NOTE: usize = 512;
/// The longest app name, in bytes.
const MAX_APP: usize = 64;

/// What the reporting program says it is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReportState {
    Idle,
    Working,
    Waiting,
    Done,
    Failed,
}

/// What a waiting program needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReportWait {
    Permission,
    Question,
    SignIn,
}

/// One status report from a pane's socket. Plain data: `Send` and `Sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    /// Its place in the sender's sequence.
    pub seq: u64,
    pub state: ReportState,
    /// What it waits for; only kept when `state` is waiting.
    pub wait: Option<ReportWait>,
    /// A stable program name, such as `claude`.
    pub app: Option<String>,
    /// The program's own words, control characters removed.
    pub note: Option<String>,
}

/// Why a line was refused. The code is what goes after `err `.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReportRefusal {
    /// The version is not one this side speaks.
    Version,
    /// The first line was not `hello`.
    NeedHello,
    /// The line did not parse.
    Syntax,
    /// A line, note or name went past its limit.
    TooLong,
}

impl ReportRefusal {
    /// The code on the wire.
    pub fn code(self) -> &'static str {
        match self {
            ReportRefusal::Version => "version",
            ReportRefusal::NeedHello => "need_hello",
            ReportRefusal::Syntax => "syntax",
            ReportRefusal::TooLong => "too_long",
        }
    }

    /// Whether the connection ends after this refusal.
    pub(super) fn ends_connection(self) -> bool {
        matches!(self, ReportRefusal::Version | ReportRefusal::TooLong)
    }
}

impl fmt::Display for ReportRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// A request line, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Request {
    Hello(u32),
    Status(StatusLine),
    Ping,
}

pub(super) fn parse_request(line: &str) -> Result<Request, ReportRefusal> {
    if line.len() > MAX_LINE {
        return Err(ReportRefusal::TooLong);
    }
    let mut words = line.split(' ').filter(|w| !w.is_empty());
    match words.next() {
        Some("hello") => {
            let version = words.next().and_then(|v| v.parse().ok());
            version.map(Request::Hello).ok_or(ReportRefusal::Syntax)
        }
        Some("ping") => Ok(Request::Ping),
        Some("status") => parse_status(words).map(Request::Status),
        _ => Err(ReportRefusal::Syntax),
    }
}

fn parse_status<'a>(mut words: impl Iterator<Item = &'a str>) -> Result<StatusLine, ReportRefusal> {
    let seq = words.next().and_then(|s| s.parse().ok());
    let state = words.next().and_then(|s| match s {
        "idle" => Some(ReportState::Idle),
        "working" => Some(ReportState::Working),
        "waiting" => Some(ReportState::Waiting),
        "done" => Some(ReportState::Done),
        "failed" => Some(ReportState::Failed),
        _ => None,
    });
    let (Some(seq), Some(state)) = (seq, state) else {
        return Err(ReportRefusal::Syntax);
    };
    let mut line = StatusLine {
        seq,
        state,
        wait: None,
        app: None,
        note: None,
    };
    for word in words {
        match word.split_once('=') {
            Some(("wait", "permission")) => line.wait = Some(ReportWait::Permission),
            Some(("wait", "question")) => line.wait = Some(ReportWait::Question),
            Some(("wait", "sign_in")) => line.wait = Some(ReportWait::SignIn),
            Some(("wait", _)) => return Err(ReportRefusal::Syntax),
            Some(("app", app)) => {
                let app = decode_note(app)?;
                if app.len() > MAX_APP {
                    return Err(ReportRefusal::TooLong);
                }
                line.app = Some(app);
            }
            Some(("note", note)) => line.note = Some(decode_note(note)?),
            _ => {}
        }
    }
    if line.state != ReportState::Waiting {
        line.wait = None;
    }
    Ok(line)
}

fn decode_note(raw: &str) -> Result<String, ReportRefusal> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3).ok_or(ReportRefusal::Syntax)?;
            let hex = std::str::from_utf8(hex).map_err(|_| ReportRefusal::Syntax)?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| ReportRefusal::Syntax)?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
        if out.len() > MAX_NOTE {
            return Err(ReportRefusal::TooLong);
        }
    }
    let text = String::from_utf8(out).map_err(|_| ReportRefusal::Syntax)?;
    Ok(text.chars().filter(|c| !c.is_control()).collect())
}

fn encode_note(note: &str) -> String {
    let mut out = String::new();
    for b in note.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b));
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The first line a client sends.
pub fn encode_hello() -> String {
    format!("hello {VERSION}\n")
}

/// A status request line, ready to write (LF included). For the `temor` command and adapters.
pub fn encode_status(line: &StatusLine) -> String {
    let state = match line.state {
        ReportState::Idle => "idle",
        ReportState::Working => "working",
        ReportState::Waiting => "waiting",
        ReportState::Done => "done",
        ReportState::Failed => "failed",
    };
    let mut out = format!("status {} {state}", line.seq);
    match line.wait {
        Some(ReportWait::Permission) => out.push_str(" wait=permission"),
        Some(ReportWait::Question) => out.push_str(" wait=question"),
        Some(ReportWait::SignIn) => out.push_str(" wait=sign_in"),
        None => {}
    }
    if let Some(app) = &line.app {
        out.push_str(" app=");
        out.push_str(&encode_note(app));
    }
    if let Some(note) = &line.note {
        out.push_str(" note=");
        out.push_str(&encode_note(note));
    }
    out.push('\n');
    out
}

/// What a server's reply line says.
pub fn parse_reply(line: &str) -> Result<(), ReportRefusal> {
    let line = line.trim_end();
    if line == "pong" || line.starts_with("ok") {
        return Ok(());
    }
    match line.strip_prefix("err ") {
        Some("version") => Err(ReportRefusal::Version),
        Some("need_hello") => Err(ReportRefusal::NeedHello),
        Some("too_long") => Err(ReportRefusal::TooLong),
        _ => Err(ReportRefusal::Syntax),
    }
}
