//! A hub over a fake port, and the pane events that follow an agent, for the watch tests.

use std::sync::Arc;
use std::time::{Duration, Instant};

use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_pane::PaneEvent;
use temor_vt::{ProgramState, ProgramStatus, ProgramWait};
use tokio::sync::mpsc::UnboundedReceiver;

use super::fake_port::Fake;
use super::jobs::Done;
use super::points_tests::{first_list, first_plan};
use super::tests::{front, front_running};
use crate::hub::model::Hub;
use crate::hub::tests::hub_after;

pub(super) struct Scene {
    pub(super) hub: Hub,
    inbox: UnboundedReceiver<Done>,
    pub(super) fake: Arc<Fake>,
    pub(super) tab: TabId,
    pub(super) pane: PaneId,
}

pub(super) fn program(state: ProgramState) -> PaneEvent {
    PaneEvent::Program(ProgramStatus {
        state,
        wait: ProgramWait::Unspecified,
        percent: None,
        record: String::new(),
        app: "claude".to_owned(),
        title: String::new(),
        message: String::new(),
    })
}

pub(super) fn in_front(name: &str) -> PaneEvent {
    front(name)
}

/// The watch `sandboxed("claude")` asks for, with the command line docket gets.
pub(super) const CLAUDE_SEEN: &str = "watch /work [claude in Shell] Agent | claude --resume 1";

/// A sandbox wrapper in front, as `claude --resume 1` or `codex` runs under one.
pub(super) fn sandboxed(agent: &str) -> PaneEvent {
    front_running("bwrap", &["/usr/bin/bwrap", "--", agent, "--resume", "1"])
}

impl Scene {
    /// A hub with one tab in `/work`, over a fake port.
    pub(super) fn start() -> Scene {
        let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
        let tab = hub.ws.front().expect("a tab");
        let pane = hub.ws.focus_of(tab).expect("a pane");
        hub.ws.cwds.insert(pane, Cwd("/work".to_owned()));
        let inbox = hub.agents.jobs.take_inbox().expect("the inbox");
        let fake = Arc::new(Fake::new(first_list(), first_plan("digest-1")));
        hub.set_checkpoints(fake.clone());
        Scene {
            hub,
            inbox,
            fake,
            tab,
            pane,
        }
    }

    pub(super) fn hear(&mut self, event: &PaneEvent) {
        self.hub.agents_observe(self.pane, event);
    }

    /// Carries on from every job until none has finished for a moment (the jobs chain: each
    /// answer starts the next call).
    pub(super) fn quiet(&mut self) {
        loop {
            let until = Instant::now() + Duration::from_millis(300);
            let done = loop {
                match self.inbox.try_recv() {
                    Ok(done) => break Some(done),
                    Err(_) if Instant::now() < until => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break None,
                }
            };
            match done {
                Some(done) => self.hub.agent_done(done),
                None => return,
            }
        }
    }

    /// Carries on from the next `count` finished jobs, waiting up to ten seconds for each.
    pub(super) fn settle(&mut self, count: usize) {
        for _ in 0..count {
            let until = Instant::now() + Duration::from_secs(10);
            let done = loop {
                match self.inbox.try_recv() {
                    Ok(done) => break done,
                    Err(_) if Instant::now() < until => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => panic!("a job did not report"),
                }
            };
            self.hub.agent_done(done);
        }
    }
}
