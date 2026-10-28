//! Where each node sits: the normalised square Ghostty calls the spatial view (used to move
//! focus and to scale a resize), and whole-pixel rectangles for a given window size. Both come
//! from one integer partition, so the parts of a split always add up to the split exactly.

use super::model::{Axis, Handle, Node, SplitTree};
use ds_core::geometry::units::{Point, Px, Rect, Size};
use temor_core::vocab::ids::PaneId;

/// The side of the normalised square, in spatial units. Far more than [`MOST_PANES`] units, so
/// every pane keeps a slot of its own however deep the tree (see `slots`); small enough that
/// squared distances fit an `i128` with room to spare.
///
/// [`MOST_PANES`]: super::MOST_PANES
pub const SPATIAL_UNIT: u64 = 1 << 30;

/// A length in whole pixels. Panes snap to whole pixels (and the renderer then to whole cells),
/// so layout is integer and tiles exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct WholePx(pub u32);

/// A pane's rectangle in whole pixels, from the tab's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PaneRect {
    /// Distance from the left edge.
    pub x: WholePx,
    /// Distance from the top edge.
    pub y: WholePx,
    /// Width.
    pub width: WholePx,
    /// Height.
    pub height: WholePx,
}

impl PaneRect {
    /// The rectangle in the design system's logical pixels, for a view to place.
    pub fn to_rect(self) -> Rect {
        // u32 -> f32 rounds above 2^24 px, far beyond any window.
        let px = |v: WholePx| Px(v.0 as f32);
        Rect {
            origin: Point {
                x: px(self.x),
                y: px(self.y),
            },
            size: Size {
                width: px(self.width),
                height: px(self.height),
            },
        }
    }
}

/// One node's place, in whatever unit the partition ran in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Slot {
    pub(super) x: u64,
    pub(super) y: u64,
    pub(super) width: u64,
    pub(super) height: u64,
}

impl Slot {
    pub(super) fn max_x(self) -> u64 {
        self.x + self.width
    }

    pub(super) fn max_y(self) -> u64 {
        self.y + self.height
    }
}

impl SplitTree {
    /// Each pane's rectangle in a tab of `width` by `height`, in reading order. The rectangles
    /// tile the tab exactly. While a pane is zoomed it alone is laid out, over the whole tab.
    pub fn layout(&self, width: WholePx, height: WholePx) -> Vec<(PaneId, PaneRect)> {
        let whole = PaneRect {
            x: WholePx(0),
            y: WholePx(0),
            width,
            height,
        };
        if let Some(pane) = self.zoomed {
            return vec![(pane, whole)];
        }
        let slots = self.slots(u64::from(width.0), u64::from(height.0));
        let whole_px = |v: u64| WholePx(u32::try_from(v).unwrap_or(u32::MAX)); // parts of a u32
        self.nodes
            .iter()
            .zip(slots)
            .filter_map(|(node, slot)| match node {
                Node::Leaf(pane) => Some((
                    *pane,
                    PaneRect {
                        x: whole_px(slot.x),
                        y: whole_px(slot.y),
                        width: whole_px(slot.width),
                        height: whole_px(slot.height),
                    },
                )),
                Node::Split { .. } => None,
            })
            .collect()
    }

    /// The panes in reading order: left to right, then top to bottom, by where each pane's
    /// top-left corner stands on screen. This is the one order for the sidebar's segments and for
    /// moving focus to the next or previous pane. (The tree's own order, [`SplitTree::panes`],
    /// is the order the tree nests in.)
    pub fn reading_order(&self) -> Vec<PaneId> {
        let mut leaves: Vec<(u64, u64, PaneId)> = self
            .nodes
            .iter()
            .zip(self.spatial())
            .filter_map(|(node, slot)| match node {
                Node::Leaf(pane) => Some((slot.x, slot.y, *pane)),
                Node::Split { .. } => None,
            })
            .collect();
        // Stable, and two panes never share a corner.
        leaves.sort_by_key(|&(x, y, _)| (x, y));
        leaves.into_iter().map(|(_, _, pane)| pane).collect()
    }

    /// Every node's place in the normalised square, indexed like the node array.
    pub(super) fn spatial(&self) -> Vec<Slot> {
        self.slots(SPATIAL_UNIT, SPATIAL_UNIT)
    }

    /// Every node's place in a `width` by `height` area, indexed like the node array. The array is
    /// in pre-order, so a parent's slot is always filled before its children's.
    ///
    /// A share is a wish, not a promise: each child keeps at least one unit per pane it lays out
    /// along the split's axis, so while the area has room for every pane (at least
    /// [`MOST_PANES`](super::MOST_PANES) units each way) no pane is ever squeezed to nothing.
    fn slots(&self, width: u64, height: u64) -> Vec<Slot> {
        let least = self.least();
        let mut slots = vec![Slot::default(); self.nodes.len()];
        if let Some(root) = slots.first_mut() {
            *root = Slot {
                x: 0,
                y: 0,
                width,
                height,
            };
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if let Node::Split {
                axis,
                ratio,
                first,
                second,
            } = *node
            {
                let part = first_part(
                    ratio.first_part(along(slots[index], axis)),
                    along(slots[index], axis),
                    along(least[first.index()], axis),
                    along(least[second.index()], axis),
                );
                let (a, b) = divide(slots[index], axis, part);
                place(&mut slots, first, a);
                place(&mut slots, second, b);
            }
        }
        slots
    }
}

impl SplitTree {
    /// The least each node needs to show every pane under it: one unit per pane side by side,
    /// as Ghostty's `dimensions` counts them. Children follow their parent in pre-order, so a
    /// walk from the end fills children first.
    fn least(&self) -> Vec<Slot> {
        let mut least = vec![Slot::default(); self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate().rev() {
            least[index] = match *node {
                Node::Leaf(_) => Slot {
                    width: 1,
                    height: 1,
                    ..Slot::default()
                },
                Node::Split {
                    axis,
                    first,
                    second,
                    ..
                } => {
                    let (a, b) = (least[first.index()], least[second.index()]);
                    match axis {
                        Axis::Row => Slot {
                            width: a.width + b.width,
                            height: a.height.max(b.height),
                            ..a
                        },
                        Axis::Column => Slot {
                            width: a.width.max(b.width),
                            height: a.height + b.height,
                            ..a
                        },
                    }
                }
            };
        }
        least
    }
}

/// The first child's length: what its share asks for, held so that each child keeps the least
/// its panes need when `extent` has room for both, and else at least one unit each.
fn first_part(wanted: u64, extent: u64, need_first: u64, need_second: u64) -> u64 {
    if extent >= need_first + need_second {
        wanted.clamp(need_first, extent - need_second)
    } else if extent >= 2 {
        wanted.clamp(1, extent - 1)
    } else {
        wanted
    }
}

fn place(slots: &mut [Slot], at: Handle, slot: Slot) {
    if let Some(target) = slots.get_mut(at.index()) {
        *target = slot;
    }
}

fn along(slot: Slot, axis: Axis) -> u64 {
    match axis {
        Axis::Row => slot.width,
        Axis::Column => slot.height,
    }
}

/// `slot` cut along `axis`, the first part `first` long and the second the rest.
fn divide(slot: Slot, axis: Axis, first: u64) -> (Slot, Slot) {
    match axis {
        Axis::Row => (
            Slot {
                width: first,
                ..slot
            },
            Slot {
                x: slot.x + first,
                width: slot.width - first,
                ..slot
            },
        ),
        Axis::Column => (
            Slot {
                height: first,
                ..slot
            },
            Slot {
                y: slot.y + first,
                height: slot.height - first,
                ..slot
            },
        ),
    }
}
