//! The downloads shelf in the window (PLAN §9): the live shelf model, the questions waiting for
//! the person, and what the person does with a row. The pane's transfer events are read in
//! `transfers`. Files are placed and the index is kept by `edge::shelf_files`; the model's
//! decisions are temor-core's (`shelf_step`, `transfer_step`).

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use temor_core::remote::Grant;
use temor_core::shelf::{Clear, Shelf, ShelfIn, ShelfOut, ShelfView, Source, Uploads, shelf_view};
use temor_core::transfer::Origin;
use temor_core::vocab::ids::{DownloadId, PaneId};
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};
use temor_pane::OfferId;
use temor_transfer::Consent;
use temor_transfer::shelf::{Clock, ShelfStore, SystemClock, UnixMillis};

use super::model::{Hub, HubRef};
use crate::edge::shelf_files::{persist, restore};
use crate::platform::{FileShell, default_file_shell};
use crate::view::launch::Launch;
use temor_core::shelf::RowAction;

/// What a program asks of the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskKind {
    /// It wants to save files; their names come after the person says yes.
    Files,
    /// It wants to save this one file.
    File { name: String, size: u64 },
    /// It wants a file from the shelf: `name`, or the newest.
    Request { name: Option<String> },
}

/// One question waiting for the person. The front one is on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub pane: PaneId,
    pub offer: OfferId,
    pub kind: AskKind,
    /// Where it comes from, in words: "a program in Build".
    pub from: String,
    /// What "allow" is remembered for.
    pub(super) stream: String,
}

/// What the person sees of a question, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskView {
    pub text: String,
    pub detail: String,
}

impl Ask {
    /// The words of the question.
    pub fn view(&self) -> AskView {
        let from = &self.from;
        match &self.kind {
            AskKind::Files => AskView {
                text: format!("{from} wants to save files to your Downloads folder"),
                detail: "You will see them on the shelf as they arrive.".to_owned(),
            },
            AskKind::File { name, size } => AskView {
                text: format!("{from} wants to save “{name}” to your Downloads folder"),
                detail: format!("{size} bytes. Nothing is saved unless you allow it."),
            },
            AskKind::Request { name } => AskView {
                text: match name {
                    Some(name) => format!("{from} wants the file “{name}” from your Downloads"),
                    None => format!("{from} wants your newest download"),
                },
                detail: "It is sent through this terminal only if you allow it.".to_owned(),
            },
        }
    }
}

/// How a file reached the shelf, kept until it ends.
#[derive(Debug, Clone)]
pub(super) struct Live {
    pub(super) source: Source,
    pub(super) origin: Origin,
}

/// Everything the window keeps about the shelf.
pub struct ShelfHub {
    pub(super) shelf: Shelf,
    pub(super) store: ShelfStore,
    pub(super) when: BTreeMap<DownloadId, UnixMillis>,
    pub(super) grants: BTreeMap<String, Grant>,
    pub(super) asks: VecDeque<Ask>,
    pub(super) live: BTreeMap<(PaneId, OfferId), Live>,
    pub(super) files: BTreeMap<(PaneId, OfferId, String), DownloadId>,
    pub(super) shell: Box<dyn FileShell>,
    /// Files on their way to other computers.
    pub(super) uploads: Uploads,
    /// The window's own clock, for the model's stamps.
    pub(super) clock: crate::edge::SystemClock,
}

impl std::fmt::Debug for ShelfHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShelfHub")
            .field("items", &self.shelf.items().len())
            .field("asks", &self.asks.len())
            .finish_non_exhaustive()
    }
}

impl ShelfHub {
    /// The shelf as the last run left it. Returns the reason when the index was unreadable.
    pub fn open(launch: &Launch) -> (Self, Option<String>) {
        let ambient = &launch.ambient;
        let user_dirs = std::fs::read_to_string(ambient.user_dirs_path()).ok();
        let downloads = ambient.downloads_dir(user_dirs.as_deref());
        let store = ShelfStore::new(ambient.shelf_dir());
        let dir = temor_core::vocab::text::ShelfPath(downloads.display().to_string());
        let (shelf, when, trouble) = restore(&store, &dir, &SystemClock);
        let hub = Self {
            shelf,
            store,
            when,
            grants: BTreeMap::new(),
            asks: VecDeque::new(),
            live: BTreeMap::new(),
            files: BTreeMap::new(),
            shell: default_file_shell(),
            uploads: Uploads::default(),
            clock: crate::edge::SystemClock::new(),
        };
        (hub, trouble)
    }

    /// The model after `input`; what it asks for comes back.
    pub(super) fn step(&mut self, input: ShelfIn) -> Vec<ShelfOut> {
        let dir = self.shelf.dir().clone();
        let current = std::mem::replace(&mut self.shelf, Shelf::new(dir));
        let (next, outs) = temor_core::shelf::shelf_step(current, input);
        self.shelf = next;
        outs
    }

    /// Writes the index. A failure comes back as words.
    pub(super) fn save(&self) -> Option<String> {
        persist(&self.store, &self.shelf, &self.when, &SystemClock)
            .err()
            .map(|_| "temor could not update the list of downloads".to_owned())
    }

    /// Notes when an item arrived.
    pub(super) fn arrived(&mut self, id: DownloadId) {
        self.when.insert(id, SystemClock.now());
    }
}

impl Hub {
    /// The shelf as rows.
    pub fn shelf_view(&self) -> ShelfView {
        shelf_view(&self.shelf.shelf)
    }

    /// The question in front, if a program is waiting on the person. Questions of panes that
    /// have gone are dropped first.
    pub fn front_ask(&mut self) -> Option<Ask> {
        let slots = &self.slots;
        self.shelf.asks.retain(|ask| slots.contains_key(&ask.pane));
        self.shelf.asks.front().cloned()
    }

    /// The person's answer to the question in front.
    pub fn answer_ask(&mut self, consent: Consent) {
        let Some(ask) = self.front_ask() else {
            return;
        };
        self.shelf.asks.pop_front();
        if matches!(ask.kind, AskKind::Request { .. }) {
            self.serve_request(&ask, consent);
            return;
        }
        let grant = match consent {
            Consent::Allow => Grant::Allowed,
            Consent::Refuse => Grant::Refused,
        };
        self.shelf.grants.insert(ask.stream.clone(), grant);
        self.decide(ask.pane, ask.offer, consent);
    }

    fn serve_request(&mut self, ask: &Ask, consent: Consent) {
        let shelf = crate::edge::shelf_files::offered(&self.shelf.shelf);
        if let Some(slot) = self.slots.get(&ask.pane)
            && let Some(session) = slot.borrow().session.as_ref()
        {
            session.pane().serve(ask.offer, consent, shelf);
        }
    }

    /// Tells a pane the person's answer to its offer.
    pub(super) fn decide(&mut self, pane: PaneId, offer: OfferId, consent: Consent) {
        if let Some(slot) = self.slots.get(&pane)
            && let Some(session) = slot.borrow().session.as_ref()
        {
            session.pane().decide(offer, consent);
        }
    }

    /// The person did something with a row of the shelf.
    pub fn shelf_act(&mut self, id: DownloadId, action: RowAction) {
        let outs = match action {
            RowAction::Open => self.shelf.step(ShelfIn::Opened { id }),
            RowAction::Reveal => self.shelf.step(ShelfIn::Revealed { id }),
            RowAction::Clear => {
                let outs = self.shelf.step(ShelfIn::Cleared(Clear::One(id)));
                self.shelf.when.remove(&id);
                self.notices.extend(self.shelf.save());
                outs
            }
            RowAction::ShowInTab(tab) => {
                self.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(tab))));
                Vec::new()
            }
        };
        for out in outs {
            self.carry_out_shelf(&out);
        }
    }

    fn carry_out_shelf(&mut self, out: &ShelfOut) {
        let result = match out {
            ShelfOut::Open(path) => self.shelf.shell.open(&PathBuf::from(path.as_str())),
            ShelfOut::Reveal(path) => self.shelf.shell.reveal(&PathBuf::from(path.as_str())),
            _ => return,
        };
        if result.is_err() {
            self.notices
                .push("temor could not open that file from here".to_owned());
        }
    }
}

impl HubRef {
    /// The shelf as rows.
    pub fn shelf_view(&self) -> ShelfView {
        self.0.borrow().shelf_view()
    }

    /// The question in front, if a program is waiting on the person.
    pub fn front_ask(&self) -> Option<Ask> {
        self.0.borrow_mut().front_ask()
    }

    /// The person's answer to the question in front; what the person should be told comes back.
    pub fn answer_ask(&self, consent: Consent) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.answer_ask(consent);
        std::mem::take(&mut hub.notices)
    }

    /// The person did something with a row; what the person should be told comes back.
    pub fn shelf_act(&self, id: DownloadId, action: RowAction) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.shelf_act(id, action);
        std::mem::take(&mut hub.notices)
    }

    /// Files were dropped on a pane; what the person should be told comes back. On a pane that
    /// runs on another computer they are sent there; anywhere else their paths are typed.
    pub fn drop_files(&self, pane: PaneId, paths: Vec<PathBuf>) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        if hub.is_host_pane(pane) {
            return hub.upload_drop(pane, &paths);
        }
        let said = hub.slots.get(&pane).and_then(|slot| {
            let mut slot = slot.borrow_mut();
            let session = slot.session.as_mut()?;
            session.drop_files(&paths);
            session.take_notice()
        });
        said.into_iter().collect()
    }

    /// Whether `pane` is a session on another computer.
    pub fn is_host_pane(&self, pane: PaneId) -> bool {
        self.0.borrow().is_host_pane(pane)
    }

    /// The files on their way to other computers, newest first.
    pub fn uploads(&self) -> Vec<temor_core::shelf::Upload> {
        self.0.borrow().shelf.uploads.items().to_vec()
    }

    /// The person took an upload off the list.
    pub fn clear_upload(&self, id: temor_core::shelf::UploadId) {
        self.0.borrow_mut().clear_upload(id);
    }
}
