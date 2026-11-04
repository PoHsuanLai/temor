//! A thread waiting on a quiet terminal can be woken from another, once per wake.

use std::time::{Duration, Instant};

use temor_pty::{Interest, Wake};

use crate::support::{Scratch, Term, bash, setsid};

#[test]
fn a_wake_ends_a_wait_on_a_quiet_terminal_and_is_used_up() {
    if crate::support::pty_unsupported("a_wake_ends_a_wait_on_a_quiet_terminal_and_is_used_up") {
        return;
    }
    let scratch = Scratch::new();
    let mut term = Term::spawn(&bash(&scratch, &["-c", "sleep 100"]), &setsid());
    let io = term.pty.io().expect("io");
    let wake = Wake::new().expect("wake");
    let waker = wake.waker();
    let woke = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        waker.wake();
    });

    let start = Instant::now();
    io.wait_or(Interest::Read, &wake, Some(Duration::from_secs(20)))
        .expect("wait");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "woken, not timed out"
    );
    woke.join().expect("waker");

    // Two wakes before a wait are both taken by it: the next waits out its time.
    let waker = wake.waker();
    waker.wake();
    waker.wake();
    io.wait_or(Interest::Read, &wake, Some(Duration::from_secs(20)))
        .expect("wait");
    let start = Instant::now();
    io.wait_or(Interest::Read, &wake, Some(Duration::from_millis(200)))
        .expect("wait");
    assert!(start.elapsed() >= Duration::from_millis(200));
    term.pty.kill().expect("kill");
}
