//! Generators for the small values the messages are made of.

use proptest::prelude::*;
use temor_wire::digest::ContentHash;
use temor_wire::hello::{Capability, HelperVersion, ProtocolVersion};
use temor_wire::ids::{RequestId, SessionId, TerminalId, WatchId};
use temor_wire::message::{
    ChangeKind, EntryKind, EnvVar, ErrorLine, ExitStatus, FileChange, FileChanges, FileMeta,
    FileStamp, IdleLimit, ListeningPort, Palette, PartEnd, Refusal, RelPath, RemotePath, Rgb,
    SessionState, Truncation, UncheckedWhy, WatchChange, WriteCheck, WriteMode,
};
use temor_wire::units::{
    ByteCount, ByteOffset, ByteRange, CellPx, Cols, ExitCode, LineCount, LineNumber, Pid, Pixels,
    Port, Rows, Seconds, Size, UnixMillis,
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
        Just(Capability::Facts),
        Just(Capability::Unknown),
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
        Just(EntryKind::Unknown),
    ]
}

pub fn file_meta() -> impl Strategy<Value = FileMeta> {
    (file_stamp(), any::<u64>(), entry_kind()).prop_map(|(stamp, len, kind)| FileMeta {
        stamp,
        len: ByteCount(len),
        kind,
    })
}

pub fn watch_change() -> impl Strategy<Value = WatchChange> {
    prop_oneof![
        Just(WatchChange::Created),
        Just(WatchChange::Modified),
        Just(WatchChange::Removed),
        Just(WatchChange::Renamed),
        Just(WatchChange::Unknown),
    ]
}

pub fn trial_change_kind() -> impl Strategy<Value = ChangeKind> {
    prop_oneof![
        Just(ChangeKind::Created),
        Just(ChangeKind::Modified),
        Just(ChangeKind::Removed),
        Just(ChangeKind::Unknown),
    ]
}

pub fn file_changes() -> impl Strategy<Value = FileChanges> {
    prop::collection::vec(
        (text(), trial_change_kind()).prop_map(|(path, how)| FileChange {
            path: RelPath(path),
            how,
        }),
        0..4,
    )
    .prop_map(|files| FileChanges { files })
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
    prop_oneof![
        Just(SessionState::Attached),
        Just(SessionState::Detached),
        Just(SessionState::Unknown),
    ]
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
        Just(Refusal::Unknown),
    ]
}

pub fn unchecked_why() -> impl Strategy<Value = UncheckedWhy> {
    prop_oneof![
        Just(UncheckedWhy::TimedOut),
        Just(UncheckedWhy::NeededNetwork),
        Just(UncheckedWhy::NoSandbox),
        Just(UncheckedWhy::Unknown),
    ]
}

pub fn error_line() -> impl Strategy<Value = ErrorLine> {
    text().prop_map(ErrorLine)
}

pub fn truncation() -> impl Strategy<Value = Truncation> {
    prop_oneof![
        Just(Truncation::Kept),
        Just(Truncation::Cut),
        Just(Truncation::Unknown),
    ]
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

pub fn cell_px() -> impl Strategy<Value = CellPx> {
    (any::<u16>(), any::<u16>()).prop_map(|(width, height)| CellPx {
        width: Pixels(width),
        height: Pixels(height),
    })
}

pub fn rgb() -> impl Strategy<Value = Rgb> {
    any::<[u8; 3]>().prop_map(|[r, g, b]| Rgb { r, g, b })
}

pub fn palette() -> impl Strategy<Value = Palette> {
    (prop::array::uniform16(rgb()), rgb(), rgb(), rgb()).prop_map(
        |(ansi, foreground, background, cursor)| Palette {
            ansi,
            foreground,
            background,
            cursor,
        },
    )
}

pub fn write_mode() -> impl Strategy<Value = WriteMode> {
    prop_oneof![
        Just(WriteMode::Whole),
        (
            any::<u64>(),
            prop_oneof![
                Just(PartEnd::More),
                Just(PartEnd::Last),
                Just(PartEnd::Abort)
            ]
        )
            .prop_map(|(offset, end)| WriteMode::Part {
                offset: ByteOffset(offset),
                end,
            }),
    ]
}
