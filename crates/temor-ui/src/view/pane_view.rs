//! One pane in the window (PLAN §2, temor-ui): the terminal region with its find bar, sticky header, block toolbar,
//! status line and paste confirmation. The component wires Dioxus events to the pane's session and
//! the session's state to elements; the session does the work (see `slot` for the state and
//! `convert` for the event values). A pane is mounted only while its tab is in front, and what it
//! holds on the page (the element handles of the input method and the rect) goes with it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::edit::composition::Composing;
use ds::edit::pointer::EditFocus;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::file_drop::drag::FileDrop;
use ds::file_drop::hook::use_file_drop;
use ds::host::document::use_document_host;
use ds::host::ime::ImeEvent;
use ds::host::measure::use_rect;
use ds::host::signals::HostSignals;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds_blitz::{Sampling, TextureFit, TextureLayer, use_gpu};
use temor_core::notify::Focus;
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_core::workspace::WorkspaceIn;
use temor_pane::keys::Action;
use temor_vt::Ime;

use super::block_header::BlockHeader;
use super::block_tools::BlockTools;
use super::blocks::{Chrome, chrome_of};
use super::confirm::PasteConfirm;
use super::convert::scale_of;
use super::fact_chips::{FactChips, chips_for};
use super::find_bar::FindBar;
use super::handlers::{
    bar_copy_command, bar_copy_output, bar_rerun, bar_save, block_hovered, find_close, find_query,
    find_step, gutter_pressed, header_press, paste_cancel, paste_confirm, restart_pane,
    then_regain,
};
use super::host_banners::HostBanners;
use super::ime::{ImeGlue, composed_sink, focus_handler};
use super::offer::OfferBar;
use super::peek::use_peek;
use super::peek_card::peek_card;
use super::pointer::use_pane_mouse;
use super::slot::{Device, Display, Slot, layout, repaint};
use super::status::StatusLine;
use super::viewer_open::{bar_view, view_if_asked};
use super::window_help::{device_pixels, focus_in, place_cursor, set_focus, take_notice};
use crate::hub::{Desk, WindowAction, workspace_input};
use crate::input::stroke;

/// Puts the pane on screen: it paints into `texture` from now on.
fn show(slot: &RefCell<Slot>, texture: ds_blitz::TextureHandle) {
    let mut s = slot.borrow_mut();
    s.texture = Some(texture);
    s.display = Display::Shown;
}

/// Takes the pane off the screen: nothing paints into a texture that is gone.
fn hide(slot: &RefCell<Slot>) {
    let mut s = slot.borrow_mut();
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
    let texture = use_hook(|| gpu.handle());
    use_hook({
        let (slot, texture) = (Rc::clone(&slot), texture.clone());
        move || show(&slot, texture)
    });
    let ime = use_hook(|| ImeGlue::new(Rc::clone(&host)));
    {
        let (slot, ime) = (Rc::clone(&slot), ime.clone());
        use_drop(move || {
            hide(&slot);
            ime.release();
        });
    }
    let ui = desk.ui;
    let preedit = use_signal(|| None::<String>);
    let hovered = use_signal(|| None::<BlockId>);
    // Whether this pane had the keyboard the last time it was drawn.
    let had_keyboard = use_hook(|| Rc::new(Cell::new(false)));
    // The ⌘-hover preview of a file path (PLAN §10).
    let peek = use_peek(Rc::clone(&slot), desk.clone(), rect, signals);

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

    // Every key the region hears (a press, a repeat, a release) as quire's raw key.
    let on_key = {
        let (slot, ime, apply, desk) = (Rc::clone(&slot), ime.clone(), apply.clone(), desk.clone());
        let peek_key = peek.key.clone();
        move |raw: RawKey| {
            peek_key(&raw);
            let composing = match ime.composing.get() {
                Composing::Active => Ime::Composing,
                Composing::Idle | Composing::Cleared => Ime::Idle,
            };
            if raw.phase != KeyPhase::Release {
                ime.settle(&apply);
            }
            if composing == Ime::Composing {
                return;
            }
            // An unidentified key is ignored; the rest go to the session.
            let outcome = match stroke(&raw, composing) {
                Some(stroke) => slot.borrow_mut().session.as_mut().map(|s| s.key(stroke)),
                None => None,
            };
            if let Some(Err(err)) = outcome {
                desk.say(err.to_string());
            }
            if let Some(message) = take_notice(&slot) {
                desk.say(message);
            }
            // A chord that is not the program's is the window's: the bar and the quick terminal
            // are its own; a tab or split chord is the workspace's. The quick terminal has no
            // tabs or splits of its own.
            let asked = slot
                .borrow_mut()
                .session
                .as_mut()
                .and_then(|s| s.take_asked());
            if let Some(Action::SelectSpace(number)) = asked {
                desk.select_space(number);
                return;
            }
            if asked == Some(Action::JumpToWaiting) {
                desk.jump_to_waiting();
                return;
            }
            if let Some(action) = asked {
                if desk.window_action(action) == WindowAction::Taken {
                    return;
                }
                if desk.hub.is_quick(pane) {
                    return;
                }
                if let Some(input) = workspace_input(action) {
                    desk.act(input);
                    return;
                }
            }
            desk.bump();
            repaint(&slot);
            place_cursor(ime.clone(), Rc::clone(&slot), rect);
        }
    };

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

    // The region has the keyboard: the workspace and the session know, and the input method is on
    // and placed.
    let gained = {
        let (ime, slot, desk) = (ime.clone(), Rc::clone(&slot), desk.clone());
        move || {
            set_focus(&slot, Focus::Focused);
            desk.pane_focused(pane);
            if desk.hub.keyboard_pane() != Some(pane) {
                desk.act(WorkspaceIn::Focus(pane));
            }
            focus_in(ime.clone(), Rc::clone(&slot), rect);
        }
    };
    let on_focus = {
        let mut inner = focus_handler(Rc::clone(&slot), ime.clone(), apply.clone(), gained.clone());
        let had = Rc::clone(&had_keyboard);
        let desk = desk.clone();
        move |focus: EditFocus| {
            // The keyboard left the page region (the sidebar took it, say): the pane takes it
            // back the next time the window is told it is the keyboard pane.
            if matches!(focus, EditFocus::Out) {
                had.set(false);
                desk.pane_blurred(pane);
            }
            inner(focus);
        }
    };

    // Where the keyboard goes back to when the find bar closes (its field leaves the page).
    let regain = EventHandler::new({
        let gained = gained.clone();
        move |()| gained()
    });

    // The pane that becomes the keyboard pane (a chord moved focus, a tile was pressed) takes the
    // keyboard on the page too; not while the bar's field has it, and back when the bar closes.
    {
        let (ime, gained, desk, had) = (
            ime.clone(),
            gained.clone(),
            desk.clone(),
            Rc::clone(&had_keyboard),
        );
        use_effect(move || {
            let _ = (desk.ui)();
            let has = desk.hub.keyboard_pane() == Some(pane) && !desk.hub.bar_is_open();
            let before = had.replace(has);
            if has && !before {
                let gained = gained.clone();
                ime.take_focus(EventHandler::new(move |()| gained()));
            }
        });
    }

    // The region's element: rect and input method hear it. The first layout gives the pane that
    // has the keyboard the keyboard, as a person's click would.
    let on_mounted = {
        let (ime, gained, desk) = (ime.clone(), gained.clone(), desk.clone());
        move |event: MountedEvent| {
            rect.on_mounted(event.clone());
            let element = event.data();
            ime.element.replace(Some(Rc::clone(&element)));
            let (ime, gained) = (ime.clone(), gained.clone());
            let keyboard = desk.hub.keyboard_pane() == Some(pane);
            spawn(async move {
                ime.listen(element, heard).await;
                if keyboard {
                    ime.take_focus(EventHandler::new(move |()| gained()));
                }
            });
        }
    };

    // The pointer over the grid also moves the preview; a press or leaving puts it away.
    let on_move = {
        let (moved, mut hover) = (
            peek.moved.clone(),
            block_hovered(Rc::clone(&slot), signals, hovered),
        );
        move |e: PointerEvent| {
            moved(e.clone());
            hover(e);
        }
    };
    let on_press = {
        let (gone, mut press) = (
            peek.gone.clone(),
            gutter_pressed(
                Rc::clone(&slot),
                signals,
                ui,
                desk.notice,
                mouse.down.clone(),
            ),
        );
        move |e: PointerEvent| {
            gone();
            press(e);
        }
    };
    let on_leave = {
        let gone = peek.gone.clone();
        move |_: PointerEvent| gone()
    };
    let _ = ui();
    let view = slot.borrow().session.as_ref().map(|s| {
        (
            s.finder().map(|f| (s.find_text().to_owned(), f.position())),
            s.pending_paste().map(|p| p.reason),
            s.ended(),
        )
    });
    let (find, pending, ended) = view.unwrap_or((None, None, false));
    let chrome = slot
        .borrow()
        .session
        .as_ref()
        .map_or_else(Chrome::default, |s| chrome_of(s, hovered()));
    let facts = desk.hub.facts_of(pane);
    let chips = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| chips_for(s, &facts));
    rsx! {
        div { class: "temor-pane",
            if let Some((query, position)) = find {
                FindBar {
                    query,
                    position,
                    on_query:find_query(Rc::clone(&slot), ui, started),
                    on_step:find_step(Rc::clone(&slot), ui),
                    on_close:find_close(Rc::clone(&slot), ui, ime.clone(), regain),
                }
            }
            RawKeySurface {
                on_key: EventHandler::new(on_key),
                on_focus: EventHandler::new(on_focus),
                common: Common {
                    aria_label: Some("Terminal".to_owned()),
                    extra_class: ExtraClass::parse("temor-keys").ok(),
                    mounted: Some(EventHandler::new(on_mounted)),
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
                    onwheel: move |e| (mouse.wheel)(e),
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
                if let Some(tools) = chrome.tools.clone() {
                    BlockTools {
                        tools,
                        on_copy_command:then_regain(bar_copy_command(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_copy_output:then_regain(bar_copy_output(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_save:then_regain(bar_save(Rc::clone(&slot), ui, desk.notice), ime.clone(), regain),
                        on_rerun:then_regain(bar_rerun(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_view:bar_view(Rc::clone(&slot), desk.clone(), pane),
                    }
                }
            }
            HostBanners { pane, desk: desk.clone() }
            OfferBar { pane, desk: desk.clone() }
            StatusLine {
                cwd: chrome.cwd.clone(),
                title: chrome.title.clone(),
                progress: chrome.progress,
                bell: chrome.bell,
            }
            if let Some(reason) = pending {
                PasteConfirm {
                    reason,
                    on_paste:paste_confirm(Rc::clone(&slot), ui),
                    on_cancel:paste_cancel(Rc::clone(&slot), ui),
                }
            }
            if ended {
                InlineBanner {
                    severity: Severity::Warn,
                    text: "The shell has ended.".to_owned(),
                    actions: rsx! {
                        Button {
                            label: "Restart".to_owned(),
                            title: Some("Start a new shell".to_owned()),
                            onclick:restart_pane(desk.clone(), pane),
                        }
                    },
                }
            }
        }
    }
}
