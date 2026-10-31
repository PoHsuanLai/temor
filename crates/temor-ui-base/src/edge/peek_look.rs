//! Looking at a file for the peek card (PLAN §10, D13): `anyview_peek`'s worker thread makes the
//! card off the window's thread, and leaves it in the mailbox for the window to take. It holds
//! the latest ask only, so sweeping the pointer over many paths looks at the last few at most.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use anyview_core::FilePath;
use anyview_fs::OnDisk;
use anyview_peek::{AnyPeeked, PeekWorker, Peeking, WorkerConfig};

use super::peek_mail::{Mail, Taken, Ticket};

/// The worker and the mailbox it answers into.
#[derive(Debug)]
pub struct PeekLook {
    worker: PeekWorker,
    mail: Arc<Mutex<Mail<AnyPeeked>>>,
}

impl PeekLook {
    /// Starts the worker thread; it ends when this is dropped.
    pub fn start() -> PeekLook {
        PeekLook {
            worker: PeekWorker::looking(
                WorkerConfig::default().with_name("temor-peek"),
                Peeking::pane(),
            ),
            mail: Arc::default(),
        }
    }

    /// Asks for the card of `file`. None for a path that is not absolute.
    pub fn ask(&self, file: &Path) -> Option<Ticket> {
        let path = FilePath::new(file).ok()?;
        let ticket = self.lock().ask();
        self.look(ticket, path);
        Some(ticket)
    }

    /// Asks for the card of a file that is not here yet (it is being fetched from another
    /// computer). The ticket is good now; the [`Landing`] says when the file is in place, and the
    /// card is made then.
    pub fn ask_later(self: &Arc<Self>) -> (Ticket, Landing) {
        let ticket = self.lock().ask();
        let landing = Landing {
            look: Arc::clone(self),
            ticket,
        };
        (ticket, landing)
    }

    fn look(&self, ticket: Ticket, path: FilePath) {
        let mail = Arc::clone(&self.mail);
        self.worker.ask(path.on_disk(), move |card| {
            mail.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .deliver(ticket, card);
        });
    }

    /// The card for `ticket`, once it is made.
    pub fn take(&self, ticket: Ticket) -> Taken<Arc<AnyPeeked>> {
        match self.lock().take(ticket) {
            Taken::Ready(card) => Taken::Ready(Arc::new(card)),
            Taken::Waiting => Taken::Waiting,
            Taken::Stale => Taken::Stale,
        }
    }

    /// The card is no longer wanted.
    pub fn cancel(&self) {
        self.lock().cancel();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Mail<AnyPeeked>> {
        self.mail.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A card asked for before its file arrived (see [`PeekLook::ask_later`]). An ask that another
/// came after is stale: its file is ignored when it lands, so it never replaces the newer ask in
/// the worker.
#[derive(Debug)]
pub struct Landing {
    look: Arc<PeekLook>,
    ticket: Ticket,
}

impl Landing {
    /// The file is in place at `file`: the card is made.
    pub fn at(self, file: &Path) {
        let current = self.look.lock().is_latest(self.ticket);
        if let (true, Ok(path)) = (current, FilePath::new(file)) {
            self.look.look(self.ticket, path);
        }
    }

    /// There is no file to look at: the ask ends, and the window stops waiting for it.
    pub fn none(self) {
        let mut mail = self.look.lock();
        if mail.is_latest(self.ticket) {
            mail.cancel();
        }
    }
}
