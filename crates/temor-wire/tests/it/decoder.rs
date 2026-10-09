//! The decoder fed in pieces: one byte at a time, and at arbitrary split points, must produce
//! exactly the frames a whole-buffer feed does.

use super::support::{any_message, any_payload};
use proptest::prelude::*;
use temor_wire::frame::{Decoder, FrameLimit, encode};
use temor_wire::message::Message;

fn decode_all_at_once(wire: &[u8]) -> Vec<(Message, Vec<u8>)> {
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    decoder.push(wire);
    drain(&mut decoder)
}

fn drain(decoder: &mut Decoder) -> Vec<(Message, Vec<u8>)> {
    let mut out = Vec::new();
    while let Some(frame) = decoder.next_frame::<Message>().unwrap() {
        out.push((frame.message, frame.payload));
    }
    out
}

#[test]
fn one_byte_at_a_time_yields_each_frame_only_when_its_last_byte_arrives() {
    let message = Message::Event(temor_wire::message::Event::Output {
        session: temor_wire::ids::SessionId(4),
    });
    let wire = encode(&message, b"hello", FrameLimit::DEFAULT).unwrap();
    let mut decoder = Decoder::new(FrameLimit::DEFAULT);
    for (index, byte) in wire.iter().enumerate() {
        decoder.push(&[*byte]);
        let frames = drain(&mut decoder);
        if index + 1 < wire.len() {
            assert!(frames.is_empty(), "frame emitted early at byte {index}");
        } else {
            assert_eq!(frames, [(message.clone(), b"hello".to_vec())]);
        }
    }
    assert!(decoder.finish().is_ok());
}

proptest! {
    #[test]
    fn any_split_of_a_stream_gives_the_same_frames(
        messages in prop::collection::vec((any_message(), any_payload()), 1..5),
        cuts in prop::collection::vec(any::<usize>(), 0..16),
    ) {
        let mut wire = Vec::new();
        for (message, payload) in &messages {
            wire.extend(encode(message, payload, FrameLimit::DEFAULT).unwrap());
        }
        let whole = decode_all_at_once(&wire);

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
            pieces.extend(drain(&mut decoder));
            from = point;
        }
        prop_assert_eq!(pieces, whole);
        prop_assert!(decoder.finish().is_ok());
    }
}
