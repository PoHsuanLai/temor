//! The pane drawn as its list of blocks: a scroller over the list's segments, only those near
//! the view mounted. A finished block's rows are tiles drawn once on its card; the running
//! block's live rows, and the shell's prompt, are shown from the pane's own picture as the
//! terminal draws them. The list follows its bottom until the person scrolls back.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::base::text::clip::clip_chars;
use ds::components::controls::press::Propagation;
use ds::components::controls::scroller::handle::{ScrollerRef, use_scroller};
use ds::components::controls::scroller::view::Scroller;
use ds::focus::request::FocusRequest;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds::style::tokens::control_size::ControlSize;
use ds_blitz::{Sampling, TexelRect, TextureFit, TextureHandle, TextureLayer, use_gpu};
use ds_core::geometry::scroll::{Scroll, ScrollSpan};
use ds_core::geometry::units::Px;
use temor_client::model::block::{Outcome, format_elapsed};
use temor_client::model::session::list::{Item, KeptCard, ListBlock};
use temor_client::model::vocab::ids::{BlockId, PaneId};
use temor_panes::render::Palette;
use temor_panes::vt::Rgb;

use super::composer::{ChoicesView, ComposerActions, ComposerCard, composer_height};
use super::segments::{
    CardPlace, Picture, Placed, Seg, Sizes, Standing, card_places, card_span, lay_out, list_map,
    near_view, picture_size, row_top, sticky_head,
};
use super::tiles::ground_of;
use crate::hub::Desk;
use crate::plugins::lens::LensPicture;
use crate::session::{CARD_MARGIN, INSET, LENS_MOST_ROWS, Session};
use crate::view::slot::{Slot, repaint};
use crate::view::tool_button::ToolButton;

/// What the composer card shows and does, while the shell waits at its prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerView {
    pub text: String,
    /// The grid's family as CSS.
    pub font: String,
    /// Whether the list has text selected (⌘C copies that, not the field's).
    pub selection: bool,
    /// Puts the caret in the field.
    pub focus: FocusRequest,
    /// Whether the field takes the keyboard as it appears: the pane has it.
    pub take: bool,
    pub actions: ComposerActions,
    /// The words the shell offered for the line's end, if any.
    pub choices: Option<ChoicesView>,
}

/// Whether the list ends at the shell's prompt, marked, with nothing running: the composer
/// takes the prompt's place.
fn composing(items: &[Item]) -> bool {
    matches!(items.last(), Some(Item::Composer { rows }) if !rows.is_empty())
}

/// How far around the view segments are mounted, in logical pixels.
const MOUNT_MARGIN: f32 = 600.0;
/// How far the offset moves before the list is laid out again, in logical pixels: well inside
/// the mount margin.
const BAND: f32 = 200.0;
/// The room a revealed find match keeps above it, for the find bar over the list's top.
const FIND_ROOM: f32 = 64.0;
/// The most rows one tile of output holds.
const TILE_ROWS: u32 = 48;

/// What a card's tools do, each to the block it names; `detach` and `stop` act on the running
/// program, `fold` folds a block's output away or brings it back.
#[derive(Debug, Clone, PartialEq)]
pub struct CardActions {
    pub fold: EventHandler<BlockId>,
    /// The card's "…" at the point under it: the block's menu, the one a right-click opens.
    pub menu: EventHandler<(BlockId, Point)>,
    /// A kept command's Put in Prompt.
    pub put: EventHandler<u32>,
}

/// Keeps the list to its bottom while it follows it, and in range when it shrinks. `total` is
/// the list's height; `seen` the offset and bottom last seen. When the offset moves the list
/// follows only from its bottom: a wheel back stops it, a wheel down to the end, or the list's
/// own move there, starts it again.
fn keep_bottom(
    scroll: Scroll,
    total: f32,
    follow: &Cell<bool>,
    seen: &Cell<Option<(f32, f32)>>,
    scroller: ScrollerRef,
) {
    if scroll.viewport.0 <= 0.0 {
        return;
    }
    let offset = scroll.offset.0;
    // The scroller's own bottom, from the content it measured: it can be reached.
    let reach = scroll.max_offset().0;
    // The list's own bottom: the scroller's measure lags a layout behind it.
    let bottom = (total - scroll.viewport.0).max(0.0);
    let (was, was_bottom) = seen.get().unwrap_or((offset, bottom));
    seen.set(Some((offset, bottom)));
    if (offset - was).abs() > 0.5 {
        follow.set(offset >= reach.min(bottom) - 1.0);
    }
    // A list that shrank (a fold) under an offset past its new end goes to that end.
    if bottom < was_bottom - 0.5 && offset > bottom + 0.5 {
        scroller.scroll_to(Px(bottom));
    } else if follow.get() && offset < reach - 0.5 {
        scroller.scroll_to(Px(reach));
    }
}

/// What the head says beside the command: a failure's status, how long it ran, and how many rows
/// are folded away.
fn meta_of(block: &ListBlock) -> String {
    let rows = block.output.len();
    let folded = (block.folded && rows > 0).then(|| match rows {
        1 => "1 line folded".to_owned(),
        n => format!("{n} lines folded"),
    });
    meta_words(block.outcome, block.took_ms, folded)
}

/// A failure's status, how long it ran, and `more`, between dots.
fn meta_words(outcome: Outcome, took_ms: Option<u64>, more: Option<String>) -> String {
    let status = match outcome {
        Outcome::Failed(code) => Some(format!("exit {}", code.0)),
        _ => None,
    };
    [status, took_ms.map(format_elapsed), more]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// The pictures lenses drew, at the size they are shown: as wide as the rows' text at most, and
/// as tall as [`LENS_MOST_ROWS`] rows.
fn pictures_of(session: Option<&Session>, lens: &[LensPicture], row: f32) -> Vec<Picture> {
    let Some((pane_width, _)) = session.and_then(Session::grid_logical) else {
        return Vec::new();
    };
    let factor = session.map_or(1.0, Session::scale_factor);
    let room = pane_width - 2.0 * INSET.horizontal;
    let most = LENS_MOST_ROWS as f32 * row;
    lens.iter()
        .map(|picture| {
            let (width, height) = picture_size((picture.width, picture.height), factor, room, most);
            Picture {
                block: picture.block,
                width,
                height,
                source: picture.source.clone(),
            }
        })
        .collect()
}

/// The block the menu acts on and the width its lens is asked to draw in, in device pixels: the
/// width of the rows' text.
pub fn lens_request(session: &Session) -> Option<(BlockId, u32)> {
    let block = session.selected_block()?;
    let (pane_width, _) = session.grid_logical()?;
    let room = (pane_width - 2.0 * INSET.horizontal).max(1.0);
    Some((block, (room * session.scale_factor()).round() as u32))
}

/// The pane's blocks as a list. `items` is the session's list now; `texture` the pane's own
/// picture, which holds the live rows; `font` the grid's family as CSS; `pictures` what lenses
/// drew for its blocks.
#[component]
#[allow(non_snake_case)]
pub fn BlockPane(
    pane: PaneId,
    desk: Desk,
    items: Vec<Item>,
    texture: TextureHandle,
    font: String,
    reveal: Option<BlockId>,
    found: Option<u32>,
    pictures: Vec<LensPicture>,
    composer: ComposerView,
    actions: CardActions,
) -> Element {
    let slot = use_hook(|| desk.hub.slot(pane).unwrap_or_default());
    let gpu = use_gpu();
    let scroller = use_scroller();
    // The block under the pointer: its "…" shows.
    let mut hovered = use_signal(|| None::<BlockId>);
    // Whether the list keeps to its bottom as it grows: until the person scrolls back. The
    // window's scroll engine takes the wheel, so the list knows only that its offset moved.
    let follow = use_hook(|| Rc::new(Cell::new(true)));
    // The offset, and the list's own bottom, as last seen.
    let seen = use_hook(|| Rc::new(Cell::new(None::<(f32, f32)>)));
    // The list's height as last laid out, for the scroll's effect.
    let total_now = use_hook(|| Rc::new(Cell::new(0.0_f32)));
    let band = use_memo(move || {
        let scroll = scroller.scroll()();
        (
            (scroll.offset.0 / BAND).floor() as i64,
            scroll.viewport.0.to_bits(),
        )
    });
    {
        let (follow, seen, total_now, slot) = (
            Rc::clone(&follow),
            Rc::clone(&seen),
            Rc::clone(&total_now),
            Rc::clone(&slot),
        );
        use_effect(move || {
            let scroll = scroller.scroll()();
            keep_bottom(scroll, total_now.get(), &follow, &seen, scroller);
            if let Some(session) = slot.borrow_mut().session.as_mut() {
                session.set_list_offset(scroll.offset.0);
            }
        });
    }
    let (row, palette, pictures) = {
        let s = slot.borrow();
        let row = s
            .session
            .as_ref()
            .and_then(Session::row_height)
            .unwrap_or(16.0);
        (
            row,
            s.palette,
            pictures_of(s.session.as_ref(), &pictures, row),
        )
    };
    let sizes = Sizes {
        row,
        head: 40.0,
        foot: 10.0,
        gap: 10.0,
        composer: row,
        tile_rows: TILE_ROWS,
    };
    // The composer card stands for the prompt's rows: they are not drawn.
    let composing = composing(&items);
    let (items, sizes) = match composing {
        true => {
            let mut items = items;
            if let Some(Item::Composer { rows }) = items.last_mut() {
                *rows = 0..0;
            }
            let sizes = Sizes {
                composer: composer_height(
                    &composer.text,
                    composer.choices.as_ref().map_or(0, |c| c.words.len()),
                ),
                ..sizes
            };
            (items, sizes)
        }
        false => (items, sizes),
    };
    let (placed, total) = lay_out(&items, &sizes, &pictures);
    // The render follows the offset only band by band: the window eases a scroll a little each
    // round, and a list drawn again each time would never settle. What is mounted reaches past
    // the view by more than a band.
    let _ = band();
    let scroll = *scroller.scroll().peek();
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
    // The find bar's match, when it moves, is scrolled to, clear of the bar over the list's top.
    let seen_found = use_hook(|| Rc::new(Cell::new(None::<u32>)));
    if seen_found.get() != found {
        seen_found.set(found);
        if let Some(top) = found.and_then(|r| row_top(&placed, r, row)) {
            follow.set(false);
            scroller.reveal(ScrollSpan {
                start: top - FIND_ROOM,
                length: FIND_ROOM + row * 2.0,
            });
        }
    }
    keep_bottom(scroll, total, &follow, &seen, scroller);
    total_now.set(total);
    let viewport = match scroll.viewport.0 > 0.0 {
        true => scroll.viewport.0,
        false => total,
    };
    let near = near_view(&placed, scroll.offset.0, viewport, MOUNT_MARGIN);
    let mounted: Vec<&Placed> = placed[near].iter().collect();
    // The live rows are the pane's own picture: it is drawn on the running card's ground.
    let live_picked = items.iter().any(|item| match item {
        Item::Running { block, .. } => block.picked,
        _ => false,
    });
    let live_ground = ground_of(&palette, Standing::Running, live_picked);
    let reground = {
        let mut s = slot.borrow_mut();
        let Slot { session, tiles, .. } = &mut *s;
        session.as_mut().is_some_and(|session| {
            session.set_list_map(Some(list_map(&placed, scroll.offset.0)));
            session.set_composer_shown(composing);
            tiles.draw(session, &mounted, &palette, || gpu.handle());
            session.set_live_ground(Some(live_ground))
        })
    };
    if reground {
        repaint(&slot);
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
                Seg::Rows { .. } => (slot.borrow().tiles.texture(&p.key), None),
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
    // The "…" shows on one card: the one under the pointer, else the one picked (its menu is up).
    let picked = items.iter().find_map(|item| match item {
        Item::Done(block) | Item::Running { block, .. } if block.picked => Some(block.id),
        _ => None,
    });
    let tools_on = hovered().or(picked);
    let cards = card_places(&placed);
    // A program that asked for the mouse hears the wheel over its live rows, not the list.
    let tracking = slot
        .borrow()
        .session
        .as_ref()
        .is_some_and(Session::mouse_tracking);
    let head = Head {
        tracking,
        tools_on,
        columns,
        actions,
    };
    let composer = composing.then_some(composer);
    rsx! {
        div {
            class: "temor-blist",
            style: "{font}",
            onpointerleave: move |_| hovered.set(None),
            Scroller {
                scroller,
                common: Common {
                    extra_class: ExtraClass::parse("temor-blist-scroll").ok(),
                    ..Common::default()
                },
                div { class: "temor-blist-content", style: "height:{total}px",
                    for (p, handle, source) in views {
                        {segment(&p, handle, source, &palette, &head, composer.as_ref(), hovered)}
                    }
                }
            }
            StickyHead { cards, scroller, palette, head: head.clone(), height: sizes.head, hovered }
        }
    }
}

/// What every card head is drawn with.
#[derive(Clone, PartialEq)]
struct Head {
    /// The running program asked for the mouse: its live rows take the wheel.
    tracking: bool,
    tools_on: Option<BlockId>,
    columns: usize,
    actions: CardActions,
}

/// The block a segment belongs to, for the pointer over it.
fn block_of(seg: &Seg) -> Option<BlockId> {
    match seg {
        Seg::Head { block, .. } => Some(block.id),
        Seg::Rows { id, .. } | Seg::Picture { id, .. } | Seg::Foot { id, .. } => Some(*id),
        Seg::Kept(_) | Seg::Composer { .. } => None,
    }
}

/// One segment, placed where the layout put it.
fn segment(
    p: &Placed,
    handle: Option<TextureHandle>,
    source: Option<TexelRect>,
    palette: &Palette,
    head: &Head,
    composer: Option<&ComposerView>,
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
        Seg::Head { block, whole } => {
            let standing = Standing::of(block.outcome);
            let ground = css(ground_of(palette, standing, block.picked));
            rsx! {
                CardHead {
                    key: "{key}",
                    block: block.clone(),
                    whole: *whole,
                    style: "{place}; background:{ground}",
                    columns: head.columns,
                    tools: head.tools_on == Some(block.id),
                    actions: head.actions.clone(),
                    on_move,
                }
            }
        }
        Seg::Rows {
            standing,
            picked,
            live,
            ..
        } => {
            let ground = css(ground_of(palette, *standing, *picked));
            rsx! {
                div {
                    key: "{key}",
                    class: "temor-bseg temor-blk-rows",
                    style: "{place}; background:{ground}",
                    // The pane hands this wheel to the program as mouse reports.
                    "data-wheel": (*live && head.tracking).then_some("capture"),
                    onpointermove: on_move,
                    if let Some(texture) = handle {
                        TextureLayer { texture: Some(texture), fit: TextureFit::Actual, sampling: Sampling::Nearest, source }
                    }
                }
            }
        }
        Seg::Picture {
            standing,
            picked,
            width,
            source,
            ..
        } => {
            let ground = css(ground_of(palette, *standing, *picked));
            let left = INSET.horizontal - CARD_MARGIN;
            rsx! {
                div {
                    key: "{key}",
                    class: "temor-bseg temor-blk-picture",
                    style: "{place}; background:{ground}",
                    onpointermove: on_move,
                    img {
                        class: "temor-blk-img",
                        alt: "",
                        src: source.to_string(),
                        style: "left:{left}px; width:{width}px; height:{p.height}px",
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
        Seg::Kept(card) => {
            let ground = css(ground_of(palette, Standing::Done, false));
            rsx! {
                KeptHead {
                    key: "{key}",
                    card: card.clone(),
                    style: "{place}; background:{ground}",
                    columns: head.columns,
                    put: head.actions.put,
                    on_move,
                }
            }
        }
        Seg::Composer { .. } if let Some(view) = composer => rsx! {
            ComposerCard {
                key: "{key}",
                style: "{place}; background:{css(ground_of(palette, Standing::Done, false))}",
                font: view.font.clone(),
                text: view.text.clone(),
                selection: view.selection,
                focus: view.focus,
                take: view.take,
                actions: view.actions.clone(),
                choices: view.choices.clone(),
            }
        },
        Seg::Composer { .. } => rsx! {
            div { key: "{key}", class: "temor-bseg temor-blk-prompt", style: "{place}",
                if let Some(texture) = handle {
                    TextureLayer { texture: Some(texture), fit: TextureFit::Actual, sampling: Sampling::Nearest, source }
                }
            }
        },
    }
}

/// A kept command's card: the command and how it went, its output not kept, and Put in Prompt.
#[component]
#[allow(non_snake_case)]
fn KeptHead(
    card: KeptCard,
    style: String,
    columns: usize,
    put: EventHandler<u32>,
    on_move: EventHandler<PointerEvent>,
) -> Element {
    let n = card.n;
    let standing = Standing::of(card.outcome);
    let meta = meta_words(card.outcome, card.took_ms, None);
    let title = clip_chars(card.title(), columns.saturating_sub(16).max(8));
    rsx! {
        div {
            class: "temor-bseg temor-blk-head temor-blk-kept",
            "data-state": standing.slug(),
            "data-whole": "true",
            style,
            onpointermove: move |e| on_move.call(e),
            // The fold chevron's room, so titles align with the pane's own cards.
            div { class: "temor-blk-fold", "data-none": true }
            span { class: "temor-blk-title", "{title}" }
            if !meta.is_empty() {
                span { class: "temor-blk-meta", "{meta}" }
            }
            div { class: "temor-blk-tools",
                ToolButton {
                    icon: Icon::Reply,
                    size: ControlSize::Mini,
                    propagation: Propagation::Stop,
                    label: "Put in Prompt",
                    tip: "Type this command at the prompt again, without running it",
                    onclick: move |()| put.call(n),
                }
            }
        }
    }
}

/// The head of the card the list is scrolled into, kept at the list's top while the card's own
/// head is scrolled away, and pushed up by the card's end. It follows every step of the scroll
/// (the list itself is laid out again only band by band) but draws again only when the card or
/// the push changes.
#[component]
#[allow(non_snake_case)]
fn StickyHead(
    cards: Vec<CardPlace>,
    scroller: ScrollerRef,
    palette: Palette,
    head: Head,
    height: f32,
    hovered: Signal<Option<BlockId>>,
) -> Element {
    let place = use_memo(use_reactive!(|cards, height| {
        let offset = scroller.scroll()().offset.0;
        sticky_head(&cards, offset, height).map(|(index, y)| (index, y.round() as i32))
    }));
    let Some((card, y)) = place().and_then(|(index, y)| Some((cards.get(index)?.clone(), y)))
    else {
        return rsx! {};
    };
    let block = card.block;
    let id = block.id;
    let standing = Standing::of(block.outcome);
    let ground = ground_of(&palette, standing, block.picked);
    let ground = format!("rgb({},{},{})", ground.r, ground.g, ground.b);
    let mut hovered = hovered;
    rsx! {
        div { class: "temor-blk-sticky",
            CardHead {
                block,
                whole: false,
                style: "top:{y}px; height:{height}px; background:{ground}",
                columns: head.columns,
                tools: head.tools_on == Some(id),
                actions: head.actions.clone(),
                on_move: move |_| {
                    if *hovered.peek() != Some(id) {
                        hovered.set(Some(id));
                    }
                },
            }
        }
    }
}

/// A card's head: the fold chevron, the command, what it came to, and, on the card the tools are
/// on, its tools: copy the output, type it again and more, or, while it runs, detach and stop.
#[component]
#[allow(non_snake_case)]
fn CardHead(
    block: ListBlock,
    /// Nothing is under the head: it is the whole card.
    whole: bool,
    style: String,
    columns: usize,
    tools: bool,
    actions: CardActions,
    on_move: EventHandler<PointerEvent>,
) -> Element {
    let id = block.id;
    let standing = Standing::of(block.outcome);
    let running = standing == Standing::Running;
    let has_output = !block.output.is_empty();
    let meta = meta_of(&block);
    let title = clip_chars(&block.command, columns.saturating_sub(16).max(8));
    let more_probe = use_rect();
    let (fold_icon, fold_label) = match block.folded {
        true => (Icon::ChevronRight, "Show output"),
        false => (Icon::ChevronDown, "Fold output"),
    };
    rsx! {
        div {
            class: "temor-bseg temor-blk-head",
            "data-state": standing.slug(),
            "data-picked": block.picked,
            "data-whole": whole.then_some("true"),
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
            if block.bookmarked {
                span { class: "temor-blk-mark", title: "Bookmarked",
                    IconView { source: IconSource::Glyph(Icon::Star), size: IconSize::Small }
                }
            }
            span { class: "temor-blk-title", "{title}" }
            if !meta.is_empty() {
                span { class: "temor-blk-meta", "{meta}" }
            }
            if tools {
                div { class: "temor-blk-tools",
                    div {
                        class: "temor-blk-more",
                        onmounted: move |event: MountedEvent| more_probe.on_mounted(event),
                        ToolButton {
                            icon: Icon::Ellipsis,
                            size: ControlSize::Mini,
                            propagation: Propagation::Stop,
                            label: "More",
                            tip: "What can be done with this command",
                            onclick: move |()| {
                                let at = more_probe.rect().map(|rect| Point {
                                    x: rect.left(),
                                    y: rect.bottom(),
                                });
                                actions.menu.call((id, at.unwrap_or_default()));
                            },
                        }
                    }
                }
            }
        }
    }
}
