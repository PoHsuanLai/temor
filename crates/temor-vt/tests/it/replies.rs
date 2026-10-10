//! `replies.bin`: DA1, DSR and DECRQM queries answer through the terminal's `ReplySink`, never
//! lossily; panicking event sinks and full resets.

use std::sync::{Arc, Mutex};

use temor_vt::{
    ClipboardAnswer, ClipboardRead, ClipboardSource, EventSink, GridSize, ReplySink, Terminal,
    TerminalConfig, VtEvent, Wrap,
};

use crate::support::{drain, fixture, term, term_with_replies};

#[test]
fn device_queries_reply_in_order() {
    let (mut t, events, replies) = term_with_replies(80, 24);
    t.write(&fixture("replies.bin"));
    assert_eq!(
        replies.try_iter().collect::<Vec<_>>(),
        [
            b"\x1b[?62;22c".to_vec(),
            b"\x1b[0n".to_vec(),
            b"\x1b[?2004;2$y".to_vec()
        ]
    );
    assert_eq!(drain(&events), [], "replies never travel as events");
}

/// A sink that keeps everything, readable from the test.
#[derive(Clone, Default)]
struct Collect(Arc<Mutex<Vec<Vec<u8>>>>);

impl ReplySink for Collect {
    fn reply(&self, bytes: &[u8]) {
        self.0.lock().expect("lock").push(bytes.to_vec());
    }
}

#[test]
fn a_burst_of_queries_loses_no_reply() {
    // Regression: replies used to share the event channel, and a bounded one dropped them.
    let replies = Collect::default();
    let (tx, _events) = std::sync::mpsc::sync_channel::<VtEvent>(2);
    struct Bounded(std::sync::mpsc::SyncSender<VtEvent>);
    impl EventSink for Bounded {
        fn send(&self, event: VtEvent) {
            let _ = self.0.try_send(event);
        }
    }
    let config = TerminalConfig::new(GridSize::new(80, 24));
    let mut t =
        Terminal::new(config, Box::new(Bounded(tx)), Box::new(replies.clone())).expect("terminal");
    t.write(b"\x07\x07\x07\x1b[5n\x1b[5n\x1b[5n\x1b[5n\x1b[5n");
    assert_eq!(replies.0.lock().expect("lock").len(), 5);
}

struct Panics;

impl EventSink for Panics {
    fn send(&self, _event: VtEvent) {
        panic!("sink failed");
    }
}

#[test]
fn a_panicking_sink_drops_the_event_and_the_terminal_carries_on() {
    let replies = Collect::default();
    let config = TerminalConfig::new(GridSize::new(20, 4));
    let mut t =
        Terminal::new(config, Box::new(Panics), Box::new(replies.clone())).expect("terminal");
    assert_eq!(t.dropped_events(), 0);
    t.write(b"\x07\x07ok\x1b[5n");
    assert_eq!(t.dropped_events(), 2);
    assert_eq!(
        replies.0.lock().expect("lock").as_slice(),
        [b"\x1b[0n".to_vec()]
    );
    assert_eq!(crate::support::row_texts(&mut t, 0).concat(), "ok");
}

#[test]
fn full_reset_is_reported_and_clears_title_and_cwd() {
    let (mut t, rx) = term(20, 4);
    t.write(b"\x1b]2;title\x07\x1b]7;file://h/tmp\x07");
    drain(&rx);
    t.write(b"\x1bc");
    // libghostty clears progress and program status records first, then reports the reset.
    let got = drain(&rx);
    assert_eq!(got.last(), Some(&VtEvent::Reset), "{got:?}");
    assert_eq!(got.iter().filter(|e| **e == VtEvent::Reset).count(), 1);
    assert_eq!((t.title(), t.pwd()), (String::new(), String::new()));
}

#[test]
fn unknown_osc_and_apc_are_reported_with_their_payload() {
    use temor_vt::{Truncation, UnknownKind};
    let (mut t, rx) = term(80, 24);
    t.write(&crate::support::fixture("unknown.bin"));
    let unknown: Vec<(UnknownKind, Vec<u8>, Truncation)> = drain(&rx)
        .into_iter()
        .filter_map(|e| match e {
            VtEvent::Unknown {
                kind,
                payload,
                truncation,
            } => Some((kind, payload, truncation)),
            _ => None,
        })
        .collect();
    assert_eq!(
        unknown,
        [
            (
                UnknownKind::Osc,
                b"5113;ac=send;id=xfer1".to_vec(),
                Truncation::Complete
            ),
            (
                UnknownKind::Apc,
                b"Zsome-apc".to_vec(),
                Truncation::Complete
            ),
        ]
    );
}

#[test]
fn an_osc_52_clipboard_query_gets_no_reply_with_clipboard_data() {
    // A program may ask to read the clipboard (OSC 52 `?`). Without a grant the terminal must
    // not answer with clipboard contents: nothing may come back on the reply channel.
    let replies = Collect::default();
    let (tx, _events) = std::sync::mpsc::channel::<VtEvent>();
    let config = TerminalConfig::new(GridSize::new(80, 24));
    let mut t = Terminal::new(config, Box::new(tx), Box::new(replies.clone())).expect("terminal");
    t.write(b"\x1b]52;c;SGVsbG8=\x07");
    t.write(b"\x1b]52;c;?\x07");
    t.write(b"\x1b]52;c;?\x1b\\");
    let got = replies.0.lock().expect("lock");
    assert!(
        got.iter()
            .all(|reply| !reply.windows(5).any(|w| w == b"Hello" || w == b"SGVsb")),
        "clipboard contents were answered: {got:?}"
    );
}

/// A source that always gives the same answer.
struct Fixed(ClipboardAnswer);

impl ClipboardSource for Fixed {
    fn read(&self, _read: &ClipboardRead) -> ClipboardAnswer {
        self.0.clone()
    }
}

#[test]
fn an_osc_52_read_is_answered_by_the_clipboard_source() {
    // (answer, the reply carries "Hello", a ClipboardRead event is sent)
    let table = [
        (ClipboardAnswer::Text("Hello".into()), true, false),
        (ClipboardAnswer::Empty, false, false),
        (ClipboardAnswer::Denied, false, false),
        (ClipboardAnswer::Ask, false, true),
    ];
    for (answer, replied, asked) in table {
        let replies = Collect::default();
        let (tx, events) = std::sync::mpsc::channel::<VtEvent>();
        let config = TerminalConfig::new(GridSize::new(80, 24));
        let mut t =
            Terminal::new(config, Box::new(tx), Box::new(replies.clone())).expect("terminal");
        t.set_clipboard_source(Box::new(Fixed(answer.clone())));
        t.write(b"\x1b]52;c;?\x07");
        let got = replies.0.lock().expect("lock");
        let hello = got.iter().any(|r| r.windows(8).any(|w| w == b"SGVsbG8="));
        assert_eq!(hello, replied, "{answer:?}: {got:?}");
        let read_events = drain(&events)
            .into_iter()
            .filter(|e| matches!(e, VtEvent::ClipboardRead { .. }))
            .count();
        assert_eq!(read_events, usize::from(asked), "{answer:?}");
    }
}

#[test]
fn paste_safety_is_libghostty_s_check() {
    use temor_vt::paste_is_safe;
    assert!(paste_is_safe(b"echo hi"));
    assert!(!paste_is_safe(b"rm -rf x\n"));
    assert!(!paste_is_safe(b"a\x1b[201~b"));
}

#[test]
fn a_soft_wrapped_row_is_reported_as_wrapped() {
    let (mut t, _events) = term(10, 4);
    t.write(b"abcdefghijklmno\r\nxyz");
    assert_eq!(
        t.row_wraps(0).expect("row 0"),
        Wrap::Continues,
        "the first row continues on the second"
    );
    assert_eq!(
        t.row_wraps(1).expect("row 1"),
        Wrap::Ends,
        "the second row ends with a hard break"
    );
}
