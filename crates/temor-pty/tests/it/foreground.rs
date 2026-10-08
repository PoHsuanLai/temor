//! The foreground process group follows a command in the foreground.

use std::time::{Duration, Instant};

use temor_pty::{Foreground, ProcRoot};

use crate::support::{Scratch, Term, bash_interactive, setsid};

fn wait_for(term: &Term, proc: &ProcRoot, want: impl Fn(&Foreground) -> bool) -> Foreground {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let now = term.pty.foreground(proc).expect("foreground");
        if want(&now) || Instant::now() > deadline {
            return now;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn foreground_follows_sleep() {
    if crate::support::pty_unsupported("foreground_follows_sleep") {
        return;
    }
    let scratch = Scratch::new();
    let proc = ProcRoot("/proc".into());
    let mut term = Term::spawn(&bash_interactive(&scratch), &setsid());
    assert!(term.read_until_has(0, "$ "), "no prompt: {:?}", term.text());
    let shell = term.pty.pid();
    let idle = wait_for(&term, &proc, |fg| fg.group == shell);
    assert_eq!(idle.group, shell);
    assert_eq!(idle.name.as_deref(), Some("bash"));

    term.send("sleep 1\r");
    // The forked child takes the foreground just before it execs, so for a moment the new
    // group's leader is still named bash.
    let busy = wait_for(&term, &proc, |fg| {
        fg.group != shell && fg.name.as_deref() == Some("sleep")
    });
    assert_ne!(busy.group, shell, "sleep never took the foreground");
    assert_eq!(busy.name.as_deref(), Some("sleep"));
    let cwd = scratch.cwd.canonicalize().expect("cwd");
    assert_eq!(busy.cwd.as_deref(), Some(cwd.as_path()));

    let back = wait_for(&term, &proc, |fg| fg.group == shell);
    assert_eq!(
        back.group, shell,
        "the shell never took the foreground back"
    );
}
