//! The find bar (PLAN §6, ⌘F): a search field, the position of the current match, and the
//! previous, next and done buttons. Enter is next, Shift+Enter is previous, Escape closes it.

use dioxus::prelude::*;
use ds::prelude::*;

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
    on_close: EventHandler<()>,
) -> Element {
    let status = match position {
        Some((index, total)) => format!("{index} of {total}"),
        None if query.is_empty() => String::new(),
        None => "No matches".to_owned(),
    };
    rsx! {
        div { style: "display:flex; align-items:center; gap:8px; padding:8px",
            TextField {
                label: "Find in terminal".to_owned(),
                value: query,
                placeholder: "Find in terminal".to_owned(),
                oninput: move |text: String| on_query.call(text),
                onsubmit: move |_: String| on_step.call(StepDirection::Forward),
                onkey: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        on_close.call(());
                    }
                },
            }
            span { "{status}" }
            Button {
                label: "Previous".to_owned(),
                title: Some("Previous match".to_owned()),
                onclick: move |_| on_step.call(StepDirection::Backward),
            }
            Button {
                label: "Next".to_owned(),
                title: Some("Next match".to_owned()),
                onclick: move |_| on_step.call(StepDirection::Forward),
            }
            Button {
                label: "Done".to_owned(),
                title: Some("Close find".to_owned()),
                onclick: move |_| on_close.call(()),
            }
        }
    }
}
