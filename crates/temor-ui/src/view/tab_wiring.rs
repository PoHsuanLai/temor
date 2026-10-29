//! What a sidebar tab's row, a pinned tab's tile and a split's segment all do the same way: report
//! where they stand to the drag, start a drag on a primary press, count a click only when no drag
//! just ended, and open the tab's menu on the context-menu event (a right click, or the key that
//! stands for one), never on a middle press or on the press itself.

use dioxus::prelude::*;
use ds::host::measure::{RectProbe, use_rect};
use ds::prelude::Selection;
use temor_client::model::vocab::ids::TabId;

use super::dnd::{Dnd, RowRects, is_primary, point_of};
use crate::hub::DragKey;

/// Measures the element this hook's component draws and tells the drag where the tab `id` stands,
/// for as long as it is on the page. Give the probe the element's `onmounted`.
pub fn use_drag_target(id: TabId, rects: &RowRects) -> RectProbe {
    let rect = use_rect();
    if let Some(at) = rect.rect() {
        rects.set(id, at);
    }
    let rects = rects.clone();
    use_drop(move || rects.forget(id));
    rect
}

/// A pointer press that starts a drag of `key` when it is the primary button's. Other buttons are
/// not the drag's: they do nothing here.
pub fn drag_press(dnd: &Dnd, key: DragKey) -> impl FnMut(PointerEvent) + 'static {
    let dnd = dnd.clone();
    move |event: PointerEvent| {
        if is_primary(&event) {
            dnd.click.reset();
            dnd.drag.down(key, point_of(&event));
        }
    }
}

/// The handler of a tab's context-menu event: `open` shows the menu, and the event is taken so
/// the sidebar's own menu (the Space's) stays shut.
pub fn tab_menu_trigger(mut open: impl FnMut() + 'static) -> impl FnMut(MouseEvent) + 'static {
    move |event: MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
        open();
    }
}

/// The `data-selected` attribute for a selection.
pub fn selected_attr(selection: Selection) -> Option<&'static str> {
    match selection {
        Selection::Selected => Some("true"),
        Selection::Unselected => None,
    }
}
