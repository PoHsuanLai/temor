//! Generators for requests, replies, events, errors and whole messages.

use super::values::*;
use proptest::prelude::*;
use temor_wire::hello::Hello;
use temor_wire::message::{
    DirEntry, Event, Message, OpenSession, RelayCall, RelayReply, RemoteError, Reply, Request,
    TrialRequest, Verdict,
};
use temor_wire::units::UnixMillis;

fn hello() -> impl Strategy<Value = Hello> {
    (
        protocol_version(),
        protocol_version(),
        helper_version(),
        prop::collection::vec(capability(), 0..5),
    )
        .prop_map(|(protocol, oldest_protocol, helper, caps)| Hello {
            protocol,
            oldest_protocol,
            helper,
            caps,
        })
}

fn open_session() -> impl Strategy<Value = OpenSession> {
    (prop::option::of(path()), path(), env_vars(), size()).prop_map(|(shell, cwd, env, size)| {
        OpenSession {
            shell,
            cwd,
            env,
            size,
        }
    })
}

fn trial_request() -> impl Strategy<Value = TrialRequest> {
    (text(), path()).prop_map(|(command, cwd)| TrialRequest { command, cwd })
}

fn relay_call() -> impl Strategy<Value = RelayCall> {
    prop_oneof![
        (
            path(),
            prop::option::of(line_number()),
            prop::option::of(line_count())
        )
            .prop_map(|(path, line, limit)| RelayCall::ReadText { path, line, limit }),
        (path(), text()).prop_map(|(path, content)| RelayCall::WriteText { path, content }),
        (
            text(),
            prop::collection::vec(text(), 0..3),
            env_vars(),
            prop::option::of(path()),
            byte_count(),
        )
            .prop_map(|(command, args, env, cwd, output_limit)| {
                RelayCall::CreateTerminal {
                    command,
                    args,
                    env,
                    cwd,
                    output_limit,
                }
            }),
        terminal_id().prop_map(|terminal| RelayCall::TerminalOutput { terminal }),
        terminal_id().prop_map(|terminal| RelayCall::WaitForExit { terminal }),
        terminal_id().prop_map(|terminal| RelayCall::KillTerminal { terminal }),
        terminal_id().prop_map(|terminal| RelayCall::ReleaseTerminal { terminal }),
    ]
}

pub fn request() -> impl Strategy<Value = Request> {
    prop_oneof![
        open_session().prop_map(Request::OpenSession),
        session_id().prop_map(|session| Request::AttachSession { session }),
        session_id().prop_map(|session| Request::DetachSession { session }),
        (session_id(), size()).prop_map(|(session, size)| Request::ResizeSession { session, size }),
        session_id().prop_map(|session| Request::SendInput { session }),
        Just(Request::ListSessions),
        idle_limit().prop_map(|limit| Request::SetIdleLimit { limit }),
        Just(Request::ListPorts),
        path().prop_map(|path| Request::ListDir { path }),
        path().prop_map(|path| Request::Stat { path }),
        (path(), byte_range()).prop_map(|(path, range)| Request::ReadFile { path, range }),
        (path(), write_check()).prop_map(|(path, check)| Request::WriteFile { path, check }),
        path().prop_map(|path| Request::Watch { path }),
        watch_id().prop_map(|watch| Request::Unwatch { watch }),
        trial_request().prop_map(Request::RunTrial),
        (path(), relay_call()).prop_map(|(workspace, call)| Request::Relay { workspace, call }),
    ]
}

fn dir_entry() -> impl Strategy<Value = DirEntry> {
    (text(), entry_kind(), any::<u64>(), any::<i64>()).prop_map(|(name, kind, len, mtime)| {
        DirEntry {
            name,
            kind,
            len: temor_wire::units::ByteCount(len),
            mtime: UnixMillis(mtime),
        }
    })
}

fn verdict() -> impl Strategy<Value = Verdict> {
    prop_oneof![
        Just(Verdict::Passed),
        exit_code().prop_map(|exit| Verdict::Failed { exit }),
        unchecked_why().prop_map(|reason| Verdict::Unchecked { reason }),
    ]
}

fn relay_reply() -> impl Strategy<Value = RelayReply> {
    prop_oneof![
        text().prop_map(RelayReply::Text),
        Just(RelayReply::Written),
        terminal_id().prop_map(RelayReply::Terminal),
        (text(), truncation(), prop::option::of(exit_status())).prop_map(
            |(text, truncation, exit)| RelayReply::Output {
                text,
                truncation,
                exit,
            }
        ),
        exit_status().prop_map(RelayReply::Exit),
        Just(RelayReply::Done),
    ]
}

pub fn reply() -> impl Strategy<Value = Reply> {
    prop_oneof![
        Just(Reply::Ack),
        session_id().prop_map(|session| Reply::Opened { session }),
        (session_id(), size()).prop_map(|(session, size)| Reply::Attached { session, size }),
        prop::collection::vec(
            (session_id(), path(), size(), session_state()).prop_map(
                |(session, cwd, size, state)| temor_wire::message::SessionInfo {
                    session,
                    cwd,
                    size,
                    state,
                }
            ),
            0..3,
        )
        .prop_map(Reply::Sessions),
        prop::collection::vec(listening_port(), 0..4).prop_map(Reply::Ports),
        prop::collection::vec(dir_entry(), 0..4).prop_map(Reply::Listing),
        file_meta().prop_map(Reply::Stat),
        (file_stamp(), byte_count()).prop_map(|(stamp, total)| Reply::Read { stamp, total }),
        file_stamp().prop_map(Reply::Written),
        watch_id().prop_map(|watch| Reply::Watching { watch }),
        verdict().prop_map(Reply::Verdict),
        relay_reply().prop_map(Reply::Relayed),
    ]
}

pub fn remote_error() -> impl Strategy<Value = RemoteError> {
    prop_oneof![
        session_id().prop_map(|session| RemoteError::NoSuchSession { session }),
        watch_id().prop_map(|watch| RemoteError::NoSuchWatch { watch }),
        terminal_id().prop_map(|terminal| RemoteError::NoSuchTerminal { terminal }),
        path().prop_map(|path| RemoteError::NotFound { path }),
        path().prop_map(|path| RemoteError::PermissionDenied { path }),
        prop::option::of(file_stamp()).prop_map(|current| RemoteError::Conflict { current }),
        refusal().prop_map(|reason| RemoteError::HostRefused { reason }),
        capability().prop_map(|capability| RemoteError::Unsupported { capability }),
        unchecked_why().prop_map(|reason| RemoteError::TrialUnavailable { reason }),
        text().prop_map(|detail| RemoteError::BadRequest { detail }),
        text().prop_map(|detail| RemoteError::Io { detail }),
    ]
}

pub fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        session_id().prop_map(|session| Event::Output { session }),
        (session_id(), exit_status())
            .prop_map(|(session, status)| Event::Exited { session, status }),
        prop::collection::vec(listening_port(), 0..4).prop_map(Event::Ports),
        (watch_id(), path(), change_kind()).prop_map(|(watch, path, kind)| Event::FileChanged {
            watch,
            path,
            kind,
        }),
    ]
}

/// Any message the protocol carries.
pub fn any_message() -> impl Strategy<Value = Message> {
    prop_oneof![
        hello().prop_map(Message::Hello),
        (request_id(), request()).prop_map(|(id, body)| Message::Request { id, body }),
        (
            request_id(),
            prop_oneof![reply().prop_map(Ok), remote_error().prop_map(Err),]
        )
            .prop_map(|(id, outcome)| Message::Response { id, outcome }),
        event().prop_map(Message::Event),
    ]
}

/// A payload of any length up to 4 KiB, with every byte value possible.
pub fn any_payload() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..4096)
}
