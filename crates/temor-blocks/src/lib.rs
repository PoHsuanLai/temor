//! temor's rules for a shell session and its blocks (sans I/O): the shell's state and marks, the blocks it ran, the notices it raises and the command offer.

pub mod block;
pub mod key;
pub mod notify;
pub mod offer;
pub mod session;
