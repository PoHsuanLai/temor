//! What the pane shows of its program beside the list (PLAN §9 blocks): the progress it reports
//! and its bell.

use super::status::{ProgressLine, progress_line};
use crate::session::Session;

/// The pane's progress and bell, read from the session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chrome {
    pub progress: Option<ProgressLine>,
    pub bell: bool,
}

/// The chrome for a session.
pub fn chrome_of(session: &Session) -> Chrome {
    Chrome {
        progress: session.progress().and_then(progress_line),
        bell: session.bell_showing(),
    }
}
