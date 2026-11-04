//! The crate's one error type.

/// What can go wrong reaching the desktop's services. A missing service is not an error the
/// person sees: the window treats [`DesktopError::NoBus`] and [`DesktopError::Unsupported`] as
/// "this desktop does not offer it".
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DesktopError {
    /// There is no session bus to talk to.
    #[error("there is no session bus to talk to")]
    NoBus,
    /// A call on the bus failed.
    #[error("could not {call}: {reason}")]
    Bus {
        /// What temor was doing.
        call: &'static str,
        /// What the bus said.
        reason: String,
    },
    /// The shipped actions file is not one the agent layer accepts. A build with a broken file
    /// never ships, so this is a packaging mistake.
    #[error("temor's actions file is not valid: {0}")]
    Manifest(String),
    /// This system or this build has no such service.
    #[error("this desktop has no {what}")]
    Unsupported {
        /// The service.
        what: &'static str,
    },
}

impl DesktopError {
    /// A bus failure while doing `call`.
    pub fn bus(call: &'static str, reason: impl std::fmt::Display) -> Self {
        DesktopError::Bus {
            call,
            reason: reason.to_string(),
        }
    }
}
