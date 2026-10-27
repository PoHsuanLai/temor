//! History as it happens (PLAN P23): each pane's session is given a lasting id when its shell
//! first says where it is, and its commands are handed to the client as each one finishes, and
//! its end when the program ends. The client keeps them (dropping a secret, as its rules say);
//! nothing is written at quit.

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use temor_client::{Client, Command};
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_schema::session::{At, RanCommand, Session as Kept, SessionId};
use temor_schema::url::Host;

use super::model::Hub;
use crate::session::{Notice, Session};

/// How long history is kept: commands older than this are forgotten when the window opens.
const KEPT_FOR_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// The sessions history is told of.
#[derive(Debug, Default)]
pub(super) struct History {
    client: Option<Client>,
    /// Each pane's session, once its shell has said where it is.
    sessions: BTreeMap<PaneId, Begun>,
}

#[derive(Debug, Clone)]
struct Begun {
    id: SessionId,
    /// The last block handed over; the ones after it are new.
    written: Option<BlockId>,
}

impl Hub {
    /// Keep history with `client`, and forget what is older than history is kept for.
    pub fn set_client(&mut self, client: Client) {
        client.act(Command::ForgetBefore(At(now().0 - KEPT_FOR_MS)));
        self.history.client = Some(client);
    }

    /// The session of `pane`, as history knows it.
    pub fn session_id(&self, pane: PaneId) -> Option<SessionId> {
        self.history.sessions.get(&pane).map(|begun| begun.id)
    }

    /// What `notice`, just taken by `session` in `pane`, means for history.
    pub(super) fn history_heard(&mut self, pane: PaneId, notice: &Notice, session: &Session) {
        let Some(client) = self.history.client.clone() else {
            return;
        };
        match notice {
            Notice::Cwd(folder) if !self.history.sessions.contains_key(&pane) => {
                let id = draw_id(pane);
                client.act(Command::Begin(Kept {
                    id,
                    host: self.history_host(pane),
                    folder: folder.clone(),
                    program: self.run_name(pane).map(str::to_owned),
                    started: now(),
                    ended: None,
                }));
                // Blocks from before the shell said where it is (a restored screen's) are not
                // this session's.
                let written = session.finished_after(None).last().map(|done| done.id);
                self.history.sessions.insert(pane, Begun { id, written });
            }
            Notice::CommandDone => {
                let Some(begun) = self.history.sessions.get_mut(&pane) else {
                    return;
                };
                let folder = session.cwd().unwrap_or_default().to_owned();
                let ended = now();
                for done in session.finished_after(begun.written) {
                    let ran = i64::try_from(done.ran_ms).unwrap_or(0);
                    client.act(Command::Ran(RanCommand {
                        session: begun.id,
                        block: done.id,
                        command: done.command,
                        folder: folder.clone(),
                        exit: done.exit,
                        started: At(ended.0 - ran),
                        ended: Some(ended),
                    }));
                    begun.written = Some(done.id);
                }
            }
            Notice::Ended(_) => {
                if let Some(begun) = self.history.sessions.remove(&pane) {
                    client.act(Command::End {
                        session: begun.id,
                        at: now(),
                    });
                }
            }
            _ => {}
        }
    }

    /// The computer `pane`'s session runs on.
    fn history_host(&self, pane: PaneId) -> Host {
        self.alias_of(pane)
            .and_then(|alias| Host::named(alias.as_str()).ok())
            .unwrap_or(Host::Local)
    }
}

/// The wall clock now. A clock set before 1970 reads as the epoch.
fn now() -> At {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    At(i64::try_from(since).unwrap_or(i64::MAX))
}

/// A new session's id: 64 bits from the process's random hash keys, the pane and the time.
fn draw_id(pane: PaneId) -> SessionId {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    pane.hash(&mut hasher);
    SystemTime::now().hash(&mut hasher);
    SessionId::from_bits(hasher.finish())
}
