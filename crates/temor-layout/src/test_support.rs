//! Fixtures the unit tests across the core share.

use crate::workspace::{ClosedState, KeepClosed, Workspace};

/// A workspace with nothing open, closed-tab history at its default.
pub(crate) fn fresh_workspace() -> Workspace {
    Workspace::new(ClosedState::new(KeepClosed::DEFAULT))
}
