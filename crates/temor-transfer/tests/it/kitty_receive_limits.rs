//! The receive machine's limits and failures: sizes, entry and open caps, transmission types,
//! aborts and failed writes.

use temor_transfer::kitty::{
    Action, Effect, EndWhy, ErrorCode, FileType, Input, Message, ReceiveSession, ReplyStatus,
    Transmission, WriteKind, WriteOp,
};
use temor_transfer::{Bytes, Consent, Count, Limits, RelPath};

use crate::receive_support::*;
use crate::support::limits;

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

fn with_limits(entries: u64, open: u64) -> ReceiveSession {
    let caps = Limits::new(Bytes::new(1 << 20), Count::new(entries), Count::new(open));
    let (s, _) = ReceiveSession::new(origin(), caps).step(Input::Frame(msg(Action::Send)));
    let (s, _) = s.step(Input::Decided(Consent::Allow));
    s
}

fn count_creates(effects: &[Effect]) -> usize {
    writes(effects)
        .iter()
        .filter(|op| matches!(op, WriteOp::CreateFile(_)))
        .count()
}

#[test]
fn ten_thousand_empty_files_stop_at_the_entry_cap() {
    let mut s = with_limits(100, 10_000);
    let mut creates = 0;
    let mut ended = 0;
    for i in 0..10_000 {
        let (next, effects) = s.step(file(
            &format!("f{i}"),
            &i.to_string(),
            FileType::Regular,
            Some(0),
        ));
        creates += count_creates(&effects);
        ended += effects
            .iter()
            .filter(|e| matches!(e, Effect::Ended(_)))
            .count();
        s = next;
    }
    assert_eq!(creates, 100);
    assert_eq!(ended, 1, "the transfer ends once, at the cap");
}

#[test]
fn going_over_the_entry_cap_replies_and_discards_open_files() {
    let mut s = with_limits(2, 10);
    let (next, _) = s.step(file("a", "1", FileType::Regular, None));
    s = next;
    let (next, _) = s.step(file("b", "2", FileType::Regular, None));
    s = next;
    let (_, effects) = s.step(file("c", "3", FileType::Regular, None));
    assert!(replies(&effects).iter().any(|status| matches!(
        status,
        ReplyStatus::Failed { code, .. } if code.code() == "EFBIG"
    )));
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
    let discarded = writes(&effects);
    assert!(discarded.contains(&WriteOp::Discard(RelPath::parse("a").expect("p"))));
    assert!(discarded.contains(&WriteOp::Discard(RelPath::parse("b").expect("p"))));
}

#[test]
fn too_many_files_open_at_once_ends_the_transfer() {
    let mut s = with_limits(1000, 3);
    for i in 0..3 {
        let (next, _) = s.step(file(
            &format!("o{i}"),
            &i.to_string(),
            FileType::Regular,
            None,
        ));
        s = next;
    }
    let (_, effects) = s.step(file("o3", "3", FileType::Regular, None));
    assert_eq!(
        writes(&effects).len(),
        3,
        "the three open files are discarded"
    );
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
}

#[test]
fn only_the_simple_transmission_type_is_offered_to_the_person() {
    let rsync = Message {
        transmission: Some(Transmission::Rsync),
        ..msg(Action::Send)
    };
    let (_, effects) = ReceiveSession::new(origin(), limits(1024)).step(Input::Frame(rsync));
    assert!(!effects.iter().any(|e| matches!(e, Effect::AskConsent(_))));
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
    assert!(replies(&effects).iter().any(|status| matches!(
        status,
        ReplyStatus::Failed { code, .. } if code.code() == "EPERM"
    )));

    let simple = Message {
        transmission: Some(Transmission::Simple),
        ..msg(Action::Send)
    };
    let (_, effects) = ReceiveSession::new(origin(), limits(1024)).step(Input::Frame(simple));
    // Nothing until the person decides: kitty's kitten reads an early answer as a refusal.
    assert!(replies(&effects).is_empty());
    assert_eq!(effects.last(), Some(&Effect::AskConsent(origin())));
}

#[test]
fn an_aborted_stream_discards_open_files_and_ends_quietly() {
    let s = allowed(1024);
    let (s, _) = s.step(file("x", "1", FileType::Regular, None));
    let (after, effects) = s.step(Input::Aborted);
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("x").expect("p"))]
    );
    assert_eq!(effects.last(), Some(&Effect::Ended(EndWhy::Aborted)));
    assert!(replies(&effects).is_empty());
    let (_, later) = after.step(file("y", "2", FileType::Regular, Some(1)));
    assert!(later.is_empty());
}

#[test]
fn a_failed_write_cancels_that_file_and_the_transfer_goes_on() {
    let s = allowed(1024);
    let (s, _) = s.step(file("w", "1", FileType::Regular, None));
    let (s, effects) = s.step(Input::WriteFailed {
        fid: Some(fid("1")),
        op: WriteKind::Append,
        error: ErrorCode::Eio,
    });
    assert_eq!(
        writes(&effects),
        vec![WriteOp::Discard(RelPath::parse("w").expect("p"))]
    );
    assert!(!effects.iter().any(|e| matches!(e, Effect::Ended(_))));
    let (_, effects) = s.step(file("next", "2", FileType::Regular, Some(1)));
    assert_eq!(count_creates(&effects), 1);
}

#[test]
fn a_failed_write_for_no_open_file_ends_the_transfer() {
    let s = allowed(1024);
    let (_, effects) = s.step(Input::WriteFailed {
        fid: None,
        op: WriteKind::Append,
        error: ErrorCode::Enoent,
    });
    assert!(effects.contains(&Effect::Ended(EndWhy::Failed)));
}
