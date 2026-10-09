//! The states, inputs and outputs of one remote edit.

use temor_ssh::RemoteStat;

/// How a call to the host failed, as the person can act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EditFault {
    /// The connection to the host is down; reconnect and save again.
    NoConnection,
    /// The host refused: no permission, or no room.
    Refused,
    /// The file is no longer on the host.
    Gone,
}

/// Whether more saves arrived while one was being written back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Queued {
    Nothing,
    /// Save again once this one is done.
    Resave,
}

/// What the host's file turned out to be when it was checked before a write-back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Found {
    /// Changed on the host since it was fetched.
    Changed(RemoteStat),
    /// Removed from the host.
    Deleted,
}

/// The person's choice about a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    /// Write the local copy over the host's file.
    Overwrite,
    /// Throw the local edits away and fetch the host's file again.
    TakeRemote,
    /// Decide later: the next save checks again.
    Later,
}

/// One remote file open for editing. `base` is the host's file as last fetched or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edit {
    /// Fetching the file.
    Opening,
    /// The local copy is open; nothing waiting.
    Open { base: RemoteStat },
    /// A save was seen; asking the host whether the file changed.
    Checking { base: RemoteStat, queued: Queued },
    /// The file is unchanged on the host; writing it back.
    Uploading { base: RemoteStat, queued: Queued },
    /// The file changed on the host (or was removed); waiting for the person to choose.
    Conflict { base: RemoteStat, found: Found },
    /// A call failed. A save or a retry tries again from `base`.
    Stalled { base: RemoteStat },
    /// The file could not be opened.
    Broken(EditFault),
    /// Closed; nothing more happens.
    Closed,
}

/// What moves an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditIn {
    /// The fetch finished. `stat` is read before the content was copied, so a change in
    /// between shows up as a conflict instead of being lost.
    Fetched { stat: RemoteStat },
    /// The file is not on the host.
    FetchGone,
    /// The local copy was saved (from `watch_step`).
    LocalSaved,
    /// The host's file, as just checked.
    Checked(temor_ssh::RemoteState),
    /// The write-back finished; `stat` is the file as it now is on the host.
    Stored { stat: RemoteStat },
    /// The person chose.
    Resolved(Resolution),
    /// A call to the host failed.
    Failed(EditFault),
    /// Try again after a failure.
    Retry,
    /// The person closed the editor.
    Closed,
}

/// What an edit asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditOut {
    /// Fetch the file (stat, then content) to the local copy, then send `Fetched` or
    /// `FetchGone`. After writing the copy, restart the watch with its new stamp, so the
    /// fetch is not mistaken for a save.
    Fetch,
    /// The local copy is ready: open it in the editor or viewer.
    OpenLocal,
    /// Read the host file's modified time and size, then send `Checked`.
    Check,
    /// Copy the local file over the host's, read its new stat, then send `Stored`.
    Store,
    /// Ask the person what to do about this conflict.
    AskConflict(Found),
    /// The host has the latest save: say so.
    Saved,
    /// Tell the person a call failed.
    Report(EditFault),
    /// Stop watching the local copy.
    StopWatching,
}
