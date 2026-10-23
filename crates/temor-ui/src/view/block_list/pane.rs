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
use ds::components::controls::press::Propagation;
use ds::components::controls::scroller::handle::use_scroller;
use ds::components::controls::scroller::view::Scroller;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds::style::tokens::control_size::ControlSize;
use ds_blitz::{
    PixelFormat, Pixels, Sampling, TexelRect, TextureFit, TextureHandle, TextureLayer, use_gpu,
};
use ds_core::geometry::scroll::ScrollSpan;
use ds_core::geometry::units::Px;
use ds_core::vocab::Shortcut;
use temor_core::block::{Outcome, format_elapsed};
use temor_core::session::list::{Item, ListBlock};
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_render::Palette;
use temor_vt::Rgb;

use super::segments::{Placed, Seg, SegKey, Sizes, Standing, card_span, lay_out, near_view};
use crate::hub::Desk;
use crate::session::{Session, WheelDirection};
use crate::view::block_tools::{More, more_menu};
use crate::view::card_colours::{CardFill, card_ground_rgb};
use crate::view::convert::wheel_lines;
use crate::view::tool_button::ToolButton;

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

/// What a card's tools do, each to the block it names; `detach` and `stop` act on the running
/// program, `fold` folds a block's output away or brings it back.
#[derive(Debug, Clone, PartialEq)]
pub struct CardActions {
    pub copy_command: EventHandler<BlockId>,
    pub copy_output: EventHandler<BlockId>,
    pub save: EventHandler<BlockId>,
    pub rerun: EventHandler<BlockId>,
    pub view: EventHandler<BlockId>,
    pub edit: EventHandler<BlockId>,
    pub fold: EventHandler<BlockId>,
    pub detach: EventHandler<()>,
    pub stop: EventHandler<()>,
}

/// What the pane offers beyond the block itself: whether a running program can move to a pane of
/// its own, whether the pane is on another computer (to edit locally), and the chord for "Copy
/// Command".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CardOffers {
    pub detach: bool,
    pub on_host: bool,
    pub copy_command_key: Option<Shortcut>,
}

/// What the head says beside the command: a failure's status, how long it ran, and how many rows
/// are folded away.
fn meta_of(block: &ListBlock) -> String {
    let status = match block.outcome {
        Outcome::Failed(code) => Some(format!("exit {}", code.0)),
        _ => None,
    };
    let rows = block.output.len();
    let folded = (block.folded && rows > 0).then(|| match rows {
        1 => "1 line folded".to_owned(),
        n => format!("{n} lines folded"),
    });
    [status, block.took_ms.map(format_elapsed), folded]
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
    actions: CardActions,
    offers: CardOffers,
) -> Element {
    let slot = use_hook(|| desk.hub.slot(pane).unwrap_or_default());
    let gpu = use_gpu();
    let scroller = use_scroller();
    let tiles = use_hook(|| Rc::new(RefCell::new(HashMap::<SegKey, Tile>::new())));
    // The block under the pointer, and the one whose "more" menu is up: its tools show.
    let mut hovered = use_signal(|| None::<BlockId>);
    let held = use_signal(|| None::<BlockId>);
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
    // A new command (the list's last block changed) takes the list back to its bottom.
    let newest = items.iter().rev().find_map(|item| match item {
        Item::Done(block) | Item::Running { block, .. } => Some(block.id),
        _ => None,
    });
    let seen_newest = use_hook(|| Rc::new(Cell::new(None::<BlockId>)));
    if seen_newest.get() != newest {
        seen_newest.set(newest);
        follow.set(true);
    }
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
    // The list's own height says where its bottom is now: the scroller's measure of the content
    // lags a layout behind, and keeps an offset past the end when the list shrinks (a fold).
    if scroll.viewport.0 > 0.0 {
        let bottom = (total - scroll.viewport.0).max(0.0);
        let past = scroll.offset.0 > bottom + 0.5;
        let behind = follow.get() && scroll.offset.0 < bottom - 0.5;
        if past || behind {
            scroller.scroll_to(Px(bottom));
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
    // The tools show on one card: the one with its menu up, else under the pointer, else picked.
    let picked = items.iter().find_map(|item| match item {
        Item::Done(block) | Item::Running { block, .. } if block.picked => Some(block.id),
        _ => None,
    });
    let tools_on = held().or(hovered()).or(picked);
    let head = Head {
        tools_on,
        columns,
        actions,
        offers,
        held,
    };
    rsx! {
        div {
            class: "temor-blist",
            style: "{font}",
            onwheel: on_wheel,
            onpointerleave: move |_| hovered.set(None),
            Scroller {
                scroller,
                common: Common {
                    extra_class: ExtraClass::parse("temor-blist-scroll").ok(),
                    ..Common::default()
                },
                div { class: "temor-blist-content", style: "height:{total}px",
                    for (p, handle, source) in views {
                        {segment(&p, handle, source, &palette, &head, hovered)}
                    }
                }
            }
        }
    }
}

/// What every card head is drawn with.
#[derive(Clone)]
struct Head {
    tools_on: Option<BlockId>,
    columns: usize,
    actions: CardActions,
    offers: CardOffers,
    held: Signal<Option<BlockId>>,
}

/// The block a segment belongs to, for the pointer over it.
fn block_of(seg: &Seg) -> Option<BlockId> {
    match seg {
        Seg::Head(block) => Some(block.id),
        Seg::Rows { id, .. } | Seg::Foot { id, .. } => Some(*id),
        Seg::Section(_) | Seg::Composer { .. } => None,
    }
}

/// One segment, placed where the layout put it.
fn segment(
    p: &Placed,
    handle: Option<TextureHandle>,
    source: Option<TexelRect>,
    palette: &Palette,
    head: &Head,
    mut hovered: Signal<Option<BlockId>>,
) -> Element {
    let key = format!("{:?}", p.key);
    let place = format!("top:{}px; height:{}px", p.top, p.height);
    let css = |c: Rgb| format!("rgb({},{},{})", c.r, c.g, c.b);
    let over = block_of(&p.seg);
    let on_move = move |_: PointerEvent| {
        if *hovered.peek() != over {
            hovered.set(over);
        }
    };
    match &p.seg {
        Seg::Section(folder) => rsx! {
            div { key: "{key}", class: "temor-bseg temor-blk-section", style: "{place}", "{folder}" }
        },
        Seg::Head(block) => {
            let standing = Standing::of(block.outcome);
            let ground = css(ground_of(palette, standing, block.picked));
            rsx! {
                CardHead {
                    key: "{key}",
                    block: block.clone(),
                    style: "{place}; background:{ground}",
                    columns: head.columns,
                    tools: head.tools_on == Some(block.id),
                    actions: head.actions.clone(),
                    offers: head.offers.clone(),
                    held: head.held,
                    on_move,
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
                    class: "temor-bseg temor-blk-rows",
                    style: "{place}; background:{ground}",
                    onpointermove: on_move,
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
                div { key: "{key}", class: "temor-bseg temor-blk-foot", style: "{place}; background:{ground}", onpointermove: on_move }
            }
        }
        Seg::Composer { .. } => rsx! {
            div { key: "{key}", class: "temor-bseg temor-blk-prompt", style: "{place}",
                if let Some(texture) = handle {
                    TextureLayer { texture: Some(texture), fit: TextureFit::Actual, sampling: Sampling::Nearest, source }
                }
            }
        },
    }
}

/// A card's head: the fold chevron, the command, what it came to, and, on the card the tools are
/// on, its tools: copy the output, type it again and more, or, while it runs, detach and stop.
#[component]
#[allow(non_snake_case)]
fn CardHead(
    block: ListBlock,
    style: String,
    columns: usize,
    tools: bool,
    actions: CardActions,
    offers: CardOffers,
    held: Signal<Option<BlockId>>,
    on_move: EventHandler<PointerEvent>,
) -> Element {
    let id = block.id;
    let standing = Standing::of(block.outcome);
    let running = standing == Standing::Running;
    let has_output = !block.output.is_empty();
    let meta = meta_of(&block);
    let title = clip_chars(&block.command, columns.saturating_sub(16).max(8));
    let more_probe = use_rect();
    let mut menu = use_signal(|| false);
    let finished = match running {
        true => Availability::Disabled,
        false => Availability::Enabled,
    };
    let items = more_menu(
        finished,
        finished,
        offers.on_host.then_some(Availability::Enabled),
        offers.copy_command_key.clone(),
    );
    let (fold_icon, fold_label) = match block.folded {
        true => (Icon::ChevronRight, "Show output"),
        false => (Icon::ChevronDown, "Fold output"),
    };
    rsx! {
        div {
            class: "temor-bseg temor-blk-head",
            "data-state": standing.slug(),
            style,
            onpointermove: move |e| on_move.call(e),
            // Held in place on every card, shown where there is output to fold, so titles align.
            div { class: "temor-blk-fold", "data-none": !(has_output && !running),
                ToolButton {
                    icon: fold_icon,
                    size: ControlSize::Mini,
                    propagation: Propagation::Stop,
                    label: fold_label,
                    tip: "Fold this command's output away, or bring it back",
                    onclick: move |()| actions.fold.call(id),
                }
            }
            span { class: "temor-blk-title", "{title}" }
            if !meta.is_empty() {
                span { class: "temor-blk-meta", "{meta}" }
            }
            if tools {
                div { class: "temor-blk-tools",
                    IconButtonGroup { label: "Command tools", size: ControlSize::Mini,
                        if running {
                            if offers.detach {
                                ToolButton {
                                    icon: Icon::Columns,
                                    size: ControlSize::Mini,
                                    propagation: Propagation::Stop,
                                    label: "Detach",
                                    tip: "Move this program to a pane of its own beside this one",
                                    onclick: move |()| actions.detach.call(()),
                                }
                            }
                            ToolButton {
                                icon: Icon::Square,
                                size: ControlSize::Mini,
                                propagation: Propagation::Stop,
                                label: "Stop",
                                tip: "Interrupt the program (Control-C)",
                                onclick: move |()| actions.stop.call(()),
                            }
                        } else {
                            ToolButton {
                                icon: Icon::Clipboard,
                                size: ControlSize::Mini,
                                propagation: Propagation::Stop,
                                label: "Copy output",
                                tip: "Copy this command's output",
                                onclick: move |()| actions.copy_output.call(id),
                            }
                            ToolButton {
                                icon: Icon::Restart,
                                size: ControlSize::Mini,
                                propagation: Propagation::Stop,
                                label: "Re-run",
                                tip: "Put this command in the prompt. Press Return to run it",
                                onclick: move |()| actions.rerun.call(id),
                            }
                        }
                        div {
                            class: "temor-blk-more",
                            onmounted: move |event: MountedEvent| more_probe.on_mounted(event),
                            ToolButton {
                                icon: Icon::Ellipsis,
                                size: ControlSize::Mini,
                                propagation: Propagation::Stop,
                                label: "More",
                                tip: "More things to do with this command",
                                onclick: move |()| {
                                    held.set(Some(id));
                                    menu.set(true);
                                },
                            }
                        }
                    }
                    if let (true, Some(anchor)) = (menu(), more_probe.anchor()) {
                        Menu {
                            placement: MenuPlacement::Popup,
                            anchor,
                            items,
                            onpick: move |pick: More| match pick {
                                More::CopyCommand => actions.copy_command.call(id),
                                More::Save => actions.save.call(id),
                                More::View => actions.view.call(id),
                                More::Edit => actions.edit.call(id),
                            },
                            onclose: move |()| {
                                held.set(None);
                                menu.set(false);
                            },
                        }
                    }
                }
            }
        }
    }
}
