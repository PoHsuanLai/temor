//! What libghostty's callbacks become: one typed event enum, sent to a caller-supplied sink.

mod model;
mod sink;
mod status;

pub use model::{
    ClipboardAnswer, ClipboardItem, ClipboardLocation, ClipboardRead, ClipboardWrite, ExitCode,
    ExitStatus, Notification, Percent, Progress, ProgressState, PromptEvent, PromptKind,
    RenderHold, UnknownKind, VtEvent,
};
pub use sink::{ClipboardSource, EventSink, ReplySink};
pub use status::{ProgramState, ProgramStatus, ProgramWait};
