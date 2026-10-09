//! The per-pane status socket (PLAN §8.1, source 2): each pane gets an id and a private Unix
//! socket where an agent, a hook or the `temor` command reports what it is doing. The protocol
//! is a few text lines with a version; the socket is a library with a caller-driven `pump`.

mod protocol;
mod server;
#[cfg(test)]
mod tests;

pub use protocol::{
    MAX_LINE, ReportRefusal, ReportState, ReportWait, StatusLine, VERSION, encode_hello,
    encode_status, parse_reply,
};
pub use server::{PaneKey, PaneSocket, SocketLimits};
