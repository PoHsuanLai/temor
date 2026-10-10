//! The receive machine's names and edge reports: duplicate names, what a failed create or finish
//! may discard, saving only after Finish, and frames that did not parse.

use temor_transfer::kitty::{
    Action, Effect, EndWhy, ErrorCode, FileType, Input, Message, ReplyStatus, WriteKind, WriteOp,
};
use temor_transfer::{Bytes, RelPath};

use crate::receive_support::*;

fn saved(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Saved(_)))
}

fn codes(effects: &[Effect]) -> Vec<String> {
    replies(effects)
        .into_iter()
        .filter_map(|status| match status {
            ReplyStatus::Failed { code, .. } => Some(code.code()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_second_file_with_the_same_name_is_refused_and_the_first_stays_intact() {
    let s = allowed(1024);
    let (s, _) = s.step(file("report.txt", "1", FileType::Regular, Some(3)));
    let (s, first) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"one".to_vec(),
        ..msg(Action::EndData)
    }));
    assert_eq!(
        writes(&first),
        vec![
            WriteOp::Append {
                path: RelPath::parse("report.txt").expect("p"),
                data: b"one".to_vec()
            },
            WriteOp::Finish {
                path: RelPath::parse("report.txt").expect("p"),
                modified: None,
                mode: None
            },
        ]
    );
    let (s, _) = s.step(Input::WriteDone { fid: fid("1") });

    let (_, second) = s.step(file("report.txt", "2", FileType::Regular, Some(3)));
    assert!(
        writes(&second).is_empty(),
        "no create, so nothing to discard"
    );
    assert_eq!(codes(&second), vec!["EEXIST".to_owned()]);
    assert!(!second.iter().any(|e| matches!(e, Effect::Ended(_))));
}

#[test]
fn a_name_already_used_by_a_folder_is_refused_too() {
    let s = allowed(1024);
    let (s, _) = s.step(file("docs", "1", FileType::Directory, None));
    let (_, effects) = s.step(file("docs", "2", FileType::Regular, Some(1)));
    assert!(writes(&effects).is_empty());
    assert_eq!(codes(&effects), vec!["EEXIST".to_owned()]);
}

#[test]
fn a_failed_create_cancels_that_file_without_discarding_anything() {
    let s = allowed(1024);
    let (s, _) = s.step(file("c.txt", "1", FileType::Regular, Some(1)));
    let (s, effects) = s.step(Input::WriteFailed {
        fid: Some(fid("1")),
        op: WriteKind::Create,
        error: ErrorCode::Eexist,
    });
    assert!(
        writes(&effects).is_empty(),
        "a failed create must not discard"
    );
    assert!(!effects.iter().any(|e| matches!(e, Effect::Ended(_))));
    let (_, next) = s.step(file("d.txt", "2", FileType::Regular, Some(1)));
    assert!(!writes(&next).is_empty(), "the transfer goes on");
}

#[test]
fn a_saved_file_is_not_saved_again_when_a_later_finish_fails() {
    let s = allowed(1024);
    let (s, _) = s.step(file("a", "1", FileType::Regular, Some(1)));
    let (s, first) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"x".to_vec(),
        ..msg(Action::EndData)
    }));
    assert!(!saved(&first), "not saved before Finish succeeds");
    let (s, done) = s.step(Input::WriteDone { fid: fid("1") });
    assert!(saved(&done));
    let (_, later) = s.step(Input::WriteFailed {
        fid: Some(fid("1")),
        op: WriteKind::Finish,
        error: ErrorCode::Eio,
    });
    assert!(!saved(&later));
}

#[test]
fn a_failed_finish_discards_only_its_own_file_and_reports_it() {
    let s = allowed(1024);
    let (s, _) = s.step(file("f", "1", FileType::Regular, Some(1)));
    let (s, _) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"x".to_vec(),
        ..msg(Action::EndData)
    }));
    let (s, effects) = s.step(Input::WriteFailed {
        fid: Some(fid("1")),
        op: WriteKind::Finish,
        error: ErrorCode::Eio,
    });
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("f").expect("p"))]
    );
    assert!(!saved(&effects));
    assert_eq!(codes(&effects), vec!["EIO".to_owned()]);
    assert!(
        !s.step(Input::WriteDone { fid: fid("1") })
            .1
            .iter()
            .any(|e| matches!(e, Effect::Saved(_)))
    );
}

#[test]
fn a_frame_that_did_not_parse_is_answered_while_asking_and_ends_a_live_transfer() {
    let id = || temor_transfer::kitty::KittyId::parse("tx1").expect("id");
    // The person is still being asked: the sender is told, and the question stands.
    let (asking, _) = send(session(1024));
    let (asking, effects) = asking.step(Input::Malformed { id: id() });
    assert_eq!(codes(&effects), vec!["EINVAL".to_owned()]);
    assert!(!effects.iter().any(|e| matches!(e, Effect::Ended(_))));
    let (_, answered) = asking.step(Input::Decided(temor_transfer::Consent::Allow));
    assert_eq!(replies(&answered), vec![ReplyStatus::Ok]);

    // Receiving, with no file open: the transfer ends with the error, not only a reply.
    let (_, effects) = allowed(1024).step(Input::Malformed { id: id() });
    assert_eq!(codes(&effects), vec!["EINVAL".to_owned()]);
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
}

#[test]
fn an_unknown_transmission_type_is_refused_with_a_reply() {
    let message = Message {
        transmission: Some(temor_transfer::kitty::Transmission::Other(
            "frobnicate".to_owned(),
        )),
        ..msg(Action::Send)
    };
    let (_, effects) = temor_transfer::kitty::ReceiveSession::new(
        origin(),
        temor_transfer::Limits::new(
            Bytes::new(1024),
            temor_transfer::Count::new(100),
            temor_transfer::Count::new(10),
        ),
    )
    .step(Input::Frame(message));
    assert!(!effects.iter().any(|e| matches!(e, Effect::AskConsent(_))));
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
    assert_eq!(codes(&effects), vec!["EPERM".to_owned()]);
}

#[test]
fn a_malformed_middle_chunk_leaves_no_saved_file() {
    let s = allowed(1024);
    let (s, _) = s.step(file("m.bin", "1", FileType::Regular, None));
    let (s, _) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"ab".to_vec(),
        ..msg(Action::Data)
    }));
    let (s, effects) = s.step(Input::Malformed {
        id: temor_transfer::kitty::KittyId::parse("tx1").expect("id"),
    });
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("m.bin").expect("p"))]
    );
    assert_eq!(codes(&effects), vec!["EINVAL".to_owned()]);
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
    assert!(!saved(&effects));
    // The rest of the file arrives, but the transfer is over: nothing is saved.
    let (s, rest) = s.step(Input::Frame(Message {
        file: Some(fid("1")),
        data: b"cd".to_vec(),
        ..msg(Action::EndData)
    }));
    assert!(rest.is_empty());
    let (_, done) = s.step(Input::WriteDone { fid: fid("1") });
    assert!(!saved(&done));
}

#[test]
fn a_malformed_frame_is_ignored_unless_it_names_the_live_transfer() {
    let s = allowed(1024);
    let (s, effects) = s.step(Input::Malformed {
        id: temor_transfer::kitty::KittyId::parse("other").expect("id"),
    });
    assert!(effects.is_empty(), "another id, while live: ignored");
    let (s, _) = s.step(Input::Frame(msg(Action::Finish)));
    let (_, effects) = s.step(Input::Malformed {
        id: temor_transfer::kitty::KittyId::parse("tx1").expect("id"),
    });
    assert!(effects.is_empty(), "the live id, after Finish: ignored");
}

#[test]
fn an_unreadable_offer_before_any_transfer_is_answered() {
    let (s, effects) = session(1024).step(Input::Malformed {
        id: temor_transfer::kitty::KittyId::parse("tx9").expect("id"),
    });
    assert_eq!(codes(&effects), vec![ErrorCode::Einval.code()]);
    let (_, next) = s.step(Input::Frame(msg(Action::Send)));
    assert!(
        next.iter()
            .any(|effect| matches!(effect, Effect::AskConsent(_))),
        "the session is still idle and takes a later offer"
    );
}
