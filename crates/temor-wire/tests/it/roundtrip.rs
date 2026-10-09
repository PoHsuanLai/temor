//! Every message survives encode and decode, with its payload, at any size the limit allows.

use super::support::{any_message, any_payload};
use proptest::prelude::*;
use temor_wire::frame::{Decoder, FrameLimit, encode};
use temor_wire::message::Message;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_message_round_trips_with_its_payload(
        message in any_message(),
        payload in any_payload(),
    ) {
        let bytes = encode(&message, &payload, FrameLimit::DEFAULT).unwrap();
        let mut decoder = Decoder::new(FrameLimit::DEFAULT);
        decoder.push(&bytes);
        let frame = decoder.next_frame::<Message>().unwrap().unwrap();
        prop_assert_eq!(frame.message, message);
        prop_assert_eq!(frame.payload, payload);
        prop_assert!(decoder.next_frame::<Message>().unwrap().is_none());
        prop_assert!(decoder.finish().is_ok());
    }

    #[test]
    fn a_stream_of_messages_comes_back_in_order(
        messages in prop::collection::vec((any_message(), any_payload()), 0..8),
    ) {
        let mut wire = Vec::new();
        for (message, payload) in &messages {
            wire.extend(encode(message, payload, FrameLimit::DEFAULT).unwrap());
        }
        let mut decoder = Decoder::new(FrameLimit::DEFAULT);
        decoder.push(&wire);
        for (message, payload) in messages {
            let frame = decoder.next_frame::<Message>().unwrap().unwrap();
            prop_assert_eq!(frame.message, message);
            prop_assert_eq!(frame.payload, payload);
        }
        prop_assert!(decoder.next_frame::<Message>().unwrap().is_none());
    }
}
