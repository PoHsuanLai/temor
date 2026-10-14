//! The block tools (PLAN §9 blocks): in the header row of the block under the pointer, or of the
//! selected one, right of how it ended (`block_meta`), a dense group of three icon buttons (copy
//! the output, type the command again, more) and a menu behind "more" with the rest: copy the
//! command, save the output to Downloads, show the files it listed, and on a pane on another
//! computer, edit here. Typing the command again never runs it.
//! It sits beside the grid, not in it, so a press on it never reaches the terminal.

use dioxus::prelude::*;
use ds::components::content::icon_source::IconSource;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::components::controls::press::Propagation;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::vocab::ids::BlockId;

use super::block_meta::MetaText;
use crate::session::ToolsHeight;

/// What the tools show and do for one block.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolsView {
    pub id: BlockId,
    /// Where the tools' measured height goes, for the session to count the rows they cover.
    pub height: ToolsHeight,
    /// Distance from the top of the pane to the header row, in logical pixels.
    pub top: f32,
    /// The header row's height, in logical pixels.
    pub row: f32,
    /// Distance from the right edge of the pane, in logical pixels.
    pub inset: f32,
    /// How the block ended, shown left of the buttons.
    pub meta: Option<String>,
    /// Whether the output can be copied or saved (the command has finished).
    pub output: Availability,
    /// Whether the command can be typed again (it has a command).
    pub rerun: Availability,
    /// Whether the output can be searched for files to show (the command has finished).
    pub view: Availability,
    /// "Edit here" on a pane on another computer: `None` on any other pane, where there is
    /// nothing to fetch.
    pub edit: Option<Availability>,
}

/// Whether the "more" menu is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoreMenu {
    Closed,
    Open,
}

/// What the "more" menu can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum More {
    CopyCommand,
    Save,
    View,
    Edit,
}

/// The menu's items: each with a tip saying what it does.
fn more_items(tools: &ToolsView) -> Vec<MenuItem<More>> {
    let mut items = vec![
        MenuItem::new(More::CopyCommand, "Copy Command")
            .with_tip("Copy this command to the clipboard"),
        MenuItem::new(More::Save, "Save Output")
            .with_tip("Save this command's output to Downloads")
            .with_availability(tools.output),
        MenuItem::new(More::View, "View Files")
            .with_tip("Show the files this command listed")
            .with_availability(tools.view),
    ];
    if let Some(availability) = tools.edit {
        items.push(
            MenuItem::new(More::Edit, "Edit Here")
                .with_tip("Open the selected file from this computer in an editor here")
                .with_availability(availability),
        );
    }
    items
}

/// One icon button of the group. `label` names it; `tip` is the hover hint.
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
            size: ControlSize::Mini,
            icon: Some(IconSource::Glyph(icon)),
            label: label.to_owned(),
            title: Some(tip.to_owned()),
            availability,
            propagation: Propagation::Stop,
            onclick: move |_| onclick.call(()),
        }
    }
}

/// The tools for one block: each action reports the block it acts on. While the "more" menu is
/// up, `held` names the block, so the tools stay when the pointer leaves the pane or crosses
/// another block on its way to the menu.
#[component]
#[allow(non_snake_case)]
pub fn BlockTools(
    tools: ToolsView,
    held: Signal<Option<BlockId>>,
    on_copy_command: EventHandler<BlockId>,
    on_copy_output: EventHandler<BlockId>,
    on_save: EventHandler<BlockId>,
    on_rerun: EventHandler<BlockId>,
    on_view: EventHandler<BlockId>,
    on_edit: EventHandler<BlockId>,
) -> Element {
    let id = tools.id;
    // The tools' real height, once laid out, replaces the estimate the session starts with.
    let probe = use_rect();
    let more_probe = use_rect();
    let mut menu = use_signal(|| MoreMenu::Closed);
    let height = tools.height.clone();
    use_effect(move || {
        if let Some(rect) = probe.rect() {
            height.measured(rect.size.height.0);
        }
    });
    let items = more_items(&tools);
    rsx! {
        div {
            class: "temor-block-tools",
            onmounted: move |event: MountedEvent| probe.on_mounted(event),
            style: "top:{tools.top}px; height:{tools.row}px; right:{tools.inset}px",
            if let Some(meta) = tools.meta.clone() {
                MetaText { text: meta }
            }
            IconButtonGroup { label: "Command tools", size: ControlSize::Mini,
                ToolButton {
                    icon: Icon::Clipboard,
                    label: "Copy output",
                    tip: "Copy this command's output",
                    availability: tools.output,
                    onclick: move |()| on_copy_output.call(id),
                }
                ToolButton {
                    icon: Icon::Restart,
                    label: "Re-run",
                    tip: "Put this command in the prompt. Press Return to run it",
                    availability: tools.rerun,
                    onclick: move |()| on_rerun.call(id),
                }
                div {
                    class: "temor-block-more",
                    onmounted: move |event: MountedEvent| more_probe.on_mounted(event),
                    ToolButton {
                        icon: Icon::Ellipsis,
                        label: "More",
                        tip: "More things to do with this command",
                        availability: Availability::Enabled,
                        onclick: move |()| {
                            held.set(Some(id));
                            menu.set(MoreMenu::Open);
                        },
                    }
                }
            }
            if let (MoreMenu::Open, Some(anchor)) = (menu(), more_probe.anchor()) {
                Menu {
                    placement: MenuPlacement::Popup,
                    anchor,
                    items,
                    onpick: move |pick: More| match pick {
                        More::CopyCommand => on_copy_command.call(id),
                        More::Save => on_save.call(id),
                        More::View => on_view.call(id),
                        More::Edit => on_edit.call(id),
                    },
                    onclose: move |()| {
                        held.set(None);
                        menu.set(MoreMenu::Closed);
                    },
                }
            }
        }
    }
}
