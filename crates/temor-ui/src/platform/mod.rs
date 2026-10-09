//! What differs by operating system (PLAN §0): the opener. Linux is written; other systems get
//! a stub that reports the open as unsupported (`TODO(platform:<os>)`).

mod opener;

pub use opener::{OpenError, Opener, default_opener};
