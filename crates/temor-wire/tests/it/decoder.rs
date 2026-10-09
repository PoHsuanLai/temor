//! The decoder fed in pieces: one byte at a time, and at arbitrary split points, must produce
//! exactly the frames a whole-buffer feed does.

use super::support::{any_message, any_payload, payload_for};
use proptest::prelude::*;
use temor_wire::frame::{Decoder, FrameLimit, encode};
use temor_wire::ids::SessionId;
use temor_wire::message::{Event, Frame, Incoming, Message};

fn decode_all(decoder: &mut Decoder) -> Vec<Incoming> {
    let mut out = Vec::new();
    while let Some(incoming) = decoder.next_message().unwrap() {
        out.push(incoming);
    }
    out
}

#[test]
fn one_byte_at_a_time_yields_each_frame_only_when_its_last_byte_arrives() {
    let message = Message::Event(Event::Output {
        session: SessionId(4),
    });
    let wire = encode(&message, Some(b"hello"), FrameLimit::DEFAULT).unwrap();
    let want = Incoming::Frame(Frame {
        message,
        payload: Some(b"hello".to_vec()),
    });
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    for (index, byte) in wire.iter().enumerate() {
        decoder.push(&[*byte]);
        let frames = decode_all(&mut decoder);
        if index + 1 < wire.len() {
            assert!(frames.is_empty(), "frame emitted early at byte {index}");
        } else {
            assert_eq!(frames, std::slice::from_ref(&want));
        }
    }
    assert!(decoder.finish().is_ok());
}

proptest! {
    #[test]
    fn any_split_of_a_stream_gives_the_same_frames(
        items in prop::collection::vec((any_message(), any_payload()), 1..5),
        cuts in prop::collection::vec(any::<usize>(), 0..16),
    ) {
        let mut wire = Vec::new();
        for (message, bytes) in &items {
            let payload = payload_for(message, bytes.clone());
            wire.extend(encode(message, payload.as_deref(), FrameLimit::DEFAULT).unwrap());
        }
        let mut whole_decoder = Decoder::new(FrameLimit::DEFAULT);
        whole_decoder.push(&wire);
        let whole = decode_all(&mut whole_decoder);

        let mut points: Vec<usize> = cuts.iter().map(|cut| cut % (wire.len() + 1)).collect();
        points.sort_unstable();
        let mut decoder = Decoder::new(FrameLimit::DEFAULT);
        let mut pieces = Vec::new();
        let mut from = 0;
        for point in points.into_iter().chain(std::iter::once(wire.len())) {
            if point < from {
                continue;
            }
            decoder.push(&wire[from..point]);
            pieces.extend(decode_all(&mut decoder));
            from = point;
        }
        prop_assert_eq!(pieces, whole);
        prop_assert!(decoder.finish().is_ok());
    }
}
