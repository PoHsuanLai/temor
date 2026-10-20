//! Moving focus between panes: by reading order, or to the nearest pane on screen in a
//! direction, as Ghostty does it (nearest top-left corner among the panes wholly on that side,
//! wrapping round the window's edge when there is none).

use super::layout::{SPATIAL_UNIT, Slot};
use super::model::{Direction, Goto, Node, SplitTree};
use crate::vocab::ids::PaneId;

/// A slot's corner, shifted for wrapping, so it can sit outside the square.
#[derive(Debug, Clone, Copy)]
struct Target {
    x: i128,
    y: i128,
    max_x: i128,
    max_y: i128,
}

impl Target {
    fn of(slot: Slot) -> Target {
        Target {
            x: i128::from(slot.x),
            y: i128::from(slot.y),
            max_x: i128::from(slot.max_x()),
            max_y: i128::from(slot.max_y()),
        }
    }

    /// This target moved one whole square the other way, so that the panes on the far edge
    /// count as lying in `direction`.
    fn wrapped(self, direction: Direction) -> Target {
        let unit = i128::from(SPATIAL_UNIT);
        let (dx, dy) = match direction {
            Direction::Left => (unit, 0),
            Direction::Right => (-unit, 0),
            Direction::Up => (0, unit),
            Direction::Down => (0, -unit),
        };
        Target {
            x: self.x + dx,
            y: self.y + dy,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
        }
    }
}

impl SplitTree {
    /// The pane focus moves to from `from`, or none when there is nowhere to go. A pane not in
    /// the tree goes nowhere.
    pub fn goto(&self, from: PaneId, to: Goto) -> Option<PaneId> {
        let panes = self.reading_order();
        let at = panes.iter().position(|pane| *pane == from)?;
        let last = panes.len() - 1;
        match to {
            Goto::Previous => at.checked_sub(1).map(|index| panes[index]),
            Goto::Next => panes.get(at + 1).copied(),
            Goto::PreviousWrapped => Some(panes[at.checked_sub(1).unwrap_or(last)]),
            Goto::NextWrapped => Some(panes[if at == last { 0 } else { at + 1 }]),
            Goto::Spatial(direction) => self.nearest_wrapped(from, direction),
        }
    }

    fn nearest_wrapped(&self, from: PaneId, direction: Direction) -> Option<PaneId> {
        let slots = self.spatial();
        let target = Target::of(slots[self.handle_of(from)?.index()]);
        self.nearest(&slots, from, direction, target)
            .or_else(|| self.nearest(&slots, from, direction, target.wrapped(direction)))
    }

    /// The pane wholly on `direction`'s side of `target` whose top-left corner is closest to
    /// `target`'s; the first in reading order wins a tie.
    fn nearest(
        &self,
        slots: &[Slot],
        from: PaneId,
        direction: Direction,
        target: Target,
    ) -> Option<PaneId> {
        let mut best: Option<(i128, PaneId)> = None;
        for (node, slot) in self.nodes.iter().zip(slots) {
            let Node::Leaf(pane) = *node else { continue };
            if pane == from {
                continue;
            }
            let here = Target::of(*slot);
            let beyond = match direction {
                Direction::Left => here.max_x <= target.x,
                Direction::Right => here.x >= target.max_x,
                Direction::Up => here.max_y <= target.y,
                Direction::Down => here.y >= target.max_y,
            };
            if !beyond {
                continue;
            }
            let (dx, dy) = (here.x - target.x, here.y - target.y);
            let distance = dx * dx + dy * dy;
            if best.is_none_or(|(closest, _)| distance < closest) {
                best = Some((distance, pane));
            }
        }
        best.map(|(_, pane)| pane)
    }
}
