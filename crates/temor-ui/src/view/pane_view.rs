//! One pane in the window (PLAN §2, temor-ui): the terminal region with its find bar, sticky header, block toolbar,
//! status line and paste confirmation. The component wires Dioxus events to the pane's session and
//! the session's state to elements; the session does the work (see `slot` for the state and
//! `convert` for the event values). A pane is mounted only while its tab is in front, and what it
//! holds on the page (the element handles of the input method and the rect) goes with it.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::file_drop::drag::FileDrop;
use ds::file_drop::hook::use_file_drop;
use ds::host::document::use_document_host;
use ds::host::ime::ImeEvent;
use ds::host::measure::use_rect;
use ds::host::signals::HostSignals;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds_blitz::{Sampling, TextureFit, TextureLayer, use_gpu};
use temor_core::remote::Grant;
use temor_core::vocab::ids::{BlockId, PaneId};

use super::block_edit::bar_edit;
use super::block_header::BlockHeader;
use super::block_tools::{BlockTools, ToolsView};
use super::blocks::{Chrome, chrome_of};
use super::clipboard_ask::{ClipboardConfirm, clipboard_answer};
use super::convert::scale_of;
use super::fact_chips::{FactChips, chips_for};
use super::find_bar::FindBar;
use super::handlers::{
    bar_copy_command, bar_copy_output, bar_rerun, bar_save, block_unhovered, find_close,
    find_query, find_step, header_press, then_regain,
};
use super::ime::{ImeGlue, composed_sink};
use super::pane_focus::{PaneFocus, use_pane_focus};
use super::pane_footer::{Shell, pane_footer};
use super::pane_keys::key_handler;
use super::pane_pointer::{GridPointer, grid_pointer};
use super::peek::use_peek;
use super::peek_card::peek_card;
use super::pointer::use_pane_mouse;
use super::slot::{Device, Display, Slot, layout};
use super::viewer_open::{bar_view, view_if_asked};
use super::window_help::device_pixels;
use crate::hub::Desk;

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
    let hovered = use_signal(|| None::<BlockId>);
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

    let on_key = key_handler(
        Rc::clone(&slot),
        ime.clone(),
        apply.clone(),
        desk.clone(),
        peek.key.clone(),
        pane,
        rect,
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
    } = grid_pointer(
        &slot,
        &peek,
        signals,
        hovered,
        ui,
        desk.notice,
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
        .map_or_else(Chrome::default, |s| chrome_of(s, hovered()));
    let clip_ask = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| s.clipboard_ask().cloned());
    let facts = desk.hub.facts_of(pane);
    let on_host = desk.hub.is_host_pane(pane);
    // "Edit here" is only for a pane on another computer.
    let tools = chrome.tools.clone().map(|tools| ToolsView {
        edit: on_host.then_some(Availability::Enabled),
        ..tools
    });
    let chips = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| chips_for(s, &facts));
    rsx! {
        div { class: "temor-pane",
            // On the pane, not the grid: the block toolbar beside the grid keeps the hover.
            onpointerleave: block_unhovered(Rc::clone(&slot), hovered),
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
                    "data-wheel": "capture",
                    "data-drop": dropper.drop_attr(),
                    onmounted: move |e: MountedEvent| {
                        dropper.mounted(e.clone());
                        mouse.capture.on_mounted(e);
                    },
                    onpointerdown: on_press,
                    onpointerup: move |e| (mouse.up)(e),
                    onpointermove: on_move,
                    onpointerleave: on_leave,
                    onwheel: move |e| {
                        (mouse.wheel)(e);
                        // The hovered block follows the wheel; draw it again.
                        let mut ui = ui;
                        ui += 1;
                    },
                    TextureLayer { texture: Some(texture.clone()), fit: TextureFit::Actual, sampling: Sampling::Nearest }
                    if let Some(text) = preedit() {
                        div { class: "temor-preedit", "{text}" }
                    }
                }
                // Beside the grid, not in it: a press on these never reaches the terminal.
                if let Some(header) = chrome.header.clone() {
                    BlockHeader {
                        header,
                        on_press:then_regain(header_press(Rc::clone(&slot), ui), ime.clone(), regain),
                    }
                }
                if let Some(view) = chips {
                    FactChips { view }
                }
                if let Some(card) = (peek.card)() {
                    {peek_card(&card, peek.measure)}
                }
                if let Some(tools) = tools {
                    BlockTools {
                        tools,
                        on_copy_command:then_regain(bar_copy_command(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_copy_output:then_regain(bar_copy_output(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_save:then_regain(bar_save(Rc::clone(&slot), ui, desk.notice), ime.clone(), regain),
                        on_rerun:then_regain(bar_rerun(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_view:bar_view(Rc::clone(&slot), desk.clone(), pane),
                        on_edit:bar_edit(Rc::clone(&slot), desk.clone(), pane),
                    }
                }
            }
            // On the pane's top-right corner, over the grid.
            if let Some((query, position)) = find {
                FindBar {
                    query,
                    position,
                    on_query:find_query(Rc::clone(&slot), ui, started),
                    on_step:find_step(Rc::clone(&slot), ui),
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
