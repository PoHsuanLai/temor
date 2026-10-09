use super::*;
use crate::WireError;
use crate::ids::RequestId;
use crate::message::Request;
use crate::message::{Message, Reply};

fn ask(id: u64) -> Message {
    Message::Request {
        id: RequestId(id),
        body: Request::ListSessions,
    }
}

#[test]
fn a_frame_round_trips_its_message_and_payload() {
    let bytes = encode(&ask(1), &[1, 2, 3], FrameLimit::DEFAULT).unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&bytes);
    let frame = decoder.next_frame::<Message>().unwrap().unwrap();
    assert_eq!(frame.message, ask(1));
    assert_eq!(frame.payload, [1, 2, 3]);
    assert!(decoder.next_frame::<Message>().unwrap().is_none());
    assert!(decoder.finish().is_ok());
}

#[test]
fn the_header_is_two_little_endian_lengths() {
    let bytes = encode(&ask(1), &[9; 5], FrameLimit::DEFAULT).unwrap();
    let json = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let payload = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(payload, 5);
    assert_eq!(bytes.len(), HEADER + json + 5);
}

#[test]
fn back_to_back_frames_are_cut_apart_and_the_buffer_is_reclaimed() {
    let mut wire = Vec::new();
    for id in 0..50 {
        wire.extend(encode(&ask(id), &[id as u8; 100], FrameLimit::DEFAULT).unwrap());
    }
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&wire);
    let mut seen = 0;
    while let Some(frame) = decoder.next_frame::<Message>().unwrap() {
        assert_eq!(frame.message, ask(seen));
        assert_eq!(frame.payload.len(), 100);
        seen += 1;
    }
    assert_eq!(seen, 50);
    assert!(decoder.finish().is_ok());
}

#[test]
fn a_frame_over_the_limit_is_refused_from_its_header_alone() {
    // The header claims 2^31 bytes of payload; only the header is ever pushed.
    let mut header = Vec::new();
    header.extend_from_slice(&2u32.to_le_bytes());
    header.extend_from_slice(&(1u32 << 31).to_le_bytes());
    let mut decoder = Decoder::new(FrameLimit(1024));
    decoder.push(&header);
    assert_eq!(
        decoder.next_frame::<Message>(),
        Err(WireError::Oversize {
            len: (1u64 << 31) + 2,
            limit: 1024,
        })
    );
    assert_eq!(decoder.next_frame::<Message>(), Err(WireError::Spent));
}

#[test]
fn encoding_refuses_a_frame_over_the_limit() {
    let result = encode(&ask(1), &[0; 2048], FrameLimit(1024));
    assert!(matches!(
        result,
        Err(WireError::Oversize { limit: 1024, .. })
    ));
}

#[test]
fn json_that_is_not_a_message_is_malformed_and_spends_the_decoder() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(b"nope!");
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&bytes);
    assert!(matches!(
        decoder.next_frame::<Message>(),
        Err(WireError::Malformed { .. })
    ));
    assert_eq!(decoder.next_frame::<Message>(), Err(WireError::Spent));
}

#[test]
fn a_stream_cut_mid_frame_is_truncated_at_finish() {
    let bytes = encode(&Reply::Ack, &[], FrameLimit::DEFAULT).unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(&bytes[..bytes.len() - 1]);
    assert!(decoder.next_frame::<Reply>().unwrap().is_none());
    assert_eq!(decoder.finish(), Err(WireError::Truncated));
}
