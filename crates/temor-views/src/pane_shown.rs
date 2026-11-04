//! What a pane's view draws of its session and of the hub, read in one go: the pane draws again
//! when this changes, not on every change to the window.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use temor_client::model::session::list::Item;
use temor_client::model::vocab::ids::{BlockId, PaneId};
use temor_hub::plugins::lens::LensPicture;
use temor_hub::{ConsoleRow, Desk, HubRef};
use temor_pane_view::session::{ClipboardAsk, LinkTipSpot, Session};
use temor_panes::pane::keys::Action;
use temor_panes::pane::paste::UnsafeReason;
use temor_ui_base::slot::Slot;

use super::block_list::ChoicesView;
use super::block_menu::BlockMenuState;
use super::blocks::{Chrome, chrome_of};
use temor_client::model::menu::BlockFacts;

/// What the pane draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaneShown {
    /// The find bar's text and position, while it is open.
    pub find: Option<(String, Option<(usize, usize)>)>,
    pub pending: Option<UnsafeReason>,
    pub ended: bool,
    /// The program a tab runs in place of the shell: Restart runs it again.
    pub run_name: Option<String>,
    pub chrome: Chrome,
    pub clip_ask: Option<ClipboardAsk>,
    /// A row's terminal has the console's bar; the quick terminal is in no row.
    pub console_row: Option<ConsoleRow>,
    pub link_tip: Option<LinkTipSpot>,
    /// The block menu's state, while it is open: it speaks of what is picked now.
    pub menu: Option<BlockMenuState>,
    /// Cards and headers are set in the grid's own family, so a command reads as it was typed.
    pub card_font: String,
    /// The pane as a list of blocks, but for a full-screen program's screen.
    pub items: Option<Vec<Item>>,
    pub picked: Option<BlockId>,
    pub found: Option<u32>,
    pub pictures: Vec<LensPicture>,
    pub composer_text: String,
    pub selection: bool,
    pub choices: Option<ChoicesView>,
    /// Whether the composer takes the keyboard: the pane has it, and nothing over it does.
    pub take: bool,
    /// The find bar's tips.
    pub tips: (String, String),
}

/// What the pane draws, from its session and the hub; `menu_open` while the block menu is up.
pub fn pane_shown(
    session: Option<&Session>,
    hub: &HubRef,
    pane: PaneId,
    menu_open: bool,
) -> PaneShown {
    let find = session.and_then(|s| s.finder().map(|f| (s.find_text().to_owned(), f.position())));
    let picked = session.and_then(Session::selected_block);
    let menu_block = picked.filter(|_| menu_open);
    let menu = session.filter(|_| menu_open).map(|s| {
        let (lenses, picture) = match menu_block {
            Some(block) => (hub.block_lenses(pane, block), hub.lens_shown(pane, block)),
            None => (Vec::new(), false),
        };
        BlockMenuState {
            facts: BlockFacts {
                picked: s.picked_blocks().len(),
                running: menu_block.is_some_and(|block| !s.block_finished(block)),
                has_output: menu_block.is_some_and(|block| s.block_foldable(block)),
                bookmarked: s.picks_bookmarked(),
                folded: s.picks_folded(),
                on_host: hub.is_host_pane(pane),
            },
            lenses,
            picture,
        }
    });
    PaneShown {
        take: hub.keyboard_pane() == Some(pane) && !hub.bar_is_open() && find.is_none(),
        find,
        pending: session.and_then(|s| s.pending_paste().map(|p| p.reason)),
        ended: session.is_some_and(Session::ended),
        run_name: hub.0.borrow().run_name(pane).map(str::to_owned),
        chrome: session.map_or_else(Chrome::default, chrome_of),
        clip_ask: session.and_then(Session::clipboard_ask),
        console_row: hub.console_row(pane),
        link_tip: session.and_then(Session::link_tip),
        menu,
        card_font: session
            .map(|s| s.grid_font().family_css())
            .unwrap_or_default(),
        items: session.and_then(Session::block_list),
        picked,
        found: session.and_then(Session::found_row),
        pictures: hub.lens_pictures(pane),
        composer_text: session
            .map(|s| s.composer_text().to_owned())
            .unwrap_or_default(),
        selection: session.is_some_and(Session::has_selection),
        choices: session
            .and_then(Session::composer_choices)
            .map(|c| ChoicesView {
                words: c.words().into_iter().map(str::to_owned).collect(),
                details: c
                    .details()
                    .into_iter()
                    .map(|d| d.map(str::to_owned))
                    .collect(),
                picked: c.picked(),
            }),
        tips: (
            hub.tooltip("Previous match", Action::FindPrevious),
            hub.tooltip("Next match", Action::FindNext),
        ),
    }
}

/// [`pane_shown`] as a memo: read again when the window hears a change (the hub's, or the
/// pane's own through its link) or the block menu opens or closes.
pub fn use_pane_shown(
    slot: &Rc<RefCell<Slot>>,
    desk: &Desk,
    pane: PaneId,
    menu_open: impl Fn() -> bool + 'static,
) -> Memo<PaneShown> {
    let (slot, desk) = (Rc::clone(slot), desk.clone());
    use_memo(move || {
        let _ = (desk.ui)();
        let open = menu_open();
        pane_shown(slot.borrow().session.as_ref(), &desk.hub, pane, open)
    })
}
