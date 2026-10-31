use super::*;
use crate::WireError;
use crate::ids::{RemoteSessionId, RequestId};
use crate::message::{Incoming, Message, Reply, Request};
use crate::units::{ByteCount, ByteOffset, ByteRange};

fn list(id: u64) -> Message {
    Message::Request {
        id: RequestId(id),
        body: Request::ListSessions,
    }
}

fn input(id: u64) -> Message {
    Message::Request {
        id: RequestId(id),
        body: Request::SendInput {
            session: RemoteSessionId(1),
        },
    }
}

fn frame(incoming: Incoming) -> (Message, Option<Vec<u8>>) {
    match incoming {
        Incoming::Frame(frame) => (frame.message, frame.payload),
        other => panic!("expected a frame, got {other:?}"),
    }
}

/// A header with the given words, followed by `body`.
fn raw(json: u32, payload: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&json.to_le_bytes());
    bytes.extend_from_slice(&payload.to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn a_frame_without_payload_round_trips_and_marks_its_payload_word_absent() {
    let bytes = encode(&list(1), None, FrameLimit::DEFAULT).unwrap();
    assert_eq!(bytes[4..8], u32::MAX.to_le_bytes());
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&bytes);
    assert_eq!(
        frame(decoder.next_message().unwrap().unwrap()),
        (list(1), None)
    );
    assert!(decoder.next_message().unwrap().is_none());
    assert!(decoder.finish().is_ok());
}

#[test]
fn a_payload_carrying_frame_round_trips_its_bytes_including_empty_ones() {
    for payload in [&[1u8, 2, 3][..], &[][..]] {
        let bytes = encode(&input(2), Some(payload), FrameLimit::DEFAULT).unwrap();
        let mut decoder = Decoder::new(FrameLimit::DEFAULT);
        decoder.push(&bytes);
        let (message, got) = frame(decoder.next_message().unwrap().unwrap());
        assert_eq!(message, input(2));
        assert_eq!(got.as_deref(), Some(payload));
    }
}

#[test]
fn back_to_back_frames_are_cut_apart_and_the_buffer_is_reclaimed() {
    let mut wire = Vec::new();
    for id in 0..50 {
        wire.extend(encode(&input(id), Some(&[id as u8; 100]), FrameLimit::DEFAULT).unwrap());
    }
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&wire);
    let mut seen = 0;
    while let Some(incoming) = decoder.next_message().unwrap() {
        let (message, payload) = frame(incoming);
        assert_eq!(message, input(seen));
        assert_eq!(payload.map(|bytes| bytes.len()), Some(100));
        seen += 1;
    }
    assert_eq!(seen, 50);
    assert!(decoder.finish().is_ok());
}

/// Each row: (name, JSON word, payload word, limit, the body length the error reports). The
/// header alone decides, so nothing after it is pushed.
const HEADER_REFUSALS: &[(&str, u32, u32, FrameLimit, u64)] = &[
    (
        "a 2 GiB payload word over a 1 KiB limit",
        2,
        1 << 31,
        FrameLimit(1024),
        (1u64 << 31) + 2,
    ),
    (
        "the maximum words, which is no payload, over the default limit",
        u32::MAX,
        u32::MAX,
        FrameLimit::DEFAULT,
        u32::MAX as u64,
    ),
];

#[test]
fn a_frame_over_the_limit_is_refused_from_its_header_alone() {
    for &(name, json, payload, limit, len) in HEADER_REFUSALS {
        let mut decoder = Decoder::new(limit);
        decoder.push(&raw(json, payload, &[]));
        assert_eq!(
            decoder.next_message(),
            Err(WireError::Oversize {
                len,
                limit: u64::from(limit.0),
            }),
            "{name}"
        );
        assert_eq!(decoder.next_message(), Err(WireError::Spent), "{name}");
    }
}

#[test]
fn encoding_refuses_a_frame_over_the_limit() {
    let result = encode(&input(1), Some(&[0; 2048]), FrameLimit(1024));
    assert!(matches!(
        result,
        Err(WireError::Oversize { limit: 1024, .. })
    ));
}

#[test]
fn a_payload_is_bound_to_its_message_in_both_directions() {
    // encode: a message with no payload field refuses one, and a payload field refuses none.
    assert!(
        matches!(
            encode(&list(1), Some(&[1]), FrameLimit::DEFAULT),
            Err(WireError::Unencodable { .. })
        ),
        "encode: a payload with a message that has none"
    );
    assert!(
        matches!(
            encode(&input(1), None, FrameLimit::DEFAULT),
            Err(WireError::Unencodable { .. })
        ),
        "encode: no payload with a message that needs one"
    );
    // decode, a message that carries no payload, sent with one.
    let json = serde_json::to_vec(&list(1)).unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&raw(
        json.len() as u32,
        2,
        &[json.as_slice(), b"hi"].concat(),
    ));
    assert!(
        matches!(decoder.next_message(), Err(WireError::Malformed { .. })),
        "decode: a payload sent with a message that has none"
    );
    // decode, a message that carries a payload, sent without one.
    let json = serde_json::to_vec(&input(1)).unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&raw(json.len() as u32, u32::MAX, &json));
    assert!(
        matches!(decoder.next_message(), Err(WireError::Malformed { .. })),
        "decode: no payload sent with a message that needs one"
    );
}

#[test]
fn json_that_is_not_a_message_is_malformed_and_spends_the_decoder() {
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&raw(5, u32::MAX, b"nope!"));
    let first = decoder.next_message().unwrap_err();
    assert!(matches!(first, WireError::Malformed { .. }));
    assert_eq!(decoder.next_message(), Err(WireError::Spent));
    // finish returns the error that stopped the stream, not Truncated.
    assert_eq!(decoder.finish(), Err(first));
}

#[test]
fn a_stream_cut_mid_frame_is_truncated_at_finish() {
    let bytes = encode(
        &Message::Response {
            id: RequestId(1),
            outcome: Ok(Reply::Ack),
        },
        None,
        FrameLimit::DEFAULT,
    )
    .unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&bytes[..bytes.len() - 1]);
    assert!(decoder.next_message().unwrap().is_none());
    assert_eq!(decoder.finish(), Err(WireError::Truncated));
}

#[test]
fn a_limit_of_u32_max_neither_overflows_nor_buffers_a_frame_it_cannot_finish() {
    let limit = FrameLimit(u32::MAX);
    // The largest JSON with no payload: the frame is exactly the limit, so it waits for its body.
    let mut decoder = Decoder::new(limit);
    decoder.push(&raw(u32::MAX, u32::MAX, &[]));
    assert!(decoder.next_message().unwrap().is_none());
    // JSON and payload that together exceed the limit are refused, without wrapping.
    let mut decoder = Decoder::new(limit);
    decoder.push(&raw(u32::MAX, 1, &[]));
    assert_eq!(
        decoder.next_message(),
        Err(WireError::Oversize {
            len: u64::from(u32::MAX) + 1,
            limit: u64::from(u32::MAX),
        })
    );
}

#[test]
fn a_range_read_frame_round_trips() {
    let message = Message::Request {
        id: RequestId(9),
        body: Request::ReadFile {
            path: crate::message::RemotePath("/a".into()),
            range: ByteRange {
                offset: ByteOffset(0),
                len: ByteCount(4),
            },
        },
    };
    let bytes = encode(&message, None, FrameLimit::DEFAULT).unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&bytes);
    assert_eq!(frame(decoder.next_message().unwrap().unwrap()).0, message);
}

#[test]
fn a_payload_of_exactly_u32_max_bytes_is_unencodable_not_oversize() {
    // Only the length is passed, so no 4 GiB buffer is allocated.
    let result = super::encode::payload_word(u32::MAX as usize, FrameLimit(u32::MAX));
    assert!(
        matches!(result, Err(WireError::Unencodable { .. })),
        "{result:?}"
    );
    // One byte less is an ordinary length.
    assert_eq!(
        super::encode::payload_word(u32::MAX as usize - 1, FrameLimit(u32::MAX)),
        Ok(u32::MAX - 1)
    );
}

#[test]
fn the_frame_size_is_exact_past_u32() {
    let lengths = super::header::Lengths {
        json: u32::MAX as usize,
        payload: Some(u32::MAX as usize),
    };
    assert_eq!(super::header::total(lengths), 8 + 2 * u64::from(u32::MAX));
}
