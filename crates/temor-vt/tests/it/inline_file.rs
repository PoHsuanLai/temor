//! `inline_file.bin`: two OSC 1337 `File=` sequences (BEL and ST terminated) are taken out before
//! libghostty and reported, however the bytes are split across writes.

use temor_vt::{UnknownKind, VtEvent};

use crate::support::{drain, fixture, term};

fn run(chunk: usize) -> (Vec<Vec<u8>>, String) {
    let (mut t, rx) = term(80, 4);
    for part in fixture("inline_file.bin").chunks(chunk) {
        t.write(part);
    }
    let files = drain(&rx)
        .into_iter()
        .map(|e| match e {
            VtEvent::Unknown {
                kind: UnknownKind::Osc1337File,
                payload,
                truncated: false,
            } => payload,
            other => panic!("unexpected event {other:?}"),
        })
        .collect();
    let text = temor_vt::Formatter::new(temor_vt::TextFormat::Plain)
        .format(&t, temor_vt::TextSpan::Screen)
        .expect("text");
    (files, text)
}

#[test]
fn inline_files_are_held_back_across_chunk_boundaries() {
    let whole = run(usize::MAX);
    assert_eq!(whole.1, "beforemiddleafter");
    assert_eq!(whole.0.len(), 2);
    assert!(
        whole
            .0
            .iter()
            .all(|p| p.starts_with(b"1337;File=name=YS50eHQ="))
    );
    assert!(whole.0[0].ends_with(b":aGVsbG8sIGlubGluZSBmaWxlCg=="));
    for chunk in [1, 2, 3, 5, 7, 11, 16] {
        assert_eq!(run(chunk), whole, "chunks of {chunk}");
    }
}
