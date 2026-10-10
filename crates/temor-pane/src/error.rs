//! The crate's errors: what a settings line can get wrong, and what a pane can
//! fail to start with.

use thiserror::Error;

/// What a pane can fail to do: start its terminal, its drawing, or its program.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PaneError {
    /// The terminal emulator refused (a libghostty call failed, or it could not start).
    #[cfg(feature = "ghostty")]
    #[error("the terminal could not start: {0}")]
    Terminal(#[from] temor_vt::VtError),
    /// The fonts or the renderer refused.
    #[cfg(feature = "ghostty")]
    #[error("drawing the terminal failed: {0}")]
    Render(#[from] temor_render::RenderError),
    /// The program could not be started on a terminal, or the terminal failed.
    #[error("the shell could not start: {0}")]
    Pty(#[from] temor_pty::PtyError),
    /// A pane id was not 1 to 32 letters, digits, `-` or `_`; the caller allocates a valid one.
    #[error("not a usable pane id: {0:?}")]
    BadPaneId(String),
    /// The pane's status socket could not be made in its directory.
    #[error("the status socket at {path} failed: {source}")]
    Socket {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// The socket's path is longer than a Unix socket address can hold; use a shorter runtime
    /// directory.
    #[error("the status socket path is too long: {0}")]
    SocketPathTooLong(std::path::PathBuf),
}

/// Why outside settings text was refused. Each variant names the fix a person can make.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// A key line was refused or could not apply, in words for the person ("Ctrl+Tab can't be
    /// used: the desktop switches windows with it"). The text comes from the keymap.
    #[error("{0}")]
    Key(String),
    /// A settings line names no setting this version knows.
    #[error("no setting called {0:?}")]
    UnknownSetting(String),
    /// A settings line has a value its setting cannot take.
    #[error("{key} cannot be {value:?}")]
    BadSettingValue { key: String, value: String },
}
