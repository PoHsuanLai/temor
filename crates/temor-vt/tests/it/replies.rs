//! `replies.bin`: DA1, DSR and DECRQM queries answer through `VtEvent::Reply`.

use temor_vt::VtEvent;

use crate::support::{drain, fixture, term};

#[test]
fn device_queries_reply_in_order() {
    let (mut t, rx) = term(80, 24);
    t.write(&fixture("replies.bin"));
    let replies: Vec<Vec<u8>> = drain(&rx)
        .into_iter()
        .filter_map(|e| match e {
            VtEvent::Reply(bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(
        replies,
        [
            b"\x1b[?62;22c".to_vec(),
            b"\x1b[0n".to_vec(),
            b"\x1b[?2004;2$y".to_vec()
        ]
    );
}

#[test]
fn unknown_osc_and_apc_are_reported_with_their_payload() {
    use temor_vt::{Truncation, UnknownKind};
    let (mut t, rx) = term(80, 24);
    t.write(&crate::support::fixture("unknown.bin"));
    let unknown: Vec<(UnknownKind, Vec<u8>, Truncation)> = drain(&rx)
        .into_iter()
        .filter_map(|e| match e {
            VtEvent::Unknown {
                kind,
                payload,
                truncation,
            } => Some((kind, payload, truncation)),
            _ => None,
        })
        .collect();
    assert_eq!(
        unknown,
        [
            (
                UnknownKind::Osc,
                b"5113;ac=send;id=xfer1".to_vec(),
                Truncation::Complete
            ),
            (
                UnknownKind::Apc,
                b"Zsome-apc".to_vec(),
                Truncation::Complete
            ),
        ]
    );
}
