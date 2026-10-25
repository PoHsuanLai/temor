//! Everything the window keeps about agents: the radar and the pane sockets. Lives in the hub, on
//! the window's thread.

use temor_core::agent::radar::{Millis, RadarParams};
use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;
use tokio::sync::mpsc::UnboundedSender;

use super::radar::Radar;
use crate::edge::{Clock, SocketHost, SystemClock};
use crate::view::launch::Launch;

/// Whether a socket problem was already told to the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Told {
    Not,
    Once,
}

/// The window's agent state.
pub struct Agents {
    pub(super) clock: Box<dyn Clock>,
    pub radar: Radar,
    pub(super) sockets: Option<SocketHost>,
    pub(super) socket_told: Told,
}

impl std::fmt::Debug for Agents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agents").finish_non_exhaustive()
    }
}

impl Agents {
    /// The agent state for a window started from `launch`. Pane sockets send their reports to
    /// `events`.
    pub fn new(launch: &Launch, events: UnboundedSender<(PaneId, PaneEvent)>) -> Agents {
        Agents {
            clock: Box::new(SystemClock::new()),
            radar: Radar::new(RadarParams::default()),
            sockets: SocketHost::start(launch.ambient.runtime_dir.clone(), events).ok(),
            socket_told: Told::Not,
        }
    }

    /// The window's clock, for the radar's timing.
    pub fn now(&self) -> Millis {
        Millis(self.clock.now().0)
    }
}
