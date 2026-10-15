//! Glyphs that reach outside their row (tall accents, deep descenders) are drawn whole, as on
//! the GPU path, so a row's band shows the overhang of the rows above and below it, and a row
//! whose glyphs overhang (now or before) takes its neighbours' bands with it when it changes.

use std::collections::BTreeSet;

use temor_vt::Row;

use crate::scene::{QuadPaint, RowScene};

/// Whether a row's glyphs stay in the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Overhang {
    #[default]
    Within,
    /// Some glyph reaches above or below the row.
    Beyond,
}

/// Whether any glyph of `scene` reaches outside the row spanning `y..y + height`.
pub(crate) fn overhang_of(scene: &RowScene, y: i32, height: u32) -> Overhang {
    let bottom = i64::from(y) + i64::from(height);
    let beyond = scene.fg.iter().any(|q| {
        matches!(q.paint, QuadPaint::Mask { .. } | QuadPaint::Color { .. })
            && (q.y < y || i64::from(q.y) + i64::from(q.h) > bottom)
    });
    if beyond {
        Overhang::Beyond
    } else {
        Overhang::Within
    }
}

/// What is known of the rows beside the one being composed: whether their glyphs may reach into
/// it. [`Overhang::Within`] is certain; [`Overhang::Beyond`] means "build their glyphs to see".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sides {
    pub above: Overhang,
    pub below: Overhang,
}

/// How each row looked the last time it was composed.
#[derive(Debug, Default)]
pub(crate) struct Overhangs {
    rows: Vec<Overhang>,
}

impl Overhangs {
    /// Forgets everything when the grid's height changed.
    pub fn fit(&mut self, rows: u16) {
        if self.rows.len() != usize::from(rows) {
            self.rows = vec![Overhang::Within; usize::from(rows)];
        }
    }

    /// How `row` looked the last time it was composed; a row off the grid stays within.
    fn get(&self, row: u16) -> Overhang {
        self.rows.get(usize::from(row)).copied().unwrap_or_default()
    }

    /// Records `now` for `row` and returns what it was.
    fn replace(&mut self, row: Row, now: Overhang) -> Overhang {
        self.rows
            .get_mut(usize::from(row.0))
            .map_or(Overhang::Within, |slot| std::mem::replace(slot, now))
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
    mut compose: impl FnMut(Row, Sides) -> Result<Overhang, E>,
) -> Result<Vec<Row>, E> {
    let asked: BTreeSet<u16> = dirty.iter().map(|r| r.0).collect();
    let mut todo = asked.clone();
    let mut done = BTreeSet::new();
    while let Some(r) = todo.pop_first() {
        let reach = |n: u16| match asked.contains(&n) && !done.contains(&n) {
            true => Overhang::Beyond,
            false => overhangs.get(n),
        };
        let sides = Sides {
            above: r.checked_sub(1).map_or(Overhang::Within, reach),
            below: r
                .checked_add(1)
                .filter(|n| *n < total)
                .map_or(Overhang::Within, reach),
        };
        let now = compose(Row(r), sides)?;
        let before = overhangs.replace(Row(r), now);
        done.insert(r);
        if asked.contains(&r) && (before, now) != (Overhang::Within, Overhang::Within) {
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
