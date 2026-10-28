//! Glyphs that reach outside their row (tall accents, deep descenders) are drawn whole, as on
//! the GPU path, so a row's band shows the overhang of the rows above and below it, and a row
//! whose glyphs overhang (now or before) takes its neighbours' bands with it when it changes.

use std::collections::BTreeSet;

use temor_vt::Row;

use crate::scene::{QuadPaint, RowScene};

/// Whether any glyph of `scene` reaches outside the row spanning `y..y + height`.
pub(crate) fn overhang_of(scene: &RowScene, y: i32, height: u32) -> bool {
    let bottom = i64::from(y) + i64::from(height);

    scene.fg.iter().any(|q| {
        matches!(q.paint, QuadPaint::Mask { .. } | QuadPaint::Color { .. })
            && (q.y < y || i64::from(q.y) + i64::from(q.h) > bottom)
    })
}

/// What is known of the rows beside the one being composed: whether their glyphs may reach into
/// it. [`false`] is certain; [`true`] means "build their glyphs to see".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sides {
    /// Some glyph reaches above the row.
    pub above: bool,
    /// Some glyph reaches below the row.
    pub below: bool,
}

/// How each row looked the last time it was composed.
#[derive(Debug, Default)]
pub(crate) struct Overhangs {
    rows: Vec<bool>,
}

impl Overhangs {
    /// Forgets everything when the grid's height changed.
    pub fn fit(&mut self, rows: u16) {
        if self.rows.len() != usize::from(rows) {
            self.rows = vec![false; usize::from(rows)];
        }
    }

    /// How `row` looked the last time it was composed; a row off the grid stays within.
    fn get(&self, row: u16) -> bool {
        self.rows.get(usize::from(row)).copied().unwrap_or_default()
    }

    /// Records `now` for `row` and returns what it was.
    fn replace(&mut self, row: Row, now: bool) -> bool {
        self.rows
            .get_mut(usize::from(row.0))
            .is_some_and(|slot| std::mem::replace(slot, now))
    }
}

/// Composes `dirty` in order through `compose` (which is told what is known of the rows beside
/// the one it composes, and reports that row's new overhang), and each
/// dirty row's neighbours too when it overhangs now or did before: their bands hold, or are to
/// hold, a piece of its glyphs. A neighbour composed earlier already drew the row's current
/// glyphs. A row beside the one composed counts as overhanging when it overhung when last
/// composed, or when it is still to be composed in this pass (what it was is then no guide).
/// Returns the rows composed, in order.
pub(crate) fn compose_with_neighbours<E>(
    dirty: &[Row],
    total: u16,
    overhangs: &mut Overhangs,
    mut compose: impl FnMut(Row, Sides) -> Result<bool, E>,
) -> Result<Vec<Row>, E> {
    let asked: BTreeSet<u16> = dirty.iter().map(|r| r.0).collect();
    let mut todo = asked.clone();
    let mut done = BTreeSet::new();
    while let Some(r) = todo.pop_first() {
        let reach = |n: u16| match asked.contains(&n) && !done.contains(&n) {
            true => true,
            false => overhangs.get(n),
        };
        let sides = Sides {
            above: r.checked_sub(1).is_some_and(reach),
            below: r.checked_add(1).filter(|n| *n < total).is_some_and(reach),
        };
        let now = compose(Row(r), sides)?;
        let before = overhangs.replace(Row(r), now);
        done.insert(r);
        if asked.contains(&r) && (before, now) != (false, false) {
            let next = r.checked_add(1).filter(|n| *n < total);
            for n in r.checked_sub(1).into_iter().chain(next) {
                if !done.contains(&n) {
                    todo.insert(n);
                }
            }
        }
    }
    Ok(done.into_iter().map(Row).collect())
}

#[cfg(test)]
mod tests;
