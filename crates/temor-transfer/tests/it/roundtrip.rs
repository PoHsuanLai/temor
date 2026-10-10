//! Whole transfers over an in-memory line: the drivers against a thread that plays the terminal,
//! and the OSC 1337 and kitty receive paths into the shelf.
#![cfg(target_os = "linux")]

use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread;
use std::time::Duration;

use temor_transfer::kitty::{
    Action, FrameScanner, Input, KittyId, Link, Message, Offered, ReceiveSession, Waits,
    apply_write, drive_receive, drive_send, parse_message, serve,
};
use temor_transfer::osc1337::{Completeness, Delivery, Osc1337Reader, ReceivedFile, encode_file};
use temor_transfer::shelf::{Clock, ShelfStore, UnixMillis};
use temor_transfer::{Bytes, Consent, FileName, HostName, Origin, Root, TransferError};

use crate::support::limits;

struct Chan {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

fn pair() -> (Chan, Chan) {
    let (a_tx, b_rx) = channel();
    let (b_tx, a_rx) = channel();
    (Chan { tx: a_tx, rx: a_rx }, Chan { tx: b_tx, rx: b_rx })
}

impl Link for Chan {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.tx
            .send(bytes.to_vec())
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    fn read(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        match self.rx.recv_timeout(timeout) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}

struct Fixed;

impl Clock for Fixed {
    fn now(&self) -> UnixMillis {
        UnixMillis::new(5_000)
    }
}

const WAITS: Waits = Waits {
    first: Duration::from_secs(10),
    next: Duration::from_secs(10),
};

fn host() -> Origin {
    Origin::Host {
        host: HostName::new("prod-1"),
    }
}

/// A terminal that takes whatever one client sends: the first frame starts a receive session
/// that says yes, and the files land in `dir`.
fn terminal_takes(mut link: Chan, dir: std::path::PathBuf) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let root = Root::open(&dir).expect("root");
        let mut session = ReceiveSession::new(host(), limits(1 << 20));
        let mut scanner = FrameScanner::new();
        while let Ok(Some(bytes)) = link.read(Duration::from_secs(5)) {
            for payload in scanner.push(&bytes) {
                let message = parse_message(&payload, limits(1 << 20)).expect("frame");
                let fid = message.file.clone();
                let mut inputs = vec![(Input::Frame(message), fid)];
                while let Some((input, fid)) = inputs.pop() {
                    let (next, effects) = session.step(input);
                    session = next;
                    for effect in effects {
                        use temor_transfer::kitty::{Effect, WriteOp};
                        match effect {
                            Effect::AskConsent(_) => {
                                inputs.push((Input::Decided(Consent::Allow), None));
                            }
                            Effect::Reply(reply) => {
                                link.write(&temor_transfer::kitty::encode_reply(&reply))
                                    .expect("reply");
                            }
                            Effect::Write(op) => {
                                apply_write(&root, &op).expect("write");
                                if let (WriteOp::Finish { .. }, Some(fid)) = (&op, fid.clone()) {
                                    inputs.push((Input::WriteDone { fid }, None));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    })
}

#[test]
fn temor_save_reaches_the_terminal_byte_for_byte() {
    let from = tempfile::tempdir().expect("from");
    let to = tempfile::tempdir().expect("to");
    let big: Vec<u8> = (0..20_000_u32).map(|n| (n % 251) as u8).collect();
    let (a, b) = (from.path().join("big.bin"), from.path().join("empty.txt"));
    std::fs::write(&a, &big).expect("big");
    std::fs::write(&b, b"").expect("empty");
    let (mut client, terminal) = pair();
    let handle = terminal_takes(terminal, to.path().to_path_buf());
    let id = KittyId::parse("save1").expect("id");
    drive_send(&mut client, id, &[a, b], limits(1 << 20), WAITS).expect("sent");
    drop(client);
    handle.join().expect("terminal");
    assert_eq!(std::fs::read(to.path().join("big.bin")).expect("big"), big);
    assert_eq!(
        std::fs::read(to.path().join("empty.txt")).expect("empty"),
        b""
    );
}

#[test]
fn a_silent_terminal_is_reported_as_no_answer() {
    let from = tempfile::tempdir().expect("from");
    let file = from.path().join("a.txt");
    std::fs::write(&file, b"x").expect("write");
    let (mut client, _terminal) = pair();
    let waits = Waits {
        first: Duration::from_millis(50),
        next: Duration::from_millis(50),
    };
    let id = KittyId::parse("save2").expect("id");
    let result = drive_send(&mut client, id, &[file], limits(1024), waits);
    assert!(matches!(result, Err(TransferError::NoAnswer)), "{result:?}");
}

/// A terminal that answers one `receive` request from `shelf`.
fn terminal_serves(mut link: Chan, shelf: Vec<Offered>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut scanner = FrameScanner::new();
        let Ok(Some(bytes)) = link.read(Duration::from_secs(5)) else {
            return;
        };
        for payload in scanner.push(&bytes) {
            let request = parse_message(&payload, limits(1024)).expect("request");
            match serve(&request, &shelf) {
                Ok(served) => {
                    drive_send(
                        &mut link,
                        request.id,
                        &[served.path],
                        limits(1 << 20),
                        WAITS,
                    )
                    .expect("served");
                }
                Err(reply) => link
                    .write(&temor_transfer::kitty::encode_reply(&reply))
                    .expect("no"),
            }
        }
    })
}

fn get_request(name: Option<&str>) -> Message {
    Message {
        name: name.map(str::to_owned),
        ..Message::new(Action::Receive, KittyId::parse("get1").expect("id"))
    }
}

#[test]
fn temor_get_pulls_a_file_from_the_shelf_into_the_folder() {
    let shelf_dir = tempfile::tempdir().expect("shelf");
    let here = tempfile::tempdir().expect("here");
    let path = shelf_dir.path().join("report (2).txt");
    std::fs::write(&path, b"quarterly").expect("write");
    let offered = Offered {
        name: FileName::sanitise("report.txt"),
        path,
        size: Bytes::new(9),
        modified: None,
    };
    let (mut client, terminal) = pair();
    let handle = terminal_serves(terminal, vec![offered]);
    let root = Root::open(here.path()).expect("root");
    let saved = drive_receive(
        &mut client,
        &root,
        &get_request(None),
        limits(1 << 20),
        WAITS,
    )
    .expect("received");
    handle.join().expect("terminal");
    assert_eq!(saved.len(), 1);
    assert_eq!(
        std::fs::read(here.path().join("report (2).txt")).expect("file"),
        b"quarterly"
    );
}

#[test]
fn temor_get_reports_the_terminals_no_in_its_words() {
    let here = tempfile::tempdir().expect("here");
    let (mut client, terminal) = pair();
    let handle = terminal_serves(terminal, Vec::new());
    let root = Root::open(here.path()).expect("root");
    let result = drive_receive(&mut client, &root, &get_request(None), limits(1024), WAITS);
    handle.join().expect("terminal");
    match result {
        Err(TransferError::Declined { message }) => {
            assert_eq!(message, "there is nothing on the shelf");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_1337_file_is_decoded_consented_and_shelved() {
    let dir = tempfile::tempdir().expect("dir");
    let store = ShelfStore::new(dir.path());
    let wire = encode_file("notes/a.txt", b"hello", Delivery::Offer);
    let payload = payload_of(&wire);
    let file = Osc1337Reader::new(limits(1024))
        .feed(&payload, Completeness::Whole)
        .expect("decodes");
    let Some(ReceivedFile::NeedsConsent(offer)) = file else {
        panic!("expected an offer, got {file:?}");
    };
    assert_eq!(offer.size(), temor_transfer::Bytes::new(5));
    assert!(offer.clone().consent(Consent::Refuse).is_none());
    let allowed = offer.consent(Consent::Allow).expect("allowed");
    let first = store
        .receive_consented(&allowed, host(), &Fixed)
        .expect("shelved");
    let second = store
        .receive_consented(&allowed, host(), &Fixed)
        .expect("again");
    assert_eq!(
        first.path.file_name().and_then(|n| n.to_str()),
        Some("notes_a.txt")
    );
    assert_eq!(
        second.path.file_name().and_then(|n| n.to_str()),
        Some("notes_a (2).txt")
    );
    assert_eq!(std::fs::read(&first.path).expect("read"), b"hello");
    let shelf = store.load(&Fixed).expect("index");
    assert_eq!(shelf.items().len(), 2);
    let mut ids: Vec<u64> = shelf.items().iter().map(|item| item.id.get()).collect();
    ids.sort_unstable();
    assert_eq!(ids, [1, 2]);
}

#[test]
fn a_1337_inline_flag_survives_and_a_cut_file_is_refused() {
    let wire = encode_file("p.png", b"png", Delivery::Inline);
    let payload = payload_of(&wire);
    let got = Osc1337Reader::new(limits(1024)).feed(&payload, Completeness::Whole);
    assert!(matches!(got, Ok(Some(ReceivedFile::Inline(_)))), "{got:?}");
    let cut = Osc1337Reader::new(limits(1024)).feed(&payload, Completeness::Cut);
    assert!(matches!(cut, Err(TransferError::Cut)), "{cut:?}");
    let small = Osc1337Reader::new(limits(2)).feed(&payload, Completeness::Whole);
    assert!(
        matches!(small, Err(TransferError::TooLarge { .. })),
        "{small:?}"
    );
}

#[test]
fn a_kitty_file_moves_from_the_transfer_folder_to_the_shelf() {
    let scratch = tempfile::tempdir().expect("scratch");
    let shelf_dir = tempfile::tempdir().expect("shelf");
    let from = tempfile::tempdir().expect("from");
    let file = from.path().join("pic.png");
    std::fs::write(&file, b"png").expect("write");
    let (mut client, terminal) = pair();
    let handle = terminal_takes(terminal, scratch.path().to_path_buf());
    let id = KittyId::parse("save3").expect("id");
    drive_send(&mut client, id, &[file], limits(1024), WAITS).expect("sent");
    drop(client);
    handle.join().expect("terminal");
    let saved = temor_transfer::kitty::SavedFile {
        path: temor_transfer::RelPath::parse("pic.png").expect("path"),
        size: Bytes::new(3),
    };
    let store = ShelfStore::new(shelf_dir.path());
    let item = store
        .receive_saved(scratch.path(), &saved, host(), &Fixed)
        .expect("shelved");
    assert_eq!(std::fs::read(&item.path).expect("read"), b"png");
    assert!(!scratch.path().join("pic.png").exists());
}

/// Cuts the payload out of one complete `ESC ] ... ESC \` sequence.
fn payload_of(wire: &[u8]) -> Vec<u8> {
    wire.strip_prefix(b"\x1b]")
        .and_then(|rest| rest.strip_suffix(b"\x1b\\"))
        .expect("one sequence")
        .to_vec()
}
