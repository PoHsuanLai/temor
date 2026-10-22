//! The grid's texture on screen. Without room under any block it is one layer, as drawn; with
//! room, one layer per slice of the drawn grid (`temor_render::RowLayout`), each showing its band
//! of the texture where the session lays it out, and the terminal's ground in the room between.
//! The pointer goes through to the grid itself, so its points are the pane's.

use dioxus::prelude::*;
use ds_blitz::{Sampling, TexelRect, TextureFit, TextureHandle, TextureLayer};

use crate::session::Session;

/// One slice of the texture: texture rows `src_y..src_y + rows`, shown `top` logical pixels down
/// and `height` tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliceView {
    pub src_y: u32,
    pub rows: u32,
    pub top: f32,
    pub height: f32,
}

/// The room between slices, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapView {
    pub top: f32,
    pub height: f32,
}

/// The grid cut where room was made under blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct GridCut {
    /// The texture's width in its own pixels.
    pub width: u32,
    pub slices: Vec<SliceView>,
    pub gaps: Vec<GapView>,
}

/// The cut of the session's grid, or `None` when no room is made and it shows whole.
pub fn grid_cut(session: &Session) -> Option<GridCut> {
    let layout = session.row_layout()?;
    if layout.gaps().is_empty() && !layout.has_folds() {
        return None;
    }
    let factor = session.scale_factor().max(f32::EPSILON);
    Some(GridCut {
        width: layout.fit().pane.width.0,
        slices: layout
            .slices()
            .iter()
            .map(|s| SliceView {
                src_y: s.src_y.0,
                rows: s.height.0,
                top: s.dst_y.0 as f32 / factor,
                height: s.height.0 as f32 / factor,
            })
            .collect(),
        gaps: layout
            .gaps()
            .iter()
            .map(|g| GapView {
                top: g.dst_y.0 as f32 / factor,
                height: g.height.0 as f32 / factor,
            })
            .collect(),
    })
}

/// The grid's texture, whole or in slices.
#[component]
#[allow(non_snake_case)]
pub fn GridTexture(texture: TextureHandle, cut: Option<GridCut>) -> Element {
    let Some(cut) = cut else {
        return rsx! {
            TextureLayer { texture: Some(texture), fit: TextureFit::Actual, sampling: Sampling::Nearest }
        };
    };
    rsx! {
        for (i, gap) in cut.gaps.iter().enumerate() {
            div {
                key: "gap-{i}",
                class: "temor-row-gap",
                style: "top:{gap.top}px; height:{gap.height}px",
            }
        }
        for (i, slice) in cut.slices.iter().enumerate() {
            div {
                key: "slice-{i}",
                class: "temor-grid-slice",
                style: "top:{slice.top}px; height:{slice.height}px",
                TextureLayer {
                    texture: Some(texture.clone()),
                    fit: TextureFit::Actual,
                    sampling: Sampling::Nearest,
                    source: Some(TexelRect::new(0, slice.src_y, cut.width, slice.rows)),
                }
            }
        }
    }
}
