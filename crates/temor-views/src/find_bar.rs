//! The find bar (PLAN §6, ⌘F): a search field that takes the keyboard when it opens, the position
//! of the current match ("3 of 12"), and chevron buttons for the previous and next match. It sits
//! on the pane's top-right corner. Enter is next, Shift+Enter is previous, Escape closes it
//! and gives the keyboard back to the pane.

use dioxus::prelude::*;
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
    /// In a console's bar, not over the pane's corner.
    #[props(default)]
    inline: bool,
) -> Element {
    let status = match position {
        Some((index, total)) => format!("{index} of {total}"),
        None if query.is_empty() => String::new(),
        None => "No matches".to_owned(),
    };
    rsx! {
        div { class: if inline { "temor-find temor-find-inline" } else { "temor-find temor-sheet" },
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
