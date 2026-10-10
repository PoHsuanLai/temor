//! The messages between temor and temor-remote: requests and their replies or errors, plus the
//! events the helper sends unasked.
//!
//! Bulk bytes ride in the frame payload, not the JSON: the input of [`Request::SendInput`], the
//! output of [`Event::Output`], the snapshot of [`Reply::Attached`], the bytes of
//! [`Reply::Read`] and [`Request::WriteFile`]. Anything else is JSON.

mod envelope;
mod event;
mod files;
mod incoming;
mod kinds;
mod palette;
mod ports;
mod relay;
mod remote_error;
mod reply;
mod request;
mod session;
mod trial;

pub use envelope::Message;
pub use event::Event;
pub use files::{
    DirEntry, EntryKind, FileMeta, FileStamp, PartEnd, RemotePath, WatchChange, WriteCheck,
    WriteMode,
};
pub use incoming::{Frame, Incoming};
pub use palette::{Palette, Rgb};
pub use ports::ListeningPort;
pub use relay::{RelayCall, RelayReply, Truncation};
pub use remote_error::{Refusal, RemoteError};
pub use reply::Reply;
pub use request::Request;
pub use session::{EnvVar, ExitStatus, IdleLimit, OpenSession, SessionInfo, SessionState};
pub use trial::{
    ChangeKind, ErrorLine, FileChange, FileChanges, RelPath, TrialRequest, TrialVerdict,
    UncheckedWhy,
};

#[cfg(test)]
mod tests;
