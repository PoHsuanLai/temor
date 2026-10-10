//! The pool the viewer panes' work runs on (PLAN §10): opening a file, reading its lines, drawing
//! a page. Anyview's panes hand it blocking jobs; every pane of the window shares the one pool.
//! It is the app edge's, so it may own threads. The threads start with the first viewer.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use anyview_pane::{Work, Workers};

/// How many jobs run at once. A job blocks its thread until it is done, so a few at a time keeps
/// a slow file from holding up the rest.
const THREADS: usize = 4;

/// A small pool of threads that take viewer jobs in the order they come.
#[derive(Debug)]
pub struct ViewerWorkers {
    jobs: Mutex<Sender<Work>>,
}

impl ViewerWorkers {
    /// A pool with its threads started. A thread that cannot start leaves the others to carry on;
    /// with none at all a job runs on the thread that submitted it.
    pub fn start() -> ViewerWorkers {
        let (jobs, inbox) = channel::<Work>();
        let inbox = Arc::new(Mutex::new(inbox));
        for number in 0..THREADS {
            let inbox = Arc::clone(&inbox);
            let started = std::thread::Builder::new()
                .name(format!("temor-viewer-{number}"))
                .spawn(move || work_through(&inbox));
            if started.is_err() {
                break;
            }
        }
        ViewerWorkers {
            jobs: Mutex::new(jobs),
        }
    }
}

/// Takes jobs one at a time until the pool is dropped.
fn work_through(inbox: &Mutex<Receiver<Work>>) {
    loop {
        let next = inbox.lock().unwrap_or_else(PoisonError::into_inner).recv();
        match next {
            Ok(work) => work.run(),
            Err(_) => return,
        }
    }
}

impl Workers for ViewerWorkers {
    fn submit(&self, work: Work) {
        let sent = self
            .jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(work);
        // No thread is left to take it: the submitter does the work, as an inline pool would.
        if let Err(failed) = sent {
            failed.0.run();
        }
    }
}
