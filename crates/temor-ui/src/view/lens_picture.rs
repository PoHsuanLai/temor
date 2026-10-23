//! The picture a lens drew, under its block (PLAN P11). It is an `<img>` beside the grid, like
//! the chips and the block header, set in the room the session makes under the block's last row
//! (the grid is shown in slices around it, `session::room`). It is placed each time the pane is
//! drawn, so it follows the block as it scrolls, and a layer that clips to the pane keeps it from
//! being drawn outside. The picture takes whole rows (its extent), and never more than
//! [`MOST_ROWS`]; while its block's last row is off the screen there is no room and no picture.

use dioxus::prelude::*;
use ds::prelude::ImageSource;
use temor_core::vocab::ids::BlockId;

use crate::plugins::lens::LensPicture;
use crate::session::{Frame, Gutter, Session};

/// The most rows a picture takes under its block.
pub const MOST_ROWS: u32 = crate::session::LENS_MOST_ROWS;

/// Where one picture goes and how much room it takes, in logical pixels in the pane.
#[derive(Debug, Clone, PartialEq)]
pub struct LensSpot {
    /// The top of the room under the block.
    pub top: f32,
    /// The left edge: the pane's padding.
    pub left: f32,
    /// The picture's width.
    pub width: f32,
    /// The room it takes: whole rows.
    pub height: f32,
    /// How many rows that is.
    pub rows: u32,
}

/// One picture to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct LensView {
    pub block: BlockId,
    pub spot: LensSpot,
    pub source: ImageSource,
}

/// How big a picture is drawn: its own size, in logical pixels, shrunk to the room between the
/// pane's paddings (never enlarged), and the rows that takes, at most [`MOST_ROWS`]. It goes in
/// the room under viewport row `last_row`; `None` where there is none.
pub fn lens_spot(
    size: (u32, u32),
    last_row: i64,
    gutter: &Gutter,
    pane_width: f32,
) -> Option<LensSpot> {
    let factor = gutter.factor.max(f32::EPSILON);
    let (width, height) = (size.0 as f32 / factor, size.1 as f32 / factor);
    let room = (pane_width - 2.0 * gutter.side).max(1.0);
    let shrink = (room / width.max(1.0)).min(1.0);
    let cell_h = gutter.cell_h.max(1.0);
    let rows = ((height * shrink / cell_h).ceil() as u32).clamp(1, MOST_ROWS);
    Some(LensSpot {
        top: gutter.insert_top(last_row)?,
        left: gutter.side,
        width: width * shrink,
        height: rows as f32 * cell_h,
        rows,
    })
}

/// The width a lens is asked to draw in, in device pixels: the room between the pane's paddings.
pub fn request_width(pane_width: f32, gutter: &Gutter, factor: f32) -> u32 {
    ((pane_width - 2.0 * gutter.side).max(1.0) * factor).round() as u32
}

/// The block the menu acts on and the width its lens is asked to draw in.
pub fn lens_request(session: &Session) -> Option<(BlockId, u32)> {
    let block = session.selected_block()?;
    let (pane_width, _) = session.grid_logical()?;
    let gutter = session.gutter_geometry()?;
    Some((
        block,
        request_width(pane_width, &gutter, session.scale_factor()),
    ))
}

/// The pictures that have a block on screen to sit under.
pub fn lens_views(session: &Session, frame: &Frame, pictures: Vec<LensPicture>) -> Vec<LensView> {
    let (Some(gutter), Some((pane_width, _))) = (session.gutter_geometry(), session.grid_logical())
    else {
        return Vec::new();
    };
    pictures
        .into_iter()
        .filter_map(|picture| {
            let last = frame.last_row(picture.block)?;
            Some(LensView {
                block: picture.block,
                spot: lens_spot((picture.width, picture.height), last, &gutter, pane_width)?,
                source: ImageSource(picture.source.to_string()),
            })
        })
        .collect()
}

/// The pictures under their blocks, in a layer that clips to the pane.
#[component]
#[allow(non_snake_case)]
pub fn LensPictures(views: Vec<LensView>) -> Element {
    rsx! {
        div { class: "temor-lens-layer",
            for view in views {
                img {
                    key: "{view.block.0}",
                    class: "temor-lens",
                    alt: "",
                    src: view.source.0.clone(),
                    style: "top:{view.spot.top}px; left:{view.spot.left}px; width:{view.spot.width}px; height:{view.spot.height}px",
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "lens_picture_tests.rs"]
mod tests;
