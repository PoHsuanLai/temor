//! Generators for the small values the messages are made of.

use proptest::prelude::*;
use temor_wire::digest::ContentHash;
use temor_wire::hello::{Capability, HelperVersion, ProtocolVersion};
use temor_wire::ids::{RequestId, SessionId, TerminalId, WatchId};
use temor_wire::message::{
    ChangeKind, EntryKind, EnvVar, ExitStatus, FileMeta, FileStamp, IdleLimit, ListeningPort,
    Refusal, RemotePath, SessionState, Truncation, UncheckedWhy, WriteCheck,
};
use temor_wire::units::{
    ByteCount, ByteOffset, ByteRange, Cols, ExitCode, LineCount, LineNumber, Pid, Port, Rows,
    Seconds, Size, UnixMillis,
};

pub fn text() -> impl Strategy<Value = String> {
    "\\PC{0,24}"
}

pub fn request_id() -> impl Strategy<Value = RequestId> {
    any::<u64>().prop_map(RequestId)
}

pub fn session_id() -> impl Strategy<Value = SessionId> {
    any::<u64>().prop_map(SessionId)
}

pub fn watch_id() -> impl Strategy<Value = WatchId> {
    any::<u64>().prop_map(WatchId)
}

pub fn terminal_id() -> impl Strategy<Value = TerminalId> {
    any::<u64>().prop_map(TerminalId)
}

pub fn size() -> impl Strategy<Value = Size> {
    (any::<u16>(), any::<u16>()).prop_map(|(cols, rows)| Size {
        cols: Cols(cols),
        rows: Rows(rows),
    })
}

pub fn content_hash() -> impl Strategy<Value = ContentHash> {
    any::<[u8; 32]>().prop_map(ContentHash)
}

pub fn helper_version() -> impl Strategy<Value = HelperVersion> {
    content_hash().prop_map(HelperVersion)
}

pub fn protocol_version() -> impl Strategy<Value = ProtocolVersion> {
    any::<u16>().prop_map(ProtocolVersion)
}

pub fn capability() -> impl Strategy<Value = Capability> {
    prop_oneof![
        Just(Capability::Sessions),
        Just(Capability::Ports),
        Just(Capability::Files),
        Just(Capability::Trial),
        Just(Capability::Relay),
    ]
}

pub fn path() -> impl Strategy<Value = RemotePath> {
    "\\PC{0,40}".prop_map(RemotePath)
}

pub fn env_vars() -> impl Strategy<Value = Vec<EnvVar>> {
    prop::collection::vec(
        (text(), text()).prop_map(|(name, value)| EnvVar { name, value }),
        0..4,
    )
}

pub fn file_stamp() -> impl Strategy<Value = FileStamp> {
    (any::<i64>(), content_hash()).prop_map(|(mtime, hash)| FileStamp {
        mtime: UnixMillis(mtime),
        hash,
    })
}

pub fn write_check() -> impl Strategy<Value = WriteCheck> {
    prop_oneof![
        Just(WriteCheck::Overwrite),
        file_stamp().prop_map(WriteCheck::Unchanged),
        Just(WriteCheck::MustBeNew),
    ]
}

pub fn entry_kind() -> impl Strategy<Value = EntryKind> {
    prop_oneof![
        Just(EntryKind::File),
        Just(EntryKind::Directory),
        Just(EntryKind::Symlink),
        Just(EntryKind::Other),
    ]
}

pub fn file_meta() -> impl Strategy<Value = FileMeta> {
    (file_stamp(), any::<u64>(), entry_kind()).prop_map(|(stamp, len, kind)| FileMeta {
        stamp,
        len: ByteCount(len),
        kind,
    })
}

pub fn change_kind() -> impl Strategy<Value = ChangeKind> {
    prop_oneof![
        Just(ChangeKind::Created),
        Just(ChangeKind::Modified),
        Just(ChangeKind::Removed),
        Just(ChangeKind::Renamed),
    ]
}

pub fn byte_range() -> impl Strategy<Value = ByteRange> {
    (any::<u64>(), any::<u64>()).prop_map(|(offset, len)| ByteRange {
        offset: ByteOffset(offset),
        len: ByteCount(len),
    })
}

pub fn idle_limit() -> impl Strategy<Value = IdleLimit> {
    prop_oneof![
        Just(IdleLimit::Never),
        any::<u64>().prop_map(|secs| IdleLimit::After(Seconds(secs))),
    ]
}

pub fn session_state() -> impl Strategy<Value = SessionState> {
    prop_oneof![Just(SessionState::Attached), Just(SessionState::Detached)]
}

pub fn exit_status() -> impl Strategy<Value = ExitStatus> {
    prop_oneof![
        any::<i32>().prop_map(|code| ExitStatus::Code(ExitCode(code))),
        text().prop_map(|name| ExitStatus::Signal { name }),
    ]
}

pub fn listening_port() -> impl Strategy<Value = ListeningPort> {
    (any::<u16>(), any::<u32>(), text()).prop_map(|(port, pid, process)| ListeningPort {
        port: Port(port),
        pid: Pid(pid),
        process,
    })
}

pub fn refusal() -> impl Strategy<Value = Refusal> {
    prop_oneof![
        Just(Refusal::OutsideWorkspace),
        Just(Refusal::DeniedByHost),
        Just(Refusal::OverLimit),
    ]
}

pub fn unchecked_why() -> impl Strategy<Value = UncheckedWhy> {
    prop_oneof![
        Just(UncheckedWhy::TimedOut),
        Just(UncheckedWhy::NeededNetwork),
        Just(UncheckedWhy::NoSandbox),
    ]
}

pub fn truncation() -> impl Strategy<Value = Truncation> {
    prop_oneof![Just(Truncation::Kept), Just(Truncation::Cut)]
}

pub fn line_number() -> impl Strategy<Value = LineNumber> {
    any::<u32>().prop_map(LineNumber)
}

pub fn line_count() -> impl Strategy<Value = LineCount> {
    any::<u32>().prop_map(LineCount)
}

pub fn exit_code() -> impl Strategy<Value = ExitCode> {
    any::<i32>().prop_map(ExitCode)
}

pub fn byte_count() -> impl Strategy<Value = ByteCount> {
    any::<u64>().prop_map(ByteCount)
}
