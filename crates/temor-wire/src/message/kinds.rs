//! The names of the messages this build knows, one list per closed set.
//!
//! A frame is classified by its tag against these lists, never by a parser's error text. The
//! unit tests in `message/tests.rs` hold each list to the enum it names, so a variant cannot be
//! added without its name appearing here.

/// The top-level message kinds.
pub(crate) const MESSAGE_KINDS: &[&str] = &["Hello", "Request", "Response", "Event"];

/// The request kinds (the variants of `Request`).
pub(crate) const REQUEST_KINDS: &[&str] = &[
    "OpenSession",
    "AttachSession",
    "DetachSession",
    "Close",
    "ResizeSession",
    "SendInput",
    "ListSessions",
    "SetIdleLimit",
    "ListPorts",
    "ListDir",
    "Stat",
    "ReadFile",
    "WriteFile",
    "Watch",
    "Unwatch",
    "RunTrial",
    "Relay",
];

/// The event kinds (the variants of `Event`).
pub(crate) const EVENT_KINDS: &[&str] = &["Output", "Exited", "Ports", "FileChanged"];

/// The reply kinds (the variants of `Reply`).
pub(crate) const REPLY_KINDS: &[&str] = &[
    "Ack", "Opened", "Attached", "Sessions", "Ports", "Listing", "Stat", "Read", "Written",
    "Watching", "Verdict", "Relayed",
];

/// The error kinds (the variants of `RemoteError`).
pub(crate) const REMOTE_ERROR_KINDS: &[&str] = &[
    "UnknownRequest",
    "NoSuchSession",
    "NoSuchWatch",
    "NoSuchTerminal",
    "NotFound",
    "PermissionDenied",
    "Conflict",
    "HostRefused",
    "Unsupported",
    "TrialUnavailable",
    "Unreadable",
    "BadRequest",
    "Io",
];
