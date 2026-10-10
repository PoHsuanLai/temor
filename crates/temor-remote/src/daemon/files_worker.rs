//! File requests on a thread of their own. A big read or write on the connection's reader thread
//! would hold up the keystrokes and pings queued behind it; here they wait only for the files
//! before them, which still run in the order they were asked.

use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};

use temor_wire::ids::RequestId;
use temor_wire::message::Request;

use super::core::Daemon;
use super::link::Link;
use crate::error::Error;

/// How many file requests may wait before the reader waits for the worker.
const QUEUE: usize = 64;

/// Whether `body` is served by the file worker.
pub(super) fn is_file_request(body: &Request) -> bool {
    matches!(
        body,
        Request::ListDir { .. }
            | Request::Stat { .. }
            | Request::ReadFile { .. }
            | Request::WriteFile { .. }
            | Request::Watch { .. }
            | Request::Unwatch { .. }
    )
}

/// A request waiting for the worker.
pub(super) struct Job {
    pub(super) id: RequestId,
    pub(super) body: Request,
    pub(super) payload: Option<Vec<u8>>,
}

/// The worker of one connection. Dropping it lets the worker finish what is queued and end.
pub(super) struct FileWorker {
    jobs: SyncSender<Job>,
}

impl FileWorker {
    /// Starts the worker for `link`.
    pub(super) fn start(daemon: &Arc<Daemon>, link: &Arc<Link>) -> Result<Self, Error> {
        let (jobs, queue) = sync_channel::<Job>(QUEUE);
        let (daemon, link) = (Arc::clone(daemon), Arc::clone(link));
        std::thread::Builder::new()
            .name(format!("temor-remote-files-{}", link.id.0))
            .spawn(move || {
                for job in queue {
                    if daemon.answer(&link, job.id, job.body, job.payload).is_err() {
                        // The connection is gone; its reader notices when it next submits.
                        break;
                    }
                }
            })
            .map_err(|error| Error::io("starting the file worker", error))?;
        Ok(Self { jobs })
    }

    /// Queues a request, waiting for room.
    pub(super) fn submit(&self, job: Job) -> Result<(), Error> {
        self.jobs.send(job).map_err(|_| Error::LinkClosed)
    }
}
