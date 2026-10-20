use super::{Master, MasterIn, MasterOut, OpenAs, master_step};
use crate::test_support::run_steps;
use crate::vocab::ids::PaneId;
use temor_ssh::MasterProbe;

fn open(pane: u64, how: OpenAs) -> MasterOut {
    MasterOut::Open {
        pane: PaneId(pane),
        how,
    }
}

fn run(from: Master, inputs: &[MasterIn]) -> (Master, Vec<MasterOut>) {
    run_steps(from, inputs.iter().copied(), master_step)
}

fn wanted(pane: u64) -> MasterIn {
    MasterIn::Wanted(PaneId(pane))
}

#[test]
fn panes_asked_for_while_the_first_connects_wait_and_then_join_in_order() {
    let (state, outs) = run(
        Master::Down,
        &[
            wanted(1),
            MasterIn::Probed(MasterProbe::NotRunning),
            wanted(2),
            wanted(3),
            MasterIn::Established,
            wanted(4),
        ],
    );
    assert_eq!(state, Master::Up);
    assert_eq!(
        outs,
        vec![
            MasterOut::Probe,
            open(1, OpenAs::First),
            open(2, OpenAs::Join),
            open(3, OpenAs::Join),
            open(4, OpenAs::Join),
        ]
    );
}

#[test]
fn a_master_left_from_an_earlier_run_is_joined_by_everyone_waiting() {
    let (state, outs) = run(
        Master::Down,
        &[wanted(1), wanted(2), MasterIn::Probed(MasterProbe::Running)],
    );
    assert_eq!(state, Master::Up);
    assert_eq!(
        outs,
        vec![
            MasterOut::Probe,
            open(1, OpenAs::Join),
            open(2, OpenAs::Join)
        ]
    );
}

#[test]
fn a_failed_first_connection_hands_over_to_the_next_pane_in_line() {
    let (state, outs) = run(
        Master::Down,
        &[
            wanted(1),
            MasterIn::Probed(MasterProbe::NotRunning),
            wanted(2),
            wanted(3),
            // A waiting pane closed before its turn.
            MasterIn::Failed(PaneId(3)),
            MasterIn::Failed(PaneId(1)),
        ],
    );
    assert_eq!(
        state,
        Master::Connecting {
            first: PaneId(2),
            waiting: vec![]
        }
    );
    assert_eq!(
        outs,
        vec![
            MasterOut::Probe,
            open(1, OpenAs::First),
            open(2, OpenAs::First)
        ]
    );
    assert_eq!(
        master_step(state, MasterIn::Failed(PaneId(2))),
        (Master::Down, vec![]),
        "the last one failing leaves nothing to start"
    );
}

#[test]
fn a_lost_master_ends_the_connection_and_the_next_pane_probes_again() {
    let (state, outs) = run(Master::Up, &[MasterIn::Lost, wanted(7)]);
    assert_eq!(
        state,
        Master::Probing {
            first: PaneId(7),
            waiting: vec![]
        }
    );
    assert_eq!(outs, vec![MasterOut::ConnectionEnded, MasterOut::Probe]);
    // A failed join does not claim the connection ended; it only makes the next pane check.
    assert_eq!(
        master_step(Master::Up, MasterIn::Failed(PaneId(1))),
        (Master::Down, vec![])
    );
}
