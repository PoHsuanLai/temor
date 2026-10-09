//! The pointer (PLAN §6): a press selects (one click cells, two words, three lines; Alt makes a
//! rectangle), or reports to the program when it asked for the mouse; the wheel scrolls the
//! scrollback unless the program takes it; ⌘-click (Ctrl+Shift) opens a hyperlink.

use std::time::Duration;

use temor_core::terminal::link::{LinkVerdict, check_link};
use temor_core::terminal::row::RowText;
use temor_core::terminal::selection::{CellPos, Granularity, Selection, Shape};
use temor_vt::{
    GridPoint, Modifiers, MouseAction, MouseButton, MouseInput, PointSpace, RowDelta,
    SelectionShape, SurfacePoint, TextSelection, ViewportScroll,
};

use super::{Drag, Session};
use crate::error::UiError;
use crate::input::{Click, next_click};

impl Session {
    /// A button went down at `at`.
    pub fn press(
        &mut self,
        at: SurfacePoint,
        button: MouseButton,
        mods: Modifiers,
        now: Duration,
    ) -> Result<(), UiError> {
        let Some(fit) = self.fit else {
            return Ok(());
        };
        let tracking = self.pane.with_term(|term| term.mouse_tracking());
        let shift = mods.contains(Modifiers::SHIFT);
        if tracking && !shift {
            let surface = fit.mouse_surface();
            let input = MouseInput {
                action: MouseAction::Press,
                button: Some(button),
                mods,
                at,
            };
            let bytes = self
                .pane
                .with_term(|term| term.encode_mouse(&input, surface))?;
            self.pane.send(&bytes);
            return Ok(());
        }
        let hit = fit.cell_at(at);
        let top = self.viewport_top()?;
        let cell = screen_cell(top, hit.pos.row.0, hit.pos.col.0);
        if mods.contains(Modifiers::CTRL) && mods.contains(Modifiers::SHIFT) {
            return self.open_link_at(cell);
        }
        let line = self.line(cell.row)?;
        let (click, state) = next_click(self.click, now, cell);
        self.click = Some(state);
        let granularity = match click {
            Click::Single => Granularity::Cells,
            Click::Double => Granularity::Words,
            Click::Triple => Granularity::Lines,
        };
        let shape = match mods.contains(Modifiers::ALT) {
            true => Shape::Block,
            false => Shape::Linear,
        };
        self.selection = Some(Selection::begin(cell, &line, granularity, shape));
        self.drag = Some(Drag { granularity, shape });
        self.mirror_selection()
    }

    /// The pointer moved at `at` while a button is held.
    pub fn drag_to(&mut self, at: SurfacePoint) -> Result<(), UiError> {
        let Some(fit) = self.fit else {
            return Ok(());
        };
        if self.drag.is_none() {
            return Ok(());
        }
        let hit = fit.cell_at(at);
        let top = self.viewport_top()?;
        let cell = screen_cell(top, hit.pos.row.0, hit.pos.col.0);
        let line = self.line(cell.row)?;
        if let Some(selection) = self.selection.as_mut() {
            selection.extend(cell, &line);
        }
        self.mirror_selection()
    }

    /// The button came up at `at`: the drag, if any, is over.
    pub fn release(
        &mut self,
        at: SurfacePoint,
        button: MouseButton,
        mods: Modifiers,
    ) -> Result<(), UiError> {
        self.drag = None;
        let Some(fit) = self.fit else {
            return Ok(());
        };
        let tracking = self.pane.with_term(|term| term.mouse_tracking());
        if tracking && !mods.contains(Modifiers::SHIFT) {
            let surface = fit.mouse_surface();
            let input = MouseInput {
                action: MouseAction::Release,
                button: Some(button),
                mods,
                at,
            };
            let bytes = self
                .pane
                .with_term(|term| term.encode_mouse(&input, surface))?;
            self.pane.send(&bytes);
        }
        Ok(())
    }

    /// The wheel turned `lines` detents up (`up`) or down, at `at`.
    pub fn wheel(
        &mut self,
        at: SurfacePoint,
        direction: WheelDirection,
        lines: u32,
        mods: Modifiers,
    ) -> Result<(), UiError> {
        if lines == 0 {
            return Ok(());
        }
        let Some(fit) = self.fit else {
            return Ok(());
        };
        let tracking = self.pane.with_term(|term| term.mouse_tracking());
        if tracking && !mods.contains(Modifiers::SHIFT) {
            let button = match direction {
                WheelDirection::Up => MouseButton::WheelUp,
                WheelDirection::Down => MouseButton::WheelDown,
            };
            let surface = fit.mouse_surface();
            let input = MouseInput {
                action: MouseAction::Press,
                button: Some(button),
                mods,
                at,
            };
            for _ in 0..lines.min(10) {
                let bytes = self
                    .pane
                    .with_term(|term| term.encode_mouse(&input, surface))?;
                self.pane.send(&bytes);
            }
            return Ok(());
        }
        let rows = isize::try_from(lines).unwrap_or(isize::MAX);
        let delta = match direction {
            WheelDirection::Up => -rows,
            WheelDirection::Down => rows,
        };
        self.pane
            .with_term(|term| term.scroll(ViewportScroll::Delta(RowDelta(delta))));
        Ok(())
    }

    /// Makes the core selection's ends the terminal's selection, so the grid draws it.
    pub(super) fn mirror_selection(&mut self) -> Result<(), UiError> {
        let text = self.selection.map(to_text_selection);
        self.pane.with_term(|term| term.set_selection(text))
    }

    /// The text of a screen row.
    pub(super) fn line(&self, row: u32) -> Result<RowText, UiError> {
        self.pane.with_term(|term| term.row_text(row))
    }

    fn open_link_at(&mut self, cell: CellPos) -> Result<(), UiError> {
        let link = self.pane.with_term(|term| {
            let point = GridPoint::new(PointSpace::Screen, cell.col, cell.row);
            term.hyperlink_at(point)
        });
        let Some(target) = link else {
            return Ok(());
        };
        match check_link(&target) {
            LinkVerdict::Open(link) => {
                if self.opener.open(&link).is_err() {
                    self.notice = Some("That link could not be opened.".to_owned());
                }
            }
            LinkVerdict::Refused(_) => {
                self.notice = Some("This pane only opens web and email links.".to_owned());
            }
        }
        Ok(())
    }
}

/// Which way the wheel turned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WheelDirection {
    /// Towards older output.
    Up,
    /// Towards the live screen.
    Down,
}

/// A viewport cell as a screen cell (the viewport starts at screen row `top`).
fn screen_cell(top: u32, row: u16, col: u16) -> CellPos {
    CellPos::new(top.saturating_add(u32::from(row)), col)
}

/// The selection as the terminal draws it, in screen points.
fn to_text_selection(selection: Selection) -> TextSelection {
    let (start, end) = selection.ends();
    TextSelection {
        start: GridPoint::new(PointSpace::Screen, start.col, start.row),
        end: GridPoint::new(PointSpace::Screen, end.col, end.row),
        shape: match selection.shape() {
            Shape::Linear => SelectionShape::Linear,
            Shape::Block => SelectionShape::Block,
        },
    }
}
