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
fn a_known_request_with_a_broken_field_is_unreadable_and_answerable_by_id() {
    let json = r#"{"Request":{"id":1,"body":{"SendInput":{"session":"x"}}}}"#;
    assert_eq!(
        one(json, Some(b"")),
        Ok(Incoming::UnreadableRequest { id: RequestId(1) })
    );
}

#[test]
fn a_known_request_with_a_newer_nested_enum_is_unreadable_not_unknown() {
    // WriteFile is known; its `check` is a kind from a newer temor.
    let json = r#"{"Request":{"id":3,"body":{"WriteFile":{"path":"/a","check":"Frobnicate"}}}}"#;
    assert_eq!(
        one(json, Some(b"x")),
        Ok(Incoming::UnreadableRequest { id: RequestId(3) })
    );
    let answer = serde_json::to_string(&Message::unreadable_request(RequestId(3))).unwrap();
    assert!(answer.contains(r#""id":3"#) && answer.contains("Unreadable"));
    assert!(!answer.contains("UnknownRequest"));
}

#[test]
fn an_exit_with_a_newer_status_is_surfaced_as_unreadable_not_ignored() {
    // ExitStatus is a newer enum here, nested inside a known event.
    let json = r#"{"Event":{"Exited":{"session":1,"status":{"Vaporised":{}}}}}"#;
    assert_eq!(
        one(json, None),
        Ok(Incoming::UnreadableEvent {
            kind: "Exited".to_string()
        })
    );
}

#[test]
fn an_unknown_top_level_event_tag_is_ignored() {
    let json = r#"{"Event":{"Hologram":{"x":1}}}"#;
    assert_eq!(one(json, None), Ok(Incoming::Ignored));
}

#[test]
fn a_known_response_with_a_newer_nested_reply_fails_its_request_by_id() {
    let json = r#"{"Response":{"id":5,"outcome":{"Ok":{"Verdict":{"Teleported":{}}}}}}"#;
    assert_eq!(
        one(json, None),
        Ok(Incoming::UnknownResponse { id: RequestId(5) })
    );
}

#[test]
fn newer_messages_never_end_the_stream_so_the_frames_after_them_still_arrive() {
    let mut wire = frame(
        r#"{"Event":{"Exited":{"session":1,"status":{"Vaporised":{}}}}}"#,
        None,
    );
    wire.extend(frame(
        r#"{"Request":{"id":3,"body":{"Teleport":{}}}}"#,
        None,
    ));
    wire.extend(frame(
        r#"{"Request":{"id":4,"body":{"WriteFile":{"path":"/a","check":"Frobnicate"}}}}"#,
        Some(b"x"),
    ));
    wire.extend(frame(r#"{"Event":{"Output":{"session":2}}}"#, Some(b"out")));
    let got = read_all(&wire);
    assert_eq!(got.len(), 4);
    assert_eq!(
        got[0],
        Ok(Incoming::UnreadableEvent {
            kind: "Exited".to_string()
        })
    );
    assert_eq!(got[1], Ok(Incoming::UnknownRequest { id: RequestId(3) }));
    assert_eq!(got[2], Ok(Incoming::UnreadableRequest { id: RequestId(4) }));
    assert!(matches!(&got[3], Ok(Incoming::Frame(_))));
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

#[test]
fn requests_from_before_the_revision_read_with_the_old_meaning() {
    use temor_wire::message::{Request, WriteMode};
    use temor_wire::units::CellPx;
    // A resize without a cell size, and a write without a mode: both read as before.
    let resize = r#"{"Request":{"id":1,"body":{"ResizeSession":{"session":1,"size":{"cols":80,"rows":24}}}}}"#;
    let Ok(Incoming::Frame(frame)) = one(resize, None) else {
        panic!("an old resize must be read");
    };
    let Message::Request {
        body: Request::ResizeSession { cell, .. },
        ..
    } = frame.message
    else {
        panic!("not a resize");
    };
    assert_eq!(cell, CellPx::DEFAULT);

    let write = r#"{"Request":{"id":2,"body":{"WriteFile":{"path":"/a","check":"Overwrite"}}}}"#;
    let Ok(Incoming::Frame(frame)) = one(write, Some(b"x")) else {
        panic!("an old write must be read");
    };
    let Message::Request {
        body: Request::WriteFile { mode, .. },
        ..
    } = frame.message
    else {
        panic!("not a write");
    };
    assert_eq!(mode, WriteMode::Whole);
}
