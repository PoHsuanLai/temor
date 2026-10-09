//! The receive machine's core flow: consent, files, folders, names and links.

use temor_transfer::kitty::{
    Action, Effect, EndWhy, FileType, Input, Message, ReplyStatus, WriteOp,
};
use temor_transfer::{Bytes, Consent, RelPath};

use crate::receive_support::*;

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
