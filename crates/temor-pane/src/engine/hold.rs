//! The frame captured when the program starts holding the screen (synchronized output, PLAN §4b).
//!
//! libghostty's advice: when a hold begins the terminal holds exactly the frame the program wants
//! left on screen, so capture it then. The terminal calls [`HoldCapture`] at that point, inside a
//! write. The window's painter asks [`Term::paint_source`] under the terminal's lock for what to
//! draw: the live terminal, or the captured frame until the hold ends or its timeout runs out.
//!
//! Capturing reads libghostty's dirty marks, which the painter's own capture also consumes, so a
//! painter that sees `captures` change starts a new capture of its own (a full redraw).

use std::sync::{Arc, Mutex, MutexGuard};

use temor_render::{FrameCapture, PaneFrame};
use temor_vt::{HoldHook, RenderHold, Terminal};

use super::term::Term;
use crate::error::PaneError;

/// What became of the capture for the current hold.
#[derive(Debug, Default)]
enum Captured {
    /// None was made (the hold began in a form that is not cut, or the capture failed).
    #[default]
    Missed,
    /// Made, and not yet taken by a painter.
    Ready(PaneFrame),
    /// Taken by a painter.
    Delivered,
}

/// What the hook leaves for the pane.
#[derive(Debug, Default)]
pub(super) struct HoldShared {
    captured: Captured,
    /// How many times a capture was tried (each consumes libghostty's dirty marks).
    captures: u64,
}

pub(super) type Shared = Arc<Mutex<HoldShared>>;

fn lock(shared: &Shared) -> MutexGuard<'_, HoldShared> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The terminal's hold hook: captures the frame into the shared state.
#[derive(Debug)]
pub(super) struct HoldCapture {
    shared: Shared,
}

impl HoldCapture {
    pub(super) fn new(shared: Shared) -> Self {
        Self { shared }
    }
}

impl HoldHook for HoldCapture {
    fn held(&mut self, term: &mut Terminal) {
        let mut shared = lock(&self.shared);
        shared.captured = Captured::Missed;
        shared.captures += 1;
        let Ok(mut capture) = FrameCapture::new() else {
            return;
        };
        let mut frame = PaneFrame::new();
        let Ok(begun) = capture.begin(term) else {
            return;
        };
        if begun.finish(&mut frame).is_ok() {
            shared.captured = Captured::Ready(frame);
        }
    }
}

/// What the painter draws.
#[derive(Debug)]
#[non_exhaustive]
pub enum PaintSource {
    /// The terminal as it is. `captures` counts the hold captures made so far: when it differs
    /// from the last paint's, the painter's own capture missed marks and must start over.
    Live { captures: u64 },
    /// A hold began and this is its frame, every row to be drawn. Later paints during the same
    /// hold get [`PaintSource::Held`].
    Fresh(Box<PaneFrame>),
    /// The same hold as the painter's last paint: keep drawing the frame it took.
    Held,
    /// The program holds the screen but no frame was captured: draw nothing new.
    Skip,
}

impl Term {
    /// What to draw now. Call it under the pane's lock, in the same hold of the lock as the
    /// capture it decides, so a frame is never begun in the middle of a hold.
    pub fn paint_source(&mut self) -> PaintSource {
        let mut shared = lock(&self.hold);
        if self.terminal.render_hold() == RenderHold::Released {
            shared.captured = Captured::Missed;
            return PaintSource::Live {
                captures: shared.captures,
            };
        }
        match std::mem::replace(&mut shared.captured, Captured::Delivered) {
            Captured::Ready(frame) => PaintSource::Fresh(Box::new(frame)),
            Captured::Delivered => PaintSource::Held,
            Captured::Missed => {
                shared.captured = Captured::Missed;
                PaintSource::Skip
            }
        }
    }

    /// Ends the program's hold: its timeout ran out, or the window wants live content. The
    /// program's later output shows as it arrives.
    pub fn end_hold(&mut self) -> Result<(), PaneError> {
        lock(&self.hold).captured = Captured::Missed;
        Ok(self.terminal.end_render_hold()?)
    }
}
