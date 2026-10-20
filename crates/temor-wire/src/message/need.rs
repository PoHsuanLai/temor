//! Which capability a request needs: the one table both ends read, so the client's refusal to
//! ask and the helper's refusal to answer cannot disagree.

use super::Request;
use crate::hello::Capability;

impl Request {
    /// The capability the helper must have agreed for this request, or none for a request every
    /// helper answers.
    pub fn capability(&self) -> Option<Capability> {
        match self {
            Self::OpenSession(_)
            | Self::AttachSession { .. }
            | Self::DetachSession { .. }
            | Self::Close { .. }
            | Self::ResizeSession { .. }
            | Self::SendInput { .. }
            | Self::SetPalette { .. }
            | Self::ListSessions
            | Self::SetIdleLimit { .. } => Some(Capability::Sessions),
            Self::Ping => None,
            Self::ListPorts => Some(Capability::Ports),
            Self::ListDir { .. }
            | Self::Stat { .. }
            | Self::ReadFile { .. }
            | Self::WriteFile { .. }
            | Self::Watch { .. }
            | Self::Unwatch { .. } => Some(Capability::Files),
            Self::Facts { .. } => Some(Capability::Facts),
            Self::RunTrial(_) => Some(Capability::Trial),
            Self::Relay { .. } => Some(Capability::Relay),
        }
    }
}
