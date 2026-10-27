//! The cursor's blink (PLAN §5), as Ghostty's: it blinks when the terminal asks for it (the
//! default, until a program asks for a steady cursor) and only in the focused pane, and it shows
//! solid again from each key, paste or focus, so it never goes dark while the person types. The
//! halves are counted on the session's clock from that moment; the window's timer repaints the
//! pane when a half turns (`blink_due`).

use ds_core::time::stamp::Stamp;
use temor_client::model::notify::Focus;
use temor_panes::pane::settings::CursorBlinking;
use temor_panes::render::BlinkPhase;

use super::Session;

/// How long each half of a blink lasts, in milliseconds.
pub const BLINK_MS: u64 = 600;

/// Where the blink stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Blink {
    pub(super) setting: CursorBlinking,
    /// When the cursor last showed solid from the start of an `On` half.
    pub(super) from: Stamp,
    /// The half the last paint drew.
    pub(super) painted: BlinkPhase,
}

/// The half `ms` after the blink began: `On` first.
pub fn blink_phase(ms: u64) -> BlinkPhase {
    match (ms / BLINK_MS) % 2 {
        0 => BlinkPhase::On,
        _ => BlinkPhase::Off,
    }
}

impl Session {
    /// The half the cursor is in now; always `On` when blinking is off.
    pub(super) fn blink_now(&self) -> BlinkPhase {
        match self.blink.setting {
            CursorBlinking::On => blink_phase(self.core().clock.now().since(self.blink.from)),
            CursorBlinking::Off => BlinkPhase::On,
        }
    }

    /// The cursor shows solid from now, its blink starting over.
    pub(super) fn wake_cursor(&mut self) {
        let now = self.core().clock.now();
        self.blink.from = now;
    }

    /// Whether the pane must be painted again for its cursor: it is focused, its cursor blinks,
    /// and the half has turned since the last paint.
    pub fn blink_due(&self) -> bool {
        let focused = self.core().focus == Focus::Focused;
        focused && self.painter.cursor_blinks() && self.blink_now() != self.blink.painted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_halves_alternate_from_on() {
        // (ms since the blink began, the half)
        for (ms, want) in [
            (0, BlinkPhase::On),
            (BLINK_MS - 1, BlinkPhase::On),
            (BLINK_MS, BlinkPhase::Off),
            (2 * BLINK_MS - 1, BlinkPhase::Off),
            (2 * BLINK_MS, BlinkPhase::On),
            (7 * BLINK_MS + 5, BlinkPhase::Off),
        ] {
            assert_eq!(blink_phase(ms), want, "{ms} ms");
        }
    }
}
