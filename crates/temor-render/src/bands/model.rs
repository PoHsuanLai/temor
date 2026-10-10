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

/// One band over `rows`: `fill` blends over every background across the pane's full width,
/// `stripe` is a solid bar at the pane's left edge, `rule` is a 1 device pixel line along the
/// top edge of `rows.first`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowBand {
    pub rows: RowSpan,
    pub fill: Option<Tint>,
    pub stripe: Option<Tint>,
    pub rule: Option<Tint>,
}

/// What one row looks like under the bands: the unit change tracking compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct RowLook {
    pub fill: Option<Tint>,
    pub stripe: Option<Tint>,
    /// Set on the band's first row only.
    pub rule: Option<Tint>,
}

/// The look of `row`; where bands overlap the later one wins, per part.
pub(crate) fn look_of(bands: &[RowBand], row: Row) -> RowLook {
    let mut look = RowLook::default();
    for b in bands.iter().filter(|b| b.rows.contains(row)) {
        look.fill = b.fill.or(look.fill);
        look.stripe = b.stripe.or(look.stripe);
        if b.rows.first == row {
            look.rule = b.rule.or(look.rule);
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
/// rule to `out.fg` (over the text).
pub(crate) fn push_quads(look: &RowLook, at: RowBox, out: &mut RowScene) {
    if let Some(q) = look
        .fill
        .and_then(|t| solid(0, at.y, at.pane_width, at.height, t))
    {
        out.bg.push(q);
    }
    if let Some(q) = look
        .stripe
        .and_then(|t| solid(0, at.y, at.stripe_width, at.height, t))
    {
        out.fg.push(q);
    }
    if let Some(q) = look.rule.and_then(|t| solid(0, at.y, at.pane_width, 1, t)) {
        out.fg.push(q);
    }
}
