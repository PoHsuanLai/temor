//! The exact JSON of a few messages, pinned. A change here is a change to the wire, and needs a
//! protocol version decision, not a quiet update of the expected text.

use temor_wire::digest::ContentHash;
use temor_wire::hello::{Capability, Hello, HelperVersion, ProtocolVersion};
use temor_wire::ids::{RequestId, SessionId};
use temor_wire::message::{
    Event, FileChange, FileChanges, Message, RelPath, RemoteError, RemotePath, Reply, Request,
    TrialVerdict, UncheckedWhy,
};
use temor_wire::units::{ExitCode, Size};

fn json(message: &Message) -> String {
    serde_json::to_string(message).unwrap()
}

fn zeros() -> String {
    "0".repeat(64)
}

#[test]
fn a_request_with_no_fields_is_its_name() {
    let message = Message::Request {
        id: RequestId(1),
        body: Request::ListSessions,
    };
    assert_eq!(
        json(&message),
        r#"{"Request":{"id":1,"body":"ListSessions"}}"#
    );
}

#[test]
fn a_request_with_fields_names_them() {
    let message = Message::Request {
        id: RequestId(5),
        body: Request::ResizeSession {
            session: SessionId(1),
            size: Size {
                cols: temor_wire::units::Cols(80),
                rows: temor_wire::units::Rows(24),
            },
        },
    };
    assert_eq!(
        json(&message),
        concat!(
            r#"{"Request":{"id":5,"body":{"ResizeSession":"#,
            r#"{"session":1,"size":{"cols":80,"rows":24}}}}}"#
        )
    );
}

#[test]
fn close_is_a_request_for_a_session() {
    let message = Message::Request {
        id: RequestId(10),
        body: Request::Close {
            session: SessionId(9),
        },
    };
    assert_eq!(
        json(&message),
        r#"{"Request":{"id":10,"body":{"Close":{"session":9}}}}"#
    );
}

#[test]
fn an_ok_reply_and_an_error_are_under_their_outcome_key() {
    let ok = Message::Response {
        id: RequestId(2),
        outcome: Ok(Reply::Ack),
    };
    assert_eq!(json(&ok), r#"{"Response":{"id":2,"outcome":{"Ok":"Ack"}}}"#);
    let err = Message::Response {
        id: RequestId(3),
        outcome: Err(RemoteError::NotFound {
            path: RemotePath("/x".into()),
        }),
    };
    assert_eq!(
        json(&err),
        r#"{"Response":{"id":3,"outcome":{"Err":{"NotFound":{"path":"/x"}}}}}"#
    );
}

#[test]
fn an_event_names_its_session() {
    let message = Message::Event(Event::Output {
        session: SessionId(4),
    });
    assert_eq!(json(&message), r#"{"Event":{"Output":{"session":4}}}"#);
}

#[test]
fn the_greeting_pins_its_fields_and_the_helper_hash() {
    let message = Message::Hello(Hello {
        protocol: ProtocolVersion(1),
        oldest_protocol: ProtocolVersion(1),
        helper: HelperVersion(ContentHash([0; 32])),
        caps: vec![Capability::Sessions],
    });
    assert_eq!(
        json(&message),
        format!(
            r#"{{"Hello":{{"protocol":1,"oldest_protocol":1,"helper":"{}","caps":["Sessions"]}}}}"#,
            zeros()
        )
    );
}

#[test]
fn trial_verdicts_read_as_plan_three_names() {
    let failed = Message::Response {
        id: RequestId(6),
        outcome: Ok(Reply::Verdict(TrialVerdict::Failed {
            exit: ExitCode(2),
            first_error: temor_wire::message::ErrorLine("boom".into()),
        })),
    };
    assert_eq!(
        json(&failed),
        concat!(
            r#"{"Response":{"id":6,"outcome":{"Ok":{"Verdict":"#,
            r#"{"Failed":{"exit":2,"first_error":"boom"}}}}}}"#
        )
    );
    let clean = Message::Response {
        id: RequestId(7),
        outcome: Ok(Reply::Verdict(TrialVerdict::Clean {
            changes: FileChanges {
                files: vec![FileChange {
                    path: RelPath("a.txt".into()),
                    how: temor_wire::message::ChangeKind::Created,
                }],
            },
        })),
    };
    assert_eq!(
        json(&clean),
        concat!(
            r#"{"Response":{"id":7,"outcome":{"Ok":{"Verdict":"#,
            r#"{"Clean":{"changes":{"files":[{"path":"a.txt","how":"Created"}]}}}}}}}"#
        )
    );
    let unchecked = Message::Response {
        id: RequestId(8),
        outcome: Ok(Reply::Verdict(TrialVerdict::Unchecked(
            UncheckedWhy::NoSandbox,
        ))),
    };
    assert_eq!(
        json(&unchecked),
        r#"{"Response":{"id":8,"outcome":{"Ok":{"Verdict":{"Unchecked":"NoSandbox"}}}}}"#
    );
}
