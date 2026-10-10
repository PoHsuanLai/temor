//! A render hold that outlives its write is captured where it began (PLAN §4b).

use std::sync::{Arc, Mutex};

use temor_vt::{HoldHook, RenderHold, Terminal, Vt};

use crate::support::term;

/// Records the screen's text each time a hold begins.
#[derive(Clone, Default)]
struct Captures(Arc<Mutex<Vec<String>>>);

impl HoldHook for Captures {
    fn held(&mut self, term: &mut Terminal) {
        let text = term.plain_text().expect("text");
        self.0.lock().expect("lock").push(text.trim().to_owned());
    }
}

impl Captures {
    fn seen(&self) -> Vec<String> {
        self.0.lock().expect("lock").clone()
    }
}

#[test]
fn the_frame_is_captured_before_what_follows_the_hold_starts() {
    // (writes, what the hook saw)
    let table: [(&[&[u8]], &[&str]); 4] = [
        (&[b"before\x1b[?2026hduring"], &["before"]),
        (&[b"before\x1b[?2026h", b"during"], &["before"]),
        (&[b"a\x1b[?2026hb\x1b[?2026lc"], &[]),
        (&[b"a\x1b[?2026hb\x1b[?2026l", b"c\x1b[?2026hd"], &["abc"]),
    ];
    for (writes, want) in table {
        let (mut t, _events) = term(40, 5);
        let hook = Captures::default();
        t.set_hold_hook(Box::new(hook.clone()));
        for w in writes {
            t.write(w);
        }
        assert_eq!(hook.seen(), want, "{writes:?}");
    }
}

#[test]
fn the_host_can_end_a_hold_the_program_never_releases() {
    let (mut t, _events) = term(40, 5);
    t.write(b"\x1b[?2026hdrawing");
    assert_eq!(t.render_hold(), RenderHold::Held);
    t.end_render_hold().expect("ends");
    assert_eq!(t.render_hold(), RenderHold::Released);
    // The program may start another hold afterwards.
    t.write(b"\x1b[?2026h");
    assert_eq!(t.render_hold(), RenderHold::Held);
}
