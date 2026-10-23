//! The sticky header (PLAN §9 blocks): while a block's prompt is scrolled out of the top of the
//! pane and its output still runs under it, the block's command stays on the top row, on an
//! opaque surface with a hairline under it, set like a card's title. Pressing the command scrolls
//! the prompt back to the top. While the command runs (a full-screen program, an agent) the
//! header is where it is moved to a pane of its own or interrupted: its card is long gone.

use dioxus::prelude::*;
use ds::components::controls::press::Propagation;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::vocab::ids::BlockId;

use super::tool_button::ToolButton;

/// What the header shows, placed by the window from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderView {
    pub id: BlockId,
    /// The command's first line, already cut to fit.
    pub command: String,
    /// The pane's padding in logical pixels: the text starts where the grid's text does.
    pub inset: f32,
    /// The top padding and the grid rows it covers, in logical pixels.
    pub height: f32,
    /// The command is still running.
    pub running: bool,
    /// It can be moved to a pane of its own (a session in the local daemon).
    pub detach: bool,
    /// The CSS `font-family` of the grid beside it.
    pub font: String,
}

/// What a press on the header asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderPress {
    /// The command: scroll its prompt back to the top.
    Command(BlockId),
    Detach,
    Stop,
}

/// The header over the pane's top rows. It sits beside the grid, not in it, so a press on it
/// never reaches the terminal.
#[component]
#[allow(non_snake_case)]
pub fn BlockHeader(header: HeaderView, on_press: EventHandler<HeaderPress>) -> Element {
    let id = header.id;
    let state = match header.running {
        true => "running",
        false => "done",
    };
    rsx! {
        div {
            class: "temor-block-header",
            "data-state": state,
            style: "height:{header.height}px; padding-left:{header.inset}px; padding-right:{header.inset}px; {header.font}",
            div {
                class: "temor-block-header-title temor-card-title",
                title: "Scroll back to this command",
                onclick: move |_| on_press.call(HeaderPress::Command(id)),
                "{header.command}"
            }
            if header.running {
                IconButtonGroup { label: "Program tools", size: ControlSize::Mini,
                    if header.detach {
                        ToolButton {
                            icon: Icon::Columns,
                            size: ControlSize::Mini,
                            propagation: Propagation::Stop,
                            label: "Detach",
                            tip: "Move this program to a pane of its own beside this one",
                            onclick: move |()| on_press.call(HeaderPress::Detach),
                        }
                    }
                    ToolButton {
                        icon: Icon::Square,
                        size: ControlSize::Mini,
                        propagation: Propagation::Stop,
                        label: "Stop",
                        tip: "Interrupt the program (Control-C)",
                        onclick: move |()| on_press.call(HeaderPress::Stop),
                    }
                }
            }
        }
    }
}
