//! The link under the pointer (PLAN §6): hovering a hyperlink (OSC 8) shows its target in a small
//! tooltip by the pointer, so the person sees where a click would go. Hovering never opens
//! anything.

use temor_core::vocab::text::display_text;
use temor_vt::{GridPoint, PointSpace};

use super::Session;

/// The longest link target shown.
const LINK_MAX_CHARS: usize = 300;

/// Whether the link under the pointer is another since it was last looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkHover {
    Same,
    Changed,
}

/// A hovered link's target and where the pointer is, in logical pixels from the grid's corner.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkTipSpot {
    pub target: String,
    pub x: f32,
    pub y: f32,
}

impl Session {
    /// The tooltip for the link under the pointer, if there is one.
    pub fn link_tip(&self) -> Option<LinkTipSpot> {
        let target = self.link_under_pointer()?;
        let at = self.pointer_over?;
        let factor = self.scale.factor();
        Some(LinkTipSpot {
            target,
            x: at.x as f32 / factor,
            y: at.y as f32 / factor,
        })
    }

    /// The target of the link under the pointer, cleaned for showing. It follows the wheel: the
    /// output moves under a pointer that did not.
    pub fn link_under_pointer(&self) -> Option<String> {
        let at = self.pointer_over?;
        let hit = self.point_hit(at)?;
        if hit.region != temor_render::HitRegion::Grid {
            return None;
        }
        let cell = hit.cell;
        let target = self.pane.with_term(|term| {
            term.hyperlink_at(GridPoint::new(PointSpace::Screen, cell.col, cell.row))
        })?;
        let shown = display_text(&target, LINK_MAX_CHARS);
        (!shown.is_empty()).then_some(shown)
    }

    /// Looks at the link under the pointer again; `Changed` when the tooltip must be drawn
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
