//! `Term`: a spawned terminal plus everything read from it so far.

use std::time::{Duration, Instant};

use temor_pty::{
    Interest, Pty, PtyIo, ReadOutcome, Ready, SessionLauncher, SpawnSpec, WriteOutcome,
};

pub struct Term {
    pub pty: Pty,
    io: PtyIo,
    pub out: Vec<u8>,
    closed: bool,
    answered: usize,
}

// Generous: shells start slowly on a loaded machine, and the wait ends as soon as the output arrives.
const PATIENCE: Duration = Duration::from_secs(30);

impl Term {
    pub fn spawn(spec: &SpawnSpec, launcher: &SessionLauncher) -> Self {
        let pty = Pty::spawn(spec, launcher).expect("spawn");
        let io = pty.io().expect("io");
        Self {
            pty,
            io,
            out: Vec::new(),
            closed: false,
            answered: 0,
        }
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.out).into_owned()
    }

    pub fn send(&mut self, text: &str) {
        let mut rest = text.as_bytes();
        let deadline = Instant::now() + PATIENCE;
        while !rest.is_empty() && Instant::now() < deadline {
            match self.io.write(rest).expect("write") {
                WriteOutcome::Wrote(n) => rest = &rest[n..],
                WriteOutcome::WouldBlock => {
                    self.io
                        .wait(Interest::Write, Some(Duration::from_millis(50)))
                        .expect("wait");
                }
                WriteOutcome::Closed => panic!("terminal closed while sending {text:?}"),
            }
        }
    }

    /// Reads until `done` holds for everything read so far, the terminal closes, or the
    /// patience runs out. Returns whether `done` held.
    pub fn read_until(&mut self, done: impl Fn(&[u8]) -> bool) -> bool {
        let deadline = Instant::now() + PATIENCE;
        while !done(&self.out) {
            let now = Instant::now();
            if now >= deadline || self.closed {
                return false;
            }
            self.pump(deadline - now);
        }
        true
    }

    /// Reads until `needle` appears after byte `from`.
    pub fn read_until_has(&mut self, from: usize, needle: &str) -> bool {
        let needle = needle.as_bytes().to_vec();
        self.read_until(|out| find(&out[from.min(out.len())..], &needle).is_some())
    }

    /// Reads until the terminal closes.
    pub fn read_to_close(&mut self) -> bool {
        let deadline = Instant::now() + PATIENCE;
        while !self.closed && Instant::now() < deadline {
            self.pump(deadline - Instant::now());
        }
        self.closed
    }

    /// Answers primary device attribute queries as a terminal would: fish 4 waits for one
    /// before its first prompt.
    fn answer_device_attributes(&mut self) {
        let asked = count(&self.out, b"\x1b[c") + count(&self.out, b"\x1b[0c");
        while self.answered < asked {
            self.answered += 1;
            self.send("\x1b[?62;22c");
        }
    }

    fn pump(&mut self, timeout: Duration) {
        let ready = self
            .io
            .wait(
                Interest::Read,
                Some(timeout.min(Duration::from_millis(100))),
            )
            .expect("wait");
        if ready.read == Ready::Later {
            return;
        }
        let mut buf = [0u8; 4096];
        loop {
            match self.io.read(&mut buf).expect("read") {
                ReadOutcome::Data(n) => self.out.extend_from_slice(&buf[..n]),
                ReadOutcome::WouldBlock => return self.answer_device_attributes(),
                ReadOutcome::Closed => {
                    self.closed = true;
                    return;
                }
            }
        }
    }
}

pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}
