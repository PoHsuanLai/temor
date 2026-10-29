//! The peek card's worker in the hub (PLAN §10): started when the first card is asked for, shared
//! by every pane (the pointer is in one pane at a time). The view's pure step (`peek_step`) decides
//! when to ask; this looks at the file and hands the card back. A file on another computer is
//! fetched first, through the host's helper (Tier B only), up to a size cap; a bigger one gets
//! its first bytes and its true length, so the card can still say what it is and how big.

use std::cell::OnceCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyview_peek::AnyPeeked;
use temor_client::model::vocab::ids::PaneId;
use temor_client::remote::message::RemotePath;

use super::model::{Hub, HubRef};
use super::ssh_edit::local_name;
use crate::edge::peek_look::{Landing, PeekLook};
use crate::edge::peek_mail::{Taken, Ticket};
use crate::platform::ensure_private_dir;
use temor_client::viewer_host::{Cap, CopyDone, CopyOutcome, copy_from_helper};

/// The biggest file of another computer that is fetched whole for a card.
const PEEK_CAP: u64 = 8 * 1024 * 1024;
/// How many fetched files are kept: the card on screen may still be reading its file.
const KEPT: usize = 4;

/// What the hub keeps for the peek.
#[derive(Default)]
pub(super) struct PeekHub {
    look: OnceCell<Arc<PeekLook>>,
    /// The folders of the files fetched for cards, oldest first.
    fetched: VecDeque<PathBuf>,
    next: u64,
    /// Leftovers of an earlier run are removed once, before the first fetch.
    swept: OnceCell<()>,
}

impl PeekHub {
    fn look(&self) -> &Arc<PeekLook> {
        self.look.get_or_init(|| Arc::new(PeekLook::start()))
    }

    /// Keeps `folder`, and removes the oldest fetched files past [`KEPT`].
    fn keep(&mut self, folder: PathBuf) {
        self.fetched.push_back(folder);
        while self.fetched.len() > KEPT {
            if let Some(old) = self.fetched.pop_front() {
                // Our own private folder; already gone is as good as removed.
                let _ = std::fs::remove_dir_all(old);
            }
        }
    }
}

/// What the fetch of a file for a card tells the peek.
struct Landed {
    landing: Landing,
    local: PathBuf,
}

impl CopyDone for Landed {
    fn done(self: Box<Self>, outcome: CopyOutcome) {
        let Landed { landing, local } = *self;
        match outcome {
            CopyOutcome::Whole | CopyOutcome::Header { .. } => landing.at(&local),
            CopyOutcome::NotAFile | CopyOutcome::Gone | CopyOutcome::Failed(_) => landing.none(),
        }
    }
}

impl Hub {
    /// Asks for the card of the file `file` that the pane `pane` names: a path on this computer,
    /// or on the host the pane is a session on. None where there is nothing to look at: a path
    /// that is not absolute, or a host with no helper.
    fn peek_ask_for(&mut self, pane: PaneId, file: &Path) -> Option<Ticket> {
        let Some(alias) = self.alias_of(pane) else {
            return self.peek.look().ask(file);
        };
        let conn = self.remote.helper_conn(&alias)?;
        let dir = self.launch.ambient.runtime_dir.join("peek");
        self.peek.swept.get_or_init(|| {
            // Our own private folder, from an earlier run.
            let _ = std::fs::remove_dir_all(&dir);
        });
        self.peek.next = self.peek.next.saturating_add(1);
        let folder = dir.join(self.peek.next.to_string());
        ensure_private_dir(&dir)
            .and_then(|()| ensure_private_dir(&folder))
            .ok()?;
        self.peek.keep(folder.clone());
        let name = file.to_string_lossy();
        let local = folder.join(local_name(&name));
        let (ticket, landing) = self.peek.look().ask_later();
        let done = Box::new(Landed {
            landing,
            local: local.clone(),
        });
        let remote = RemotePath(name.into_owned());
        copy_from_helper(done, conn, (remote, local), Cap::Bytes(PEEK_CAP));
        Some(ticket)
    }
}

impl HubRef {
    /// Asks for the peek card of `file`, as named in `pane`; none when there is nothing to look
    /// at.
    pub fn peek_ask(&self, pane: PaneId, file: &Path) -> Option<Ticket> {
        self.0.borrow_mut().peek_ask_for(pane, file)
    }

    /// The card for `ticket`, once it is made.
    pub fn peek_take(&self, ticket: Ticket) -> Taken<Arc<AnyPeeked>> {
        match self.0.borrow().peek.look.get() {
            Some(look) => look.take(ticket),
            None => Taken::Stale,
        }
    }

    /// The card is no longer wanted.
    pub fn peek_cancel(&self) {
        if let Some(look) = self.0.borrow().peek.look.get() {
            look.cancel();
        }
    }
}
