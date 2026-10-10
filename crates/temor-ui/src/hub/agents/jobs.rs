//! Running git on worker threads: a job is a closure over the git runner that returns what
//! happened as a [`Done`]; the window's loop receives them and carries on (PLAN §4b: workers do
//! this, never the render path). The hub never waits for one.

use std::sync::Arc;

use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::worktree::overlap::{BaseKey, BaseProbe, PairKey, PairProbe, ProbeJob};
use temor_core::worktree::{Git, ListedWorktree, MergeOutcome, Oid, Snapshot};
use temor_desktop::{DesktopError, PointId, PointList, PointPlan, SessionKey, TurnNo};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::work::Made;
use crate::error::UiError;

/// What a finished job reports.
#[derive(Debug)]
pub enum Done {
    /// A pane's restore points were read.
    PointsListed {
        pane: PaneId,
        result: Result<PointList, DesktopError>,
    },
    /// What restoring one point would change.
    PointPlanned {
        pane: PaneId,
        session: SessionKey,
        id: PointId,
        result: Result<PointPlan, DesktopError>,
    },
    /// The files were put back (or could not be).
    PointRestored {
        pane: PaneId,
        turn: Option<TurnNo>,
        result: Result<(), DesktopError>,
    },
    /// Old snapshot refs were swept from a repository; how many were deleted.
    Swept { removed: usize },
    /// A worktree was made for a pane's repository.
    Made { result: Result<Made, UiError> },
    /// A worktree was snapshotted for the overlap check.
    Snapshotted {
        tab: TabId,
        result: Result<(Oid, Snapshot), UiError>,
    },
    /// An overlap comparison ran.
    Pair(PairKey, Result<PairProbe, UiError>, ProbeJob),
    /// An overlap comparison against the base ran.
    Base(BaseKey, Result<BaseProbe, UiError>, ProbeJob),
    /// A worktree was merged back.
    Merged {
        tab: TabId,
        result: Result<MergeOutcome, UiError>,
    },
    /// A worktree and its branch were removed.
    Removed {
        path: String,
        result: Result<(), UiError>,
    },
    /// The leftover agent worktrees of a repository.
    Listed {
        repo: String,
        result: Result<Vec<ListedWorktree>, UiError>,
    },
}

/// The git runner, if git was found, and the way back to the window.
pub struct Jobs {
    git: Option<Arc<dyn Git>>,
    done: UnboundedSender<Done>,
    inbox: Option<UnboundedReceiver<Done>>,
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs")
            .field("git", &self.git.is_some())
            .finish_non_exhaustive()
    }
}

/// Said when a job needs git and there is none.
pub const NO_GIT: &str = "git is not installed, so this is not available";

impl Jobs {
    /// Jobs over `git` (none when git was not found on `PATH`).
    pub fn new(git: Option<Arc<dyn Git>>) -> Jobs {
        let (done, inbox) = unbounded_channel();
        Jobs {
            git,
            done,
            inbox: Some(inbox),
        }
    }

    /// The window's loop takes the receiving end once.
    pub fn take_inbox(&mut self) -> Option<UnboundedReceiver<Done>> {
        self.inbox.take()
    }

    /// Whether jobs can run.
    pub fn available(&self) -> Result<(), &'static str> {
        match self.git {
            Some(_) => Ok(()),
            None => Err(NO_GIT),
        }
    }

    /// Runs `work` on a thread of its own and sends what it returns to the window. Nothing runs
    /// without git.
    pub fn run(&self, work: impl FnOnce(&dyn Git) -> Done + Send + 'static) {
        let Some(git) = self.git.clone() else {
            return;
        };
        self.spawn(move || work(git.as_ref()));
    }

    /// Runs `work` on a thread of its own and sends what it returns to the window. For jobs that
    /// need no git (restore points are docket's).
    pub fn spawn(&self, work: impl FnOnce() -> Done + Send + 'static) {
        let done = self.done.clone();
        // If the thread cannot start the job never reports; the next trigger asks again.
        let _ = std::thread::Builder::new()
            .name("temor-agent-job".to_owned())
            .spawn(move || {
                // A closed channel means the window is gone.
                let _ = done.send(work());
            });
    }
}
