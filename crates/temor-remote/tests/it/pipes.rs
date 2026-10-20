//! Pipes between a client and the in-process daemon, and a line that delays and slows them.

use std::io::{PipeReader, PipeWriter, Read, Write};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

/// One direction of the line.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub one_way: Duration,
    pub bytes_per_sec: u64,
}

/// Carries `from` to `to` as `line` would: each chunk leaves once the ones before it have gone
/// and its own bytes have had their time at the rate, and arrives `one_way` after that.
pub fn carry(
    mut from: impl Read + Send + 'static,
    mut to: impl Write + Send + 'static,
    line: Line,
) {
    let (tx, rx) = channel::<(Instant, Vec<u8>)>();
    thread::spawn(move || {
        let mut buf = vec![0u8; 16 << 10];
        let mut free_at = Instant::now();
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 {
                break;
            }
            let start = free_at.max(Instant::now());
            free_at = start + Duration::from_secs_f64(n as f64 / line.bytes_per_sec as f64);
            if tx
                .send((free_at + line.one_way, buf[..n].to_vec()))
                .is_err()
            {
                break;
            }
        }
    });
    thread::spawn(move || {
        for (due, bytes) in rx {
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                thread::sleep(wait);
            }
            if to.write_all(&bytes).and_then(|()| to.flush()).is_err() {
                break;
            }
        }
    });
}

/// One direction of a connection: the end to read from and the end to write to. Over `line` the
/// bytes written come out of the reader after its delay and at its rate.
pub fn one_way(line: Option<Line>) -> (PipeReader, PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("pipe");
    let Some(line) = line else {
        return (reader, writer);
    };
    let (slow_in, near_writer) = std::io::pipe().expect("pipe");
    carry(slow_in, writer, line);
    (reader, near_writer)
}
