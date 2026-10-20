//! Killing the process group and reaping the leader leaves nothing behind.

use std::time::Duration;

use temor_pty::{ChildExit, PtyError, SignalKind};
use temor_testkit::eventually;

use crate::support::{Scratch, Term, bash, proc_alive, setsid};

#[test]
fn kill_reaps_the_leader_and_ends_the_group() {
    if crate::support::pty_unsupported("kill_reaps_the_leader_and_ends_the_group") {
        return;
    }
    let scratch = Scratch::new();
    let spec = bash(&scratch, &["-c", "sleep 100 & echo \"job=$!.\"; wait"]);
    let mut term = Term::spawn(&spec, &setsid());
    assert!(term.read_until_has(0, "."), "{:?}", term.text());
    let text = term.text();
    let job: i32 = text
        .split("job=")
        .nth(1)
        .and_then(|rest| rest.split('.').next())
        .and_then(|pid| pid.parse().ok())
        .expect("background job pid");
    let leader = term.pty.pid().0;
    assert!(proc_alive(leader) && proc_alive(job));
    assert_eq!(term.pty.try_exit().expect("try_exit"), None);

    let exit = term.pty.kill().expect("kill");
    assert_eq!(exit, ChildExit::Signaled(9));
    // Reaped at once: no zombie under our pid.
    assert!(!proc_alive(leader), "leader {leader} is still in /proc");
    // The job got the same signal; whoever adopted it reaps it.
    eventually(
        &format!("job {job} to die with the group kill"),
        Duration::from_secs(5),
        || !proc_alive(job),
    );

    assert_eq!(term.pty.try_exit().expect("try_exit"), Some(exit));
    assert!(matches!(
        term.pty.signal_group(SignalKind::Hangup),
        Err(PtyError::Reaped)
    ));
    assert!(term.read_to_close(), "the terminal did not report closing");
}

#[test]
fn exit_status_is_reported() {
    if crate::support::pty_unsupported("exit_status_is_reported") {
        return;
    }
    let scratch = Scratch::new();
    let mut term = Term::spawn(&bash(&scratch, &["-c", "exit 7"]), &setsid());
    assert!(term.read_to_close());
    assert_eq!(term.pty.wait_exit().expect("wait"), ChildExit::Exited(7));
}
