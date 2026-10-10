//! The block toolbar (PLAN §9 blocks): small icon buttons at the top right of the block under the
//! pointer, or of the selected one. Copy the command, copy the output, save the output to
//! Downloads, type the command again (it is never run), and show the files it listed. A caption
//! says how the block ended.
//! It sits beside the grid, not in it, so a press on it never reaches the terminal.

use dioxus::prelude::*;
use ds::components::content::icon_source::IconSource;
use ds::components::content::label::LabelStyle;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::components::controls::press::Propagation;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::vocab::ids::BlockId;

use crate::session::ToolsHeight;

/// What the toolbar shows and does for one block.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolsView {
    pub id: BlockId,
    /// Where the toolbar's measured height goes, for the session to count the rows it covers.
    pub height: ToolsHeight,
    /// Distance from the top of the pane, in logical pixels.
    pub top: f32,
    /// Distance from the right edge of the pane, in logical pixels.
    pub inset: f32,
    /// How it ended, in words.
    pub summary: String,
    /// Whether the output can be copied or saved (the command has finished).
    pub output: Availability,
    /// Whether the command can be typed again (it has a command).
    pub rerun: Availability,
    /// Whether the output can be searched for files to show (the command has finished).
    pub view: Availability,
}

/// One icon button of the toolbar. `label` names it; `tip` is the hover hint.
#[component]
#[allow(non_snake_case)]
fn ToolButton(
    icon: Icon,
    label: &'static str,
    tip: &'static str,
    availability: Availability,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        Button {
            bezel: Bezel::Toolbar,
            image: ImagePosition::Only,
            size: ControlSize::Small,
            icon: Some(IconSource::Glyph(icon)),
            label: label.to_owned(),
            title: Some(tip.to_owned()),
            availability,
            propagation: Propagation::Stop,
            onclick: move |_| onclick.call(()),
        }
    }
}

/// The toolbar for one block: each action reports the block it acts on.
#[component]
#[allow(non_snake_case)]
pub fn BlockTools(
    tools: ToolsView,
    on_copy_command: EventHandler<BlockId>,
    on_copy_output: EventHandler<BlockId>,
    on_save: EventHandler<BlockId>,
    on_rerun: EventHandler<BlockId>,
    on_view: EventHandler<BlockId>,
) -> Element {
    let id = tools.id;
    // The toolbar's real height, once laid out, replaces the estimate the session starts with.
    let probe = use_rect();
    let height = tools.height.clone();
    use_effect(move || {
        if let Some(rect) = probe.rect() {
            height.measured(rect.size.height.0);
        }
    });
    rsx! {
        div {
            class: "temor-block-tools",
            onmounted: move |event: MountedEvent| probe.on_mounted(event),
            style: "top:{tools.top}px; right:{tools.inset}px",
            Label { text: tools.summary.clone(), style: LabelStyle::Caption }
            ToolButton {
                icon: Icon::Copy,
                label: "Copy command",
                tip: "Copy this command",
                availability: Availability::Enabled,
                onclick: move |()| on_copy_command.call(id),
            }
            ToolButton {
                icon: Icon::Clipboard,
                label: "Copy output",
                tip: "Copy this command's output",
                availability: tools.output,
                onclick: move |()| on_copy_output.call(id),
            }
            ToolButton {
                icon: Icon::Download,
                label: "Save output",
                tip: "Save this command's output to Downloads",
                availability: tools.output,
                onclick: move |()| on_save.call(id),
            }
            ToolButton {
                icon: Icon::Restart,
                label: "Re-run",
                tip: "Put this command in the prompt. Press Return to run it",
                availability: tools.rerun,
                onclick: move |()| on_rerun.call(id),
            }
            ToolButton {
                icon: Icon::File,
                label: "View",
                tip: "Show the files this command listed",
                availability: tools.view,
                onclick: move |()| on_view.call(id),
            }
        }
    }
}
