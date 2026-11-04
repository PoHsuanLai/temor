//! The openers a view is handed (PLAN §0), over the desktop's own (`SystemOpen`).

mod files;
mod opener;

pub use files::{FileShell, default_file_shell};
pub use opener::{OpenError, Opener, default_opener, open_link};
pub use temor_client::keep::private::{ensure_private_dir, write_private_file};
