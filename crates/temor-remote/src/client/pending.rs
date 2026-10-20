//! Requests waiting for their answers.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, MutexGuard};

use temor_wire::ids::RequestId;
use temor_wire::message::{RemoteError, Reply};

use crate::error::Error;
use crate::sync::lock;

/// A response: the reply or the typed error, and the payload that came with it.
#[derive(Debug)]
pub(super) struct Answer {
    pub(super) outcome: Result<Reply, RemoteError>,
    pub(super) payload: Option<Vec<u8>>,
}

type Waiting = HashMap<RequestId, Sender<Result<Answer, Error>>>;

/// The calls in flight. Once the link has ended there is nothing to wait for, so the table is
/// gone (`None`) and a new call fails at once instead of waiting for an answer that cannot come.
#[derive(Debug)]
pub(super) struct Pending {
    calls: Mutex<Option<Waiting>>,
}

impl Pending {
    pub(super) fn new() -> Self {
        Self {
            calls: Mutex::new(Some(HashMap::new())),
        }
    }

    /// Starts waiting for `id`'s answer.
    pub(super) fn register(&self, id: RequestId) -> Result<Receiver<Result<Answer, Error>>, Error> {
        let (sender, receiver) = channel();
        self.lock()
            .as_mut()
            .ok_or(Error::LinkClosed)?
            .insert(id, sender);
        Ok(receiver)
    }

    /// Stops waiting for `id` (the request could not be sent).
    pub(super) fn forget(&self, id: RequestId) {
        if let Some(calls) = self.lock().as_mut() {
            calls.remove(&id);
        }
    }

    /// Hands `id` its answer. An id nobody waits for is dropped.
    pub(super) fn complete(&self, id: RequestId, answer: Result<Answer, Error>) {
        let sender = self.lock().as_mut().and_then(|calls| calls.remove(&id));
        if let Some(sender) = sender {
            // The caller may have given up waiting.
            let _ = sender.send(answer);
        }
    }

    /// The link ended: every call in flight fails.
    pub(super) fn close(&self) {
        let calls = self.lock().take().unwrap_or_default();
        for (_, sender) in calls {
            let _ = sender.send(Err(Error::LinkClosed));
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Waiting>> {
        lock(&self.calls)
    }
}
