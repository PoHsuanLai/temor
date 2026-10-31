//! The crate's one error: what can go wrong at the app's edges.

use thiserror::Error;

/// Why a pane, the settings file or the window could not do what was asked.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UiError {
    /// The fonts or the renderer refused.
    #[error("drawing the terminal failed: {0}")]
    Render(#[from] temor_panes::render::RenderError),
    /// The pane engine could not start or draw (see `temor-pane`).
    #[error(transparent)]
    Pane(#[from] temor_panes::pane::PaneError),
    /// The settings file could not be read.
    #[error("the settings file could not be read: {0}")]
    SettingsRead(#[source] std::io::Error),
    /// The settings could not be written.
    #[error("the settings could not be saved: {0}")]
    SettingsWrite(#[source] ds_settings::SettingsError),
    /// A change made in the Settings window cannot be kept: what the setting would hold is not
    /// something it can, said in words for the person.
    #[error("{0}")]
    SettingsRefused(String),
    /// This program's own path is not known, so the shell cannot be started through it.
    #[error("the shell could not be started: this program's location is unknown ({0})")]
    Launcher(#[source] std::io::Error),
    /// The folder for shell integration is missing, or is not private to this user.
    #[error("the folder for shell setup is not private to you: {0}")]
    RuntimeDir(#[source] std::io::Error),
    /// One of temor's own rules refused.
    #[error(transparent)]
    Core(#[from] temor_client::model::error::CoreError),
    /// A pane could not open, draw or do what was asked of it.
    #[error(transparent)]
    View(#[from] temor_pane_view::error::ViewError),
}

impl From<temor_panes::start::StartError> for UiError {
    fn from(error: temor_panes::start::StartError) -> Self {
        match error {
            temor_panes::start::StartError::RuntimeDir(io) => UiError::RuntimeDir(io),
            temor_panes::start::StartError::Pane(pane) => UiError::Pane(pane),
            other => UiError::RuntimeDir(std::io::Error::other(other.to_string())),
        }
    }
}
