//! The head of a live card: a running program moved to a terminal of its own, shown in the pane
//! under this strip. It says the command and, as the block cards do, only the exception in words
//! (stopping, how it ended); its icons size the card, show it over the whole tab, stop it, and
//! put an ended card away.

use dioxus::prelude::*;
use ds::base::text::clip::clip_chars;
use ds::components::controls::press::Propagation;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::session::Life;

use super::tool_button::ToolButton;
use crate::hub::{LiveHead, LiveIcon, live_cards_grace_secs};

/// Most characters of the command the head shows.
const TITLE_CHARS: usize = 120;

/// What the head says beside the command: nothing while the program runs.
fn status_words(life: Life) -> Option<String> {
    match life {
        Life::Running => None,
        Life::Stopping => Some("stopping".to_owned()),
        Life::Ended(Some(code)) => Some(format!("exit {code}")),
        Life::Ended(None) => Some("ended".to_owned()),
    }
}

fn state_name(life: Life) -> &'static str {
    match life {
        Life::Running => "running",
        Life::Stopping => "stopping",
        Life::Ended(_) => "ended",
    }
}

#[component]
#[allow(non_snake_case)]
pub fn LiveCardHead(head: LiveHead, font: String, on_icon: EventHandler<LiveIcon>) -> Element {
    let title = clip_chars(head.command.trim(), TITLE_CHARS);
    let status = status_words(head.life);
    let ended = matches!(head.life, Life::Ended(_));
    let stop = match head.life {
        Life::Running => Availability::Enabled,
        _ => Availability::Disabled,
    };
    let stop_tip = format!(
        "Interrupt the program. If it is still there after {} s, it is hung up",
        live_cards_grace_secs()
    );
    rsx! {
        div { class: "temor-live-head", "data-state": state_name(head.life), style: font,
            span { class: "temor-live-title", "{title}" }
            if let Some(words) = status {
                span { class: "temor-live-status", "{words}" }
            }
            IconButtonGroup { label: "Card tools", size: ControlSize::Mini,
                if ended {
                    ToolButton {
                        icon: Icon::Archive,
                        size: ControlSize::Mini,
                        propagation: Propagation::Stop,
                        label: "Put away",
                        tip: "Close this card",
                        onclick: move |()| on_icon.call(LiveIcon::Dismiss),
                    }
                } else {
                    ToolButton {
                        icon: Icon::ChevronsUpDown,
                        size: ControlSize::Mini,
                        propagation: Propagation::Stop,
                        label: "Size",
                        tip: "Make the card smaller or taller",
                        onclick: move |()| on_icon.call(LiveIcon::Size),
                    }
                    ToolButton {
                        icon: Icon::Maximize,
                        size: ControlSize::Mini,
                        propagation: Propagation::Stop,
                        label: "Full",
                        tip: "Show the card over the whole tab, or back",
                        onclick: move |()| on_icon.call(LiveIcon::Full),
                    }
                    ToolButton {
                        icon: Icon::Square,
                        size: ControlSize::Mini,
                        propagation: Propagation::Stop,
                        label: "Stop",
                        tip: stop_tip,
                        availability: stop,
                        onclick: move |()| on_icon.call(LiveIcon::Stop),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exception_is_said_in_words() {
        assert_eq!(status_words(Life::Running), None);
        assert_eq!(status_words(Life::Stopping).as_deref(), Some("stopping"));
        assert_eq!(
            status_words(Life::Ended(Some(130))).as_deref(),
            Some("exit 130")
        );
        assert_eq!(status_words(Life::Ended(None)).as_deref(), Some("ended"));
    }
}
