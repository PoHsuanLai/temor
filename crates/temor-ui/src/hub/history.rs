//! History as it happens (PLAN P23): each pane's session is given a lasting id when its shell
//! first says where it is, and its commands are handed to the client as each one finishes, and
//! its end when the program ends. The client keeps them (dropping a secret, as its rules say);
//! nothing is written at quit. A pane brought back at launch goes on with the session it had
//! (`carried`, from the layout): its new commands are numbered after the ones kept before. So
//! does a closed session reopened from its history page, its commands shown above the new shell.

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::time::SystemTime;

use temor_client::{Client, Command};
use temor_core::session::list::kept_cards;
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_core::vocab::text::Cwd;
use temor_schema::session::{At, RanCommand, Session as Kept, SessionId};
use temor_schema::url::Host;

use super::model::{Hub, HubRef};
use crate::edge::local_zone::wall_now as now;
use crate::session::{Notice, Session};

/// How long history is kept: commands older than this are forgotten when the window opens.
const KEPT_FOR_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// The sessions history is told of.
#[derive(Debug, Default)]
pub(super) struct History {
    client: Option<Client>,
    /// Each pane's session, once its shell has said where it is.
    sessions: BTreeMap<PaneId, Begun>,
    /// The sessions the panes brought back at launch had, until their shells say where they are.
    carried: BTreeMap<PaneId, SessionId>,
}

#[derive(Debug, Clone)]
struct Begun {
    id: SessionId,
    /// The last block handed over; the ones after it are new.
    written: Option<BlockId>,
    /// What is added to the pane's block numbers in history: past the commands a session kept
    /// before this launch, whose numbers the pane counts again from the start.
    base: u64,
}

impl Hub {
    /// Keep history with `client`, and forget what is older than history is kept for.
    pub fn set_client(&mut self, client: Client) {
        client.act(Command::ForgetBefore(At(now().0 - KEPT_FOR_MS)));
        self.history.client = Some(client);
    }

    /// The client the window keeps through, when it has one.
    pub(super) fn client(&self) -> Option<&Client> {
        self.history.client.as_ref()
    }

    /// The sessions the panes brought back at launch go on with.
    pub(super) fn carry_sessions(&mut self, carried: BTreeMap<PaneId, SessionId>) {
        self.history.carried = carried;
    }

    /// Each pane's session, for the next launch to go on with.
    pub(super) fn sessions_to_carry(&self) -> BTreeMap<PaneId, SessionId> {
        self.history
            .sessions
            .iter()
            .map(|(pane, begun)| (*pane, begun.id))
            .chain(self.history.carried.iter().map(|(pane, id)| (*pane, *id)))
            .filter(|(pane, _)| self.slots.contains_key(pane))
            .collect()
    }

    /// The session of `pane`, as history knows it.
    pub fn session_id(&self, pane: PaneId) -> Option<SessionId> {
        self.history.sessions.get(&pane).map(|begun| begun.id)
    }

    /// The tab the live session `id` is in.
    pub fn tab_of_session(&self, id: SessionId) -> Option<temor_core::vocab::ids::TabId> {
        let (pane, _) = self
            .history
            .sessions
            .iter()
            .find(|(_, begun)| begun.id == id)?;
        self.book.ws.tab_of(*pane)
    }

    /// The session of the pane with the keys in `tab`, or of its first that has one.
    pub fn session_of_tab(&self, tab: temor_core::vocab::ids::TabId) -> Option<SessionId> {
        let focus = self.book.ws.focus_of(tab);
        let panes = self.book.ws.tabs.tab(tab)?.tree.panes();
        focus
            .into_iter()
            .chain(panes)
            .find_map(|pane| self.session_id(pane))
    }

    /// The sessions still running in this window.
    pub fn running_sessions(&self) -> std::collections::BTreeSet<SessionId> {
        self.history
            .sessions
            .values()
            .map(|begun| begun.id)
            .collect()
    }

    /// Opens the closed session `id` again: a tab in its folder whose shell goes on as that
    /// session, with the commands it ran above it as cards (their output was not kept).
    pub fn reopen_session(&mut self, id: SessionId) {
        let Some(client) = self.history.client.clone() else {
            return;
        };
        // Once is enough: a tab that goes on with it already is brought to the front.
        let going_on = self
            .history
            .sessions
            .iter()
            .filter(|(_, begun)| begun.id == id)
            .map(|(pane, _)| *pane)
            .chain(
                self.history
                    .carried
                    .iter()
                    .filter(|(_, carried)| **carried == id)
                    .map(|(pane, _)| *pane),
            )
            .find_map(|pane| self.book.ws.tab_of(pane));
        if let Some(tab) = going_on {
            return self.act(super::select_tab(tab));
        }
        let store = client.store();
        let Some(kept) = store.session(id).ok().flatten() else {
            return;
        };
        let cards = kept_cards(&store.commands_of(id).unwrap_or_default());
        let Some(pane) = self.open_tab(Some(Cwd(kept.folder))) else {
            return;
        };
        self.history.carried.insert(pane, id);
        if let Some(slot) = self.slots.get(&pane)
            && let Some(session) = slot.borrow_mut().session.as_mut()
        {
            session.set_kept(cards);
        }
    }

    /// What `notice`, just taken by `session` in `pane`, means for history.
    pub(super) fn history_heard(&mut self, pane: PaneId, notice: &Notice, session: &Session) {
        let Some(client) = self.history.client.clone() else {
            return;
        };
        match notice {
            Notice::Cwd(folder) if !self.history.sessions.contains_key(&pane) => {
                // A pane brought back goes on with its session, if history still has it.
                let store = client.store();
                let before = self
                    .history
                    .carried
                    .remove(&pane)
                    .and_then(|id| store.session(id).ok().flatten());
                let base = before.as_ref().map_or(0, |kept| {
                    store
                        .commands_of(kept.id)
                        .ok()
                        .and_then(|commands| commands.last().map(|ran| ran.block.0 + 1))
                        .unwrap_or(0)
                });
                let id = before
                    .as_ref()
                    .map_or_else(|| draw_id(pane), |kept| kept.id);
                client.act(Command::Begin(Kept {
                    id,
                    host: self.history_host(pane),
                    folder: folder.clone(),
                    program: self.run_name(pane).map(str::to_owned),
                    started: before.map_or_else(now, |kept| kept.started),
                    ended: None,
                }));
                // Blocks from before the shell said where it is (a restored screen's) are not
                // this session's, or were handed over before.
                let written = session.finished_after(None).last().map(|done| done.id);
                self.history
                    .sessions
                    .insert(pane, Begun { id, written, base });
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
                        block: BlockId(begun.base + done.id.0),
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

/// A new session's id: 64 bits from the process's random hash keys, the pane and the time.
fn draw_id(pane: PaneId) -> SessionId {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    pane.hash(&mut hasher);
    SystemTime::now().hash(&mut hasher);
    SessionId::from_bits(hasher.finish())
}

impl HubRef {
    /// See [`Hub::reopen_session`]; answers the notices it left.
    pub fn reopen_session(&self, id: SessionId) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.reopen_session(id);
        std::mem::take(&mut hub.notices)
    }

    /// See [`Hub::tab_of_session`].
    pub fn tab_of_session(&self, id: SessionId) -> Option<temor_core::vocab::ids::TabId> {
        self.0.borrow().tab_of_session(id)
    }

    /// See [`Hub::session_of_tab`].
    pub fn session_of_tab(&self, tab: temor_core::vocab::ids::TabId) -> Option<SessionId> {
        self.0.borrow().session_of_tab(tab)
    }

    /// See [`Hub::running_sessions`].
    pub fn running_sessions(&self) -> std::collections::BTreeSet<SessionId> {
        self.0.borrow().running_sessions()
    }
}
