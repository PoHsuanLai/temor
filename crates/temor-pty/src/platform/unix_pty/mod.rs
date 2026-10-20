//! The Unix pseudo-terminal (Linux and macOS), through rustix and without `unsafe`.

mod foreground;
mod io;
mod leader;
mod process;
mod spawn;

pub use io::PtyIo;
pub use leader::run_session_leader;
pub use spawn::Pty;
