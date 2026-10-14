//! The crate's one error: what can go wrong at the app's edges.

use thiserror::Error;

/// Why a pane, the settings file or the window could not do what was asked.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UiError {
    /// The fonts or the renderer refused.
    #[error("drawing the terminal failed: {0}")]
    Render(#[from] temor_render::RenderError),
    /// The pane engine could not start or draw (see `temor-pane`).
    #[error(transparent)]
    Pane(#[from] temor_pane::PaneError),
    /// The program could not be started on a terminal, or the terminal failed.
    #[error("the shell could not start: {0}")]
    Pty(#[from] temor_pty::PtyError),
    /// The settings file could not be read.
    #[error("the settings file could not be read: {0}")]
    SettingsRead(#[source] std::io::Error),
    /// The settings could not be written.
    #[error("the settings could not be saved: {0}")]
    SettingsWrite(#[source] ds_settings::SettingsError),
    /// This program's own path is not known, so the shell cannot be started through it.
    #[error("the shell could not be started: this program's location is unknown ({0})")]
    Launcher(#[source] std::io::Error),
    /// The folder for shell integration is missing, or is not private to this user.
    #[error("the folder for shell setup is not private to you: {0}")]
    RuntimeDir(#[source] std::io::Error),
    /// The pane's background work could not start.
    #[error("the terminal could not start its background work: {0}")]
    Thread(#[source] std::io::Error),
    /// The session file could not be read or written.
    #[error("the session file could not be saved: {0}")]
    Session(#[source] std::io::Error),
    /// A Space's key agent could not be started.
    #[error("the Space's key agent could not start: {0}")]
    Agent(String),
    /// The output could not be saved to the downloads shelf.
    #[error("the output could not be saved: {0}")]
    Save(String),
    /// git could not do what an agent worktree needed.
    #[error(transparent)]
    Git(#[from] temor_core::error::CoreError),
    /// A file of an agent worktree could not be copied, written back or removed.
    #[error("{path}: {source}")]
    AgentFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
