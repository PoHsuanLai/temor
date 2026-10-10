//! The pane's transfer state without a terminal: sequences go in as the payloads temor-vt
//! reports, and the replies and events come out. Files land in a scratch staging folder.

use temor_transfer::kitty::{
    Action, FileId, FileType, KittyId, Message, ReplyStatus, Transmission, encode_message,
    parse_reply,
};
use temor_transfer::osc1337::{Delivery, encode_file};
use temor_transfer::{Bytes, Consent, Count, Limits};
use temor_vt::{Truncation, UnknownKind};

use super::event::{OfferId, OfferKind, TransferEnd, TransferEvent, TransferSetup};
use super::state::{Reaction, Transfers};

fn limits() -> Limits {
    Limits::new(Bytes::new(1 << 20), Count::new(100), Count::new(8))
}

fn transfers(dir: &std::path::Path) -> Transfers {
    Transfers::new(TransferSetup {
        staging: dir.join("staging"),
        limits: limits(),
    })
}

/// The payload temor-vt reports for a sequence: everything between `ESC ]` and the terminator.
fn payload(sequence: &[u8]) -> Vec<u8> {
    sequence[2..sequence.len() - 2].to_vec()
}

fn kitty(t: &mut Transfers, message: &Message) -> Reaction {
    t.on_unknown(
        UnknownKind::Osc,
        &payload(&encode_message(message)),
        Truncation::Complete,
    )
}

fn id() -> KittyId {
    KittyId::parse("tx1").expect("id")
}

fn fid() -> FileId {
    FileId::parse("1").expect("fid")
}

fn statuses(reaction: &Reaction) -> Vec<ReplyStatus> {
    reaction
        .replies
        .iter()
        .map(|bytes| {
            parse_reply(&payload(bytes), limits())
                .expect("a status")
                .status
        })
        .collect()
}

fn offered(t: &mut Transfers) -> OfferId {
    let offer = Message {
        transmission: Some(Transmission::Simple),
        ..Message::new(Action::Send, id())
    };
    let reaction = kitty(t, &offer);
    assert_eq!(statuses(&reaction), vec![ReplyStatus::Started]);
    match reaction.events.as_slice() {
        [TransferEvent::Offered { offer, kind }] => {
            assert_eq!(*kind, OfferKind::Files);
            *offer
        }
        other => panic!("expected one offer, got {other:?}"),
    }
}

fn file_frames(name: &str, data: &[u8]) -> [Message; 3] {
    [
        Message {
            file: Some(fid()),
            name: Some(name.to_owned()),
            file_type: Some(FileType::Regular),
            size: Some(Bytes::new(data.len() as u64)),
            ..Message::new(Action::File, id())
        },
        Message {
            file: Some(fid()),
            data: data.to_vec(),
            ..Message::new(Action::EndData, id())
        },
        Message::new(Action::Finish, id()),
    ]
}

#[test]
fn a_kitty_offer_waits_for_yes_then_stages_the_file_and_reports_it() {
    let dir = tempfile::tempdir().expect("scratch");
    let mut t = transfers(dir.path());
    let offer = offered(&mut t);
    // Frames before the answer change nothing: the file is not even named yet.
    let [meta, data, finish] = file_frames("a.txt", b"hello");
    assert!(kitty(&mut t, &meta).events.is_empty());
    assert!(!dir.path().join("staging").exists());

    let allowed = t.decide(offer, Consent::Allow);
    assert_eq!(statuses(&allowed), vec![ReplyStatus::Ok]);

    let started = kitty(&mut t, &meta);
    assert!(matches!(
        started.events.as_slice(),
        [TransferEvent::Started { name, size: Some(5), .. }] if name == "a.txt"
    ));
    let sent = kitty(&mut t, &data);
    let saved: Vec<_> = sent
        .events
        .iter()
        .filter_map(|event| match event {
            TransferEvent::Saved { path, size, .. } => Some((path.clone(), *size)),
            _ => None,
        })
        .collect();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].1, 5);
    assert_eq!(std::fs::read(&saved[0].0).expect("staged"), b"hello");

    let done = kitty(&mut t, &finish);
    assert!(done.events.contains(&TransferEvent::Ended {
        offer,
        how: TransferEnd::Finished
    }));
}

#[test]
fn a_refused_offer_writes_nothing_and_tells_the_program() {
    let dir = tempfile::tempdir().expect("scratch");
    let mut t = transfers(dir.path());
    let offer = offered(&mut t);
    let refused = t.decide(offer, Consent::Refuse);
    assert!(matches!(
        statuses(&refused).as_slice(),
        [ReplyStatus::Failed { message, .. }] if message == "User refused the transfer"
    ));
    assert!(refused.events.contains(&TransferEvent::Ended {
        offer,
        how: TransferEnd::Refused
    }));
    assert!(!dir.path().join("staging").exists());
}

#[test]
fn an_iterm_file_is_kept_back_until_yes_and_not_at_all_after_no() {
    let dir = tempfile::tempdir().expect("scratch");
    let mut t = transfers(dir.path());
    let sequence = encode_file("note.txt", b"inline body", Delivery::Offer);
    let offer_of = |t: &mut Transfers| {
        let reaction = t.on_unknown(
            UnknownKind::Osc1337File,
            &payload(&sequence),
            Truncation::Complete,
        );
        match reaction.events.as_slice() {
            [
                TransferEvent::Offered {
                    offer,
                    kind: OfferKind::Inline { name, size: 11 },
                },
            ] if name == "note.txt" => *offer,
            other => panic!("expected an inline offer, got {other:?}"),
        }
    };
    let no = offer_of(&mut t);
    t.decide(no, Consent::Refuse);
    assert!(!dir.path().join("staging").exists());

    let yes = offer_of(&mut t);
    let allowed = t.decide(yes, Consent::Allow);
    let path = allowed
        .events
        .iter()
        .find_map(|event| match event {
            TransferEvent::Saved { path, .. } => Some(path.clone()),
            _ => None,
        })
        .expect("saved");
    assert_eq!(std::fs::read(path).expect("staged"), b"inline body");
}

#[test]
fn a_cut_iterm_file_is_unreadable_and_asks_nothing() {
    let dir = tempfile::tempdir().expect("scratch");
    let mut t = transfers(dir.path());
    let sequence = encode_file("big.bin", b"abc", Delivery::Offer);
    let reaction = t.on_unknown(
        UnknownKind::Osc1337File,
        &payload(&sequence),
        Truncation::Truncated,
    );
    assert!(matches!(
        reaction.events.as_slice(),
        [TransferEvent::Unreadable { .. }]
    ));
}

#[test]
fn offers_beyond_the_most_at_once_are_refused_not_queued() {
    let dir = tempfile::tempdir().expect("scratch");
    let mut t = transfers(dir.path());
    for n in 0..super::state::MOST_AT_ONCE {
        let message = Message::new(Action::Send, KittyId::parse(&format!("t{n}")).expect("id"));
        assert_eq!(kitty(&mut t, &message).events.len(), 1);
    }
    let one_more = kitty(
        &mut t,
        &Message::new(Action::Send, KittyId::parse("late").expect("id")),
    );
    assert!(one_more.events.is_empty());
    assert!(matches!(
        statuses(&one_more).as_slice(),
        [ReplyStatus::Failed { .. }]
    ));
}

#[test]
fn a_request_is_served_from_the_shelf_only_after_yes() {
    let dir = tempfile::tempdir().expect("scratch");
    let shelf_file = dir.path().join("report.txt");
    std::fs::write(&shelf_file, b"quarterly").expect("file");
    let shelf = [temor_transfer::kitty::Offered {
        name: temor_transfer::FileName::sanitise("report.txt"),
        path: shelf_file,
        size: Bytes::new(9),
        modified: None,
    }];
    let mut t = transfers(dir.path());
    let wanted = kitty(&mut t, &Message::new(Action::Receive, id()));
    let offer = match wanted.events.as_slice() {
        [TransferEvent::Wanted { offer, name: None }] => *offer,
        other => panic!("expected a request, got {other:?}"),
    };
    assert!(wanted.replies.is_empty());
    let served = t.serve(offer, Consent::Allow, &shelf);
    // The offer goes out as a send frame under the request's id.
    assert!(!served.replies.is_empty());
    assert!(
        served.replies[0]
            .windows(b"ac=send".len())
            .any(|w| w == b"ac=send")
    );
}
