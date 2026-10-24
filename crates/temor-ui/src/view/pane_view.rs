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
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;

use super::block_edit::{bar_edit, edit_on_this_computer};
use super::block_list::pane::lens_request;
use super::block_list::{BlockPane, CardActions, CardOffers, ComposerActions, ComposerView};
use super::block_menu::{
    BlockMenu, BlockMenuAt, BlockMenuState, BlockPick, PaneKind, PictureShown,
};
use super::blocks::{Chrome, chrome_of};
use super::clipboard_ask::{ClipboardConfirm, clipboard_answer};
use super::convert::scale_of;
use super::find_bar::FindBar;
use super::handlers::{
    bar_copy_command, bar_copy_output, bar_rerun, bar_save, block_stop, composer_history,
    composer_input, composer_submit, find_close, find_query, find_step, list_fold, pointer_gone,
    then_regain,
};
use super::ime::{ImeGlue, composed_sink};
use super::link_tip::LinkTip;
use super::live_head::LiveCardHead;
use super::pane_focus::{PaneFocus, use_pane_focus};
use super::pane_footer::{Shell, pane_footer};
use super::pane_keys::{KeyRegion, key_handler};
use super::pane_pointer::{GridPointer, GridState, grid_pointer};
use super::peek::use_peek;
use super::peek_card::peek_card;
use super::pointer::use_pane_mouse;
use super::slot::{Device, Display, Slot, layout, repaint};
use super::viewer_open::{bar_view, view_if_asked};
use super::window_help::device_pixels;
use crate::hub::Desk;
use crate::hub::LiveIcon;
use temor_pane::keys::Action;

/// Puts the pane on screen: it paints into `texture` from now on.
fn show(slot: &RefCell<Slot>, texture: ds_blitz::TextureHandle) {
    let mut s = slot.borrow_mut();
    s.texture = Some(texture);
    s.display = Display::Shown;
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
    s.display = Display::Away;
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
    let ui = desk.ui;
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
            let device = match gpu.device().is_some() {
                true => Device::Ready,
                false => Device::NotYet,
            };
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
        Rc::clone(&slot),
        ime.clone(),
        apply.clone(),
        desk.clone(),
        peek.key.clone(),
        KeyRegion {
            pane,
            rect,
            composer: composer_focus,
        },
    );
    // The pane that becomes the keyboard pane while the composer is shown gives it the caret.
    {
        let (slot, desk) = (Rc::clone(&slot), desk.clone());
        let wanted = use_hook(|| Rc::new(std::cell::Cell::new(false)));
        use_effect(move || {
            let _ = (desk.ui)();
            let s = slot.borrow();
            let want = desk.hub.keyboard_pane() == Some(pane)
                && !desk.hub.bar_is_open()
                && s.session
                    .as_ref()
                    .is_some_and(|s| s.composer_shown() && s.finder().is_none());
            if want && !wanted.replace(want) {
                composer_focus.request();
            }
            wanted.set(want);
        });
    }
    // A chord typed in the composer's field is the terminal's, as if typed in the region.
    let composer_chord = key_handler(
        Rc::clone(&slot),
        ime.clone(),
        apply.clone(),
        desk.clone(),
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
        let (slot, desk) = (Rc::clone(&slot), desk.clone());
        Rc::new(move || view_if_asked(&slot, &desk, pane))
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
            for text in desk.hub.drop_files(pane, files.paths) {
                desk.say(text);
            }
            desk.bump();
        }
    });

    let PaneFocus {
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
        &slot,
        &peek,
        signals,
        GridState {
            ui,
            notice: desk.notice,
            menu: block_menu,
            rect,
        },
        mouse.down.clone(),
    );
    let _ = ui();
    let view = slot.borrow().session.as_ref().map(|s| {
        (
            s.finder().map(|f| (s.find_text().to_owned(), f.position())),
            s.pending_paste().map(|p| p.reason),
            s.ended(),
        )
    });
    let (find, pending, ended) = view.unwrap_or((None, None, false));
    let shell = match ended {
        true => Shell::Ended,
        false => Shell::Running,
    };
    let chrome = slot
        .borrow()
        .session
        .as_ref()
        .map_or_else(Chrome::default, chrome_of);
    let clip_ask = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| s.clipboard_ask().cloned());
    let facts = desk.hub.facts_of(pane);
    let plugin_facts = desk.hub.plugin_facts_of(pane);
    let folder = desk.hub.chip_folder(pane);
    let on_host = desk.hub.is_host_pane(pane);
    // What the shell's folder and facts are now is kept for the block the prompt becomes: its
    // card's folder line says where it ran.
    if let Some((folder, s)) = folder.as_ref().zip(slot.borrow_mut().session.as_mut()) {
        s.keep_header_facts(folder, &facts, &plugin_facts);
    }
    let link_tip = slot.borrow().session.as_ref().and_then(|s| s.link_tip());
    // The block menu speaks of what is picked now.
    let menu_block = match block_menu() {
        Some(_) => slot
            .borrow()
            .session
            .as_ref()
            .and_then(|s| s.selected_block()),
        None => None,
    };
    let (lenses, picture) = match menu_block {
        Some(block) => (
            desk.hub.block_lenses(pane, block),
            match desk.hub.lens_shown(pane, block) {
                true => PictureShown::Yes,
                false => PictureShown::No,
            },
        ),
        None => (Vec::new(), PictureShown::No),
    };
    let menu_state = slot.borrow().session.as_ref().map(|s| BlockMenuState {
        picked: s.picked_blocks().len(),
        bookmarked: s.picks_bookmarked(),
        folded: s.picks_folded(),
        detachable: desk.hub.can_detach(pane),
        pane: match on_host {
            true => PaneKind::OnHost,
            false => PaneKind::ThisComputer,
        },
        lenses,
        picture,
    });
    let menu_pick = {
        let (slot, mut ui, mut menu) = (Rc::clone(&slot), ui, block_menu);
        let desk = desk.clone();
        move |pick: BlockPick| {
            menu.set(None);
            match pick {
                BlockPick::Do(Action::DetachProgram) => desk.detach_block(pane),
                BlockPick::Do(action) => {
                    if let Some(s) = slot.borrow_mut().session.as_mut() {
                        let _ = s.run(action);
                    }
                }
                BlockPick::Lens(lens) => {
                    let asked = slot.borrow().session.as_ref().and_then(lens_request);
                    if let Some((block, width)) = asked {
                        desk.lens_run(pane, block, &lens, width);
                    }
                }
                BlockPick::HidePicture => {
                    let block = slot
                        .borrow()
                        .session
                        .as_ref()
                        .and_then(|s| s.selected_block());
                    if let Some(block) = block {
                        desk.lens_hide(pane, block);
                    }
                }
                BlockPick::EditLocally => edit_on_this_computer(&slot, &desk, pane, None),
            }
            ui += 1;
            repaint(&slot);
        }
    };
    let live_head = desk.hub.live_head(pane);
    // Cards and headers are set in the grid's own family, so a command reads as it was typed.
    let card_font = slot
        .borrow()
        .session
        .as_ref()
        .map(|s| s.grid_font().family_css())
        .unwrap_or_default();
    // The pane as a list of blocks, but for a full-screen program's screen.
    let block_items = slot.borrow().session.as_ref().and_then(|s| s.block_list());
    let listed = block_items.is_some();
    let reground = !listed
        && slot.borrow_mut().session.as_mut().is_some_and(|s| {
            s.set_list_map(None);
            s.set_composer_shown(false);
            s.set_live_ground(None)
        });
    if reground {
        repaint(&slot);
    }
    let picked_block = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| s.selected_block());
    let live_icon = {
        let desk = desk.clone();
        move |icon: LiveIcon| desk.live_icon(pane, icon)
    };
    let wheel_slot = Rc::clone(&slot);
    rsx! {
        div { class: "temor-pane",
            onpointerleave: pointer_gone(Rc::clone(&slot)),
            if let Some(head) = live_head {
                LiveCardHead { head, font: card_font.clone(), on_icon: live_icon }
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
                            found: slot.borrow().session.as_ref().and_then(|s| s.found_row()),
                            pictures: desk.hub.lens_pictures(pane),
                            composer: ComposerView {
                                text: slot.borrow().session.as_ref().map(|s| s.composer_text().to_owned()).unwrap_or_default(),
                                font: card_font.clone(),
                                folder: folder.clone(),
                                selection: slot.borrow().session.as_ref().is_some_and(|s| s.has_selection()),
                                focus: composer_focus,
                                take: desk.hub.keyboard_pane() == Some(pane) && !desk.hub.bar_is_open() && find.is_none(),
                                actions: ComposerActions {
                                    input: EventHandler::new(composer_input(Rc::clone(&slot), ui)),
                                    submit: EventHandler::new(then_regain(composer_submit(Rc::clone(&slot), ui), ime.clone(), regain)),
                                    history: EventHandler::new(composer_history(Rc::clone(&slot), ui)),
                                    chord: EventHandler::new(composer_chord),
                                    gone: EventHandler::new({ let ime = ime.clone(); move |()| ime.take_focus(regain) }),
                                    key: EventHandler::new({ let key = peek.key.clone(); move |raw: RawKey| key(&raw) }),
                                },
                            },
                            actions: CardActions {
                                copy_command: EventHandler::new(then_regain(bar_copy_command(Rc::clone(&slot), ui), ime.clone(), regain)),
                                copy_output: EventHandler::new(then_regain(bar_copy_output(Rc::clone(&slot), ui), ime.clone(), regain)),
                                save: EventHandler::new(then_regain(bar_save(Rc::clone(&slot), ui, desk.notice), ime.clone(), regain)),
                                rerun: EventHandler::new(then_regain(bar_rerun(Rc::clone(&slot), ui), ime.clone(), regain)),
                                view: EventHandler::new(bar_view(Rc::clone(&slot), desk.clone(), pane)),
                                edit: EventHandler::new(bar_edit(Rc::clone(&slot), desk.clone(), pane)),
                                fold: EventHandler::new(then_regain(list_fold(Rc::clone(&slot), ui), ime.clone(), regain)),
                                detach: EventHandler::new({ let desk = desk.clone(); move |()| desk.detach_block(pane) }),
                                stop: EventHandler::new(then_regain(block_stop(Rc::clone(&slot), ui), ime.clone(), regain)),
                                menu_closed: EventHandler::new({ let ime = ime.clone(); move |()| ime.take_focus(regain) }),
                            },
                            offers: CardOffers {
                                detach: desk.hub.can_detach(pane),
                                on_host,
                                copy_command_key: desk.hub.shortcut_of(Action::CopyCommands),
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
            // On the pane's top-right corner, over the grid.
            if let Some((query, position)) = find {
                FindBar {
                    query,
                    position,
                    on_query:find_query(Rc::clone(&slot), ui, started),
                    on_step:find_step(Rc::clone(&slot), ui),
                    previous_tip: desk.hub.tooltip("Previous match", Action::FindPrevious),
                    next_tip: desk.hub.tooltip("Next match", Action::FindNext),
                    on_close:find_close(Rc::clone(&slot), ui, ime.clone(), regain),
                }
            }
            if let Some(ask) = clip_ask {
                ClipboardConfirm {
                    ask,
                    on_allow:clipboard_answer(Rc::clone(&slot), ui, Grant::Allowed, { let hub = desk.hub.clone(); move |g| hub.clipboard_answered(pane, g) }),
                    on_refuse:clipboard_answer(Rc::clone(&slot), ui, Grant::Refused, { let hub = desk.hub.clone(); move |g| hub.clipboard_answered(pane, g) }),
                }
            }
            {pane_footer(pane, &desk, &slot, ui, &chrome, pending, shell)}
        }
    }
}

#[cfg(test)]
#[path = "pane_view_tests.rs"]
mod tests;
