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

use super::support::{bash_spec, scratch, screen, tools_present};

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
        if let Ok(PaneEvent::Transfer(event)) = events.recv_timeout(Duration::from_millis(50)) {
            return event;
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
    // The program offers (with the line discipline out of the way, so replies reach `read`), waits
    // for the terminal's OK (it ends in the backslash of `ESC \`; kitty's own client hears nothing
    // before the person decides), and sends `a.txt` holding "hello".
    let script = "\
stty -icanon -echo
printf '\\033]5113;ac=send;id=p1\\033\\\\'
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

/// How the script writes each sequence to the terminal.
#[derive(Clone, Copy)]
enum Writes {
    /// One `printf` per sequence.
    Whole,
    /// Each part's sequence in three writes with a pause between, so the pane's reader sees the
    /// introducer, the text and the terminator at different times.
    Pieces,
}

const MULTIPART_BODY: &str = "multipart body, byte for byte: 0123456789 \u{e9}\u{4e2d}";

/// A script that sends `MULTIPART_BODY` as `parts.txt` in a multipart file whose base64 text is
/// cut every `step` characters.
fn multipart_script(step: usize, writes: Writes) -> String {
    let part = match writes {
        Writes::Whole => {
            "printf '\\033]1337;FilePart=%s\\033\\\\' \"${b64:$i:$step}\"\n".to_owned()
        }
        Writes::Pieces => "printf '\\033]1337;File'; sleep 0.05\n\
printf 'Part=%s' \"${b64:$i:$step}\"; sleep 0.05\n\
printf '\\033'; sleep 0.05; printf '\\\\'\n"
            .to_owned(),
    };
    format!(
        "data='{MULTIPART_BODY}'\n\
b64=$(printf '%s' \"$data\" | base64 | tr -d '\\n')\n\
step={step}\n\
size=$(($(printf '%s' \"$data\" | wc -c)))\n\
printf '\\033]1337;MultipartFile=name=%s;size=%s\\033\\\\' \"$(printf parts.txt | base64 | tr -d '\\n')\" \"$size\"\n\
i=0\n\
while [ $i -lt ${{#b64}} ]; do\n\
{part}\
i=$((i + step))\n\
done\n\
printf '\\033]1337;FileEnd\\033\\\\'\n"
    )
}

#[test]
fn a_multipart_iterm_file_arrives_whole_however_it_is_cut() {
    if !tools_present() {
        return;
    }
    // Step 7 ends inside a base64 quartet; 4 on one; 1 is a character at a time; 4000 is one part.
    let rows = [
        (7, Writes::Whole),
        (4, Writes::Whole),
        (1, Writes::Whole),
        (4000, Writes::Whole),
        (7, Writes::Pieces),
    ];
    for (n, (step, writes)) in rows.into_iter().enumerate() {
        let home = scratch(&format!("xfer-multipart-{n}")).join("home");
        let (pane, events) = pane_with_transfers(&home);
        run_script(&pane, &home, &multipart_script(step, writes));
        let TransferEvent::Offered { offer, kind } = next_transfer(&events, "the offer") else {
            panic!("expected an offer at step {step}");
        };
        assert_eq!(
            kind,
            OfferKind::Inline {
                name: "parts.txt".to_owned(),
                size: MULTIPART_BODY.len() as u64
            },
            "step {step}"
        );
        assert!(!home.join("staging").exists(), "nothing is kept before yes");
        pane.decide(offer, Consent::Allow);
        loop {
            if let TransferEvent::Saved { path, .. } = next_transfer(&events, "the file") {
                assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("parts.txt"));
                assert_eq!(
                    std::fs::read(path).expect("staged"),
                    MULTIPART_BODY.as_bytes(),
                    "step {step}"
                );
                break;
            }
        }
    }
}

#[test]
fn a_multipart_end_or_part_without_a_start_makes_no_file_and_output_goes_on() {
    if !tools_present() {
        return;
    }
    let home = scratch("xfer-multipart-orphans").join("home");
    let (pane, events) = pane_with_transfers(&home);
    run_script(
        &pane,
        &home,
        "printf '\\033]1337;FileEnd\\033\\\\'\n\
printf '\\033]1337;FilePart=aGVsbG8=\\033\\\\'\n\
printf '\\033]1337;FileEnd\\033\\\\'\n\
echo STILL-HERE\n",
    );
    let start = Instant::now();
    let mut seen = Vec::new();
    while !screen(&pane).contains("STILL-HERE") {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "later output never showed; screen:\n{}",
            screen(&pane)
        );
        while let Ok(event) = events.recv_timeout(Duration::from_millis(20)) {
            seen.push(event);
        }
    }
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(
        !seen.iter().any(|event| matches!(
            event,
            PaneEvent::Transfer(TransferEvent::Offered { .. } | TransferEvent::Saved { .. })
        )),
        "no file from pieces that never began"
    );
    assert!(!home.join("staging").exists(), "nothing was written");
}
