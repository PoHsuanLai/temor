//! Files a program sends through the terminal, end to end on a real PTY: a script prints the
//! sequences, the pane raises the offer, the answer lets the file in, and it lands in staging.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use temor_pane::{
    OfferKind, PaneEvent, PaneHandle, TransferEnd, TransferEvent, TransferSetup, start,
};
use temor_transfer::{Bytes, Consent, Count, Limits};
use temor_vt::{CellPx, GridSize};

use super::support::{bash_spec, scratch, tools_present};

/// A pane in `home` with transfers staged under `home/staging`, and its events.
fn pane_with_transfers(home: &Path) -> (PaneHandle, Receiver<PaneEvent>) {
    let (tx, events) = channel();
    let mut spec = bash_spec(home, GridSize::new(80, 24), CellPx::new(8, 16));
    spec.transfers = Some(TransferSetup {
        staging: home.join("staging"),
        limits: Limits::new(Bytes::new(1 << 20), Count::new(100), Count::new(8)),
    });
    let (pane, runner) = start(spec, Arc::new(tx)).expect("pane starts");
    std::thread::spawn(move || runner.run());
    (pane, events)
}

/// The next transfer event, skipping the rest.
fn next_transfer(events: &Receiver<PaneEvent>, what: &str) -> TransferEvent {
    let start = Instant::now();
    loop {
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(PaneEvent::Transfer(event)) => return event,
            Ok(_) | Err(_) => {}
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
    }
}

/// Runs `script` in the pane's shell, after waiting for its prompt.
fn run_script(pane: &PaneHandle, home: &Path, script: &str) {
    let path: PathBuf = home.join("send.sh");
    std::fs::write(&path, script).expect("script");
    pane.send(format!("bash {}\r", path.display()).as_bytes());
}

#[test]
fn an_iterm_file_is_offered_and_staged_once_allowed() {
    if !tools_present() {
        return;
    }
    let home = scratch("xfer-iterm").join("home");
    let (pane, events) = pane_with_transfers(&home);
    // "note.txt" and "hello" in base64.
    run_script(
        &pane,
        &home,
        "printf '\\033]1337;File=name=bm90ZS50eHQ=;size=5:aGVsbG8=\\033\\\\'\n",
    );
    let TransferEvent::Offered { offer, kind } = next_transfer(&events, "the offer") else {
        panic!("expected an offer");
    };
    assert_eq!(
        kind,
        OfferKind::Inline {
            name: "note.txt".to_owned(),
            size: 5
        }
    );
    assert!(!home.join("staging").exists(), "nothing is kept before yes");
    pane.decide(offer, Consent::Allow);
    loop {
        if let TransferEvent::Saved { path, size, .. } = next_transfer(&events, "the file") {
            assert_eq!(size, 5);
            assert_eq!(std::fs::read(path).expect("staged"), b"hello");
            return;
        }
    }
}

#[test]
fn a_kitty_transfer_waits_for_yes_then_receives_its_files() {
    if !tools_present() {
        return;
    }
    let home = scratch("xfer-kitty").join("home");
    let (pane, events) = pane_with_transfers(&home);
    // The program offers (with the line discipline out of the way, so replies reach `read`), waits for the terminal's STARTED and then its OK (each ends in the
    // backslash of `ESC \`), and sends `a.txt` holding "hello".
    let script = "\
stty -icanon -echo
printf '\\033]5113;ac=send;id=p1\\033\\\\'
read -r -d '\\' -t 10 started
read -r -d '\\' -t 10 answer
printf '\\033]5113;ac=file;id=p1;fid=1;n=YS50eHQ=;ft=regular;sz=5\\033\\\\'
printf '\\033]5113;ac=end_data;id=p1;fid=1;d=aGVsbG8=\\033\\\\'
printf '\\033]5113;ac=finish;id=p1\\033\\\\'
";
    run_script(&pane, &home, script);
    let TransferEvent::Offered { offer, kind } = next_transfer(&events, "the offer") else {
        panic!("expected an offer");
    };
    assert_eq!(kind, OfferKind::Files);
    pane.decide(offer, Consent::Allow);
    let mut saved = None;
    loop {
        match next_transfer(&events, "the transfer's end") {
            TransferEvent::Saved { path, .. } => saved = Some(path),
            TransferEvent::Ended { how, .. } => {
                assert_eq!(how, TransferEnd::Finished);
                break;
            }
            _ => {}
        }
    }
    let path = saved.expect("a saved file");
    assert_eq!(std::fs::read(path).expect("staged"), b"hello");
}
