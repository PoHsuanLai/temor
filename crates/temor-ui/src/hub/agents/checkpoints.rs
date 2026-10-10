//! The hub's side of checkpoints (D9): a snapshot when an agent pane starts a turn, and putting
//! the files back to one, with the plan shown first. The git work is on worker threads.

use temor_core::checkpoint::{restore_plan, take};
use temor_core::vocab::ids::{BlockId, CheckpointRef, PaneId, TabId};
use temor_core::vocab::text::Cwd;

use super::jobs::Done;
use super::state::Sheet;
use super::work::{self, Seeded};
use crate::error::UiError;
use crate::hub::model::Hub;

impl Hub {
    /// The folder `pane` is in: where its shell said it is, else where it was started.
    pub(super) fn pane_cwd(&self, pane: PaneId) -> Option<String> {
        let live = self.slots.get(&pane).and_then(|slot| {
            slot.borrow()
                .session
                .as_ref()
                .and_then(|s| s.cwd().map(str::to_owned))
        });
        live.or_else(|| self.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()))
    }

    /// An agent pane started a turn: its folder's repository is read, then a snapshot is taken.
    pub(super) fn start_turn(&mut self, pane: PaneId) {
        if self.agents.jobs.available().is_err() {
            return;
        }
        let Some(cwd) = self.pane_cwd(pane) else {
            return;
        };
        self.agents.runs.entry(pane).or_default().turn_started();
        self.agents.jobs.run(move |git| Done::Seeded {
            pane,
            seeded: work::seed(git, &cwd),
        });
    }

    pub(super) fn checkpoint_seeded(&mut self, pane: PaneId, seeded: &Seeded) {
        let Some(run) = self.agents.runs.get_mut(&pane) else {
            return;
        };
        let takes = run.seeded(seeded);
        let Some(root) = seeded.root.clone() else {
            return;
        };
        for at in takes {
            let root = root.clone();
            self.agents.jobs.run(move |git| Done::Taken {
                pane,
                at,
                result: take(git, &root, at).map(|_| ()).map_err(UiError::from),
            });
        }
    }

    pub(super) fn checkpoint_taken(
        &mut self,
        pane: PaneId,
        at: CheckpointRef,
        result: Result<(), UiError>,
    ) {
        if let Err(err) = result {
            if let Some(run) = self.agents.runs.get_mut(&pane) {
                run.take_failed(at);
            }
            self.notices
                .push(format!("A checkpoint could not be saved: {err}"));
        }
    }

    /// The person chose a checkpoint in `tab`'s menu: what restoring it would change is worked
    /// out first, and shown in a sheet before anything is touched.
    pub(super) fn ask_restore(&mut self, tab: TabId, block: BlockId) {
        let Some(pane) = self.ws.focus_of(tab) else {
            return;
        };
        let found = self.agents.runs.get(&pane).and_then(|run| {
            let mark = run.marks().iter().find(|m| m.block == block)?;
            Some((mark.at, run.root.clone()?))
        });
        let Some((at, root)) = found else {
            self.notices
                .push("That checkpoint is no longer there".to_owned());
            return;
        };
        self.agents.jobs.run(move |git| Done::Planned {
            pane,
            block,
            at,
            plan: restore_plan(git, &root, at).map_err(UiError::from),
            root,
        });
    }

    pub(super) fn checkpoint_planned(
        &mut self,
        (pane, block, at): (PaneId, BlockId, CheckpointRef),
        root: String,
        plan: Result<temor_core::checkpoint::RestorePlan, UiError>,
    ) {
        match plan {
            Ok(plan) => {
                self.agents.sheet = Some(Sheet::Restore {
                    pane,
                    block,
                    at,
                    root,
                    plan,
                });
            }
            Err(err) => self
                .notices
                .push(format!("The checkpoint could not be read: {err}")),
        }
    }

    /// The person pressed Restore in the sheet.
    pub fn confirm_restore(&mut self) {
        let Some(Sheet::Restore {
            pane,
            block,
            at,
            root,
            plan,
        }) = self.agents.sheet.take()
        else {
            return;
        };
        self.agents.jobs.run(move |git| Done::Restored {
            pane,
            block,
            result: work::restore(git, &root, at, &plan),
        });
    }

    pub(super) fn checkpoint_restored(
        &mut self,
        pane: PaneId,
        block: BlockId,
        result: Result<usize, UiError>,
    ) {
        match result {
            Ok(_) => {
                if let Some(run) = self.agents.runs.get_mut(&pane) {
                    run.restored(block);
                }
                self.notices.push(format!(
                    "Files are back as they were before turn {}",
                    block.0
                ));
            }
            Err(err) => self
                .notices
                .push(format!("The files could not be put back: {err}")),
        }
    }
}

/// The pane's folder as the workspace names it, for a new tab.
pub(super) fn cwd_of(path: &str) -> Cwd {
    Cwd(path.to_owned())
}
