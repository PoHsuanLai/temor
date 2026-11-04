//! Where each terminal event goes: what the window hears, what the shell answered, what the
//! transfers take, and what is dropped. Pure: the I/O loop carries the route out.

use std::time::Instant;

use temor_vt::{UnknownKind, VtEvent};

use super::event::PaneEvent;

/// What the payload of a shell state report starts with (`temor_shell::report::OSC_NUMBER`).
const REPORT_HEAD: &[u8] = b"7503;";

/// What the payload of a shell's completion answer starts with
/// (`temor_shell::complete::OSC_NUMBER`).
const COMPLETE_HEAD: &[u8] = b"7504;";

/// Where one terminal event goes.
#[derive(Debug)]
pub(super) enum Route {
    /// To the window.
    Send(PaneEvent),
    /// A title other than the last one sent: to the window, and remembered.
    Title(String),
    /// A sequence the terminal does not know: to the transfers, when the pane takes files.
    Unknown {
        kind: UnknownKind,
        payload: Vec<u8>,
        truncated: bool,
    },
    /// Nowhere: the window does not use it, or it says nothing new.
    Drop,
}

/// Where `event`, read at `read`, goes, when `last_title` is the title last sent.
pub(super) fn route(event: VtEvent, read: Instant, last_title: Option<&str>) -> Route {
    match event {
        VtEvent::Unknown {
            kind: UnknownKind::Osc,
            payload,
            truncated: false,
        } if payload.starts_with(REPORT_HEAD) || payload.starts_with(COMPLETE_HEAD) => {
            let (head, make): (_, fn(String) -> PaneEvent) = match payload.starts_with(REPORT_HEAD)
            {
                true => (REPORT_HEAD, PaneEvent::ShellReport),
                false => (COMPLETE_HEAD, PaneEvent::Completions),
            };
            match std::str::from_utf8(&payload[head.len()..]) {
                Ok(text) => Route::Send(make(text.to_owned())),
                Err(_) => Route::Drop,
            }
        }
        VtEvent::Unknown {
            kind,
            payload,
            truncated,
        } => Route::Unknown {
            kind,
            payload,
            truncated,
        },
        VtEvent::Title(title) if last_title == Some(title.as_str()) => Route::Drop,
        VtEvent::Title(title) => Route::Title(title),
        event => map_event(event, read).map_or(Route::Drop, Route::Send),
    }
}

/// The window's view of a terminal event, read at `read`; events the window does not use yet are
/// dropped.
pub(super) fn map_event(event: VtEvent, read: Instant) -> Option<PaneEvent> {
    Some(match event {
        VtEvent::Title(title) => PaneEvent::Title(title),
        VtEvent::Bell => PaneEvent::Bell,
        VtEvent::Cwd(url) => PaneEvent::Cwd(url),
        VtEvent::Clipboard { write } => PaneEvent::Clipboard(write),
        VtEvent::ClipboardRead { read } => PaneEvent::ClipboardRead(read),
        VtEvent::Notification(note) => PaneEvent::Notification(note),
        VtEvent::MarkBegin { seq } => PaneEvent::MarkBegin { seq },
        VtEvent::Prompt(mark) => PaneEvent::Prompt(mark),
        VtEvent::Anchor { seq, anchor } => PaneEvent::Anchor {
            seq,
            anchor,
            at: read,
        },
        VtEvent::Progress(progress) => PaneEvent::Progress(progress),
        VtEvent::Reset => PaneEvent::Reset,
        VtEvent::RenderHold(hold) => PaneEvent::RenderHold(hold),
        VtEvent::ProgramStatus(status) => PaneEvent::Program(status),
        // Colour-scheme queries and unknown sequences have no UI yet
        // (PLAN §4b, §9); they are dropped here, in one place.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use temor_vt::{UnknownKind, VtEvent};

    use super::{Route, route};
    use crate::engine::event::PaneEvent;

    fn osc(payload: &[u8], truncated: bool) -> VtEvent {
        VtEvent::Unknown {
            kind: UnknownKind::Osc,
            payload: payload.to_vec(),
            truncated,
        }
    }

    #[test]
    fn each_event_goes_where_it_belongs() {
        let now = Instant::now();
        #[rustfmt::skip]
        let cases: Vec<(&str, VtEvent, Option<&str>, Route)> = vec![
            ("a shell report", osc(b"7503;cwd=/tmp", false), None, Route::Send(PaneEvent::ShellReport("cwd=/tmp".into()))),
            ("a completion answer", osc(b"7504;ls", false), None, Route::Send(PaneEvent::Completions("ls".into()))),
            ("a report that is not text", osc(b"7503;\xff", false), None, Route::Drop),
            ("a cut-off report goes to the transfers", osc(b"7503;x", true), None, Route::Unknown { kind: UnknownKind::Osc, payload: b"7503;x".to_vec(), truncated: true }),
            ("another sequence goes to the transfers", osc(b"1337;File=", false), None, Route::Unknown { kind: UnknownKind::Osc, payload: b"1337;File=".to_vec(), truncated: false }),
            ("a new title", VtEvent::Title("vim".into()), Some("zsh"), Route::Title("vim".into())),
            ("the same title again", VtEvent::Title("vim".into()), Some("vim"), Route::Drop),
            ("a bell", VtEvent::Bell, None, Route::Send(PaneEvent::Bell)),
        ];
        for (what, event, last, want) in cases {
            // `PaneEvent` has no `PartialEq` (what some of its variants carry has none).
            let got = route(event, now, last);
            assert_eq!(format!("{got:?}"), format!("{want:?}"), "{what}");
        }
    }
}
