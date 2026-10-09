//! Random input never panics: every parser and the cleaners return a value or a typed error.

use proptest::prelude::*;
use temor_transfer::Consent;
use temor_transfer::kitty::{Input, ReceiveSession, parse_message};
use temor_transfer::osc1337::{Completeness, Osc1337Reader};
use temor_transfer::{FileName, HostName, Origin, RelPath};

use crate::support::limits;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn random_bytes_never_panic_the_osc1337_reader(payload in proptest::collection::vec(any::<u8>(), 0..512)) {
        let mut reader = Osc1337Reader::new(limits(256));
        let _ = reader.feed(&payload, Completeness::Whole);
    }

    #[test]
    fn random_tails_after_the_osc1337_prefixes_never_panic(
        prefix in prop_oneof![
            Just("1337;File="),
            Just("1337;MultipartFile="),
            Just("1337;FilePart="),
            Just("1337;FileEnd"),
        ],
        tail in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        let mut payload = prefix.as_bytes().to_vec();
        payload.extend_from_slice(&tail);
        let mut reader = Osc1337Reader::new(limits(256));
        let _ = reader.feed(&payload, Completeness::Whole);
        let _ = reader.feed(&payload, Completeness::Cut);
    }

    #[test]
    fn random_bytes_never_panic_the_kitty_parser(payload in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse_message(&payload, limits(256));
        let mut with_prefix = b"5113;".to_vec();
        with_prefix.extend_from_slice(&payload);
        let _ = parse_message(&with_prefix, limits(256));
    }

    #[test]
    fn random_kitty_frames_never_escape_the_receive_machine(
        names in proptest::collection::vec(".{0,40}", 1..6),
    ) {
        let origin = Origin::Host { host: HostName::new("h") };
        let mut session = ReceiveSession::new(origin, limits(1024));
        let send = "5113;ac=send;id=t";
        let (next, _) = session.step(Input::Frame(parse_message(send.as_bytes(), limits(64)).expect("send")));
        session = next;
        let (next, _) = session.step(Input::Decided(Consent::Allow));
        session = next;
        for name in names {
            let encoded = temor_transfer::kitty::encode_message(&temor_transfer::kitty::Message {
                file: temor_transfer::kitty::FileId::parse("1").ok(),
                name: Some(name.clone()),
                ..temor_transfer::kitty::Message::new(
                    temor_transfer::kitty::Action::File,
                    temor_transfer::kitty::KittyId::parse("t").expect("id"),
                )
            });
            let payload = &encoded[2..encoded.len() - 2];
            if let Ok(message) = parse_message(payload, limits(64)) {
                let (next, effects) = session.step(Input::Frame(message));
                for effect in &effects {
                    if let temor_transfer::kitty::Effect::Write(op) = effect {
                        let path = match op {
                            temor_transfer::kitty::WriteOp::MakeDir(p)
                            | temor_transfer::kitty::WriteOp::CreateFile(p) => p,
                            _ => continue,
                        };
                        prop_assert!(!path.components().iter().any(|c| c == ".." || c == "." || c.is_empty() || c.contains('/')));
                    }
                }
                session = next;
            }
        }
    }

    #[test]
    fn random_text_cleans_to_a_safe_name(raw in ".*") {
        let name = FileName::sanitise(&raw);
        prop_assert!(!name.as_str().is_empty());
        prop_assert!(!name.as_str().contains('/'));
        prop_assert!(!name.as_str().contains('\\'));
        prop_assert!(!name.as_str().starts_with('.'));
        prop_assert!(name.as_str().len() <= 200);
    }

    #[test]
    fn a_relative_path_that_parses_has_only_real_components(raw in ".*") {
        if let Ok(path) = RelPath::parse(&raw) {
            for part in path.components() {
                prop_assert!(!part.is_empty() && part != "." && part != "..");
                prop_assert!(!part.contains('/') && !part.contains('\\'));
            }
        }
    }
}
