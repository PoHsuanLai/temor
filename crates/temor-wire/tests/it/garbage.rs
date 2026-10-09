//! Hostile and broken input never panics: whatever arrives, the decoder either yields frames,
//! asks for more, or returns a typed error and stays spent.

use proptest::prelude::*;
use temor_wire::WireError;
use temor_wire::frame::{Decoder, FrameLimit};
use temor_wire::message::Message;

/// Feeds bytes to a decoder with a small limit, pulling frames as it goes, and reports how it
/// ended. Panicking here is the failure this test exists to catch.
fn feed(bytes: &[u8], limit: FrameLimit) -> Result<usize, WireError> {
    let mut decoder = Decoder::new(limit);
    decoder.push(bytes);
    let mut frames = 0;
    loop {
        match decoder.next_frame::<Message>() {
            Ok(Some(_)) => frames += 1,
            Ok(None) => return decoder.finish().map(|()| frames),
            Err(error) => {
                // Once refused, the decoder must stay refused.
                assert_eq!(decoder.next_frame::<Message>(), Err(WireError::Spent));
                return Err(error);
            }
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let _ = feed(&bytes, FrameLimit(512));
    }

    #[test]
    fn arbitrary_headers_over_arbitrary_bodies_never_panic(
        json_len in any::<u32>(),
        payload_len in any::<u32>(),
        body in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&json_len.to_le_bytes());
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes.extend_from_slice(&body);
        let _ = feed(&bytes, FrameLimit::DEFAULT);
    }

    #[test]
    fn valid_json_of_the_wrong_shape_is_malformed_not_a_panic(
        text in "\\PC{0,64}",
    ) {
        let json = format!("\"{}\"", text.replace(['\\', '"'], ""));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(json.as_bytes());
        let result = feed(&bytes, FrameLimit::DEFAULT);
        let malformed = matches!(result, Err(WireError::Malformed { .. }));
        prop_assert!(malformed, "expected Malformed, got {:?}", result);
    }
}

#[test]
fn a_header_claiming_the_maximum_is_refused_before_anything_is_buffered() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        feed(&bytes, FrameLimit::DEFAULT),
        Err(WireError::Oversize { .. })
    ));
}

#[test]
fn a_frame_exactly_at_the_limit_is_accepted_and_one_byte_over_is_not() {
    let json = b"\"x\"";
    let make = |payload: usize| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(payload as u32).to_le_bytes());
        bytes.extend_from_slice(json);
        bytes.extend(std::iter::repeat_n(0u8, payload));
        bytes
    };
    let limit = FrameLimit(100);
    let at_limit = make(100 - json.len());
    let mut decoder = Decoder::new(limit);
    decoder.push(&at_limit);
    // The body is valid JSON for a string, not a Message, so the limit is what this test checks:
    // the error must be Malformed (the frame was accepted), never Oversize.
    assert!(matches!(
        decoder.next_frame::<Message>(),
        Err(WireError::Malformed { .. })
    ));
    let over = make(100 - json.len() + 1);
    let mut decoder = Decoder::new(limit);
    decoder.push(&over);
    assert!(matches!(
        decoder.next_frame::<Message>(),
        Err(WireError::Oversize {
            len: 101,
            limit: 100
        })
    ));
}
