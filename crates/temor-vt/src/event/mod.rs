//! What libghostty's callbacks become: one typed event enum, sent to a caller-supplied sink.

mod model;
mod sink;
mod status;

pub use model::{
    ClipboardGrant, ClipboardItem, ClipboardLocation, ClipboardWrite, ExitCode, ExitStatus,
    Notification, Percent, Progress, ProgressState, PromptEvent, PromptKind, RenderHold,
    Truncation, UnknownKind, VtEvent,
};
pub use sink::{EventSink, ReplySink};
pub use status::{ProgramState, ProgramStatus, ProgramWait};
