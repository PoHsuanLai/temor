//! Why a request to a plugin got no answer.

/// Why a plugin could not answer. Each variant names the plugin; none of them is something the
/// person has to act on at once, so the app logs it and shows nothing for that plugin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// The plugin could not be started, was silent too long, crashed, or spoke out of turn.
    #[error("the plugin {plugin} gave no answer: {reason}")]
    Failed {
        /// The plugin's id.
        plugin: String,
        /// What happened, in bayonet's words.
        reason: String,
    },
    /// The request was cancelled (a newer keystroke, the person closed the bar).
    #[error("the plugin {plugin} was stopped")]
    Cancelled {
        /// The plugin's id.
        plugin: String,
    },
    /// The plugin said the request failed.
    #[error("the plugin {plugin} said: {message}")]
    Said {
        /// The plugin's id.
        plugin: String,
        /// Its words.
        message: String,
    },
    /// The plugin answered with something that is not an answer to the request.
    #[error("the plugin {plugin} sent something that does not answer: {reason}")]
    Broke {
        /// The plugin's id.
        plugin: String,
        /// What was wrong.
        reason: String,
    },
    /// Plugins cannot run on this system yet.
    #[error("plugins cannot run on this system")]
    Unsupported,
}

impl PluginError {
    /// Whether this is a cancel, which is not worth a log line, or real trouble.
    pub fn kind(&self) -> CancelKind {
        match self {
            PluginError::Cancelled { .. } => CancelKind::Cancelled,
            PluginError::Failed { .. }
            | PluginError::Said { .. }
            | PluginError::Broke { .. }
            | PluginError::Unsupported => CancelKind::Trouble,
        }
    }
}

/// Whether an error is a cancel or a real trouble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelKind {
    /// The request was cancelled on purpose.
    Cancelled,
    /// Something went wrong with the plugin.
    Trouble,
}
