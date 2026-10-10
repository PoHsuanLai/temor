//! Everything the window keeps about agents: the radar, the checkpoints of each run, the agent
//! worktrees, the pane sockets and the git jobs. Lives in the hub, on the window's thread.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use temor_core::agent::radar::{Millis, RadarParams};
use temor_core::checkpoint::RestorePlan;
use temor_core::vocab::ids::{BlockId, CheckpointRef, PaneId, TabId};
use temor_core::worktree::Git;
use temor_pane::PaneEvent;
use tokio::sync::mpsc::UnboundedSender;

use super::jobs::Jobs;
use super::radar::Radar;
use super::runs::Run;
use super::trees::Trees;
use crate::edge::git::ProcessGit;
use crate::edge::{Clock, SocketHost, SystemClock};
use crate::view::launch::Launch;

/// The Space agent worktrees are filed under until the window has Spaces.
pub(super) const SPACE: &str = "main";

/// What a sheet over the window is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum Sheet {
    /// What putting the files back to a checkpoint would change, before it is done.
    Restore {
        pane: PaneId,
        block: BlockId,
        at: CheckpointRef,
        root: String,
        plan: RestorePlan,
    },
    /// An agent worktree's overlap findings and the way to merge it back.
    Details(TabId),
    /// The agent worktrees no tab uses, in this repository.
    Leftovers { repo: String },
}

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
    pub(super) runs: BTreeMap<PaneId, Run>,
    pub(super) trees: Trees,
    pub(super) jobs: Jobs,
    pub(super) sockets: Option<SocketHost>,
    pub(super) socket_told: Told,
    pub(super) sheet: Option<Sheet>,
    pub(super) data_dir: PathBuf,
}

impl std::fmt::Debug for Agents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agents").finish_non_exhaustive()
    }
}

impl Agents {
    /// The agent state for a window started from `launch`. Pane sockets send their reports to
    /// `events`. git is looked for on the environment's `PATH`.
    pub fn new(launch: &Launch, events: UnboundedSender<(PaneId, PaneEvent)>) -> Agents {
        let path = launch
            .ambient
            .env
            .get("PATH")
            .map_or_else(|| "/usr/bin:/bin".into(), std::ffi::OsStr::to_os_string);
        let git = temor_pty::find_program("git", &path)
            .map(|binary| Arc::new(ProcessGit::new(binary)) as Arc<dyn Git>);
        Agents {
            clock: Box::new(SystemClock::new()),
            radar: Radar::new(RadarParams::default()),
            runs: BTreeMap::new(),
            trees: Trees::default(),
            jobs: Jobs::new(git),
            sockets: SocketHost::start(launch.ambient.runtime_dir.clone(), events).ok(),
            socket_told: Told::Not,
            sheet: None,
            data_dir: launch.ambient.data_dir(),
        }
    }

    /// The window's clock, for the radar's timing.
    pub fn now(&self) -> Millis {
        Millis(self.clock.now().0)
    }
}
