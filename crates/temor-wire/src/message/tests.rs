//! The name lists in `kinds.rs` against the enums they name. Each `*_kind` match is exhaustive,
//! so a new variant fails to compile until it is named here; the samples must then cover every
//! name, and serde's tag for each sample must be that name.

use super::kinds::{EVENT_KINDS, MESSAGE_KINDS, REMOTE_ERROR_KINDS, REPLY_KINDS, REQUEST_KINDS};
use super::*;
use crate::digest::ContentHash;
use crate::hello::{Capability, Hello, HelperVersion, ProtocolVersion};
use crate::ids::{RequestId, SessionId, TerminalId, WatchId};
use crate::units::{
    ByteCount, ByteOffset, ByteRange, CellPx, Cols, Pid, Port, Rows, Size, UnixMillis,
};
use std::collections::BTreeSet;

fn size() -> Size {
    Size {
        cols: Cols(80),
        rows: Rows(24),
    }
}

fn path() -> RemotePath {
    RemotePath("/w".to_string())
}

fn stamp() -> FileStamp {
    FileStamp {
        mtime: UnixMillis(1),
        hash: ContentHash([0; 32]),
    }
}

fn palette() -> Palette {
    let grey = Rgb { r: 9, g: 9, b: 9 };
    Palette {
        ansi: [grey; 16],
        foreground: grey,
        background: grey,
        cursor: grey,
    }
}

fn request_kind(value: &Request) -> &'static str {
    match value {
        Request::OpenSession(_) => "OpenSession",
        Request::AttachSession { .. } => "AttachSession",
        Request::DetachSession { .. } => "DetachSession",
        Request::Close { .. } => "Close",
        Request::ResizeSession { .. } => "ResizeSession",
        Request::SendInput { .. } => "SendInput",
        Request::ListSessions => "ListSessions",
        Request::SetIdleLimit { .. } => "SetIdleLimit",
        Request::SetPalette { .. } => "SetPalette",
        Request::Ping => "Ping",
        Request::ListPorts => "ListPorts",
        Request::ListDir { .. } => "ListDir",
        Request::Stat { .. } => "Stat",
        Request::ReadFile { .. } => "ReadFile",
        Request::WriteFile { .. } => "WriteFile",
        Request::Watch { .. } => "Watch",
        Request::Unwatch { .. } => "Unwatch",
        Request::Facts { .. } => "Facts",
        Request::RunTrial(_) => "RunTrial",
        Request::Relay { .. } => "Relay",
    }
}

fn request_samples() -> Vec<Request> {
    vec![
        Request::OpenSession(OpenSession {
            shell: None,
            cwd: path(),
            env: vec![],
            size: size(),
            cell: CellPx::DEFAULT,
            palette: Some(palette()),
        }),
        Request::AttachSession {
            session: SessionId(1),
        },
        Request::DetachSession {
            session: SessionId(1),
        },
        Request::Close {
            session: SessionId(1),
        },
        Request::ResizeSession {
            session: SessionId(1),
            size: size(),
            cell: CellPx::DEFAULT,
        },
        Request::SendInput {
            session: SessionId(1),
        },
        Request::ListSessions,
        Request::SetIdleLimit {
            limit: IdleLimit::Never,
        },
        Request::SetPalette {
            session: SessionId(1),
            palette: palette(),
        },
        Request::Ping,
        Request::ListPorts,
        Request::ListDir { path: path() },
        Request::Stat { path: path() },
        Request::ReadFile {
            path: path(),
            range: ByteRange {
                offset: ByteOffset(0),
                len: ByteCount(1),
            },
        },
        Request::WriteFile {
            path: path(),
            check: WriteCheck::Overwrite,
            mode: WriteMode::Whole,
        },
        Request::Watch { path: path() },
        Request::Unwatch { watch: WatchId(1) },
        Request::Facts {
            session: SessionId(1),
        },
        Request::RunTrial(TrialRequest {
            command: "true".to_string(),
            cwd: path(),
        }),
        Request::Relay {
            workspace: path(),
            call: RelayCall::KillTerminal {
                terminal: TerminalId(1),
            },
        },
    ]
}

fn event_kind(value: &Event) -> &'static str {
    match value {
        Event::Output { .. } => "Output",
        Event::Exited { .. } => "Exited",
        Event::Ports(_) => "Ports",
        Event::FileChanged { .. } => "FileChanged",
    }
}

fn event_samples() -> Vec<Event> {
    vec![
        Event::Output {
            session: SessionId(1),
        },
        Event::Exited {
            session: SessionId(1),
            status: ExitStatus::Code(crate::units::ExitCode(0)),
        },
        Event::Ports(vec![ListeningPort {
            port: Port(8080),
            pid: Pid(1),
            process: "node".to_string(),
        }]),
        Event::FileChanged {
            watch: WatchId(1),
            path: path(),
            kind: WatchChange::Modified,
        },
    ]
}

fn reply_kind(value: &Reply) -> &'static str {
    match value {
        Reply::Ack => "Ack",
        Reply::Pong => "Pong",
        Reply::Opened { .. } => "Opened",
        Reply::Attached { .. } => "Attached",
        Reply::Sessions(_) => "Sessions",
        Reply::Ports(_) => "Ports",
        Reply::Listing(_) => "Listing",
        Reply::Stat(_) => "Stat",
        Reply::Read { .. } => "Read",
        Reply::Written(_) => "Written",
        Reply::Watching { .. } => "Watching",
        Reply::Facts(_) => "Facts",
        Reply::Verdict(_) => "Verdict",
        Reply::Relayed(_) => "Relayed",
    }
}

fn reply_samples() -> Vec<Reply> {
    vec![
        Reply::Ack,
        Reply::Pong,
        Reply::Opened {
            session: SessionId(1),
        },
        Reply::Attached {
            session: SessionId(1),
            size: size(),
        },
        Reply::Sessions(vec![]),
        Reply::Ports(vec![]),
        Reply::Listing(vec![]),
        Reply::Stat(FileMeta {
            stamp: stamp(),
            len: ByteCount(0),
            kind: EntryKind::File,
        }),
        Reply::Read {
            stamp: stamp(),
            total: ByteCount(0),
        },
        Reply::Written(stamp()),
        Reply::Watching { watch: WatchId(1) },
        Reply::Facts(FolderFacts {
            dir: path(),
            git: None,
            venv: None,
            root: None,
            folders: vec![],
        }),
        Reply::Verdict(TrialVerdict::Unchecked(UncheckedWhy::TimedOut)),
        Reply::Relayed(RelayReply::Done),
    ]
}

fn remote_error_kind(value: &RemoteError) -> &'static str {
    match value {
        RemoteError::UnknownRequest => "UnknownRequest",
        RemoteError::NoSuchSession { .. } => "NoSuchSession",
        RemoteError::NoSuchWatch { .. } => "NoSuchWatch",
        RemoteError::NoSuchTerminal { .. } => "NoSuchTerminal",
        RemoteError::NotFound { .. } => "NotFound",
        RemoteError::PermissionDenied { .. } => "PermissionDenied",
        RemoteError::Conflict { .. } => "Conflict",
        RemoteError::HostRefused { .. } => "HostRefused",
        RemoteError::Unsupported { .. } => "Unsupported",
        RemoteError::TrialUnavailable { .. } => "TrialUnavailable",
        RemoteError::Unreadable { .. } => "Unreadable",
        RemoteError::BadRequest { .. } => "BadRequest",
        RemoteError::Io { .. } => "Io",
    }
}

fn remote_error_samples() -> Vec<RemoteError> {
    vec![
        RemoteError::UnknownRequest,
        RemoteError::NoSuchSession {
            session: SessionId(1),
        },
        RemoteError::NoSuchWatch { watch: WatchId(1) },
        RemoteError::NoSuchTerminal {
            terminal: TerminalId(1),
        },
        RemoteError::NotFound { path: path() },
        RemoteError::PermissionDenied { path: path() },
        RemoteError::Conflict { current: None },
        RemoteError::HostRefused {
            reason: Refusal::DeniedByHost,
        },
        RemoteError::Unsupported {
            capability: Capability::Files,
        },
        RemoteError::TrialUnavailable {
            reason: UncheckedWhy::NoSandbox,
        },
        RemoteError::Unreadable {
            detail: String::new(),
        },
        RemoteError::BadRequest {
            detail: String::new(),
        },
        RemoteError::Io {
            detail: String::new(),
        },
    ]
}

fn message_kind(value: &Message) -> &'static str {
    match value {
        Message::Hello(_) => "Hello",
        Message::Request { .. } => "Request",
        Message::Response { .. } => "Response",
        Message::Event(_) => "Event",
    }
}

fn message_samples() -> Vec<Message> {
    vec![
        Message::Hello(Hello {
            protocol: ProtocolVersion(1),
            oldest_protocol: ProtocolVersion(1),
            helper: HelperVersion(ContentHash([0; 32])),
            caps: vec![],
        }),
        Message::Request {
            id: RequestId(1),
            body: Request::ListSessions,
        },
        Message::Response {
            id: RequestId(1),
            outcome: Ok(Reply::Ack),
        },
        Message::Event(Event::Output {
            session: SessionId(1),
        }),
    ]
}

/// The serde tag of a value: a string for a unit variant, else the one key of its object.
fn tag<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value).unwrap() {
        serde_json::Value::String(name) => name,
        serde_json::Value::Object(entries) => entries.keys().next().unwrap().clone(),
        other => panic!("unexpected encoding {other}"),
    }
}

/// Holds one name list to its samples: every sample's serde tag is its name, every name is listed,
/// and every listed name is produced by some sample. `label` names the list in a failure.
fn check<T: serde::Serialize>(
    label: &str,
    list: &[&str],
    samples: &[T],
    name: fn(&T) -> &'static str,
) {
    let listed: BTreeSet<&str> = list.iter().copied().collect();
    assert_eq!(
        listed.len(),
        list.len(),
        "{label}: duplicate name in the list"
    );
    let mut produced = BTreeSet::new();
    for sample in samples {
        let kind = name(sample);
        assert_eq!(
            tag(sample),
            kind,
            "{label}: serde tag differs from its name"
        );
        produced.insert(kind);
    }
    assert_eq!(produced, listed, "{label}: the list and the enum disagree");
}

#[test]
fn the_name_lists_match_their_enums() {
    check("requests", REQUEST_KINDS, &request_samples(), request_kind);
    check("events", EVENT_KINDS, &event_samples(), event_kind);
    check("replies", REPLY_KINDS, &reply_samples(), reply_kind);
    check(
        "errors",
        REMOTE_ERROR_KINDS,
        &remote_error_samples(),
        remote_error_kind,
    );
    check("messages", MESSAGE_KINDS, &message_samples(), message_kind);
}
