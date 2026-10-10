//! The link under the pointer (PLAN §6): hovering a hyperlink (OSC 8) shows its target in the
//! status line, so the person sees where a click would go. Hovering never opens anything.

use temor_core::vocab::text::display_text;
use temor_vt::{GridPoint, PointSpace};

use super::Session;
use super::pointer::screen_cell;

/// The longest link target shown.
const LINK_MAX_CHARS: usize = 300;

/// Whether the link under the pointer is another since it was last looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkHover {
    Same,
    Changed,
}

impl Session {
    /// The target of the link under the pointer, cleaned for showing. It follows the wheel: the
    /// output moves under a pointer that did not.
    pub fn link_under_pointer(&self) -> Option<String> {
        let at = self.pointer_over?;
        let fit = self.fit?;
        let hit = fit.cell_at(at);
        if hit.region != temor_render::HitRegion::Grid {
            return None;
        }
        let top = self.viewport_top().ok()?;
        let cell = screen_cell(top, hit.pos.row.0, hit.pos.col.0);
        let target = self.pane.with_term(|term| {
            term.hyperlink_at(GridPoint::new(PointSpace::Screen, cell.col, cell.row))
        })?;
        let shown = display_text(&target, LINK_MAX_CHARS);
        (!shown.is_empty()).then_some(shown)
    }

    /// Looks at the link under the pointer again; `Changed` when the status line must be drawn
    /// again.
    pub fn refresh_link(&mut self) -> LinkHover {
        let now = self.link_under_pointer();
        self.note_link(now)
    }

    /// The pointer left the grid: no link is hovered.
    pub fn link_left(&mut self) -> LinkHover {
        self.note_link(None)
    }

    fn note_link(&mut self, now: Option<String>) -> LinkHover {
        match self.link_hover == now {
            true => LinkHover::Same,
            false => {
                self.link_hover = now;
                LinkHover::Changed
            }
        }
    }
}
