//! The sending side: scanning frames out of a stream, reading replies, the send machine's flow
//! and what `serve` offers from the shelf.

use std::path::PathBuf;

use temor_transfer::kitty::{
    Action, ChunkEnd, ErrorCode, FrameScanner, KittyId, Message, Offered, Reply, ReplyStatus,
    SendEffect, SendEnd, SendFile, SendInput, SendSession, UnixNanos, describe, encode_reply,
    parse_reply, read_chunk, serve,
};
use temor_transfer::{Bytes, FileName, TransferError};

use crate::support::{b64, limits};

fn id() -> KittyId {
    KittyId::parse("tx1").expect("id")
}

fn reply(file: Option<&str>, status: ReplyStatus) -> SendInput {
    SendInput::Reply(Reply {
        id: id(),
        file: file.map(|f| temor_transfer::kitty::FileId::parse(f).expect("fid")),
        status,
    })
}

fn emitted(effects: &[SendEffect]) -> Vec<(Action, Option<String>, usize)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            SendEffect::Emit(m) => Some((m.action, m.name.clone(), m.data.len())),
            _ => None,
        })
        .collect()
}

fn file(name: &str, size: u64) -> SendFile {
    SendFile {
        name: FileName::sanitise(name),
        size: Bytes::new(size),
        modified: Some(UnixNanos::new(7)),
    }
}

#[test]
fn the_scanner_finds_frames_however_the_stream_is_cut() {
    let one = b"\x1b]5113;ac=status;id=a\x1b\\".to_vec();
    let two = b"\x1b]5113;ac=status;id=b\x07".to_vec();
    let noise = b"hello\x1b]0;a title\x07\x1b[31m".to_vec();
    let stream: Vec<u8> = [noise.clone(), one, noise, two].concat();
    let want: Vec<Vec<u8>> = vec![
        b"5113;ac=status;id=a".to_vec(),
        b"5113;ac=status;id=b".to_vec(),
    ];
    for cut in 0..=stream.len() {
        let mut scanner = FrameScanner::new();
        let mut got = scanner.push(&stream[..cut]);
        got.extend(scanner.push(&stream[cut..]));
        assert_eq!(got, want, "cut at {cut}");
    }
    let mut long = b"\x1b]5113;".to_vec();
    long.extend(std::iter::repeat_n(b'a', 70_000));
    long.extend_from_slice(b"\x1b\\");
    assert!(FrameScanner::new().push(&long).is_empty());
}

#[test]
fn replies_read_back_what_was_written() {
    let fid = temor_transfer::kitty::FileId::parse("3").expect("fid");
    #[rustfmt::skip]
    let cases = [
        ("ok", None, ReplyStatus::Ok),
        ("started for a file", Some(fid.clone()), ReplyStatus::Started),
        ("cancelled", None, ReplyStatus::Cancelled),
        ("a known code", Some(fid), ReplyStatus::Failed { code: ErrorCode::Eperm, message: "User refused the transfer".into() }),
    ];
    for (name, file, status) in cases {
        let want = Reply {
            id: id(),
            file,
            status,
        };
        let wire = encode_reply(&want);
        let payload = FrameScanner::new().push(&wire).remove(0);
        assert_eq!(
            parse_reply(&payload, limits(1024)).expect(name),
            want,
            "{name}"
        );
    }
    let odd = format!("5113;ac=status;id=tx1;st={}", b64(b"ENOPE:what"));
    assert_eq!(
        parse_reply(odd.as_bytes(), limits(1024))
            .expect("odd")
            .status,
        ReplyStatus::Failed {
            code: ErrorCode::Eio,
            message: "ENOPE:what".into()
        }
    );
    for bad in [
        "5113;ac=send;id=tx1;st=T0s=",
        "5113;ac=status;id=tx1",
        "5113;ac=status;st=T0s=",
    ] {
        assert!(parse_reply(bad.as_bytes(), limits(1024)).is_err(), "{bad}");
    }
}

#[test]
fn a_send_offers_streams_each_file_and_finishes() {
    let (s, out) = SendSession::new(id(), vec![file("a.txt", 4), file("b.txt", 0)]);
    assert_eq!(emitted(&out), vec![(Action::Send, None, 0)]);
    let (s, out) = s.step(reply(None, ReplyStatus::Ok));
    assert_eq!(emitted(&out), vec![(Action::File, Some("a.txt".into()), 0)]);
    assert!(matches!(out[1], SendEffect::Read { file: 0, .. }));
    let (s, out) = s.step(SendInput::Chunk {
        data: b"ab".to_vec(),
        end: ChunkEnd::More,
    });
    assert_eq!(emitted(&out), vec![(Action::Data, None, 2)]);
    assert!(matches!(out[1], SendEffect::Read { file: 0, .. }));
    // A started reply or a per-file ok changes nothing.
    let (s, out) = s.step(reply(Some("1"), ReplyStatus::Started));
    assert!(out.is_empty());
    let (s, out) = s.step(SendInput::Chunk {
        data: b"cd".to_vec(),
        end: ChunkEnd::Last,
    });
    assert_eq!(
        emitted(&out),
        vec![
            (Action::EndData, None, 2),
            (Action::File, Some("b.txt".into()), 0)
        ]
    );
    let (s, out) = s.step(SendInput::Chunk {
        data: vec![],
        end: ChunkEnd::Last,
    });
    assert_eq!(
        emitted(&out),
        vec![(Action::EndData, None, 0), (Action::Finish, None, 0)]
    );
    let (s, out) = s.step(reply(Some("1"), ReplyStatus::Ok));
    assert!(out.is_empty());
    let (_, out) = s.step(reply(None, ReplyStatus::Ok));
    assert_eq!(out, vec![SendEffect::Ended(SendEnd::Delivered)]);
}

#[test]
fn a_send_ends_on_no_on_a_failed_file_and_on_abort() {
    let refusal = ReplyStatus::Failed {
        code: ErrorCode::Eperm,
        message: "User refused the transfer".into(),
    };
    let (s, _) = SendSession::new(id(), vec![file("a.txt", 1)]);
    let (_, out) = s.clone().step(reply(None, refusal.clone()));
    assert_eq!(
        out,
        vec![SendEffect::Ended(SendEnd::Refused {
            message: "User refused the transfer".into()
        })]
    );
    // A reply about another transfer is ignored.
    let other = SendInput::Reply(Reply {
        id: KittyId::parse("other").expect("id"),
        file: None,
        status: ReplyStatus::Ok,
    });
    assert!(s.clone().step(other).1.is_empty());
    let (streaming, _) = s.clone().step(reply(None, ReplyStatus::Ok));
    let (_, out) = streaming.clone().step(reply(Some("1"), refusal));
    assert_eq!(emitted(&out), vec![(Action::Cancel, None, 0)]);
    assert!(matches!(out[1], SendEffect::Ended(SendEnd::Failed { .. })));
    let (done, out) = streaming.step(SendInput::Aborted);
    assert_eq!(emitted(&out), vec![(Action::Cancel, None, 0)]);
    assert!(done.step(SendInput::Aborted).1.is_empty());
}

fn shelf() -> Vec<Offered> {
    let item = |name: &str, path: &str| Offered {
        name: FileName::sanitise(name),
        path: PathBuf::from(path),
        size: Bytes::new(3),
        modified: None,
    };
    vec![
        item("new.txt", "/d/new.txt"),
        item("old.txt", "/d/old (2).txt"),
    ]
}

fn request(name: Option<&str>) -> Message {
    Message {
        name: name.map(str::to_owned),
        ..Message::new(Action::Receive, id())
    }
}

#[test]
fn a_request_gets_the_newest_or_the_named_file_or_a_plain_no() {
    let path_of = |name: Option<&str>| serve(&request(name), &shelf()).map(|s| s.path);
    assert_eq!(path_of(None).ok(), Some(PathBuf::from("/d/new.txt")));
    assert_eq!(
        path_of(Some("old.txt")).ok(),
        Some(PathBuf::from("/d/old (2).txt"))
    );
    assert_eq!(
        path_of(Some("old (2).txt")).ok(),
        Some(PathBuf::from("/d/old (2).txt"))
    );
    let no = |request: Message, shelf: &[Offered]| match serve(&request, shelf) {
        Err(Reply {
            status: ReplyStatus::Failed { code, message },
            ..
        }) => (code, message),
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(
        no(request(Some("x")), &shelf()),
        (
            ErrorCode::Enoent,
            "no file with that name is on the shelf".into()
        )
    );
    assert_eq!(
        no(request(None), &[]),
        (ErrorCode::Enoent, "there is nothing on the shelf".into())
    );
    assert_eq!(
        no(Message::new(Action::Send, id()), &shelf()).0,
        ErrorCode::Einval
    );
    let served = serve(&request(None), &shelf()).expect("served");
    assert_eq!(emitted(&served.first), vec![(Action::Send, None, 0)]);
}

#[test]
fn files_are_described_and_read_in_chunks() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join(".hidden.txt");
    std::fs::write(&path, b"abcdefg").expect("write");
    let described = describe(&path).expect("described");
    assert_eq!(
        (described.name.as_str(), described.size),
        ("hidden.txt", Bytes::new(7))
    );
    assert!(described.modified.is_some());
    assert!(matches!(
        describe(dir.path()),
        Err(TransferError::NotAFile { .. })
    ));
    assert!(matches!(
        describe(&dir.path().join("nope")),
        Err(TransferError::Io { .. })
    ));
    let mut reader = std::fs::File::open(&path).expect("open");
    let mut got = Vec::new();
    loop {
        let (data, end) = read_chunk(&mut reader, Bytes::new(3), &path).expect("chunk");
        got.push((data, end));
        if end == ChunkEnd::Last {
            break;
        }
    }
    assert_eq!(
        got,
        vec![
            (b"abc".to_vec(), ChunkEnd::More),
            (b"def".to_vec(), ChunkEnd::More),
            (b"g".to_vec(), ChunkEnd::Last),
        ]
    );
}
