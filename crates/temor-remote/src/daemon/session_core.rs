//! What a session decides on its own (PLAN §4c, "machines are steps"): who is attached, since
//! when nobody is, its size and folder, and when it ends. Its pump thread owns it, feeds it what
//! happened, and carries out what it answers; the screen and the terminal stay with the pump.

use temor_wire::ids::RemoteSessionId;
use temor_wire::link::Monotonic;
use temor_wire::message::{ExitStatus, IdleLimit, RemotePath, SessionInfo, SessionState};
use temor_wire::units::Size;

use super::link::{Link, LinkId};
use crate::idle::{IdleVerdict, idle_step};

/// What a session sends to: a connection, or in tests only its number.
pub(super) trait Subscriber: Clone {
    fn id(&self) -> LinkId;
}

impl Subscriber for std::sync::Arc<Link> {
    fn id(&self) -> LinkId {
        self.id
    }
}

/// What happens to a session.
#[derive(Debug)]
pub(super) enum SessionIn<L> {
    /// The program wrote, and the screen took it.
    Output,
    /// `L` has the screen's snapshot: the output goes to it too from now on.
    Attached(L),
    /// These detached, went away, or could not keep up.
    Left(Vec<LinkId>),
    /// The screen and the terminal are this size now.
    Resized(Size),
    /// The program said its folder.
    Cwd(String),
    /// A client set how long a detached session is kept.
    Idle(IdleLimit),
    /// Time passed.
    Tick,
    /// A client closed it.
    Close,
    /// The program ended.
    Ended(ExitStatus),
}

/// What a session asks of its pump.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SessionOut<L> {
    /// Send the output to these.
    Forward(Vec<L>),
    /// Hang the program up; the pump then ends and reaps it.
    HangUp,
    /// Tell these the program ended.
    Exited(Vec<L>, ExitStatus),
}

#[derive(Debug)]
pub(super) struct SessionCore<L> {
    size: Size,
    cwd: String,
    /// Every client attached: each gets the output, so several windows can show one session.
    attached: Vec<L>,
    detached_at: Option<Monotonic>,
    idle: IdleLimit,
    /// It was closed (or kept too long): a hang-up alone can be lost (a program that has not yet
    /// made its own process group misses it), so the pump ends on this and reaps.
    closing: bool,
}

impl<L: Subscriber> SessionCore<L> {
    /// A session that begins detached at `now`.
    pub(super) fn new(size: Size, cwd: String, idle: IdleLimit, now: Monotonic) -> Self {
        Self {
            size,
            cwd,
            attached: Vec::new(),
            detached_at: Some(now),
            idle,
            closing: false,
        }
    }

    /// The session after `input` at `now`, and what its pump is to do.
    pub(super) fn step(&mut self, now: Monotonic, input: SessionIn<L>) -> Vec<SessionOut<L>> {
        match input {
            SessionIn::Output if self.attached.is_empty() => vec![],
            SessionIn::Output => vec![SessionOut::Forward(self.attached.clone())],
            SessionIn::Attached(link) => {
                // A link that attaches again is counted once.
                self.attached.retain(|held| held.id() != link.id());
                self.attached.push(link);
                self.detached_at = None;
                vec![]
            }
            SessionIn::Left(gone) => {
                let before = self.attached.len();
                self.attached.retain(|held| !gone.contains(&held.id()));
                if before > 0 && self.attached.is_empty() {
                    self.detached_at = Some(now);
                }
                vec![]
            }
            SessionIn::Resized(size) => {
                self.size = size;
                vec![]
            }
            SessionIn::Cwd(cwd) => {
                self.cwd = cwd;
                vec![]
            }
            SessionIn::Idle(limit) => {
                self.idle = limit;
                vec![]
            }
            SessionIn::Tick => match self.detached_at {
                Some(since) if idle_step(self.idle, since, now) == IdleVerdict::End => self.close(),
                _ => vec![],
            },
            SessionIn::Close => self.close(),
            SessionIn::Ended(status) => {
                vec![SessionOut::Exited(
                    std::mem::take(&mut self.attached),
                    status,
                )]
            }
        }
    }

    fn close(&mut self) -> Vec<SessionOut<L>> {
        match std::mem::replace(&mut self.closing, true) {
            true => vec![],
            false => vec![SessionOut::HangUp],
        }
    }

    pub(super) fn closing(&self) -> bool {
        self.closing
    }

    pub(super) fn size(&self) -> Size {
        self.size
    }

    pub(super) fn info(&self, session: RemoteSessionId) -> SessionInfo {
        SessionInfo {
            session,
            cwd: RemotePath(self.cwd.clone()),
            size: self.size,
            state: match self.attached.is_empty() {
                true => SessionState::Detached,
                false => SessionState::Attached,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::units::{Cols, Rows, Seconds};

    impl Subscriber for LinkId {
        fn id(&self) -> LinkId {
            *self
        }
    }

    const A: LinkId = LinkId(1);
    const B: LinkId = LinkId(2);

    fn session() -> SessionCore<LinkId> {
        let size = Size {
            cols: Cols(80),
            rows: Rows(24),
        };
        SessionCore::new(
            size,
            "/".into(),
            IdleLimit::After(Seconds(60)),
            Monotonic(0),
        )
    }

    fn at(secs: u64) -> Monotonic {
        Monotonic(secs * 1000)
    }

    #[test]
    fn output_goes_to_every_client_attached_and_one_attaching_again_is_counted_once() {
        let mut core = session();
        assert_eq!(core.step(at(0), SessionIn::Output), vec![]);
        core.step(at(0), SessionIn::Attached(A));
        core.step(at(0), SessionIn::Attached(B));
        core.step(at(0), SessionIn::Attached(A));
        assert_eq!(
            core.step(at(0), SessionIn::Output),
            vec![SessionOut::Forward(vec![B, A])]
        );
        assert_eq!(core.info(RemoteSessionId(1)).state, SessionState::Attached);
    }

    #[test]
    fn a_session_is_detached_from_when_its_last_client_left_and_ends_after_the_whole_limit() {
        let mut core = session();
        core.step(at(0), SessionIn::Attached(A));
        core.step(at(0), SessionIn::Attached(B));
        // Attached, nothing expires however long it runs.
        assert_eq!(core.step(at(500), SessionIn::Tick), vec![]);
        // One too slow to keep up is dropped; the other still holds it.
        core.step(at(100), SessionIn::Left(vec![A]));
        assert_eq!(core.step(at(500), SessionIn::Tick), vec![]);
        core.step(at(100), SessionIn::Left(vec![B]));
        assert_eq!(core.info(RemoteSessionId(1)).state, SessionState::Detached);
        assert_eq!(core.step(at(159), SessionIn::Tick), vec![]);
        assert_eq!(
            core.step(at(160), SessionIn::Tick),
            vec![SessionOut::HangUp]
        );
        assert!(core.closing());
        assert_eq!(core.step(at(161), SessionIn::Tick), vec![], "hung up once");
    }

    #[test]
    fn a_link_leaving_a_session_it_never_attached_does_not_restart_its_idle_time() {
        let mut core = session();
        core.step(at(30), SessionIn::Left(vec![A]));
        assert_eq!(core.step(at(60), SessionIn::Tick), vec![SessionOut::HangUp]);
    }

    #[test]
    fn a_new_limit_counts_from_when_the_session_was_left() {
        let mut core = session();
        core.step(at(0), SessionIn::Idle(IdleLimit::Never));
        assert_eq!(core.step(at(1_000_000), SessionIn::Tick), vec![]);
        core.step(at(0), SessionIn::Idle(IdleLimit::After(Seconds(10))));
        assert_eq!(core.step(at(10), SessionIn::Tick), vec![SessionOut::HangUp]);
    }

    #[test]
    fn closing_hangs_up_once_and_the_end_is_told_to_whoever_is_attached() {
        let mut core = session();
        core.step(at(0), SessionIn::Attached(A));
        assert_eq!(core.step(at(0), SessionIn::Close), vec![SessionOut::HangUp]);
        assert_eq!(core.step(at(0), SessionIn::Close), vec![]);
        let status = ExitStatus::Code(temor_wire::units::ExitCode(0));
        assert_eq!(
            core.step(at(0), SessionIn::Ended(status.clone())),
            vec![SessionOut::Exited(vec![A], status)]
        );
        assert_eq!(core.info(RemoteSessionId(1)).state, SessionState::Detached);
    }
}
