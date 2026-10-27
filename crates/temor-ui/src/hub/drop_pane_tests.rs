//! A pane that goes, whichever way it goes, leaves nothing behind that still waits on it.

use ds::components::app::spaces::SpaceId;
use temor_client::desktop::{CLOSED_WHILE_RUNNING, RunEnd, RunEnds, RunTicket};
use temor_client::model::workspace::WorkspaceIn;

use super::run::Found;
use super::tests::hub_after;

fn run_tab(ticket: RunTicket) -> Found {
    Found {
        program: "/bin/sh".into(),
        args: Vec::new(),
        env: Default::default(),
        name: "sh".to_owned(),
        ticket: Some(ticket),
        status: None,
    }
}

#[test]
fn deleting_a_space_ends_the_launch_waiting_on_its_run_tab_as_hung_up() {
    let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let (ends, mut heard) = tokio::sync::mpsc::unbounded_channel();
    hub.set_run_ends(RunEnds::new(ends));
    let ticket = RunTicket(4);
    // The Space's only tab is a run tab; the Space is put away, then deleted.
    let parked = std::mem::replace(&mut hub.book.ws, temor_client::spaces::empty_workspace());
    let pane = parked.tabs.panes()[0];
    hub.run.panes.insert(pane, run_tab(ticket));
    hub.book.place(SpaceId(7), parked);

    hub.forget_space(SpaceId(7));

    assert_eq!(
        heard.try_recv().expect("the launch is told"),
        RunEnd {
            ticket,
            status: CLOSED_WHILE_RUNNING
        }
    );
    assert!(hub.run.panes.is_empty());
    assert_eq!(hub.book.parked().count(), 0);
}
