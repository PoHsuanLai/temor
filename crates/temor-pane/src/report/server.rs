//! The pane's socket: bind, accept, read lines, answer. Nothing blocks and no thread is started:
//! the caller calls [`PaneSocket::pump`] when it wants reports (on a timer, or when the socket's
//! descriptor is readable). `Send`: move it to the one thread that pumps it.

use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use super::protocol::{MAX_LINE, ReportRefusal, Request, StatusLine, VERSION, parse_request};
use crate::error::PaneError;
use crate::platform;

/// A pane's id as the socket and `TEMOR_PANE_ID` carry it: 1 to 32 of letters, digits, `-`, `_`,
/// so it can be a file name and never climbs out of the directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaneKey(String);

impl PaneKey {
    /// `id` as a key, or why not.
    pub fn new(id: &str) -> Result<PaneKey, PaneError> {
        let ok = (1..=32).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        match ok {
            true => Ok(PaneKey(id.to_owned())),
            false => Err(PaneError::BadPaneId(id.to_owned())),
        }
    }

    /// The id text, for `TEMOR_PANE_ID`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How much one socket will take on at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketLimits {
    /// Connections held open; one more is closed on arrival.
    pub clients: usize,
    /// Request lines read from one connection per pump.
    pub lines_per_pump: usize,
}

impl Default for SocketLimits {
    fn default() -> Self {
        SocketLimits {
            clients: 8,
            lines_per_pump: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Greeting {
    Waiting,
    Done,
}

#[derive(Debug)]
struct Client {
    stream: UnixStream,
    buf: Vec<u8>,
    greeting: Greeting,
}

/// What reading a client ended with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    Keep,
    Drop,
}

/// One pane's listening socket, `<dir>/<id>.sock`, removed when dropped.
#[derive(Debug)]
pub struct PaneSocket {
    listener: UnixListener,
    path: PathBuf,
    key: PaneKey,
    limits: SocketLimits,
    clients: Vec<Client>,
}

impl PaneSocket {
    /// Listens on `<dir>/<id>.sock`. `dir` is made private (0700, ours) if it is not: it should
    /// sit inside temor's private runtime directory, which must already exist. An old socket
    /// file of the same name (a crashed run) is replaced.
    pub fn bind(dir: &Path, key: PaneKey, limits: SocketLimits) -> Result<PaneSocket, PaneError> {
        let io_err = |source| PaneError::Socket {
            path: dir.to_path_buf(),
            source,
        };
        temor_shell::ensure_private_dir(dir).map_err(|error| match error {
            temor_shell::ShellError::PrivateDir { source, .. }
                if source.kind() == ErrorKind::NotFound =>
            {
                PaneError::SocketDirMissing(dir.to_path_buf())
            }
            other => PaneError::from(other),
        })?;
        let path = dir.join(format!("{}.sock", key.as_str()));
        if path.as_os_str().len() > temor_shell::SOCKET_PATH_MAX {
            return Err(PaneError::SocketPathTooLong(path));
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(io_err(err)),
        }
        let listener = UnixListener::bind(&path).map_err(io_err)?;
        // Only we may connect, whatever the umask made of the file.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(io_err)?;
        listener.set_nonblocking(true).map_err(io_err)?;
        Ok(PaneSocket {
            listener,
            path,
            key,
            limits,
            clients: Vec::new(),
        })
    }

    /// The socket's path, for `TEMOR_SOCKET`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The pane this socket belongs to.
    pub fn key(&self) -> &PaneKey {
        &self.key
    }

    /// Takes new connections and reads what clients have sent, without waiting. Returns the
    /// status reports in arrival order. A client of another user is closed unanswered; a bad
    /// line is answered `err <code>`; a client that cannot take its reply is closed.
    pub fn pump(&mut self) -> Vec<StatusLine> {
        self.accept();
        let mut reports = Vec::new();
        let per_pump = self.limits.lines_per_pump;
        self.clients
            .retain_mut(|client| match read_client(client, per_pump, &mut reports) {
                Fate::Keep => true,
                Fate::Drop => false,
            });
        reports
    }

    fn accept(&mut self) {
        let me = platform::own_uid();
        while let Ok((stream, _)) = self.listener.accept() {
            let ours = platform::peer_uid(&stream).is_ok_and(|uid| uid == me);
            let room = self.clients.len() < self.limits.clients;
            if !ours || !room || stream.set_nonblocking(true).is_err() {
                continue;
            }
            self.clients.push(Client {
                stream,
                buf: Vec::new(),
                greeting: Greeting::Waiting,
            });
        }
    }
}

impl Drop for PaneSocket {
    fn drop(&mut self) {
        // Best effort: a missing file is the goal anyway.
        let _ = std::fs::remove_file(&self.path);
    }
}

fn read_client(client: &mut Client, max_lines: usize, reports: &mut Vec<StatusLine>) -> Fate {
    let mut chunk = [0u8; 1024];
    loop {
        match client.stream.read(&mut chunk) {
            Ok(0) => return drain(client, max_lines, reports, Fate::Drop),
            Ok(n) => {
                client.buf.extend_from_slice(&chunk[..n]);
                if client.buf.len() > 4 * MAX_LINE {
                    // A flood of unread input: stop reading until the next pump.
                    break;
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => break,
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Fate::Drop,
        }
    }
    drain(client, max_lines, reports, Fate::Keep)
}

/// Answers every whole line in the buffer (up to `max_lines`); `after` is the fate when all went well.
fn drain(
    client: &mut Client,
    max_lines: usize,
    reports: &mut Vec<StatusLine>,
    after: Fate,
) -> Fate {
    for _ in 0..max_lines {
        let Some(end) = client.buf.iter().position(|b| *b == b'\n') else {
            let too_long = client.buf.len() > MAX_LINE;
            if too_long {
                let _ = reply(client, &format!("err {}\n", ReportRefusal::TooLong));
                return Fate::Drop;
            }
            return after;
        };
        let raw: Vec<u8> = client.buf.drain(..=end).collect();
        let line = std::str::from_utf8(&raw[..end])
            .map(|s| s.trim_end_matches('\r'))
            .map_err(|_| ReportRefusal::Syntax)
            .and_then(parse_request);
        let fate = match line {
            Ok(request) => handle(client, request, reports),
            Err(refusal) => refuse(client, refusal),
        };
        if fate == Fate::Drop {
            return Fate::Drop;
        }
    }
    Fate::Keep
}

fn handle(client: &mut Client, request: Request, reports: &mut Vec<StatusLine>) -> Fate {
    match (client.greeting, request) {
        (Greeting::Waiting, Request::Hello(VERSION)) => {
            client.greeting = Greeting::Done;
            answer(client, &format!("ok {VERSION}\n"))
        }
        (Greeting::Waiting, Request::Hello(_)) => refuse(client, ReportRefusal::Version),
        (Greeting::Waiting, _) => refuse(client, ReportRefusal::NeedHello),
        (Greeting::Done, Request::Hello(_)) => refuse(client, ReportRefusal::Syntax),
        (Greeting::Done, Request::Ping) => answer(client, "pong\n"),
        (Greeting::Done, Request::Status(line)) => {
            let seq = line.seq;
            reports.push(line);
            answer(client, &format!("ok {seq}\n"))
        }
    }
}

fn refuse(client: &mut Client, refusal: ReportRefusal) -> Fate {
    let sent = reply(client, &format!("err {refusal}\n"));
    match sent.is_ok() && !refusal.ends_connection() {
        true => Fate::Keep,
        false => Fate::Drop,
    }
}

fn answer(client: &mut Client, text: &str) -> Fate {
    match reply(client, text) {
        Ok(()) => Fate::Keep,
        Err(_) => Fate::Drop,
    }
}

/// A reply is a few bytes; if the socket cannot take it at once the client is not reading.
fn reply(client: &mut Client, text: &str) -> io::Result<()> {
    client.stream.write_all(text.as_bytes())
}
