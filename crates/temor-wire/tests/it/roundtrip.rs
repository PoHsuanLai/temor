//! Every message survives encode and decode, with its payload exactly when it carries one.

use super::support::{any_message, any_payload, payload_for};
use proptest::prelude::*;
use temor_wire::frame::{Decoder, FrameLimit, encode};
use temor_wire::message::{Frame, Incoming};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_message_round_trips_with_its_payload(
        message in any_message(),
        bytes in any_payload(),
    ) {
        let payload = payload_for(&message, bytes);
        let wire = encode(&message, payload.as_deref(), FrameLimit::DEFAULT).unwrap();
        let mut decoder = Decoder::new(FrameLimit::DEFAULT);
        decoder.push(&wire);
        let expected = Incoming::Frame(Frame { message, payload });
        prop_assert_eq!(decoder.next_message().unwrap(), Some(expected));
        prop_assert!(decoder.next_message().unwrap().is_none());
        prop_assert!(decoder.finish().is_ok());
    }

    #[test]
    fn a_stream_of_messages_comes_back_in_order(
        items in prop::collection::vec((any_message(), any_payload()), 0..8),
    ) {
        let mut wire = Vec::new();
        let mut expected = Vec::new();
        for (message, bytes) in items {
            let payload = payload_for(&message, bytes);
            wire.extend(encode(&message, payload.as_deref(), FrameLimit::DEFAULT).unwrap());
            expected.push(Incoming::Frame(Frame { message, payload }));
        }
        let mut decoder = Decoder::new(FrameLimit::DEFAULT);
        decoder.push(&wire);
        for want in expected {
            prop_assert_eq!(decoder.next_message().unwrap(), Some(want));
        }
        prop_assert!(decoder.next_message().unwrap().is_none());
    }
}
