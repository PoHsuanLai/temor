//! History as it happens (PLAN P23): each pane's session is given a lasting id when its shell
//! first says where it is, and its commands are handed to the client as each one finishes, and
//! its end when the program ends. The client keeps them (dropping a secret, as its rules say);
//! nothing is written at quit. A pane brought back at launch goes on with the session it had
//! (`carried`, from the layout): its new commands are numbered after the ones kept before. So
//! does a closed session reopened from its history page, from the last screen it showed when one
//! was kept (`keep::screens`), else with its commands shown above the new shell. A pane brought
//! back with commands an older temor kept for it, and no session, has them written as the start
//! of its session: the older checkpoints are read into history once.

use std::collections::BTreeMap;

use temor_client::model::session::CommandRecord;
use temor_client::model::session::list::kept_cards;
use temor_client::model::vocab::ids::{BlockId, PaneId};
use temor_client::model::vocab::text::Cwd;
use temor_client::{Client, Command};
use temor_schema::session::{RanCommand, Session as Kept, SessionId};
use temor_schema::time::UnixMillis;
use temor_schema::url::Host;

use super::model::{Hub, HubRef};
use temor_pane_view::session::{Finished, Notice, Session};
use temor_panes::vt::Snapshot;

/// The sessions history is told of.
#[derive(Debug, Default)]
pub(super) struct History {
    client: Option<Client>,
    /// The screen a reopened session's tab starts from, until its pane takes it.
    pub(super) screen: Option<Snapshot>,
}

/// Where one pane's session stands, kept in its record.
#[derive(Debug)]
pub(super) enum PaneHistory {
    /// Its shell has not said where it is yet.
    Waiting {
        /// The session it goes on with: it was brought back at launch, or reopened.
        carried: Option<SessionId>,
        /// Commands an older temor kept for it, which start its session when it has none.
        imported: Vec<CommandRecord>,
    },
    /// Its shell said where it is, and its session began.
    Begun(Begun),
}

impl Default for PaneHistory {
    fn default() -> PaneHistory {
        PaneHistory::Waiting {
            carried: None,
            imported: Vec::new(),
        }
    }
}

impl PaneHistory {
    /// The session it has begun, or goes on with once it begins.
    fn session(&self) -> Option<SessionId> {
        match self {
            PaneHistory::Waiting { carried, .. } => *carried,
            PaneHistory::Begun(begun) => Some(begun.id),
        }
    }

    /// The session it has begun.
    fn begun(&self) -> Option<&Begun> {
        match self {
            PaneHistory::Begun(begun) => Some(begun),
            PaneHistory::Waiting { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Begun {
    id: SessionId,
    /// What is added to the pane's block numbers in history: past the commands a session kept
    /// before this launch, whose numbers the pane counts again from the start.
    base: u64,
}

impl Hub {
    /// Keep history with `client`.
    pub fn set_client(&mut self, client: Client) {
        self.history.client = Some(client);
    }

    /// The client the window keeps through, when it has one.
    pub(super) fn client(&self) -> Option<&Client> {
        self.history.client.as_ref()
    }

    /// The sessions the panes brought back at launch go on with.
    pub(super) fn carry_sessions(&mut self, carried: BTreeMap<PaneId, SessionId>) {
        for (pane, id) in carried {
            self.panes.record(pane).history = PaneHistory::Waiting {
                carried: Some(id),
                imported: Vec::new(),
            };
        }
    }

    /// The session `pane` was brought back to go on with, if any.
    pub(super) fn history_carried(&self, pane: PaneId) -> Option<SessionId> {
        match &self.panes.get(pane)?.history {
            PaneHistory::Waiting { carried, .. } => *carried,
            PaneHistory::Begun(_) => None,
        }
    }

    /// `commands`, kept for `pane` by an older temor, start the session it begins.
    pub(super) fn history_import(&mut self, pane: PaneId, commands: &[CommandRecord]) {
        if !commands.is_empty()
            && self.history.client.is_some()
            && let PaneHistory::Waiting { imported, .. } = &mut self.panes.record(pane).history
        {
            *imported = commands.to_vec();
        }
    }

    /// Each pane's session, for the next launch to go on with.
    pub(super) fn sessions_to_carry(&self) -> BTreeMap<PaneId, SessionId> {
        self.panes
            .iter()
            .filter(|(_, record)| record.slot.is_some())
            .filter_map(|(pane, record)| Some((pane, record.history.session()?)))
            .collect()
    }

    /// The session of `pane`, as history knows it.
    pub fn session_id(&self, pane: PaneId) -> Option<SessionId> {
        Some(self.panes.get(pane)?.history.begun()?.id)
    }

    /// The tab the live session `id` is in.
    pub fn tab_of_session(&self, id: SessionId) -> Option<temor_client::model::vocab::ids::TabId> {
        let (pane, _) = self
            .panes
            .iter()
            .find(|(_, record)| record.history.begun().is_some_and(|begun| begun.id == id))?;
        self.book.ws().tab_of(pane)
    }

    /// The session of the pane with the keys in `tab`, or of its first that has one.
    pub fn session_of_tab(&self, tab: temor_client::model::vocab::ids::TabId) -> Option<SessionId> {
        let focus = self.book.ws().focus_of(tab);
        let panes = self.book.ws().tabs.tab(tab)?.tree.panes();
        focus
            .into_iter()
            .chain(panes)
            .find_map(|pane| self.session_id(pane))
    }

    /// The sessions still running in this window.
    pub fn running_sessions(&self) -> std::collections::BTreeSet<SessionId> {
        self.panes
            .iter()
            .filter_map(|(_, record)| Some(record.history.begun()?.id))
            .collect()
    }

    /// Opens the closed session `id` again: a tab in its folder whose shell goes on as that
    /// session, from the last screen it showed; or, when none was kept (or the tab's shell runs
    /// in the daemon, which starts from its own), with the commands it ran above it as cards.
    pub fn reopen_session(&mut self, id: SessionId) {
        let Some(client) = self.history.client.clone() else {
            return;
        };
        // Once is enough: a tab that goes on with it already is brought to the front.
        let going_on = self
            .panes
            .iter()
            .filter(|(_, record)| record.history.session() == Some(id))
            .find_map(|(pane, _)| self.book.ws().tab_of(pane));
        if let Some(tab) = going_on {
            return self.act(super::select_tab(tab));
        }
        let Some(kept) = client.session(id).ok().flatten() else {
            return;
        };
        let cards = kept_cards(&client.commands_of(id).unwrap_or_default());
        let screen = self
            .state_dir
            .as_deref()
            .and_then(|dir| temor_client::keep::screens::read(dir, id))
            .map(Snapshot::from_bytes);
        let had_screen = screen.is_some();
        self.history.screen = screen;
        let opened = self.open_tab(Some(Cwd(kept.folder)));
        // A pane that took the screen shows the session as it was; one that did not (a daemon
        // pane) shows its commands.
        let shown = had_screen && self.history.screen.take().is_none();
        let Some(pane) = opened else {
            return;
        };
        self.panes.record(pane).history = PaneHistory::Waiting {
            carried: Some(id),
            imported: Vec::new(),
        };
        if shown {
            return;
        }
        if let Some(slot) = self.panes.slot(pane)
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
            Notice::Cwd(folder) if self.session_id(pane).is_none() => {
                let PaneHistory::Waiting { carried, imported } =
                    std::mem::take(&mut self.panes.record(pane).history)
                else {
                    return;
                };
                // A pane brought back goes on with its session, if history still has it.
                let before = carried.and_then(|id| client.session(id).ok().flatten());
                // Commands an older temor kept for it start a session that is new.
                let imported = if before.is_none() {
                    imported
                } else {
                    Vec::new()
                };
                let base = before.as_ref().map_or(imported.len() as u64, |kept| {
                    client
                        .commands_of(kept.id)
                        .ok()
                        .and_then(|commands| commands.last().map(|ran| ran.block.0 + 1))
                        .unwrap_or(0)
                });
                let id = before
                    .as_ref()
                    .map_or_else(|| client.fresh_session(pane), |kept| kept.id);
                client.act(Command::Begin(Kept {
                    id,
                    host: self.history_host(pane),
                    folder: folder.clone(),
                    program: self.run_name(pane).map(str::to_owned),
                    started: before.map_or_else(|| client.local_time().now(), |kept| kept.started),
                    ended: None,
                }));
                let at = client.local_time().now();
                for (block, kept) in (0u64..).zip(imported) {
                    let took = kept.took_ms.and_then(|ms| i64::try_from(ms).ok());
                    client.act(Command::Ran(RanCommand {
                        session: id,
                        block: BlockId(block),
                        command: kept.command,
                        folder: folder.clone(),
                        exit: kept.exit,
                        started: UnixMillis(at.0 - took.unwrap_or(0)),
                        ended: Some(at),
                    }));
                }
                // Commands that finished before the shell said where it is (a restored screen's)
                // were heard before the session began, and are not its.
                self.panes.record(pane).history = PaneHistory::Begun(Begun { id, base });
            }
            Notice::Finished(finished) => {
                let Some(begun) = self
                    .panes
                    .get(pane)
                    .and_then(|record| record.history.begun())
                else {
                    return;
                };
                let folder = session.cwd().unwrap_or_default().to_owned();
                let ended = client.local_time().now();
                for done in finished {
                    client.act(Command::Ran(ran(begun, &folder, done, ended)));
                }
            }
            Notice::Ended(_) => {
                let begun = self
                    .panes
                    .get_mut(pane)
                    .and_then(|record| match record.history {
                        PaneHistory::Begun(_) => Some(std::mem::take(&mut record.history)),
                        PaneHistory::Waiting { .. } => None,
                    });
                if let Some(PaneHistory::Begun(begun)) = begun {
                    client.act(Command::End {
                        session: begun.id,
                        at: client.local_time().now(),
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

impl HubRef {
    /// See [`Hub::reopen_session`]; answers the notices it left.
    pub fn reopen_session(&self, id: SessionId) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.reopen_session(id);
        std::mem::take(&mut hub.notices)
    }

    /// See [`Hub::tab_of_session`].
    pub fn tab_of_session(&self, id: SessionId) -> Option<temor_client::model::vocab::ids::TabId> {
        self.0.borrow().tab_of_session(id)
    }

    /// See [`Hub::session_of_tab`].
    pub fn session_of_tab(&self, tab: temor_client::model::vocab::ids::TabId) -> Option<SessionId> {
        self.0.borrow().session_of_tab(tab)
    }

    /// See [`Hub::running_sessions`].
    pub fn running_sessions(&self) -> std::collections::BTreeSet<SessionId> {
        self.0.borrow().running_sessions()
    }
}

/// What history keeps of `done`, a command of `begun`'s session in `folder` heard to end at
/// `ended`.
fn ran(begun: &Begun, folder: &str, done: &Finished, ended: UnixMillis) -> RanCommand {
    let took = i64::try_from(done.ran_ms).unwrap_or(0);
    RanCommand {
        session: begun.id,
        block: BlockId(begun.base + done.id.0),
        command: done.command.clone(),
        folder: folder.to_owned(),
        exit: done.exit,
        started: UnixMillis(ended.0 - took),
        ended: Some(ended),
    }
}
