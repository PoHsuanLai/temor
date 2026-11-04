//! One pane in the window (PLAN §2, temor-ui): the terminal region drawn as its list of blocks
//! (or, while a full-screen program holds it, as that program's screen), with its find bar,
//! block menu, status line and paste confirmation. The component wires Dioxus events to the pane's session and
//! the session's state to elements; the session does the work (see `slot` for the state and
//! `convert` for the event values). A pane is mounted only while its tab is in front, and what it
//! holds on the page (the element handles of the input method and the rect) goes with it.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::edit::raw_key::RawKey;
use ds::file_drop::drag::FileDrop;
use ds::file_drop::hook::use_file_drop;
use ds::focus::request::use_focus_request;
use ds::host::caret::InitialCaret;
use ds::host::document::use_document_host;
use ds::host::ime::ImeEvent;
use ds::host::measure::use_rect;
use ds::host::signals::HostSignals;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds_blitz::{Sampling, TextureFit, TextureLayer, use_gpu};
use temor_client::model::remote::Grant;
use temor_client::model::vocab::ids::{BlockId, PaneId};

use super::block_edit::edit_on_this_computer;
use super::block_list::pane::{hand_list_view, lens_request};
use super::block_list::{BlockPane, CardActions, ComposerActions, ComposerView};
use super::block_menu::{BlockMenu, BlockMenuAt, BlockOwn};
use super::clipboard_ask::{ClipboardConfirm, clipboard_answer};
use super::console_bar::ConsoleTools;
use super::convert::scale_of;
use super::find_bar::FindBar;
use super::focus_edge::use_focus_edge;
use super::handlers::{
    composer_close, composer_complete, composer_history, composer_input, composer_pick,
    composer_submit, composer_take, find_close, find_query, find_step, list_fold, list_put,
    pointer_gone, then_regain,
};
use super::ime::{ImeGlue, composed_sink};
use super::link_tip::LinkTip;
use super::menu::Pick;
use super::pane_focus::{FocusWiring, use_pane_focus};
use super::pane_footer::{Shell, pane_footer};
use super::pane_keys::{KeyRegion, key_handler};
use super::pane_link::{PaneLink, Redraw};
use super::pane_pointer::{GridPointer, GridState, grid_pointer};
use super::pane_shown::{PaneShown, use_pane_shown};
use super::peek::use_peek;
use super::peek_card::peek_card;
use super::pointer::use_pane_mouse;
use super::viewer_open::bar_view;
use super::window_help::device_pixels;
use temor_client::model::menu::Item;
use temor_hub::Desk;
use temor_panes::pane::keys::Action;
use temor_ui_base::slot::{Slot, layout};

/// Puts the pane on screen: it paints into `texture` from now on.
fn show(slot: &RefCell<Slot>, texture: ds_blitz::TextureHandle) {
    let mut s = slot.borrow_mut();
    s.texture = Some(texture);
    s.shown = true;
}

/// Takes the pane off the screen: nothing paints into a texture that is gone. Only when the slot
/// still paints into this view's `texture`: a split or a move mounts the pane's new view before
/// the old one is dropped, and the old one must not take the new one's texture away.
fn hide(slot: &RefCell<Slot>, texture: &ds_blitz::TextureHandle) {
    let mut s = slot.borrow_mut();
    if s.texture.as_ref() != Some(texture) {
        return;
    }
    s.texture = None;
    s.shown = false;
}

/// One terminal pane. `started` is when the window opened (the pointer's click clock).
#[component]
#[allow(non_snake_case)]
pub fn PaneView(pane: PaneId, desk: Desk, started: Instant) -> Element {
    let host = use_hook(use_document_host);
    let gpu = use_gpu();
    let rect = use_rect();
    let signals = use_hook(try_consume_context::<HostSignals>);
    let slot = use_hook(|| desk.hub.slot(pane).unwrap_or_default());
    // A host's programs write to the clipboard as the person last answered for that host.
    use_hook(|| {
        if let (Some(grant), Some(s)) = (
            desk.hub.clipboard_consent(pane),
            slot.borrow_mut().session.as_mut(),
        ) {
            s.set_clipboard_consent(grant);
        }
    });
    let texture = use_hook(|| gpu.handle());
    use_hook({
        let (slot, texture) = (Rc::clone(&slot), texture.clone());
        move || show(&slot, texture)
    });
    let ime = use_hook(|| ImeGlue::new(Rc::clone(&host)));
    {
        let (slot, ime, texture) = (Rc::clone(&slot), ime.clone(), texture.clone());
        use_drop(move || {
            hide(&slot, &texture);
            ime.release();
        });
    }
    let link = PaneLink {
        slot: Rc::clone(&slot),
        desk: desk.clone(),
        pane,
    };
    let preedit = use_signal(|| None::<String>);
    let block_menu = use_signal(|| None::<BlockMenuAt>);
    // The ⌘-hover preview of a file path (PLAN §10).
    let peek = use_peek(pane, Rc::clone(&slot), desk.clone(), rect, signals);

    // Every change of the pane's size, scale or device, or of the session behind it, lays it out.
    {
        let (slot, signals, gpu, desk) = (Rc::clone(&slot), signals, gpu.clone(), desk.clone());
        use_effect(move || {
            let _ = (desk.ui)();
            let scale = scale_of(signals.as_ref());
            let Some(logical) = rect.rect() else { return };
            let pixels = device_pixels(logical.size.width.0, logical.size.height.0, scale);
            let device = gpu.device().is_some();
            if let Some(text) = layout(&slot, pixels, scale, device) {
                desk.say(text);
            }
        });
    }

    let apply = composed_sink(Rc::clone(&slot), preedit);
    let heard = {
        let (ime, apply) = (ime.clone(), apply.clone());
        use_callback(move |event: ImeEvent| ime.event(event, &apply))
    };

    // The composer's field takes the caret at its end: typing goes on from what is there.
    let composer_focus = use_focus_request().with_caret(InitialCaret::End);
    let on_key = key_handler(
        link.clone(),
        ime.clone(),
        apply.clone(),
        peek.key.clone(),
        KeyRegion {
            pane,
            rect,
            composer: composer_focus,
        },
    );
    // The pane that becomes the keyboard pane while the composer is shown gives it the caret,
    // and again when a menu's pick gives the keyboard back.
    {
        let (slot, desk) = (Rc::clone(&slot), desk.clone());
        use_focus_edge(
            move || {
                let _ = (desk.ui)();
                let want = desk.hub.keyboard_pane() == Some(pane)
                    && !desk.hub.bar_is_open()
                    && slot
                        .borrow()
                        .session
                        .as_ref()
                        .is_some_and(|s| s.composer_shown() && s.finder().is_none());
                (want, (desk.regain)())
            },
            move || composer_focus.request(),
        );
    }
    // A chord typed in the composer's field is the terminal's, as if typed in the region.
    let composer_chord = key_handler(
        link.clone(),
        ime.clone(),
        apply.clone(),
        peek.key.clone(),
        KeyRegion {
            pane,
            rect,
            composer: composer_focus,
        },
    );

    // A ⌘-click's viewer opens once the press is released: opening it splits the pane, which must
    // not happen while the press holds the pointer.
    let released: Rc<dyn Fn()> = {
        let link = link.clone();
        Rc::new(move || link.settle())
    };
    let mouse = use_pane_mouse(
        Rc::clone(&slot),
        rect,
        signals,
        desk.notice,
        started,
        released,
    );

    // Files dropped on the pane are typed into its prompt, quoted for its shell.
    let dropper = use_file_drop({
        let desk = desk.clone();
        move |files: FileDrop| {
            desk.tell(desk.hub.drop_files(pane, files.paths));
        }
    });

    let FocusWiring {
        regain,
        on_focus,
        on_mounted,
    } = use_pane_focus(pane, &desk, &slot, &ime, rect, apply, heard);

    // The pointer over the grid also moves the preview; a press or leaving puts it away.
    let GridPointer {
        moved: on_move,
        pressed: on_press,
        left: on_leave,
        context: on_context,
    } = grid_pointer(
        &link,
        &peek,
        signals,
        GridState {
            menu: block_menu,
            rect,
        },
        mouse.down.clone(),
    );
    let PaneShown {
        find,
        pending,
        ended,
        chrome,
        clip_ask,
        console_row,
        link_tip,
        menu: menu_state,
        card_font,
        items: block_items,
        picked: picked_block,
        found,
        pictures,
        composer_text,
        selection,
        choices,
        take,
        tips: (previous_tip, next_tip),
    } = use_pane_shown(&slot, &desk, pane, move || block_menu().is_some())();
    let shell = match ended {
        true => Shell::Ended,
        false => Shell::Running,
    };
    let find_toggle = {
        let link = link.clone();
        let open = find.is_some();
        let mut close = find_close(link.clone(), ime.clone(), regain);
        move |()| match open {
            true => close(()),
            false => {
                link.send(Redraw::WithGrid, |s| s.run(Action::Find));
            }
        }
    };
    let menu_pick = {
        let (link, mut menu) = (link.clone(), block_menu);
        move |pick: Pick<BlockOwn>| {
            menu.set(None);
            let (slot, desk) = (&link.slot, &link.desk);
            let picked = slot
                .borrow()
                .session
                .as_ref()
                .and_then(|s| s.selected_block());
            let run = |action: Action| {
                link.send(Redraw::WithGrid, |s| s.run(action));
            };
            match pick {
                Pick::Item(Item::PutInPrompt) => run(Action::ReinputCommands),
                Pick::Item(Item::Bookmark { .. }) => run(Action::ToggleBookmark),
                Pick::Item(Item::Fold { .. }) => run(Action::ToggleFold),
                Pick::Item(Item::SelectAllBlocks) => run(Action::SelectAllBlocks),
                Pick::Item(Item::Stop) => {
                    link.send(Redraw::WithGrid, |s| s.send_bytes(b"\x03"));
                }
                Pick::Item(Item::SaveOutput) => {
                    if let Some(block) = picked
                        && let Some(Err(err)) =
                            link.send(Redraw::WithGrid, |s| s.save_output(block))
                    {
                        desk.say(err.to_string());
                    }
                }
                Pick::Item(Item::ViewFiles) => {
                    if let Some(block) = picked {
                        bar_view(link.clone())(block);
                    }
                }
                Pick::Item(Item::EditLocally) => edit_on_this_computer(slot, desk, pane, None),
                Pick::Item(_) => {}
                Pick::Own(BlockOwn::Lens(lens)) => {
                    let asked = slot.borrow().session.as_ref().and_then(lens_request);
                    if let Some((block, width)) = asked {
                        desk.lens_run(pane, block, &lens, width);
                    }
                }
                Pick::Own(BlockOwn::HidePicture) => {
                    if let Some(block) = picked {
                        desk.lens_hide(pane, block);
                    }
                }
            }
            link.redraw(Redraw::WithGrid);
        }
    };
    let listed = block_items.is_some();
    // A pane that stops being a list says so once it is drawn (the list says what it shows).
    let (unlisted, unlisted_desk) = (Rc::clone(&slot), desk.clone());
    use_effect(use_reactive!(|listed| {
        if !listed {
            hand_list_view(&unlisted, &unlisted_desk, None);
        }
    }));
    let wheel_slot = Rc::clone(&slot);
    rsx! {
        div { class: "temor-pane",
            onpointerleave: pointer_gone(Rc::clone(&slot)),
            if let Some(row) = console_row.clone() {
                div { class: "temor-console-bar",
                    if let Some((query, position)) = find.clone() {
                        FindBar {
                            query,
                            position,
                            on_query: find_query(link.clone(), started),
                            on_step: find_step(link.clone()),
                            previous_tip: previous_tip.clone(),
                            next_tip: next_tip.clone(),
                            on_close: find_close(link.clone(), ime.clone(), regain),
                            inline: true,
                        }
                    } else {
                        span { class: "temor-console-grow" }
                    }
                    ConsoleTools { pane, row, finding: find.is_some(), desk: desk.clone(), on_find: find_toggle }
                }
            }
            RawKeySurface {
                on_key: EventHandler::new(on_key),
                on_focus,
                common: Common {
                    aria_label: Some("Terminal".to_owned()),
                    extra_class: ExtraClass::parse("temor-keys").ok(),
                    mounted: Some(on_mounted),
                    ..Common::default()
                },
                div {
                    class: "temor-grid",
                    // A list of blocks scrolls itself: the wheel is the window's to give it.
                    "data-wheel": (!listed).then_some("capture"),
                    "data-drop": dropper.drop_attr(),
                    onmounted: move |e: MountedEvent| {
                        dropper.mounted(e.clone());
                        mouse.capture.on_mounted(e);
                    },
                    onpointerdown: on_press,
                    onpointerup: move |e| (mouse.up)(e),
                    onpointermove: on_move,
                    onpointerleave: on_leave,
                    oncontextmenu: on_context,
                    onwheel: move |e| {
                        // A list scrolls itself; only a program that asked for the mouse hears
                        // the wheel over its live rows (they capture it), and never with Shift,
                        // which would scroll the terminal under the list.
                        let program = wheel_slot.borrow().session.as_ref().is_some_and(|s| s.mouse_tracking());
                        if listed && !(program && !e.modifiers().contains(Modifiers::SHIFT)) {
                            return;
                        }
                        (mouse.wheel)(e);
                    },
                    if let Some(items) = block_items {
                        BlockPane {
                            pane,
                            desk: desk.clone(),
                            items,
                            texture: texture.clone(),
                            font: card_font.clone(),
                            reveal: picked_block,
                            found,
                            pictures,
                            composer: ComposerView {
                                text: composer_text,
                                font: card_font.clone(),
                                selection,
                                focus: composer_focus,
                                take,
                                actions: ComposerActions {
                                    input: EventHandler::new(composer_input(link.clone())),
                                    submit: EventHandler::new(then_regain(composer_submit(link.clone()), ime.clone(), regain)),
                                    history: EventHandler::new(composer_history(link.clone())),
                                    chord: EventHandler::new(composer_chord),
                                    gone: EventHandler::new({ let ime = ime.clone(); move |()| ime.take_focus(regain) }),
                                    key: EventHandler::new({ let key = peek.key.clone(); move |raw: RawKey| key(&raw) }),
                                    complete: EventHandler::new(composer_complete(link.clone())),
                                    pick: EventHandler::new(composer_pick(link.clone())),
                                    take: EventHandler::new(composer_take(link.clone())),
                                    close: EventHandler::new(composer_close(link.clone())),
                                },
                                choices,
                            },
                            actions: CardActions {
                                fold: EventHandler::new(then_regain(list_fold(link.clone()), ime.clone(), regain)),
                                put: EventHandler::new(then_regain(list_put(link.clone()), ime.clone(), regain)),
                                menu: EventHandler::new({
                                    let (link, mut menu) = (link.clone(), block_menu);
                                    move |(block, point): (BlockId, Point)| {
                                        link.send(Redraw::WithGrid, |s| s.block_menu_for(block));
                                        menu.set(Some(BlockMenuAt { point }));
                                    }
                                }),
                            },
                        }
                    } else {
                        TextureLayer { texture: Some(texture.clone()), fit: TextureFit::Actual, sampling: Sampling::Nearest }
                    }
                    if let Some(text) = preedit() {
                        div { class: "temor-preedit", "{text}" }
                    }
                }
                if let Some(spot) = link_tip {
                    LinkTip { spot }
                }
                if let Some(card) = (peek.card)() {
                    {peek_card(&card, peek.measure)}
                }
            }
            if let (Some(at), Some(state)) = (block_menu(), menu_state) {
                BlockMenu {
                    at,
                    state,
                    onpick: then_regain(menu_pick, ime.clone(), regain),
                    onclose: move |()| {
                        let mut menu = block_menu;
                        menu.set(None);
                    },
                }
            }
            // The quick terminal's: on the pane's top-right corner, over the grid.
            if let (None, Some((query, position))) = (&console_row, find) {
                FindBar {
                    query,
                    position,
                    on_query:find_query(link.clone(), started),
                    on_step:find_step(link.clone()),
                    previous_tip,
                    next_tip,
                    on_close:find_close(link.clone(), ime.clone(), regain),
                }
            }
            if let Some(ask) = clip_ask {
                ClipboardConfirm {
                    ask,
                    on_allow:clipboard_answer(link.clone(), Grant::Allowed, { let hub = desk.hub.clone(); move |g| hub.clipboard_answered(pane, g) }),
                    on_refuse:clipboard_answer(link.clone(), Grant::Refused, { let hub = desk.hub.clone(); move |g| hub.clipboard_answered(pane, g) }),
                }
            }
            {pane_footer(&link, &chrome, pending, shell)}
        }
    }
}

#[cfg(test)]
#[path = "pane_view_tests.rs"]
mod tests;
