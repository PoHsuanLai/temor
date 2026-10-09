use temor_transfer::kitty::{
    Action, Consent, Effect, EndWhy, FileId, FileType, Input, KittyId, Message, ReceiveSession,
    ReplyStatus, WriteOp, apply_write,
};
use temor_transfer::{Bytes, HostName, Origin, RelPath, TransferError};

use crate::support::limits;

fn origin() -> Origin {
    Origin::Host {
        host: HostName::new("prod-1"),
    }
}

fn session(max: u64) -> ReceiveSession {
    ReceiveSession::new(origin(), limits(max))
}

fn msg(action: Action) -> Message {
    Message::new(action, KittyId::parse("tx1").expect("id"))
}

fn fid(raw: &str) -> FileId {
    FileId::parse(raw).expect("fid")
}

fn send(s: ReceiveSession) -> (ReceiveSession, Vec<Effect>) {
    s.step(Input::Frame(msg(Action::Send)))
}

fn allowed(max: u64) -> ReceiveSession {
    let (s, _) = send(session(max));
    let (s, _) = s.step(Input::Decided(Consent::Allow));
    s
}

fn file(name: &str, id: &str, kind: FileType, size: Option<u64>) -> Input {
    Input::Frame(Message {
        file: Some(fid(id)),
        name: Some(name.to_owned()),
        file_type: Some(kind),
        size: size.map(Bytes::new),
        ..msg(Action::File)
    })
}

fn writes(effects: &[Effect]) -> Vec<WriteOp> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Write(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

fn replies(effects: &[Effect]) -> Vec<ReplyStatus> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Reply(reply) => Some(reply.status.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_offer_asks_for_consent_from_its_origin_and_writes_nothing() {
    let (_, effects) = send(session(1024));
    assert_eq!(effects, vec![Effect::AskConsent(origin())]);
}

#[test]
fn frames_before_consent_are_held_back() {
    let (s, _) = send(session(1024));
    let (_, effects) = s.step(file("a.txt", "1", FileType::Regular, Some(1)));
    assert!(effects.is_empty());
}

#[test]
fn refusal_replies_with_permission_denied_and_ends_without_writes() {
    let (s, _) = send(session(1024));
    let (_, effects) = s.step(Input::Decided(Consent::Refuse));
    assert!(writes(&effects).is_empty());
    assert!(effects.contains(&Effect::Ended(EndWhy::Refused)));
    assert_eq!(replies(&effects).len(), 1);
    assert!(matches!(
        &replies(&effects)[0],
        ReplyStatus::Failed { message, .. } if message == "User refused the transfer"
    ));
}

#[test]
fn a_regular_file_is_created_appended_finished_and_saved() {
    let s = allowed(1024);
    let (s, effects) = s.step(file("a.txt", "1", FileType::Regular, Some(5)));
    assert_eq!(
        writes(&effects),
        vec![WriteOp::CreateFile(RelPath::parse("a.txt").expect("path"))]
    );
    assert_eq!(replies(&effects), vec![ReplyStatus::Started]);

    let last = Input::Frame(Message {
        file: Some(fid("1")),
        data: b"hello".to_vec(),
        ..msg(Action::EndData)
    });
    let (_, effects) = s.step(last);
    let path = RelPath::parse("a.txt").expect("path");
    assert_eq!(
        writes(&effects),
        vec![
            WriteOp::Append {
                path: path.clone(),
                data: b"hello".to_vec()
            },
            WriteOp::Finish {
                path: path.clone(),
                modified: None
            },
        ]
    );
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Saved(saved) if saved.path == path && saved.size == Bytes::new(5)
    )));
}

#[test]
fn chunks_arrive_in_pieces_before_the_last() {
    let s = allowed(1024);
    let (s, _) = s.step(file("b.bin", "9", FileType::Regular, None));
    let (s, first) = s.step(Input::Frame(Message {
        file: Some(fid("9")),
        data: b"ab".to_vec(),
        ..msg(Action::Data)
    }));
    assert_eq!(writes(&first).len(), 1);
    let (_, last) = s.step(Input::Frame(Message {
        file: Some(fid("9")),
        data: b"cd".to_vec(),
        ..msg(Action::EndData)
    }));
    assert!(
        last.iter()
            .any(|effect| matches!(effect, Effect::Saved(saved) if saved.size == Bytes::new(4)))
    );
}

#[test]
fn a_folder_and_its_children_land_under_it() {
    let s = allowed(1024);
    let (s, effects) = s.step(file("docs", "1", FileType::Directory, None));
    assert_eq!(
        writes(&effects),
        vec![WriteOp::MakeDir(RelPath::parse("docs").expect("p"))]
    );
    let child = Input::Frame(Message {
        file: Some(fid("2")),
        parent: Some(fid("1")),
        name: Some("note.txt".to_owned()),
        file_type: Some(FileType::Regular),
        size: Some(Bytes::new(0)),
        ..msg(Action::File)
    });
    let (s, effects) = s.step(child);
    assert_eq!(
        writes(&effects),
        vec![WriteOp::CreateFile(
            RelPath::parse("docs/note.txt").expect("p")
        )]
    );
    let (_, effects) = s.step(Input::Frame(Message {
        file: Some(fid("2")),
        ..msg(Action::EndData)
    }));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Saved(saved) if saved.path == RelPath::parse("docs/note.txt").expect("p")
    )));
}

#[test]
fn traversal_names_are_refused_and_never_written() {
    for hostile in [
        "../evil",
        "/etc/passwd",
        "a/../../b",
        "a//b",
        "a\\..\\b",
        "./x",
        "..",
    ] {
        let s = allowed(1024);
        let (s, effects) = s.step(file(hostile, "1", FileType::Regular, Some(1)));
        assert!(writes(&effects).is_empty(), "{hostile} must not be written");
        assert!(
            matches!(
                &replies(&effects)[0],
                ReplyStatus::Failed { code, .. } if code.code() == "EPERM"
            ),
            "{hostile} must be refused"
        );
        let (_, effects) = s.step(Input::Frame(Message {
            file: Some(fid("1")),
            data: b"x".to_vec(),
            ..msg(Action::EndData)
        }));
        assert!(
            writes(&effects).is_empty(),
            "{hostile} must not receive data"
        );
    }
}

#[test]
fn an_unknown_parent_is_refused() {
    let s = allowed(1024);
    let (_, effects) = s.step(Input::Frame(Message {
        file: Some(fid("5")),
        parent: Some(fid("404")),
        name: Some("x".to_owned()),
        file_type: Some(FileType::Regular),
        ..msg(Action::File)
    }));
    assert!(writes(&effects).is_empty());
    assert!(
        matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "ENOENT")
    );
}

#[test]
fn links_are_refused_and_nothing_is_created() {
    for kind in [FileType::Symlink, FileType::Link] {
        let s = allowed(1024);
        let (_, effects) = s.step(file("l", "1", kind, None));
        assert!(writes(&effects).is_empty());
        assert!(
            matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "EPERM")
        );
    }
}

#[test]
fn an_announced_size_over_the_limit_is_refused_before_any_write() {
    let s = allowed(10);
    let (s, effects) = s.step(file("big", "1", FileType::Regular, Some(11)));
    assert!(writes(&effects).is_empty());
    assert!(
        matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "EFBIG")
    );
    // The session keeps going: a smaller file still arrives.
    let (_, effects) = s.step(file("small", "2", FileType::Regular, Some(3)));
    assert_eq!(writes(&effects).len(), 1);
}

#[test]
fn data_past_the_limit_fails_the_session_and_discards_partial_files() {
    let s = allowed(4);
    let (s, _) = s.step(file("p", "1", FileType::Regular, None));
    let (s, _) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"abc".to_vec(),
        ..msg(Action::Data)
    }));
    let (s, effects) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"defg".to_vec(),
        ..msg(Action::EndData)
    }));
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("p").expect("p"))]
    );
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
    // A finished session takes nothing more.
    let (_, after) = s.step(file("q", "2", FileType::Regular, Some(1)));
    assert!(after.is_empty());
}

#[test]
fn a_size_that_does_not_match_the_announcement_fails_the_file() {
    let s = allowed(1024);
    let (s, _) = s.step(file("s", "1", FileType::Regular, Some(9)));
    let (_, effects) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"abc".to_vec(),
        ..msg(Action::EndData)
    }));
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("s").expect("p"))]
    );
}

#[test]
fn cancel_discards_open_files_and_says_so() {
    let s = allowed(1024);
    let (s, _) = s.step(file("c", "1", FileType::Regular, None));
    let (_, effects) = s.step(Input::Frame(msg(Action::Cancel)));
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("c").expect("p"))]
    );
    assert!(effects.contains(&Effect::Ended(EndWhy::Cancelled)));
    assert_eq!(replies(&effects), vec![ReplyStatus::Cancelled]);
}

#[test]
fn data_for_an_unknown_file_gets_a_reply_and_no_write() {
    let s = allowed(1024);
    let (s, effects) = s.step(Input::Frame(Message {
        file: Some(fid("77")),
        data: b"x".to_vec(),
        ..msg(Action::Data)
    }));
    assert!(writes(&effects).is_empty());
    assert!(
        matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "ENOENT")
    );
    let (_, effects) = s.step(Input::Frame(msg(Action::Unknown)));
    assert!(
        matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "EINVAL")
    );
}

#[test]
fn applying_writes_creates_files_under_the_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rel = RelPath::parse("sub/out.txt").expect("path");
    apply_write(
        dir.path(),
        &WriteOp::MakeDir(RelPath::parse("sub").expect("p")),
    )
    .expect("dir");
    apply_write(dir.path(), &WriteOp::CreateFile(rel.clone())).expect("create");
    apply_write(
        dir.path(),
        &WriteOp::Append {
            path: rel.clone(),
            data: b"hi".to_vec(),
        },
    )
    .expect("append");
    apply_write(
        dir.path(),
        &WriteOp::Finish {
            path: rel.clone(),
            modified: None,
        },
    )
    .expect("finish");
    assert_eq!(
        std::fs::read(dir.path().join("sub/out.txt")).expect("read"),
        b"hi"
    );
    assert!(matches!(
        apply_write(dir.path(), &WriteOp::CreateFile(rel)),
        Err(TransferError::Io { .. })
    ));
}

#[cfg(unix)]
#[test]
fn symlinks_already_on_disk_are_never_followed() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).expect("symlink");
    let through = RelPath::parse("link/planted.txt").expect("path");
    assert!(matches!(
        apply_write(root.path(), &WriteOp::CreateFile(through)),
        Err(TransferError::UnsafePath { .. })
    ));
    assert!(!outside.path().join("planted.txt").exists());
    assert!(matches!(
        apply_write(
            root.path(),
            &WriteOp::Discard(RelPath::parse("link").expect("p"))
        ),
        Err(TransferError::UnsafePath { .. })
    ));
}
