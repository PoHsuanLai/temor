//! `offer_step`: when the offer appears for a listed command. The pane's block log says which
//! command is running; the shell prints nothing of its own once an inner shell has taken over,
//! so the rule is time: the command has run for [`SETTLE_MS`] with no end mark, the screen is
//! not an alternate one, and the output has been quiet for [`QUIET_MS`].

use super::list::Program;
use crate::vocab::ids::BlockId;
use ds_core::time::stamp::Stamp;

/// How long a listed command must have run, with no end mark, before it is offered.
pub const SETTLE_MS: u64 = 1_500;
/// How long the output must have been quiet.
pub const QUIET_MS: u64 = 500;
/// How long a Yes waits for the inner shell to name itself before the offer gives up.
pub const PROBE_MS: u64 = 4_000;

/// Where one pane stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OfferState {
    /// No listed command is running.
    #[default]
    Idle,
    /// A listed command is running; the offer waits for the timing rule.
    Watching {
        /// The block of the command.
        block: BlockId,
        /// Which listed command.
        program: Program,
        /// When the command started.
        started: Stamp,
        /// When output last arrived.
        last_output: Stamp,
    },
    /// The offer is showing.
    Asking {
        /// The block of the command.
        block: BlockId,
        /// Which listed command.
        program: Program,
    },
    /// Yes was said and a one-line question went to the inner shell: which shell are you? The
    /// integration is typed only once the answer is on the screen.
    Probing {
        /// The block of the command.
        block: BlockId,
        /// When the question was typed.
        since: Stamp,
    },
    /// Answered; nothing more is offered for this command.
    Settled {
        /// The block of the command.
        block: BlockId,
    },
}

/// The screen a tick found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Normal,
    /// A full-screen program has the screen.
    Alternate,
}

/// The last line on the screen, as far as the offer cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Plain,
    /// It asks for a secret or a yes or no (see [`super::asks_for_secret`]).
    Asking,
}

/// The person's answer to the offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Yes,
    NotNow,
    Never,
}

/// What the screen shows of the inner shell's answer to the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Nothing yet.
    Silent,
    /// A shell temor has an integration for named itself.
    Known,
    /// Some other shell named itself (dash, ash, ...).
    Other,
}

/// What moves the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferIn {
    /// The pane's running command changed: `Some` is a listed command (its block and which
    /// one), `None` is nothing running, or a command that is not listed.
    Running {
        now: Option<(BlockId, Program)>,
        at: Stamp,
    },
    /// Output arrived.
    Output { at: Stamp },
    /// Time passed; what the screen shows now.
    Tick {
        at: Stamp,
        screen: Screen,
        line: Line,
    },
    /// The person answered, at this time.
    Reply(Reply, Stamp),
    /// Time passed while the question to the inner shell is out; what the screen shows of it.
    Heard { at: Stamp, answer: Answer },
}

/// What the machine asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferOut {
    /// The offer appeared (or went away): draw the pane again.
    Redraw,
    /// Type the one-line question that makes the inner shell name itself.
    Probe,
    /// The shell has named itself: type the integration written for it.
    Load,
    /// The shell never answered, or is one temor has no integration for.
    Unsure,
    /// Add this command to the remove list, for good.
    Never(Program),
}

/// One step. Pure: the caller reads the clock and the screen.
pub fn offer_step(state: OfferState, input: OfferIn) -> (OfferState, Vec<OfferOut>) {
    use OfferState as S;
    match (state, input) {
        (state, OfferIn::Running { now, at }) => running(state, now, at),
        (
            S::Watching {
                block,
                program,
                started,
                ..
            },
            OfferIn::Output { at },
        ) => (
            S::Watching {
                block,
                program,
                started,
                last_output: at,
            },
            vec![],
        ),
        // New output under the offer: the program moved on (a retried password, a banner).
        (S::Asking { block, program }, OfferIn::Output { at }) => (
            S::Watching {
                block,
                program,
                started: at,
                last_output: at,
            },
            vec![OfferOut::Redraw],
        ),
        (
            S::Watching {
                block,
                program,
                started,
                last_output,
            },
            OfferIn::Tick {
                at,
                screen: Screen::Normal,
                line: Line::Plain,
            },
        ) if at.since(started) >= SETTLE_MS && at.since(last_output) >= QUIET_MS => {
            (S::Asking { block, program }, vec![OfferOut::Redraw])
        }
        (S::Asking { block, program }, OfferIn::Reply(reply, since)) => match reply {
            Reply::Yes => (
                S::Probing { block, since },
                vec![OfferOut::Redraw, OfferOut::Probe],
            ),
            Reply::NotNow => (S::Settled { block }, vec![OfferOut::Redraw]),
            Reply::Never => (
                S::Settled { block },
                vec![OfferOut::Redraw, OfferOut::Never(program)],
            ),
        },
        (S::Probing { block, since }, OfferIn::Heard { at, answer }) => match answer {
            Answer::Known => (S::Settled { block }, vec![OfferOut::Load]),
            Answer::Other => (S::Settled { block }, vec![OfferOut::Unsure]),
            Answer::Silent if at.since(since) >= PROBE_MS => {
                (S::Settled { block }, vec![OfferOut::Unsure])
            }
            Answer::Silent => (S::Probing { block, since }, vec![]),
        },
        (state, _) => (state, vec![]),
    }
}

fn running(
    state: OfferState,
    now: Option<(BlockId, Program)>,
    at: Stamp,
) -> (OfferState, Vec<OfferOut>) {
    use OfferState as S;
    let (block, hide) = match &state {
        S::Idle => (None, false),
        S::Watching { block, .. } => (Some(*block), false),
        S::Asking { block, .. } => (Some(*block), true),
        S::Probing { block, .. } => (Some(*block), false),
        S::Settled { block } => (Some(*block), false),
    };
    match now {
        Some((id, _)) if block == Some(id) => (state, vec![]),
        Some((id, program)) => {
            let redraw = if hide { vec![OfferOut::Redraw] } else { vec![] };
            (
                S::Watching {
                    block: id,
                    program,
                    started: at,
                    last_output: at,
                },
                redraw,
            )
        }
        None => (S::Idle, if hide { vec![OfferOut::Redraw] } else { vec![] }),
    }
}

impl OfferState {
    /// The listed command being offered, while the offer shows.
    pub fn asking(&self) -> Option<&Program> {
        match self {
            OfferState::Asking { program, .. } => Some(program),
            _ => None,
        }
    }
}
