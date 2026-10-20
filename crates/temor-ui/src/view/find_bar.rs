//! The find bar (PLAN §6, ⌘F): a search field that takes the keyboard when it opens, the position
//! of the current match ("3 of 12"), and chevron buttons for the previous and next match. It sits
//! on the pane's top-right corner. Enter is next, Shift+Enter is previous, Escape closes it
//! and gives the keyboard back to the pane.

use dioxus::prelude::*;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;

use super::tool_button::ToolButton;

/// Which way the find bar steps through matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepDirection {
    Forward,
    Backward,
}

/// The find bar. `query` and `position` come from the session; the callbacks change it.
#[component]
#[allow(non_snake_case)]
pub fn FindBar(
    query: String,
    position: Option<(usize, usize)>,
    on_query: EventHandler<String>,
    on_step: EventHandler<StepDirection>,
    /// The hover hints of the two steps, with the chords that do the same.
    previous_tip: String,
    next_tip: String,
    on_close: EventHandler<()>,
    /// The bar's height once laid out: what lies under it on the pane's top rows is hidden.
    on_height: EventHandler<f32>,
) -> Element {
    let probe = use_rect();
    use_effect(move || {
        if let Some(rect) = probe.rect() {
            on_height.call(rect.size.height.0);
        }
    });
    let status = match position {
        Some((index, total)) => format!("{index} of {total}"),
        None if query.is_empty() => String::new(),
        None => "No matches".to_owned(),
    };
    rsx! {
        div { class: "temor-find",
            onmounted: move |event: MountedEvent| probe.on_mounted(event),
            div { class: "temor-find-field",
                TextField {
                    label: "Find in terminal".to_owned(),
                    value: query,
                    placeholder: "Find in terminal".to_owned(),
                    size: ControlSize::Regular,
                    // The caret goes in the field as the bar opens: typing is the field's.
                    focus: FieldFocus::OnMount,
                    oninput: move |text: String| on_query.call(text),
                    onsubmit: move |_: String| on_step.call(StepDirection::Forward),
                    onkey: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            on_close.call(());
                        }
                    },
                }
            }
            Label { text: status }
            ToolButton {
                icon: Icon::ChevronUp,
                label: "Previous match",
                tip: previous_tip,
                size: ControlSize::Regular,
                onclick: move |()| on_step.call(StepDirection::Backward),
            }
            ToolButton {
                icon: Icon::ChevronDown,
                label: "Next match",
                tip: next_tip,
                size: ControlSize::Regular,
                onclick: move |()| on_step.call(StepDirection::Forward),
            }
        }
    }
}
