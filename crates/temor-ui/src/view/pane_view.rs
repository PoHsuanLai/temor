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
use ds_blitz::use_gpu;
use temor_core::remote::Grant;
use temor_core::vocab::ids::{BlockId, PaneId};

use super::block_edit::{bar_edit, edit_on_this_computer};
use super::block_header::BlockHeader;
use super::block_menu::{
    BlockMenu, BlockMenuAt, BlockMenuState, BlockPick, PaneKind, PictureShown,
};
use super::block_meta::BlockMetas;
use super::block_tools::{BlockTools, ToolsView};
use super::blocks::{Chrome, chrome_of};
use super::clipboard_ask::{ClipboardConfirm, clipboard_answer};
use super::convert::scale_of;
use super::fact_chips::{ChipPlace, FactChips, chips_for, header_chips};
use super::find_bar::FindBar;
use super::first_hint::{FirstHint, hint_for};
use super::grid_slices::{GridTexture, grid_cut};
use super::handlers::{
    bar_copy_command, bar_copy_output, bar_rerun, bar_save, block_unhovered, find_close,
    find_query, find_step, header_press, then_regain,
};
use super::ime::{ImeGlue, composed_sink};
use super::lens_picture::{LensPictures, lens_request, lens_views};
use super::link_tip::LinkTip;
use super::pane_focus::{PaneFocus, use_pane_focus};
use super::pane_footer::{Shell, pane_footer};
use super::pane_keys::key_handler;
use super::pane_pointer::{GridPointer, GridState, grid_pointer};
use super::peek::use_peek;
use super::peek_card::peek_card;
use super::pointer::use_pane_mouse;
use super::slot::{Device, Display, Slot, layout, repaint};
use super::viewer_open::{bar_view, view_if_asked};
use super::window_help::device_pixels;
use crate::hub::Desk;
use temor_pane::keys::Action;

/// How far the find bar sits below the pane's top (`.temor-find`'s `top`, `--s-4`).
const FIND_TOP: f32 = 4.0;

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
    // The block whose "more" menu is up keeps its tools whatever the pointer does.
    let held = use_signal(|| None::<BlockId>);
    let find_height = use_signal(|| 0.0_f32);
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
        context: on_context,
    } = grid_pointer(
        &slot,
        &peek,
        signals,
        GridState {
            hovered,
            ui,
            notice: desk.notice,
            menu: block_menu,
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
        .map_or_else(Chrome::default, |s| chrome_of(s, hovered(), held()));
    let clip_ask = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| s.clipboard_ask().cloned());
    // The find bar stands over the pane's top rows: the meta and tools there would lie under it.
    let under_find = |top: f32| find.is_some() && top < FIND_TOP + find_height();
    let metas: Vec<_> = chrome
        .metas
        .iter()
        .filter(|meta| !under_find(meta.top))
        .cloned()
        .collect();
    let facts = desk.hub.facts_of(pane);
    let plugin_facts = desk.hub.plugin_facts_of(pane);
    let folder = desk.hub.chip_folder(pane);
    let on_host = desk.hub.is_host_pane(pane);
    // "Edit Locally" is only for a pane on another computer.
    let tools = chrome
        .tools
        .clone()
        .filter(|tools| !under_find(tools.top))
        .map(|tools| ToolsView {
            edit: on_host.then_some(Availability::Enabled),
            copy_command_key: desk.hub.shortcut_of(Action::CopyCommands),
            ..tools
        });
    let chips = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| chips_for(s, &facts, &plugin_facts, folder.as_deref()));
    // What the prompt's chips say now is kept for the block it becomes.
    let over = chips.as_ref().map(|c| c.place) == Some(ChipPlace::Over);
    if let Some((folder, s)) = folder
        .as_ref()
        .filter(|_| over)
        .zip(slot.borrow_mut().session.as_mut())
    {
        s.keep_header_facts(folder, &facts, &plugin_facts);
    }
    let older_chips: Vec<_> = {
        let slot = slot.borrow();
        let palette = slot.palette;
        slot.session
            .as_ref()
            .map(|s| header_chips(s, &metas, &palette))
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, view)| !under_find(view.top))
            .collect()
    };
    let link_tip = slot.borrow().session.as_ref().and_then(|s| s.link_tip());
    // The grid in slices around the room under blocks with pictures, as the last paint laid it out.
    let cut = slot.borrow().session.as_ref().and_then(grid_cut);
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
        pane: match on_host {
            true => PaneKind::OnHost,
            false => PaneKind::ThisComputer,
        },
        lenses,
        picture,
    });
    // Pictures lenses drew, each under its block.
    let lens_pictures = {
        let pictures = desk.hub.lens_pictures(pane);
        match pictures.is_empty() {
            true => Vec::new(),
            false => slot
                .borrow()
                .session
                .as_ref()
                .map(|s| lens_views(s, pictures))
                .unwrap_or_default(),
        }
    };
    let menu_pick = {
        let (slot, mut ui, mut menu) = (Rc::clone(&slot), ui, block_menu);
        let desk = desk.clone();
        move |pick: BlockPick| {
            menu.set(None);
            match pick {
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
    // One quiet line under a new window's first prompt, until the first command runs.
    let hint = desk.hub.first_hint(pane).and_then(|text| {
        slot.borrow()
            .session
            .as_ref()
            .and_then(|s| hint_for(s, text))
    });
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
                    oncontextmenu: on_context,
                    onwheel: move |e| {
                        (mouse.wheel)(e);
                        // The hovered block follows the wheel; draw it again.
                        let mut ui = ui;
                        ui += 1;
                    },
                    GridTexture { texture: texture.clone(), cut: cut.clone() }
                    if let Some(text) = preedit() {
                        div { class: "temor-preedit", "{text}" }
                    }
                }
                // Beside the grid, not in it: a press on these never reaches the terminal.
                if !lens_pictures.is_empty() {
                    LensPictures { views: lens_pictures }
                }
                if let Some(header) = chrome.header.clone() {
                    BlockHeader {
                        header,
                        on_press:then_regain(header_press(Rc::clone(&slot), ui), ime.clone(), regain),
                    }
                }
                if let Some(view) = chips {
                    FactChips { view }
                }
                for (id, view) in older_chips {
                    FactChips { key: "{id.0}", view }
                }
                if let Some(spot) = link_tip {
                    LinkTip { spot }
                }
                BlockMetas { metas }
                if let Some(view) = hint {
                    FirstHint { view }
                }
                if let Some(card) = (peek.card)() {
                    {peek_card(&card, peek.measure)}
                }
                if let Some(tools) = tools {
                    BlockTools {
                        tools,
                        held,
                        on_copy_command:then_regain(bar_copy_command(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_copy_output:then_regain(bar_copy_output(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_save:then_regain(bar_save(Rc::clone(&slot), ui, desk.notice), ime.clone(), regain),
                        on_rerun:then_regain(bar_rerun(Rc::clone(&slot), ui), ime.clone(), regain),
                        on_view:bar_view(Rc::clone(&slot), desk.clone(), pane),
                        on_edit:bar_edit(Rc::clone(&slot), desk.clone(), pane),
                    }
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
                    on_height: move |height: f32| {
                        let mut find_height = find_height;
                        if *find_height.peek() != height {
                            find_height.set(height);
                        }
                    },
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
