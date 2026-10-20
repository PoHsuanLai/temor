//! temor's file transfers (PLAN §9): decoding what a remote program sends, the receive machine
//! for kitty's file transfer protocol, and the downloads shelf.
//!
//! Three parts, none of them doing I/O on their own:
//! - [`osc1337`]: iTerm2's `File=` and multipart files, decoded into [`ReceivedFile`]s. The app
//!   hands over the payloads temor-vt pre-scans; limits are injected.
//! - [`kitty`]: the OSC 5113 wire format and the pure [`ReceiveSession`] step machine. It asks for
//!   consent through an effect and takes the answer as input. The writer, [`apply_write`], runs
//!   the file-system effects under a destination root and refuses symlinks.
//! - [`shelf`]: the downloads shelf model and [`ShelfStore`], which keeps an atomic JSON index and
//!   names files without collisions. The folder and the clock come from the caller.
//!
//! Remote names and paths are untrusted. [`FileName::sanitise`] always yields a usable name, and
//! [`RelPath::parse`] refuses anything that could leave the destination.
//!
//! # Example
//!
//! Remote names are untrusted: a name always becomes a safe file name, and a path that
//! could leave the folder is refused.
//!
//! ```
//! use temor_transfer::{FileName, RelPath};
//!
//! let name = FileName::sanitise("../../etc/passwd");
//! assert!(!name.as_str().contains('/'));
//! assert!(!name.as_str().starts_with('.'));
//! assert!(RelPath::parse("../escape.txt").is_err());
//! assert!(RelPath::parse("reports/q3.pdf").is_ok());
//! ```

mod base64;
mod consent;
mod error;
mod fields;
pub mod kitty;
mod name;
mod origin;
pub mod osc1337;
mod platform;
pub mod shelf;
mod units;

pub use consent::Consent;
pub use error::{Malformed, TransferError};
pub use name::{FileName, RelPath};
pub use origin::{BlockKey, HostName, Origin, PaneKey, TabKey};
pub use platform::Root;
pub use units::{Bytes, Count, Limits};
