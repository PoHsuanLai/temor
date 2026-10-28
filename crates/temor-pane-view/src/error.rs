//! What can go wrong showing a pane.

use thiserror::Error;

/// Why a pane could not open, draw or do what was asked of it.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ViewError {
    /// The fonts or the renderer refused.
    #[error("drawing the terminal failed: {0}")]
    Render(#[from] temor_panes::render::RenderError),
    /// The pane engine could not start or draw (see `temor-pane`).
    #[error(transparent)]
    Pane(#[from] temor_panes::pane::PaneError),
    /// The program could not be started on a terminal, or the terminal failed.
    #[error("the shell could not start: {0}")]
    Pty(#[from] temor_panes::pty::PtyError),
    /// The output could not be saved to the downloads shelf.
    #[error("the output could not be saved: {0}")]
    Save(String),
    /// The pane could not open (see `temor-panes`).
    #[error(transparent)]
    Open(#[from] temor_panes::OpenError),
    /// One of temor's own rules refused.
    #[error(transparent)]
    Core(#[from] temor_client::model::error::CoreError),
}
