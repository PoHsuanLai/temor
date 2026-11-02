//! A pane that goes, whichever way it goes, leaves nothing behind that still waits on it.

use ds::components::app::spaces::SpaceId;
use temor_client::model::workspace::WorkspaceIn;
use temor_desktop::{CLOSED_WHILE_RUNNING, RunEnd, RunEnds, RunTicket};

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
    let parked = hub
        .book
        .edit(|ws| std::mem::replace(ws, temor_client::spaces::empty_workspace()));
    let pane = parked.tabs.panes()[0];
    hub.panes.record(pane).run = Some(run_tab(ticket));
    hub.book.place(SpaceId(7), parked);

    hub.forget_space(SpaceId(7));

    assert_eq!(
        heard.try_recv().expect("the launch is told"),
        RunEnd {
            ticket,
            status: CLOSED_WHILE_RUNNING
        }
    );
    assert!(hub.run_of(pane).is_none());
    assert_eq!(hub.book.parked().count(), 0);
}

#[test]
fn a_pane_that_goes_while_its_offer_shows_leaves_no_offer_behind() {
    use temor_client::model::offer::{OfferState, Program};
    use temor_client::model::vocab::ids::BlockId;

    let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let pane = hub.book.ws().tabs.panes()[0];
    let asking = OfferState::Asking {
        block: BlockId(1),
        program: Program("python".to_owned()),
    };
    hub.panes.record(pane).offer.state = asking;
    assert!(hub.offer_asking(pane).is_some());

    hub.drop_pane(pane);

    assert_eq!(hub.offer_asking(pane), None);
    assert_eq!(hub.panes.len(), 0);
}

#[test]
fn a_pane_that_goes_before_its_shell_says_where_it_is_leaves_no_session_waiting() {
    use std::collections::BTreeMap;
    use temor_schema::session::SessionId;

    let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
    let pane = hub.book.ws().tabs.panes()[0];
    hub.carry_sessions(BTreeMap::from([(pane, SessionId::from_bits(9))]));
    assert_eq!(hub.history_carried(pane), Some(SessionId::from_bits(9)));

    hub.drop_pane(pane);

    assert_eq!(hub.history_carried(pane), None);
    assert_eq!(hub.panes.len(), 0);
}
