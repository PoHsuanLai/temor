//! `master_step`.

use temor_core::vocab::ids::PaneId;
use temor_ssh::MasterProbe;

/// Where a host's shared connection stands.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Master {
    /// No master is known to be running.
    #[default]
    Down,
    /// Checking whether one is running (`probe_master`).
    Probing { first: PaneId, waiting: Vec<PaneId> },
    /// `first` is making the connection; `waiting` panes join when it is up.
    Connecting { first: PaneId, waiting: Vec<PaneId> },
    /// The master is up: a pane joins it at once.
    Up,
}

/// What moves the master.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MasterIn {
    /// A pane wants a session on this host (a new tab, or a split of a pane on it).
    Wanted(PaneId),
    /// The probe's answer.
    Probed(MasterProbe),
    /// The first session is connected (the host answered, its key is verified).
    Established,
    /// A pane's session ended before it was up, or a join failed.
    Failed(PaneId),
    /// The master is gone: its socket is dead or its process ended.
    Lost,
}

/// How a pane's session starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenAs {
    /// Start the session; it makes the master and asks for any password.
    First,
    /// Start the session; it joins the running master.
    Join,
}

/// What the master asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MasterOut {
    /// Run `probe_master`, then send `Probed`.
    Probe,
    /// Start this pane's session (`session_argv`).
    Open { pane: PaneId, how: OpenAs },
    /// The master ended: its forwards are gone (feed `ForwardIn::ConnectionLost`).
    ConnectionEnded,
}

fn join_all(panes: impl IntoIterator<Item = PaneId>) -> Vec<MasterOut> {
    panes
        .into_iter()
        .map(|pane| MasterOut::Open {
            pane,
            how: OpenAs::Join,
        })
        .collect()
}

/// The master after `input`.
pub fn master_step(state: Master, input: MasterIn) -> (Master, Vec<MasterOut>) {
    use Master as M;
    use MasterIn as I;
    match (state, input) {
        (M::Down, I::Wanted(pane)) => (
            M::Probing {
                first: pane,
                waiting: vec![],
            },
            vec![MasterOut::Probe],
        ),
        (M::Up, I::Wanted(pane)) => (M::Up, join_all([pane])),
        (M::Probing { first, mut waiting }, I::Wanted(pane)) => {
            waiting.push(pane);
            (M::Probing { first, waiting }, vec![])
        }
        (M::Connecting { first, mut waiting }, I::Wanted(pane)) => {
            waiting.push(pane);
            (M::Connecting { first, waiting }, vec![])
        }

        (M::Probing { first, waiting }, I::Probed(MasterProbe::Running)) => {
            (M::Up, join_all([first].into_iter().chain(waiting)))
        }
        (M::Probing { first, waiting }, I::Probed(_)) => (
            M::Connecting { first, waiting },
            vec![MasterOut::Open {
                pane: first,
                how: OpenAs::First,
            }],
        ),

        (M::Connecting { waiting, .. }, I::Established) => (M::Up, join_all(waiting)),
        (M::Connecting { first, mut waiting }, I::Failed(pane)) if pane == first => {
            // The first attempt failed (wrong password, unreachable): the next pane in line
            // tries, so one failure does not strand the others.
            match waiting.is_empty() {
                true => (M::Down, vec![]),
                false => {
                    let next = waiting.remove(0);
                    (
                        M::Connecting {
                            first: next,
                            waiting,
                        },
                        vec![MasterOut::Open {
                            pane: next,
                            how: OpenAs::First,
                        }],
                    )
                }
            }
        }
        (M::Connecting { first, mut waiting }, I::Failed(pane)) => {
            waiting.retain(|p| *p != pane);
            (M::Connecting { first, waiting }, vec![])
        }
        (M::Probing { first, mut waiting }, I::Failed(pane)) => {
            waiting.retain(|p| *p != pane);
            (M::Probing { first, waiting }, vec![])
        }

        // A join that failed may mean the master died; probe again on the next request.
        (M::Up, I::Failed(_)) => (M::Down, vec![]),
        (M::Up, I::Lost) => (M::Down, vec![MasterOut::ConnectionEnded]),
        (state, _) => (state, vec![]),
    }
}
