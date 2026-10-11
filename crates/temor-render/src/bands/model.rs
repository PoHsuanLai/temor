//! Row bands: the tinted fill, left stripe and top rule a caller lays over runs of viewport
//! rows (temor's command cards). Colours arrive ready-made; this crate only blends them.

use temor_vt::{Rgb, Row};

use crate::scene::{Premul, Quad, QuadPaint, RowScene};

/// The stripe's width in logical pixels (it lives inside the left padding).
pub const STRIPE_LOGICAL: f32 = 3.0;

/// How opaque a tint is, 0 (invisible) to 255 (solid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Alpha(pub u8);

/// A colour laid over what is already drawn, at some opacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tint {
    pub color: Rgb,
    pub alpha: Alpha,
}

impl Tint {
    fn premul(self) -> Option<Premul> {
        let a = u32::from(self.alpha.0);
        if a == 0 {
            return None;
        }
        let m = |v: u8| ((u32::from(v) * a + 127) / 255) as u8;
        Some(Premul([
            m(self.color.r),
            m(self.color.g),
            m(self.color.b),
            self.alpha.0,
        ]))
    }
}

/// Viewport rows `first..=last`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowSpan {
    pub first: Row,
    pub last: Row,
}

impl RowSpan {
    pub fn contains(&self, row: Row) -> bool {
        self.first <= row && row <= self.last
    }
}

/// How heavy a band's text is drawn. `Bold` draws every cell of the rows in the bold face, on top
/// of what the program asked for (a card's command line); `Normal` leaves the cells alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RowWeight {
    #[default]
    Normal,
    Bold,
}

/// Where a band begins in its first row (`top`) or stops in its last (`bottom`): at the row's
/// edge, or across its middle, so a card can share a blank row with its neighbour and keep half a
/// row of room above and below its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cut {
    #[default]
    Edge,
    Middle,
}

/// One band over `rows`: `fill` blends over every background across the pane's full width,
/// `stripe` is a solid bar at the pane's left edge, `rule` is a 1 device pixel line where the
/// band begins in `rows.first`, `weight` can make the text of the rows bold. `top` and `bottom`
/// say where in its end rows the fill, stripe and rule begin and stop; the weight is per row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowBand {
    pub rows: RowSpan,
    pub top: Cut,
    pub bottom: Cut,
    pub fill: Option<Tint>,
    pub stripe: Option<Tint>,
    pub rule: Option<Tint>,
    pub weight: RowWeight,
}

/// The fill and stripe over one half of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct HalfLook {
    pub fill: Option<Tint>,
    pub stripe: Option<Tint>,
}

/// What one row looks like under the bands: the unit change tracking compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct RowLook {
    pub upper: HalfLook,
    pub lower: HalfLook,
    /// Set on a band's first row only, at its edge or across its middle.
    pub rule: Option<(Tint, Cut)>,
    pub weight: RowWeight,
}

/// The look of `row`; where bands overlap the later one wins, per part and per half.
pub(crate) fn look_of(bands: &[RowBand], row: Row) -> RowLook {
    let mut look = RowLook::default();
    for b in bands.iter().filter(|b| b.rows.contains(row)) {
        let upper = !(b.rows.first == row && b.top == Cut::Middle);
        let lower = !(b.rows.last == row && b.bottom == Cut::Middle);
        for (covered, half) in [(upper, &mut look.upper), (lower, &mut look.lower)] {
            if covered {
                half.fill = b.fill.or(half.fill);
                half.stripe = b.stripe.or(half.stripe);
            }
        }
        if b.weight == RowWeight::Bold {
            look.weight = RowWeight::Bold;
        }
        if let (true, Some(rule)) = (b.rows.first == row, b.rule) {
            look.rule = Some((rule, b.top));
        }
    }
    look
}

/// Where a row sits and how wide the pane and stripe are, in device pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RowBox {
    pub y: i32,
    pub height: u32,
    pub pane_width: u32,
    pub stripe_width: u32,
}

fn solid(x: i32, y: i32, w: u32, h: u32, tint: Tint) -> Option<Quad> {
    tint.premul().map(|c| Quad {
        x,
        y,
        w,
        h,
        paint: QuadPaint::Solid(c),
    })
}

/// Adds the fill to `out.bg` (after the cell backgrounds, so it tints them) and the stripe and
/// rule to `out.fg` (over the text). A row whose halves look alike is painted in one piece.
pub(crate) fn push_quads(look: &RowLook, at: RowBox, out: &mut RowScene) {
    let half = at.height / 2;
    let mid = at.y + half as i32;
    let pieces = match look.upper == look.lower {
        true => [Some((at.y, at.height, look.upper)), None],
        false => [
            Some((at.y, half, look.upper)),
            Some((mid, at.height - half, look.lower)),
        ],
    };
    for (y, h, part) in pieces.into_iter().flatten() {
        if let Some(q) = part.fill.and_then(|t| solid(0, y, at.pane_width, h, t)) {
            out.bg.push(q);
        }
        if let Some(q) = part.stripe.and_then(|t| solid(0, y, at.stripe_width, h, t)) {
            out.fg.push(q);
        }
    }
    let rule = look.rule.and_then(|(t, cut)| {
        let y = match cut {
            Cut::Edge => at.y,
            Cut::Middle => mid,
        };
        solid(0, y, at.pane_width, 1, t)
    });
    if let Some(q) = rule {
        out.fg.push(q);
    }
}
