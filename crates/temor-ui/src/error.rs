//! The crate's one error: what can go wrong at the app's edges.

use thiserror::Error;

/// Why a pane, the settings file or the window could not do what was asked.
#[derive(Debug, Error)]
pub enum UiError {
    /// The terminal emulator refused (a libghostty call failed, or it could not start).
    #[error("the terminal could not start: {0}")]
    Terminal(#[from] temor_vt::VtError),
    /// The fonts or the renderer refused.
    #[error("drawing the terminal failed: {0}")]
    Render(#[from] temor_render::RenderError),
    /// The program could not be started on a terminal, or the terminal failed.
    #[error("the shell could not start: {0}")]
    Pty(#[from] temor_pty::PtyError),
    /// The settings file could not be read.
    #[error("the settings file could not be read: {0}")]
    SettingsRead(#[source] std::io::Error),
    /// This program's own path is not known, so the shell cannot be started through it.
    #[error("the shell's launcher could not be found: {0}")]
    Launcher(#[source] std::io::Error),
    /// The pane's I/O thread could not start.
    #[error("the pane could not start its thread: {0}")]
    Thread(#[source] std::io::Error),
}
