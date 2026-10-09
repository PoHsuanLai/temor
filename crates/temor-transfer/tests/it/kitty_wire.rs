use temor_transfer::kitty::{
    Action, Compression, ErrorCode, FileId, FileType, KittyId, Message, Mode, Reply, ReplyStatus,
    Transmission, UnixNanos, encode_message, encode_reply, parse_message, readable_header,
};
use temor_transfer::{Malformed, TransferError};

use crate::support::limits;

fn id() -> KittyId {
    KittyId::parse("tx1").expect("valid id")
}

#[test]
fn a_message_round_trips_through_the_wire() {
    let message = Message {
        file: Some(FileId::parse("7").expect("fid")),
        parent: Some(FileId::parse("2").expect("pr")),
        name: Some("dir/a b.txt".to_owned()),
        file_type: Some(FileType::Regular),
        size: Some(temor_transfer::Bytes::new(5)),
        mtime: Some(UnixNanos::new(-42)),
        permissions: Some(Mode::new(0o644)),
        compression: Some(Compression::None),
        data: b"hello".to_vec(),
        ..Message::new(Action::EndData, id())
    };
    let wire = encode_message(&message);
    assert!(wire.starts_with(b"\x1b]5113;"));
    assert!(wire.ends_with(b"\x1b\\"));
    let payload = &wire[2..wire.len() - 2];
    assert_eq!(parse_message(payload, limits(64)).expect("parses"), message);
}

#[test]
fn short_and_long_keys_mean_the_same_and_unknown_keys_are_ignored() {
    let payload = b"5113;ac=file;id=tx1;fid=3;ft=directory;future=1;n=ZA==";
    let short = parse_message(payload, limits(64)).expect("short form");
    let long = parse_message(
        b"5113;action=file;id=tx1;file_id=3;file_type=directory;n=ZA==",
        limits(64),
    )
    .expect("long form");
    assert_eq!(short, long);
    assert_eq!(short.action, Action::File);
    assert_eq!(short.file_type, Some(FileType::Directory));
}

#[test]
fn a_missing_action_or_id_is_malformed() {
    assert!(matches!(
        parse_message(b"5113;id=tx1", limits(64)),
        Err(TransferError::Malformed(Malformed::MissingField {
            key: "ac"
        }))
    ));
    assert!(matches!(
        parse_message(b"5113;ac=send", limits(64)),
        Err(TransferError::Malformed(Malformed::MissingField {
            key: "id"
        }))
    ));
    assert!(matches!(
        parse_message(b"1337;File=", limits(64)),
        Err(TransferError::Malformed(Malformed::NotATransfer))
    ));
}

#[test]
fn an_unknown_action_parses_as_unknown() {
    let message = parse_message(b"5113;ac=frobnicate;id=tx1", limits(64)).expect("parses");
    assert_eq!(message.action, Action::Unknown);
}

#[test]
fn ids_must_be_safe_strings() {
    assert!(KittyId::parse("a b").is_err());
    assert!(KittyId::parse("").is_err());
    assert!(KittyId::parse(&"x".repeat(129)).is_err());
    assert!(FileId::parse("ok_1:2/3@4-5.6").is_ok());
    assert!(parse_message(b"5113;ac=send;id=a;b", limits(64)).is_ok());
}

#[test]
fn data_over_the_limit_is_too_large() {
    let message = Message {
        data: vec![7; 40],
        ..Message::new(Action::Data, id())
    };
    let wire = encode_message(&message);
    let payload = &wire[2..wire.len() - 2];
    assert!(matches!(
        parse_message(payload, limits(8)),
        Err(TransferError::TooLarge { .. })
    ));
}

#[test]
fn replies_carry_their_status_in_base64() {
    let refused = Reply {
        id: id(),
        file: None,
        status: ReplyStatus::Failed {
            code: ErrorCode::Eperm,
            message: "User refused the transfer".to_owned(),
        },
    };
    let wire = String::from_utf8(encode_reply(&refused)).expect("ascii");
    assert!(wire.starts_with("\x1b]5113;ac=status;id=tx1;st="));
    assert!(wire.ends_with("\x1b\\"));
    let ok = encode_reply(&Reply {
        id: id(),
        file: Some(FileId::parse("4").expect("fid")),
        status: ReplyStatus::Ok,
    });
    assert!(
        String::from_utf8(ok)
            .expect("ascii")
            .contains("fid=4;st=T0s=")
    );
}

#[test]
fn both_finish_spellings_are_accepted() {
    assert_eq!(Action::from_wire("finish"), Action::Finish);
    assert_eq!(Action::from_wire("finished"), Action::Finish);
    let message = parse_message(
        b"5113;ac=file;id=t;fid=1;ftype=directory;n=ZA==",
        limits(64),
    )
    .expect("parses");
    assert_eq!(message.file_type, Some(FileType::Directory));
}

#[test]
fn an_oversized_payload_is_refused_before_it_is_read() {
    let payload = vec![b'a'; 20_000];
    assert!(matches!(
        parse_message(&payload, limits(16)),
        Err(TransferError::TooLarge { .. })
    ));
}

#[test]
fn the_transmission_type_is_read() {
    let message = parse_message(b"5113;ac=send;id=a;tt=rsync", limits(64)).expect("parses");
    assert_eq!(message.transmission, Some(Transmission::Rsync));
}

#[test]
fn an_unknown_transmission_word_is_kept_for_a_reply() {
    let message = parse_message(b"5113;ac=send;id=a;tt=frobnicate", limits(64)).expect("parses");
    assert_eq!(
        message.transmission,
        Some(temor_transfer::kitty::Transmission::Other(
            "frobnicate".to_owned()
        ))
    );
}

#[test]
fn a_frame_that_does_not_parse_keeps_its_readable_id() {
    let bad = b"5113;ac=file;id=tx9;n=!!!!";
    assert!(parse_message(bad, limits(64)).is_err());
    assert_eq!(
        readable_header(bad, limits(64)).map(|id| id.as_str().to_owned()),
        Some("tx9".to_owned())
    );
    assert!(
        readable_header(b"5113;id=tx9", limits(64)).is_none(),
        "no action readable"
    );
    assert!(
        readable_header(b"5113;ac=file", limits(64)).is_none(),
        "no id readable"
    );
}
