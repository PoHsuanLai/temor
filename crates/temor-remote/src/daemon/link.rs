//! One connection as the daemon sees it: a bounded queue of encoded frames drained by a writer
//! thread. Answers wait for room; events never do. A client too slow to keep up is hung up on
//! rather than slowing the terminal, and reattaching gives it a snapshot instead of what it
//! missed.

use std::io::{PipeWriter, Write};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};

use temor_wire::frame::{FrameLimit, encode};
use temor_wire::hello::Capability;
use temor_wire::message::Message;

use crate::error::Error;
use crate::files::WatchTable;
use crate::platform::Half;

/// How many frames may wait for the writer.
pub(super) const QUEUE: usize = 512;

/// A connection's number, unique for the daemon's life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct LinkId(pub u64);

/// The outgoing side of a connection. `close` ends it for the other end once the daemon is done
/// with the connection, which dropping a clone of a socket cannot do.
pub trait Outgoing: Write + Send + 'static {
    /// Ends the outgoing direction so the other end reads end-of-stream.
    fn close(&mut self) {}
}

impl Outgoing for PipeWriter {}

impl Outgoing for Half {
    fn close(&mut self) {
        self.shutdown_write();
    }
}

/// A connection. `Send` and `Sync`: sessions and the daemon's timer hold it too.
#[derive(Debug)]
pub(super) struct Link {
    pub(super) id: LinkId,
    tx: SyncSender<Vec<u8>>,
    limit: FrameLimit,
    caps: OnceLock<Vec<Capability>>,
    pub(super) watches: Mutex<WatchTable>,
    hung_up: Arc<OnceLock<()>>,
}

impl Link {
    pub(super) fn new(id: LinkId, tx: SyncSender<Vec<u8>>, limit: FrameLimit) -> Self {
        Self {
            id,
            tx,
            limit,
            caps: OnceLock::new(),
            watches: Mutex::new(WatchTable::default()),
            hung_up: Arc::new(OnceLock::new()),
        }
    }

    /// Records what the handshake agreed.
    pub(super) fn agree(&self, caps: Vec<Capability>) {
        // Agreed once per connection; a second call changes nothing.
        let _ = self.caps.set(caps);
    }

    /// Whether `cap` was agreed.
    pub(super) fn has(&self, cap: Capability) -> bool {
        self.caps.get().is_some_and(|caps| caps.contains(&cap))
    }

    /// Queues a frame, waiting for room. For answers.
    pub(super) fn send(&self, message: &Message, payload: Option<&[u8]>) -> Result<(), Error> {
        let bytes = encode(message, payload, self.limit)?;
        self.tx.send(bytes).map_err(|_| Error::LinkClosed(None))
    }

    /// Queues a frame without waiting. A full queue hangs the connection up. For events.
    pub(super) fn try_send(&self, message: &Message, payload: Option<&[u8]>) -> Result<(), Error> {
        let bytes = encode(message, payload, self.limit)?;
        match self.tx.try_send(bytes) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.hang_up();
                Err(Error::LinkClosed(None))
            }
            Err(TrySendError::Disconnected(_)) => Err(Error::LinkClosed(None)),
        }
    }

    /// Stops writing: the writer thread ends the connection.
    pub(super) fn hang_up(&self) {
        // Set once; later calls change nothing.
        let _ = self.hung_up.set(());
    }

    /// The flag the writer thread watches, shared so the thread does not keep the link alive.
    pub(super) fn hang_up_flag(&self) -> Arc<OnceLock<()>> {
        Arc::clone(&self.hung_up)
    }
}

/// The writer thread's loop: frames out, until the queue closes, a write fails or the link is
/// hung up. Then the connection's outgoing side is closed.
pub(super) fn run_writer<W: Outgoing>(
    frames: Receiver<Vec<u8>>,
    mut out: W,
    hung_up: Arc<OnceLock<()>>,
) {
    for bytes in frames {
        if hung_up.get().is_some() {
            break;
        }
        if out.write_all(&bytes).and_then(|()| out.flush()).is_err() {
            break;
        }
    }
    out.close();
}
