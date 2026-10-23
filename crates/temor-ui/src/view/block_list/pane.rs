//! The pane drawn as its list of blocks: a scroller over the list's segments, only those near
//! the view mounted. A finished block's rows are tiles drawn once on its card; the running
//! block's live rows, and the shell's prompt, are shown from the pane's own picture as the
//! terminal draws them. The list follows its bottom until the person scrolls back.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::base::text::clip::clip_chars;
use ds::components::controls::scroller::handle::use_scroller;
use ds::components::controls::scroller::view::Scroller;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds_blitz::{
    PixelFormat, Pixels, Sampling, TexelRect, TextureFit, TextureHandle, TextureLayer, use_gpu,
};
use ds_core::geometry::scroll::ScrollSpan;
use temor_core::block::{Outcome, format_elapsed};
use temor_core::session::list::Item;
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_render::Palette;
use temor_vt::Rgb;

use super::segments::{Placed, Seg, SegKey, Sizes, Standing, card_span, lay_out, near_view};
use crate::hub::Desk;
use crate::session::{Session, WheelDirection};
use crate::view::card_colours::{CardFill, card_ground_rgb};
use crate::view::convert::wheel_lines;

/// How far around the view segments are mounted, in logical pixels.
const MOUNT_MARGIN: f32 = 600.0;
/// The most rows one tile of output holds.
const TILE_ROWS: u32 = 48;

/// What a tile was drawn with: when any of it changes the tile is drawn again.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    rows: Range<u32>,
    width: u32,
    palette: Palette,
    ground: Rgb,
}

/// A tile's texture and what it shows.
struct Tile {
    texture: TextureHandle,
    look: Look,
}

/// The colour a card's rows are drawn on: raised off the background, tinted when it failed, the
/// background itself while it runs (its live rows are the terminal's own); a picked card is
/// drawn as selected text is.
fn ground_of(palette: &Palette, standing: Standing, picked: bool) -> Rgb {
    match (picked, standing) {
        (true, _) => card_ground_rgb(palette, CardFill::Picked),
        (false, Standing::Running) => palette.background,
        (false, Standing::Failed) => card_ground_rgb(palette, CardFill::Failed),
        (false, Standing::Done) => card_ground_rgb(palette, CardFill::None),
    }
}

/// What the head says beside the command: a failure's status, and how long it ran.
fn meta_of(outcome: Outcome, took_ms: Option<u64>) -> String {
    let status = match outcome {
        Outcome::Failed(code) => Some(format!("exit {}", code.0)),
        _ => None,
    };
    [status, took_ms.map(format_elapsed)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// Draws the tiles near the view that are missing or stale, and forgets the rest.
fn draw_tiles(
    session: &mut Session,
    tiles: &mut HashMap<SegKey, Tile>,
    mounted: &[&Placed],
    palette: &Palette,
    new_texture: impl Fn() -> TextureHandle,
) {
    let width = session.fit().map_or(0, |fit| fit.pane.width.0);
    tiles.retain(|key, _| mounted.iter().any(|p| p.key == *key));
    for placed in mounted {
        let Seg::Rows {
            rows,
            standing,
            picked,
            live: false,
            ..
        } = &placed.seg
        else {
            continue;
        };
        let look = Look {
            rows: rows.clone(),
            width,
            palette: *palette,
            ground: ground_of(palette, *standing, *picked),
        };
        if tiles.get(&placed.key).is_some_and(|tile| tile.look == look) {
            continue;
        }
        let texture = tiles
            .remove(&placed.key)
            .map_or_else(&new_texture, |tile| tile.texture);
        let _ = session.draw_rows(rows.clone(), Some(look.ground), |bytes, size| {
            if let Ok(pixels) = Pixels::new(
                PixelFormat::Rgba8Premultiplied,
                size.width.0,
                size.height.0,
                bytes,
            ) {
                let _ = texture.update(&pixels);
            }
        });
        tiles.insert(placed.key, Tile { texture, look });
    }
}

/// The pane's blocks as a list. `items` is the session's list now; `texture` the pane's own
/// picture, which holds the live rows; `font` the grid's family as CSS.
#[component]
#[allow(non_snake_case)]
pub fn BlockPane(
    pane: PaneId,
    desk: Desk,
    items: Vec<Item>,
    texture: TextureHandle,
    font: String,
    reveal: Option<BlockId>,
) -> Element {
    let slot = use_hook(|| desk.hub.slot(pane).unwrap_or_default());
    let gpu = use_gpu();
    let scroller = use_scroller();
    let tiles = use_hook(|| Rc::new(RefCell::new(HashMap::<SegKey, Tile>::new())));
    // Whether the list keeps to its bottom as it grows: until the person scrolls back.
    let follow = use_hook(|| Rc::new(Cell::new(true)));
    {
        let follow = Rc::clone(&follow);
        use_effect(move || {
            let scroll = scroller.scroll()();
            let bottom = scroll.max_offset();
            if scroll.offset.0 >= bottom.0 - 1.0 {
                follow.set(true);
            } else if follow.get() {
                scroller.scroll_to(bottom);
            }
        });
    }
    let (row, palette) = {
        let s = slot.borrow();
        let row = s.session.as_ref().and_then(Session::row_height);
        (row.unwrap_or(16.0), s.palette)
    };
    let sizes = Sizes {
        row,
        section: 28.0,
        head: 40.0,
        foot: 10.0,
        gap: 10.0,
        composer: row,
        tile_rows: TILE_ROWS,
    };
    let (placed, total) = lay_out(&items, &sizes);
    let scroll = scroller.scroll()();
    // A block picked from the keyboard is scrolled to, once: the list stops following its bottom.
    let revealed = use_hook(|| Rc::new(Cell::new(None::<BlockId>)));
    if revealed.get() != reveal {
        revealed.set(reveal);
        if let Some((top, bottom)) = reveal.and_then(|id| card_span(&placed, id)) {
            follow.set(false);
            let length = (bottom - top).min(scroll.viewport.0.max(1.0));
            scroller.reveal(ScrollSpan { start: top, length });
        }
    }
    let viewport = match scroll.viewport.0 > 0.0 {
        true => scroll.viewport.0,
        false => total,
    };
    let near = near_view(&placed, scroll.offset.0, viewport, MOUNT_MARGIN);
    let mounted: Vec<&Placed> = placed[near].iter().collect();
    if let Some(session) = slot.borrow_mut().session.as_mut() {
        draw_tiles(session, &mut tiles.borrow_mut(), &mounted, &palette, || {
            gpu.handle()
        });
    }
    // Where live rows are in the pane's picture.
    let live_rect = |rows: &Range<u32>| {
        slot.borrow()
            .session
            .as_ref()
            .and_then(|s| s.live_rows(rows.clone()))
            .map(|r| TexelRect::new(r.src_x, r.src_y, r.width, r.height))
    };
    let views: Vec<(Placed, Option<TextureHandle>, Option<TexelRect>)> = mounted
        .iter()
        .map(|p| {
            let (handle, source) = match &p.seg {
                Seg::Rows {
                    rows, live: true, ..
                }
                | Seg::Composer { rows } => (Some(texture.clone()), live_rect(rows)),
                Seg::Rows { .. } => (
                    tiles.borrow().get(&p.key).map(|tile| tile.texture.clone()),
                    None,
                ),
                _ => (None, None),
            };
            ((*p).clone(), handle, source)
        })
        .collect();
    let columns = slot
        .borrow()
        .session
        .as_ref()
        .and_then(Session::fit)
        .map_or(80, |fit| usize::from(fit.grid.cols.0));
    let on_wheel = {
        let follow = Rc::clone(&follow);
        move |event: WheelEvent| {
            if wheel_lines(event.delta()).0 == WheelDirection::Up {
                follow.set(false);
            }
        }
    };
    rsx! {
        div {
            class: "temor-blist",
            style: "{font}",
            onwheel: on_wheel,
            Scroller {
                scroller,
                common: Common {
                    extra_class: ExtraClass::parse("temor-blist-scroll").ok(),
                    ..Common::default()
                },
                div { class: "temor-blist-content", style: "height:{total}px",
                    for (p, handle, source) in views {
                        {segment(&p, handle, source, &palette, columns)}
                    }
                }
            }
        }
    }
}

/// One segment, placed where the layout put it.
fn segment(
    p: &Placed,
    handle: Option<TextureHandle>,
    source: Option<TexelRect>,
    palette: &Palette,
    columns: usize,
) -> Element {
    let key = format!("{:?}", p.key);
    let place = format!("top:{}px; height:{}px", p.top, p.height);
    let css = |c: Rgb| format!("rgb({},{},{})", c.r, c.g, c.b);
    match &p.seg {
        Seg::Section(folder) => rsx! {
            div { key: "{key}", class: "temor-seg temor-blk-section", style: "{place}", "{folder}" }
        },
        Seg::Head(block) => {
            let standing = Standing::of(block.outcome);
            let ground = css(ground_of(palette, standing, block.picked));
            let meta = meta_of(block.outcome, block.took_ms);
            let title = clip_chars(&block.command, columns.saturating_sub(16).max(8));
            rsx! {
                div {
                    key: "{key}",
                    class: "temor-seg temor-blk-head",
                    "data-state": standing.slug(),
                    style: "{place}; background:{ground}",
                    span { class: "temor-blk-title", "{title}" }
                    if !meta.is_empty() {
                        span { class: "temor-blk-meta", "{meta}" }
                    }
                }
            }
        }
        Seg::Rows {
            standing, picked, ..
        } => {
            let ground = css(ground_of(palette, *standing, *picked));
            rsx! {
                div {
                    key: "{key}",
                    class: "temor-seg temor-blk-rows",
                    style: "{place}; background:{ground}",
                    if let Some(texture) = handle {
                        TextureLayer { texture: Some(texture), fit: TextureFit::Actual, sampling: Sampling::Nearest, source }
                    }
                }
            }
        }
        Seg::Foot {
            standing, picked, ..
        } => {
            let ground = css(ground_of(palette, *standing, *picked));
            rsx! {
                div { key: "{key}", class: "temor-seg temor-blk-foot", style: "{place}; background:{ground}" }
            }
        }
        Seg::Composer { .. } => rsx! {
            div { key: "{key}", class: "temor-seg temor-blk-prompt", style: "{place}",
                if let Some(texture) = handle {
                    TextureLayer { texture: Some(texture), fit: TextureFit::Actual, sampling: Sampling::Nearest, source }
                }
            }
        },
    }
}
