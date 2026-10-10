//! The sticky header (PLAN §9 blocks): while a block's prompt is scrolled out of the top of the
//! pane and its output still runs under it, the block's command stays on the top row, on an
//! opaque quire surface with a hairline under it. Pressing it scrolls the prompt back to the top.

use dioxus::prelude::*;
use ds::components::controls::button_model::Bezel;
use ds::components::controls::press::Propagation;
use ds::prelude::*;
use temor_core::vocab::ids::BlockId;

/// What the header shows, placed by the window from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderView {
    pub id: BlockId,
    /// The command's first line, already cut to fit.
    pub command: String,
    /// The pane's padding in logical pixels: the text starts where the grid's text does.
    pub inset: f32,
    /// The top padding and the first grid row, in logical pixels: what the header covers.
    pub height: f32,
}

/// The header over the pane's top row. It sits beside the grid, not in it, so a press on it never
/// reaches the terminal.
#[component]
#[allow(non_snake_case)]
pub fn BlockHeader(header: HeaderView, on_press: EventHandler<BlockId>) -> Element {
    let id = header.id;
    rsx! {
        div {
            class: "temor-block-header ds-mono",
            style: "height:{header.height}px; padding-top:{header.inset}px; padding-left:{header.inset}px",
            Button {
                label: header.command.clone(),
                bezel: Bezel::Inline,
                title: Some("Scroll back to this command".to_owned()),
                propagation: Propagation::Stop,
                onclick: move |_| on_press.call(id),
            }
        }
    }
}
