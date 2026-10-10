//! The hub's side of restore points (PLAN P19): docket keeps one run's points, and the window
//! lists them in a tab's menu, shows what a restore would change in a sheet, and asks docket to
//! do it. The calls run on worker threads; the answers come back as [`Done`]s. Restore is held
//! back while docket's list says a turn is running: it would write over what was put back.

use std::sync::Arc;

use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_desktop::{
    CheckpointPort, DesktopError, PointId, PointList, PointPlan, SessionKey, TurnNo,
};

use super::jobs::Done;
use super::points::WAIT_FOR_AGENT;
use super::state::Sheet;
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

    /// Where the restore points come from from now on.
    pub fn set_checkpoints(&mut self, port: Arc<dyn CheckpointPort>) {
        self.agents.points.port = port;
    }

    /// The docket session `pane` runs, or none once it ends. Its points are read again.
    pub fn set_agent_session(&mut self, pane: PaneId, session: Option<SessionKey>) {
        match session {
            Some(session) => {
                self.agents.points.sessions.insert(pane, session);
                self.refresh_points(pane);
            }
            None => self.agents.points.forget(pane),
        }
    }

    /// Reads `pane`'s restore points again, if it runs a session docket knows.
    pub fn refresh_points(&mut self, pane: PaneId) {
        let Some(session) = self.agents.points.sessions.get(&pane).cloned() else {
            return;
        };
        let port = Arc::clone(&self.agents.points.port);
        self.agents.jobs.spawn(move || Done::PointsListed {
            pane,
            result: port.list(&session),
        });
    }

    /// Reads the points of the pane in front of `tab` (its menu is opening).
    pub fn refresh_points_of(&mut self, tab: TabId) {
        if let Some(pane) = self.ws.focus_of(tab) {
            self.refresh_points(pane);
        }
    }

    pub(super) fn points_listed(&mut self, pane: PaneId, result: Result<PointList, DesktopError>) {
        // A pane that ended while the call ran is not given a list back.
        if !self.agents.points.sessions.contains_key(&pane) {
            return;
        }
        match result {
            Ok(list) => {
                self.agents.points.lists.insert(pane, list);
            }
            Err(err) => self.notices.push(format!(
                "The restore points could not be read: {}",
                words(&err)
            )),
        }
    }

    /// The person chose a restore point in `tab`'s menu: what restoring it would change is
    /// asked of docket first, and shown in a sheet before anything is touched.
    pub(super) fn ask_restore(&mut self, tab: TabId, id: PointId) {
        let Some(pane) = self.ws.focus_of(tab) else {
            return;
        };
        if self.agents.points.running(pane) {
            self.notices.push(WAIT_FOR_AGENT.to_owned());
            return;
        }
        let Some(session) = self.agents.points.sessions.get(&pane).cloned() else {
            self.notices
                .push("That restore point is no longer there".to_owned());
            return;
        };
        let port = Arc::clone(&self.agents.points.port);
        self.agents.jobs.spawn(move || Done::PointPlanned {
            pane,
            result: port.plan(&session, id),
            session,
            id,
        });
    }

    pub(super) fn point_planned(
        &mut self,
        (pane, session, id): (PaneId, SessionKey, PointId),
        result: Result<PointPlan, DesktopError>,
    ) {
        match result {
            Ok(plan) => {
                let turn = self.agents.points.turn_of(pane, id);
                self.agents.sheet = Some(Sheet::Restore {
                    pane,
                    session,
                    id,
                    turn,
                    plan,
                });
            }
            Err(err) => self.notices.push(format!(
                "The restore point could not be read: {}",
                words(&err)
            )),
        }
    }

    /// The person pressed Restore… in the sheet, which only shows the plan. docket gets back the
    /// digest of the plan the sheet showed, so it refuses if the folder changed since, and asks
    /// the person itself before it puts anything back.
    pub fn perform_restore(&mut self) {
        let Some(Sheet::Restore {
            pane,
            session,
            id,
            turn,
            plan,
        }) = self.agents.sheet.take()
        else {
            return;
        };
        if self.agents.points.running(pane) {
            self.notices.push(WAIT_FOR_AGENT.to_owned());
            return;
        }
        let port = Arc::clone(&self.agents.points.port);
        self.agents.jobs.spawn(move || Done::PointRestored {
            pane,
            turn,
            result: port.restore(&session, id, &plan.digest),
        });
    }

    pub(super) fn point_restored(
        &mut self,
        pane: PaneId,
        turn: Option<TurnNo>,
        result: Result<(), DesktopError>,
    ) {
        match result {
            Ok(()) => {
                self.notices.push(match turn {
                    Some(turn) => format!("Files are back as they were before turn {}", turn.0),
                    None => "Files are back as they were".to_owned(),
                });
                self.refresh_points(pane);
            }
            Err(err) => self
                .notices
                .push(format!("The files could not be put back: {}", words(&err))),
        }
    }
}

/// What went wrong, in the person's words.
pub(super) fn words(err: &DesktopError) -> String {
    match err {
        DesktopError::Checkpoint(fault) => fault.words().to_owned(),
        _ => "the restore points are not reachable".to_owned(),
    }
}

/// The pane's folder as the workspace names it, for a new tab.
pub(super) fn cwd_of(path: &str) -> Cwd {
    Cwd(path.to_owned())
}
