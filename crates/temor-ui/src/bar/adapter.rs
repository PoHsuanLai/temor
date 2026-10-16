//! The bar on quire's palette machine (PLAN §7). `bar_step` ranks and says when the rows
//! changed; the palette walks them (the highlight, Up and Down that stop at the ends, Enter and
//! a click) and never re-ranks. This is the glue: the rows go in as the palette's params, a
//! `Refreshed` follows every change, and the row the palette runs goes back as the pick.
//! Pure over the context it is given.

use ds::prelude::{PaletteIn, PaletteIndex, PaletteOut, PaletteParams, PaletteState};
use ds_core::machine::Machine;
use ds_core::text::typed::TypedText;
use ds_core::time::stamp::Stamp;
use temor_core::bar::{BarContext, BarIn, BarOut, BarRow, BarState, RowKey, bar_step};

/// The bar and the palette that walks its rows.
#[derive(Debug, Clone, PartialEq)]
pub struct BarModel {
    bar: BarState,
    palette: PaletteState<BarRow>,
}

impl Default for BarModel {
    fn default() -> Self {
        BarModel {
            bar: BarState::Closed,
            palette: PaletteState::Closed,
        }
    }
}

// The plugins' rows come in through a child module so it can reach the model's fields.
#[path = "../plugins/provider_rows.rs"]
mod provider_rows;

type PaletteStep = (PaletteState<BarRow>, Vec<PaletteOut<BarRow>>);

/// The palette after `input` over `rows`. It keeps no timer, so the time is nothing.
fn walk(state: PaletteState<BarRow>, input: PaletteIn, rows: &[BarRow]) -> PaletteStep {
    let params = PaletteParams {
        rows: rows.to_vec(),
    };
    state.step(input, Stamp(0), &params, &())
}

impl BarModel {
    /// Whether the bar is showing.
    pub fn is_open(&self) -> bool {
        matches!(self.bar, BarState::Open(_))
    }

    /// The ranked rows, none while closed.
    pub fn rows(&self) -> &[BarRow] {
        match &self.bar {
            BarState::Open(bar) => &bar.rows,
            BarState::Closed | BarState::Picked(_) => &[],
        }
    }

    /// What was typed.
    pub fn query(&self) -> &str {
        match &self.bar {
            BarState::Open(bar) => &bar.query,
            BarState::Closed | BarState::Picked(_) => "",
        }
    }

    /// The highlighted row's place; the palette's.
    pub fn selected(&self) -> usize {
        match &self.palette {
            PaletteState::Open { selection, .. } => selection.0,
            PaletteState::Closed => 0,
        }
    }

    /// ⌘T: opens the bar, or closes it while it is open.
    pub fn toggled(self, cx: BarContext<'_>) -> BarModel {
        let was_open = self.is_open();
        let (bar, outs) = bar_step(self.bar, BarIn::Toggle, cx);
        let mut model = BarModel {
            bar,
            palette: self.palette,
        };
        match was_open {
            true => model.close_palette(),
            false => model.open_palette(&outs),
        }
        model
    }

    /// Escape or a click outside.
    pub fn closed(self, cx: BarContext<'_>) -> BarModel {
        let (bar, _) = bar_step(self.bar, BarIn::Close, cx);
        let mut model = BarModel {
            bar,
            palette: self.palette,
        };
        model.close_palette();
        model
    }

    /// The field now holds `text`.
    pub fn typed(self, text: String, cx: BarContext<'_>) -> BarModel {
        let (bar, outs) = bar_step(self.bar, BarIn::Typed(text.clone()), cx);
        let rows = rows_of(&bar);
        let (palette, _) = walk(self.palette, PaletteIn::Typed(TypedText::new(text)), &rows);
        let palette = match outs.contains(&BarOut::Refreshed) {
            true => walk(palette, PaletteIn::Refreshed, &rows).0,
            false => palette,
        };
        BarModel { bar, palette }
    }

    /// The highlight moved to `index` (the arrows, or the pointer): the palette keeps it where
    /// the rows end.
    pub fn selecting(self, index: usize) -> BarModel {
        self.selecting_among(index, 0)
    }

    /// The highlight moved to `index` among the bar's rows and `extra` more rows the window lists
    /// after them (a viewer's commands), which the bar does not rank.
    pub fn selecting_among(self, index: usize, extra: usize) -> BarModel {
        let rows = self.rows().len() + extra;
        let palette = match self.palette {
            PaletteState::Open { query, .. } => {
                PaletteState::open(query, PaletteIndex(index.min(rows.saturating_sub(1))))
            }
            PaletteState::Closed => PaletteState::Closed,
        };
        BarModel { palette, ..self }
    }

    /// The row `key` was run (Enter or a click): the palette runs it and closes, the bar is
    /// done, and the row to carry out comes back. Nothing for a key that is not a row.
    pub fn picked(self, key: &RowKey, cx: BarContext<'_>) -> (BarModel, Option<BarRow>) {
        let rows = self.rows().to_vec();
        let Some(index) = rows.iter().position(|row| row.key == *key) else {
            return (self, None);
        };
        let (palette, outs) = walk(self.palette, PaletteIn::Pick(PaletteIndex(index)), &rows);
        let ran = outs.into_iter().find_map(|out| match out {
            PaletteOut::Run(row) => Some(row),
            PaletteOut::Opened | PaletteOut::Closed => None,
        });
        let Some(ran) = ran else {
            return (BarModel { palette, ..self }, None);
        };
        let (bar, outs) = bar_step(self.bar, BarIn::Pick(ran.key), cx);
        let row = outs.into_iter().find_map(|out| match out {
            BarOut::Run(row) => Some(row),
            BarOut::Refreshed | BarOut::AskProviders(_) | BarOut::Closed => None,
        });
        (BarModel { bar, palette }, row)
    }

    /// The palette opens with the bar and is told its first rows.
    fn open_palette(&mut self, outs: &[BarOut]) {
        let rows = rows_of(&self.bar);
        let state = std::mem::replace(&mut self.palette, PaletteState::Closed);
        let (state, _) = walk(state, PaletteIn::Open, &rows);
        self.palette = match outs.contains(&BarOut::Refreshed) {
            true => walk(state, PaletteIn::Refreshed, &rows).0,
            false => state,
        };
    }

    fn close_palette(&mut self) {
        let state = std::mem::replace(&mut self.palette, PaletteState::Closed);
        self.palette = walk(state, PaletteIn::Close, &[]).0;
    }
}

fn rows_of(bar: &BarState) -> Vec<BarRow> {
    match bar {
        BarState::Open(bar) => bar.rows.clone(),
        BarState::Closed | BarState::Picked(_) => Vec::new(),
    }
}
