//! Frames from a newer peer, written by hand as that peer would send them. None of them may end
//! the stream: unknown requests are answered, unknown events skipped, unknown values read as
//! `Unknown`, and a known message that is simply broken is still malformed.

use temor_wire::WireError;
use temor_wire::frame::{Decoder, FrameLimit};
use temor_wire::hello::Capability;
use temor_wire::ids::RequestId;
use temor_wire::message::{EntryKind, Frame, Incoming, Message, RemoteError, Reply};

/// One frame as bytes: JSON as written, and the payload word (`u32::MAX` for none).
fn frame(json: &str, payload: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    let word = payload.map_or(u32::MAX, |bytes| bytes.len() as u32);
    bytes.extend_from_slice(&word.to_le_bytes());
    bytes.extend_from_slice(json.as_bytes());
    if let Some(payload) = payload {
        bytes.extend_from_slice(payload);
    }
    bytes
}

fn read_all(wire: &[u8]) -> Vec<Result<Incoming, WireError>> {
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(wire);
    let mut out = Vec::new();
    loop {
        match decoder.next_message() {
            Ok(Some(incoming)) => out.push(Ok(incoming)),
            Ok(None) => return out,
            Err(error) => {
                out.push(Err(error));
                return out;
            }
        }
    }
}

fn one(json: &str, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    read_all(&frame(json, payload)).remove(0)
}

fn zeros() -> String {
    "0".repeat(64)
}

#[test]
fn an_unknown_request_is_reported_with_its_id_so_it_can_be_answered() {
    let json = r#"{"Request":{"id":77,"body":{"FrobnicateWidgets":{"x":1}}}}"#;
    let incoming = one(json, None).unwrap();
    assert_eq!(incoming, Incoming::UnknownRequest { id: RequestId(77) });
    // The answer a helper sends for it, under the same id.
    let answer = serde_json::to_string(&Message::unknown_request(RequestId(77))).unwrap();
    assert_eq!(
        answer,
        r#"{"Response":{"id":77,"outcome":{"Err":"UnknownRequest"}}}"#
    );
}

#[test]
fn an_unknown_event_is_skipped_and_the_frames_after_it_still_arrive() {
    let mut wire = frame(r#"{"Event":{"Hologram":{"x":1}}}"#, None);
    wire.extend(frame(r#"{"Request":{"id":1,"body":"ListSessions"}}"#, None));
    let got = read_all(&wire);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], Ok(Incoming::Ignored));
    assert_eq!(
        got[1],
        Ok(Incoming::Frame(Frame {
            message: Message::Request {
                id: RequestId(1),
                body: temor_wire::message::Request::ListSessions,
            },
            payload: None,
        }))
    );
}

#[test]
fn an_unknown_message_kind_is_skipped() {
    assert_eq!(one(r#"{"Telepathy":{"x":1}}"#, None), Ok(Incoming::Ignored));
}

#[test]
fn an_unknown_reply_under_a_response_fails_that_request_by_id() {
    let json = r#"{"Response":{"id":5,"outcome":{"Ok":{"Teleported":{}}}}}"#;
    assert_eq!(
        one(json, None),
        Ok(Incoming::UnknownResponse { id: RequestId(5) })
    );
}

#[test]
fn an_unknown_error_under_a_response_fails_that_request_by_id() {
    let json = r#"{"Response":{"id":6,"outcome":{"Err":{"Exploded":{"why":"x"}}}}}"#;
    assert_eq!(
        one(json, None),
        Ok(Incoming::UnknownResponse { id: RequestId(6) })
    );
}

#[test]
fn a_newer_greeting_is_read_with_its_unknown_capability_as_unknown() {
    let json = format!(
        concat!(
            r#"{{"Hello":{{"protocol":2,"oldest_protocol":1,"helper":"{}","#,
            r#""caps":["Sessions","Hologram"]}}}}"#
        ),
        zeros()
    );
    let Ok(Incoming::Frame(frame)) = one(&json, None) else {
        panic!("a greeting from a newer peer must be read");
    };
    let Message::Hello(hello) = frame.message else {
        panic!("expected a greeting");
    };
    assert_eq!(hello.protocol.0, 2);
    assert_eq!(hello.caps, [Capability::Sessions, Capability::Unknown]);
}

#[test]
fn an_unknown_entry_kind_inside_a_known_reply_reads_as_unknown() {
    let json = concat!(
        r#"{"Response":{"id":6,"outcome":{"Ok":{"Listing":"#,
        r#"[{"name":"a","kind":"Hologram","len":1,"mtime":2}]}}}}"#
    );
    let Ok(Incoming::Frame(frame)) = one(json, None) else {
        panic!("a reply with a newer kind inside must be read");
    };
    let Message::Response {
        outcome: Ok(Reply::Listing(entries)),
        ..
    } = frame.message
    else {
        panic!("expected a listing");
    };
    assert_eq!(entries[0].kind, EntryKind::Unknown);
}

#[test]
fn a_known_request_with_a_broken_field_is_malformed_not_unknown() {
    let json = r#"{"Request":{"id":1,"body":{"SendInput":{"session":"x"}}}}"#;
    assert!(matches!(
        one(json, Some(b"")),
        Err(WireError::Malformed { .. })
    ));
}

#[test]
fn a_request_without_an_id_cannot_be_answered_so_it_is_malformed() {
    let json = r#"{"Request":{"body":{"FrobnicateWidgets":{}}}}"#;
    assert!(matches!(one(json, None), Err(WireError::Malformed { .. })));
}

#[test]
fn an_unknown_refusal_in_an_error_reads_as_unknown_not_a_failure() {
    let json = r#"{"Response":{"id":9,"outcome":{"Err":{"HostRefused":{"reason":"Hologram"}}}}}"#;
    let Ok(Incoming::Frame(frame)) = one(json, None) else {
        panic!("a refusal from a newer peer must be read");
    };
    assert_eq!(
        frame.message,
        Message::Response {
            id: RequestId(9),
            outcome: Err(RemoteError::HostRefused {
                reason: temor_wire::message::Refusal::Unknown,
            }),
        }
    );
}
