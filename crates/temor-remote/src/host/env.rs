//! What the helper reads from its surroundings, gathered once at the edge and passed in.

use std::ffi::OsString;
use std::path::PathBuf;

/// The process's surroundings. The binary builds it from the real environment; tests build it by
/// hand.
#[derive(Debug, Clone)]
pub struct HostEnv {
    /// `XDG_RUNTIME_DIR`.
    pub xdg_runtime_dir: Option<OsString>,
    /// `HOME`.
    pub home: Option<OsString>,
    /// `SHELL`.
    pub shell: Option<OsString>,
    /// This binary.
    pub exe: PathBuf,
}
