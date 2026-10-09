//! The messages between temor and temor-remote: requests and their replies or errors, plus the
//! events the helper sends unasked.
//!
//! Bulk bytes ride in the frame payload, not the JSON: the input of [`Request::SendInput`], the
//! output of [`Event::Output`], the snapshot of [`Reply::Attached`], the bytes of
//! [`Reply::Read`] and [`Request::WriteFile`]. Anything else is JSON.

mod envelope;
mod event;
mod files;
mod ports;
mod relay;
mod remote_error;
mod reply;
mod request;
mod session;
mod trial;

pub use envelope::Message;
pub use event::Event;
pub use files::{ChangeKind, DirEntry, EntryKind, FileMeta, FileStamp, RemotePath, WriteCheck};
pub use ports::ListeningPort;
pub use relay::{RelayCall, RelayReply, Truncation};
pub use remote_error::{Refusal, RemoteError};
pub use reply::Reply;
pub use request::Request;
pub use session::{EnvVar, ExitStatus, IdleLimit, OpenSession, SessionInfo, SessionState};
pub use trial::{TrialRequest, UncheckedWhy, Verdict};
