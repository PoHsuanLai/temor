//! The receive machine's core flow: consent, files, folders, names and links.

use temor_transfer::kitty::{
    Action, Effect, EndWhy, FileType, Input, LinkPolicy, Message, Mode, ReplyStatus, WriteOp,
};
use temor_transfer::{Bytes, Consent, RelPath};

use crate::receive_support::*;

#[test]
fn an_offer_is_answered_started_then_asks_for_consent_and_writes_nothing() {
    let (_, effects) = send(session(1024));
    assert_eq!(replies(&effects), vec![ReplyStatus::Started]);
    assert_eq!(effects.last(), Some(&Effect::AskConsent(origin())));
    assert!(writes(&effects).is_empty());
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
    let (s, effects) = s.step(last);
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
                modified: None,
                mode: None
            },
        ]
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Saved(_)))
    );
    let (_, done) = s.step(Input::WriteDone { fid: fid("1") });
    assert!(done.iter().any(|effect| matches!(
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
    let (s, _) = s.step(Input::Frame(Message {
        file: Some(fid("9")),
        data: b"cd".to_vec(),
        ..msg(Action::EndData)
    }));
    let (_, done) = s.step(Input::WriteDone { fid: fid("9") });
    assert!(
        done.iter()
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
    let (s, _) = s.step(Input::Frame(Message {
        file: Some(fid("2")),
        ..msg(Action::EndData)
    }));
    let (_, done) = s.step(Input::WriteDone { fid: fid("2") });
    assert!(done.iter().any(|effect| matches!(
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
fn links_are_refused_by_default_and_nothing_is_created() {
    for kind in [FileType::Symlink, FileType::Link] {
        let s = allowed(1024);
        let (_, effects) = s.step(file("l", "1", kind, None));
        assert!(writes(&effects).is_empty());
        assert!(
            matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "EPERM")
        );
    }
}

fn making_links() -> temor_transfer::kitty::ReceiveSession {
    let (s, _) = send(session(1024).with_links(LinkPolicy::Make));
    s.step(Input::Decided(Consent::Allow)).0
}

fn target_chunks(id: &str, parts: &[&str]) -> Vec<Input> {
    let last = parts.len().saturating_sub(1);
    parts
        .iter()
        .enumerate()
        .map(|(at, part)| {
            let action = if at == last {
                Action::EndData
            } else {
                Action::Data
            };
            Input::Frame(Message {
                file: Some(fid(id)),
                data: part.as_bytes().to_vec(),
                ..msg(action)
            })
        })
        .collect()
}

#[test]
fn a_link_is_made_when_its_target_stays_inside_and_refused_when_it_leaves() {
    #[rustfmt::skip]
    let cases: &[(&str, &str, &[&str], Option<&str>)] = &[
        ("a sibling", "a/l", &["x.txt"], Some("x.txt")),
        ("one up", "a/l", &["../b"], Some("../b")),
        ("a target in two chunks", "a/l", &["../", "b"], Some("../b")),
        ("up past the top", "l", &["../b"], None),
        ("absolute", "l", &["/etc/passwd"], None),
        ("nothing", "l", &[""], None),
    ];
    for (name, at, parts, made) in cases {
        let (mut s, started) = making_links().step(file(at, "1", FileType::Symlink, None));
        assert_eq!(replies(&started), vec![ReplyStatus::Started], "{name}");
        assert!(
            writes(&started).is_empty(),
            "{name}: nothing before the target is whole"
        );
        let mut seen = Vec::new();
        for input in target_chunks("1", parts) {
            let (next, effects) = s.step(input);
            s = next;
            seen.extend(effects);
        }
        let want = made.map(|target| WriteOp::Symlink {
            path: RelPath::parse(at).expect("path"),
            target: target.to_owned(),
        });
        assert_eq!(writes(&seen).into_iter().next(), want, "{name}");
        let refused = replies(&seen)
            .iter()
            .any(|r| matches!(r, ReplyStatus::Failed { code, .. } if code.code() == "EPERM" || code.code() == "EINVAL"));
        assert_eq!(refused, made.is_none(), "{name}");
    }
}

#[test]
fn a_hard_link_is_refused_even_where_symbolic_links_are_made() {
    let (_, effects) = making_links().step(file("l", "1", FileType::Link, None));
    assert!(writes(&effects).is_empty());
    assert!(
        matches!(&replies(&effects)[0], ReplyStatus::Failed { code, .. } if code.code() == "EPERM")
    );
}

#[test]
fn a_remote_mode_is_carried_to_the_writes() {
    let with_mode = |name: &str, id: &str, kind, size, mode| match file(name, id, kind, size) {
        Input::Frame(message) => Input::Frame(Message {
            permissions: Some(Mode::new(mode)),
            ..message
        }),
        other => other,
    };
    let (s, folder) = allowed(1024).step(with_mode("d", "1", FileType::Directory, None, 0o750));
    assert_eq!(
        writes(&folder),
        vec![
            WriteOp::MakeDir(RelPath::parse("d").expect("path")),
            WriteOp::SetDirMode {
                path: RelPath::parse("d").expect("path"),
                mode: Mode::new(0o750)
            },
        ]
    );
    let (s, _) = s.step(with_mode("f", "2", FileType::Regular, Some(0), 0o4755));
    let (_, done) = s.step(Input::Frame(Message {
        file: Some(fid("2")),
        ..msg(Action::EndData)
    }));
    assert!(matches!(
        writes(&done).last(),
        Some(WriteOp::Finish { mode: Some(mode), .. }) if mode.permission_bits() == 0o755
    ));
}
