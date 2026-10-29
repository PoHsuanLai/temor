//! What differs by operating system (PLAN §0): the opener. Linux is written; other systems get
//! a stub that reports the open as unsupported (`TODO(platform:<os>)`).

mod files;
mod opener;

pub use files::{FileShell, default_file_shell};
pub use opener::{OpenError, Opener, default_opener, open_link};
pub use temor_client::keep::private::{ensure_private_dir, write_private_file};
