//! A tooltip on a whole sidebar row, tile or segment: quire's `Tooltip` hooked to a block
//! `HoverTarget`, so the thing it names keeps its full width (the plain `Tooltip` wraps its
//! child in an inline box, and a row in one shrinks to its words).

use dioxus::prelude::*;
use ds::components::overlays::hover_card::target::{HoverTarget, TargetElement};
use ds::components::overlays::tooltip::Tooltip;
use ds::motion::hover_intent::HoverProfile;
use ds::stack::hover_hub::HoverKey;

/// `children`, as wide as their place, with `text` on hover. `name` tells this tip from every
/// other in the window (a tab's or a pane's id).
#[component]
#[allow(non_snake_case)]
pub fn RowTip(name: String, text: String, children: Element) -> Element {
    let key = HoverKey(format!("temor:{name}"));
    rsx! {
        Tooltip { text, hover_key: Some(key.clone()),
            HoverTarget { hover_key: key, profile: HoverProfile::Tip, as_: TargetElement::Div,
                {children}
            }
        }
    }
}
