//! Work a window hands off and awaits, done in the order it was handed over on a thread of its
//! own: a file read again, changed and written back, or a call to the password store, which may
//! wait on IPC or on the person. Two changes made quickly land in the order they were made.

use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use parking_lot::Mutex;

type Job = Box<dyn FnOnce() + Send>;

/// A line of work with one thread, started when it is first given something. Make one per kind
/// of work, as a `static`.
pub struct Serial {
    name: &'static str,
    jobs: OnceLock<Mutex<Option<Sender<Job>>>>,
}

impl std::fmt::Debug for Serial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Serial")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Serial {
    /// A line whose thread is called `name`.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            jobs: OnceLock::new(),
        }
    }

    /// What `job` returns, once every job given before it is done. `None` when the line's thread
    /// could not start.
    pub async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        let (tell, answer) = tokio::sync::oneshot::channel();
        let job: Job = Box::new(move || {
            // The asker may have gone.
            let _ = tell.send(job());
        });
        self.give(job).ok()?;
        answer.await.ok()
    }

    /// Does `job` once every job given before it is done, with nobody waiting for it. False when
    /// the line's thread could not start, and `job` is dropped.
    pub fn start(&self, job: impl FnOnce() + Send + 'static) -> bool {
        self.give(Box::new(job)).is_ok()
    }

    fn give(&self, job: Job) -> Result<(), ()> {
        let jobs = self.jobs.get_or_init(|| Mutex::new(None));
        let mut jobs = jobs.lock();
        let job = match jobs.as_ref() {
            Some(line) => match line.send(job) {
                Ok(()) => return Ok(()),
                // The thread ended (a job panicked): start another.
                Err(back) => back.0,
            },
            None => job,
        };
        let (line, taken) = channel::<Job>();
        std::thread::Builder::new()
            .name(self.name.to_owned())
            .spawn(move || {
                for job in taken {
                    job();
                }
            })
            .map_err(|_| ())?;
        line.send(job).map_err(|_| ())?;
        *jobs = Some(line);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn jobs_are_done_in_the_order_given_on_the_lines_own_thread() {
        static LINE: Serial = Serial::new("temor-serial-test");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut waiting = Vec::new();
        for n in 0..20 {
            let seen = seen.clone();
            waiting.push(LINE.run(move || {
                seen.lock().push(n);
                std::thread::current().name().map(str::to_owned)
            }));
        }
        for answer in waiting {
            assert_eq!(answer.await.flatten().as_deref(), Some("temor-serial-test"));
        }
        assert_eq!(*seen.lock(), (0..20).collect::<Vec<_>>());
    }
}
