use super::{Fingerprint, HostId, ShownName, fingerprint_from_keygen};
use crate::error::SshError;

const FP: &str = "SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s";

#[test]
fn parses_keygen_lf_output() {
    let out = format!("256 {FP} example.com (ED25519)\n");
    let fp = fingerprint_from_keygen(&out).expect("fingerprint");
    assert_eq!(fp.as_str(), FP);
}

#[test]
fn skips_comment_lines_from_keygen_f() {
    let out = format!("# Host example.com found: line 3\n256 {FP} example.com (ED25519)\n");
    assert_eq!(
        fingerprint_from_keygen(&out).expect("fingerprint").as_str(),
        FP
    );
}

#[test]
fn missing_or_malformed_fingerprints_are_errors() {
    assert!(matches!(
        fingerprint_from_keygen("nothing here\n"),
        Err(SshError::NoFingerprint)
    ));
    assert!(matches!(
        fingerprint_from_keygen("256 SHA256:short host (ED25519)\n"),
        Err(SshError::BadFingerprint(_))
    ));
    assert!(matches!(
        fingerprint_from_keygen("256 SHA256:$(touch x)aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa host\n"),
        Err(SshError::BadFingerprint(_))
    ));
}

#[test]
fn md5_fingerprints_are_not_accepted_as_identity() {
    let out = "2048 MD5:16:27:ac:a5:76:28:2d:36:63:1b:56:4d:eb:df:a6:48 host (RSA)\n";
    assert!(matches!(
        fingerprint_from_keygen(out),
        Err(SshError::NoFingerprint)
    ));
}

#[test]
fn host_id_pairs_the_fingerprint_with_the_shown_name() {
    let fp = Fingerprint::new(FP).expect("fingerprint");
    let shown = ShownName::new("prod").expect("name");
    let id = HostId::new(fp.clone(), shown);
    assert_eq!(id.fingerprint(), &fp);
    assert_eq!(id.shown().as_str(), "prod");
    assert!(ShownName::new("   ").is_none());
    assert!(ShownName::new("a\nb").is_none());
}

#[test]
fn same_machine_compares_fingerprints_only() {
    const OTHER: &str = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let fp = Fingerprint::new(FP).expect("fingerprint");
    let prod = HostId::new(fp.clone(), ShownName::new("prod").expect("name"));
    let alias = HostId::new(fp, ShownName::new("bastion").expect("name"));
    let elsewhere = HostId::new(
        Fingerprint::new(OTHER).expect("fingerprint"),
        ShownName::new("prod").expect("name"),
    );
    assert!(prod.same_machine(&alias));
    assert!(!prod.same_machine(&elsewhere));
    assert_ne!(
        prod, alias,
        "the shown name still distinguishes the entries"
    );
}
