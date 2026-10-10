//! What differs by operating system (PLAN §0): the opener. Linux is written; other systems get
//! a stub that reports the open as unsupported (`TODO(platform:<os>)`).

mod files;
mod open_file;
mod opener;
mod private_dir;

pub use files::{FileShell, default_file_shell};
pub use open_file::{FileOpener, default_file_opener};
pub use opener::{OpenError, Opener, default_opener};
pub use private_dir::{ensure_private_dir, write_private_file};
