use temor_transfer::osc1337::{Completeness, Dimension, Osc1337Reader, ReceivedFile};
use temor_transfer::{Consent, Malformed, TransferError};

use crate::support::{b64, limits};

fn feed_one(payload: &[u8]) -> Result<Option<ReceivedFile>, TransferError> {
    Osc1337Reader::new(limits(1024)).feed(payload, Completeness::Whole)
}

fn file_payload(args: &str, data: &[u8]) -> Vec<u8> {
    format!("1337;File={args}:{}", b64(data)).into_bytes()
}

#[test]
fn a_whole_offer_decodes_with_its_name_and_data() {
    let payload = file_payload(&format!("name={};size=5", b64(b"a.txt")), b"hello");
    match feed_one(&payload).expect("decodes") {
        Some(ReceivedFile::NeedsConsent(offer)) => {
            assert_eq!(offer.name().as_str(), "a.txt");
            assert_eq!(
                offer.consent(Consent::Allow).expect("allowed").data(),
                b"hello"
            );
        }
        other => panic!("expected an offer, got {other:?}"),
    }
}

#[test]
fn inline_one_is_an_image_with_its_dimensions() {
    let payload = file_payload(
        "inline=1;width=10px;height=50%;preserveAspectRatio=0",
        b"png",
    );
    match feed_one(&payload).expect("decodes") {
        Some(ReceivedFile::Inline(image)) => {
            assert_eq!(image.width, Dimension::Pixels(10));
            assert_eq!(image.height, Dimension::Percent(50));
            assert_eq!(image.data, b"png");
        }
        other => panic!("expected an inline image, got {other:?}"),
    }
}

#[test]
fn inline_zero_is_an_offer_and_auto_is_the_default_size() {
    let payload = file_payload("inline=0;width=auto", b"x");
    assert!(matches!(
        feed_one(&payload),
        Ok(Some(ReceivedFile::NeedsConsent(_)))
    ));
    let payload = file_payload("width=7", b"x");
    match feed_one(&payload).expect("decodes") {
        Some(ReceivedFile::NeedsConsent(_)) => {}
        other => panic!("expected an offer, got {other:?}"),
    }
}

#[test]
fn unknown_keys_are_ignored() {
    let payload = file_payload("future=thing;name=eA==", b"x");
    assert!(matches!(
        feed_one(&payload),
        Ok(Some(ReceivedFile::NeedsConsent(_)))
    ));
}

#[test]
fn a_size_that_disagrees_with_the_data_is_refused() {
    let payload = file_payload("size=9", b"hello");
    assert!(matches!(
        feed_one(&payload),
        Err(TransferError::Malformed(Malformed::SizeMismatch { .. }))
    ));
}

#[test]
fn a_cut_sequence_is_refused_whatever_it_holds() {
    let payload = file_payload("", b"hello");
    let result = Osc1337Reader::new(limits(1024)).feed(&payload, Completeness::Cut);
    assert!(matches!(result, Err(TransferError::Cut)));
}

#[test]
fn bad_base64_and_bad_values_are_typed_errors() {
    assert!(matches!(
        feed_one(b"1337;File=:@@@@"),
        Err(TransferError::Malformed(Malformed::BadBase64))
    ));
    assert!(matches!(
        feed_one(b"1337;File=inline=2:eA=="),
        Err(TransferError::Malformed(Malformed::BadValue {
            key: "inline"
        }))
    ));
    for bad in ["width=101%", "width=ten", "width=-1px", "size=-3"] {
        let payload = file_payload(bad, b"x");
        assert!(feed_one(&payload).is_err(), "{bad} should be refused");
    }
    assert!(matches!(
        feed_one(b"1337;File=type=text"),
        Err(TransferError::Malformed(Malformed::NoSeparator))
    ));
}

#[test]
fn a_payload_over_the_limit_is_too_large() {
    let payload = file_payload("", &[0u8; 64]);
    let result = Osc1337Reader::new(limits(16)).feed(&payload, Completeness::Whole);
    assert!(matches!(result, Err(TransferError::TooLarge { .. })));
}

#[test]
fn a_non_transfer_sequence_is_refused() {
    assert!(matches!(
        feed_one(b"1337;CurrentDir=/tmp"),
        Err(TransferError::Malformed(Malformed::NotATransfer))
    ));
    assert!(matches!(
        feed_one(b"52;c;aGk="),
        Err(TransferError::Malformed(Malformed::NotATransfer))
    ));
}

#[test]
fn hostile_names_are_cleaned_never_used_as_paths() {
    let payload = file_payload(&format!("name={}", b64(b"../../etc/passwd")), b"x");
    match feed_one(&payload).expect("decodes") {
        Some(ReceivedFile::NeedsConsent(offer)) => {
            let name = offer.name().as_str();
            assert!(!name.contains('/'));
            assert!(!name.starts_with('.'));
            assert!(!name.is_empty());
        }
        other => panic!("expected an offer, got {other:?}"),
    }
}

#[test]
fn multipart_pieces_join_in_order_and_end_the_file() {
    let mut reader = Osc1337Reader::new(limits(1024));
    let start = format!("1337;MultipartFile=name={};size=6", b64(b"m.bin"));
    assert_eq!(
        reader
            .feed(start.as_bytes(), Completeness::Whole)
            .expect("start"),
        None
    );
    let text = b64(b"abcdef");
    let (first, second) = text.split_at(4);
    for piece in [first, second] {
        let part = format!("1337;FilePart={piece}");
        assert_eq!(
            reader
                .feed(part.as_bytes(), Completeness::Whole)
                .expect("part"),
            None
        );
    }
    match reader
        .feed(b"1337;FileEnd", Completeness::Whole)
        .expect("end")
    {
        Some(ReceivedFile::NeedsConsent(offer)) => {
            assert_eq!(offer.name().as_str(), "m.bin");
            assert_eq!(
                offer.consent(Consent::Allow).expect("allowed").data(),
                b"abcdef"
            );
        }
        other => panic!("expected the joined file, got {other:?}"),
    }
}

#[test]
fn multipart_order_errors_reset_the_reader() {
    let mut reader = Osc1337Reader::new(limits(1024));
    assert!(matches!(
        reader.feed(b"1337;FilePart=eA==", Completeness::Whole),
        Err(TransferError::Malformed(Malformed::PartWithoutStart))
    ));
    assert!(matches!(
        reader.feed(b"1337;FileEnd", Completeness::Whole),
        Err(TransferError::Malformed(Malformed::EndWithoutStart))
    ));
    reader
        .feed(b"1337;MultipartFile=", Completeness::Whole)
        .expect("start");
    assert!(matches!(
        reader.feed(b"1337;MultipartFile=", Completeness::Whole),
        Err(TransferError::Malformed(Malformed::StartWhileOpen))
    ));
    // The error above dropped the open file, so a fresh start works.
    assert!(matches!(
        reader.feed(b"1337;FileEnd", Completeness::Whole),
        Err(TransferError::Malformed(Malformed::EndWithoutStart))
    ));
    assert!(matches!(
        reader.feed(b"1337;File=:eA==", Completeness::Whole),
        Ok(Some(ReceivedFile::NeedsConsent(_)))
    ));
}

#[test]
fn multipart_text_over_the_cap_is_too_large() {
    let mut reader = Osc1337Reader::new(limits(3));
    reader
        .feed(b"1337;MultipartFile=", Completeness::Whole)
        .expect("start");
    let big = format!("1337;FilePart={}", "A".repeat(20_000));
    assert!(matches!(
        reader.feed(big.as_bytes(), Completeness::Whole),
        Err(TransferError::TooLarge { .. })
    ));
}

#[test]
fn a_refused_offer_yields_no_data() {
    let payload = file_payload(&format!("name={}", b64(b"x.bin")), b"secret");
    match feed_one(&payload).expect("decodes") {
        Some(ReceivedFile::NeedsConsent(offer)) => {
            assert!(offer.consent(Consent::Refuse).is_none())
        }
        other => panic!("expected an offer, got {other:?}"),
    }
}

#[test]
fn a_payload_over_the_raw_cap_is_refused_before_decoding() {
    let mut payload = b"1337;File=:".to_vec();
    payload.resize(20_000, b'A');
    assert!(matches!(
        feed_one(&payload),
        Err(TransferError::TooLarge { .. })
    ));
}
