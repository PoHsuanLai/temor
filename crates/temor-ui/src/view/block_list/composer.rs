//! The composer card at the list's end (PLAN: block list): the folder, and a field where the
//! next command is typed while the shell waits at its prompt. Enter sends it, the arrows step
//! through the pane's commands, Ctrl+C empties it, and a ⌘ chord the field does not take (a
//! block's, the window's) goes to the terminal's keys as if the field were not there.
//!
//! The field lies inside the terminal's key surface: its keys and presses stop at the card, so
//! the surface neither hears them a second time nor starts a selection under the card.

use dioxus::prelude::*;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::focus::request::FocusRequest;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;

use crate::session::HistoryStep;

/// What the composer card does with what is typed in it.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerActions {
    /// The text changed.
    pub input: EventHandler<String>,
    /// Enter: send the text to the shell.
    pub submit: EventHandler<()>,
    /// ↑ or ↓: the command one older or newer.
    pub history: EventHandler<HistoryStep>,
    /// A chord for the terminal's keys.
    pub chord: EventHandler<RawKey>,
    /// The card went away with the caret in its field: the keyboard goes back to the terminal.
    pub gone: EventHandler<()>,
}

/// Whether a ⌘ chord is the field's own: select all, copy, cut, paste, undo and redo, and the
/// caret's moves (to the line's ends, by word).
fn field_takes(event: &KeyboardEvent, selection: bool) -> bool {
    match event.key() {
        Key::Character(c) => match c.to_lowercase().as_str() {
            // With text selected in the list, ⌘C copies that.
            "c" => !selection,
            "a" | "x" | "v" | "z" => true,
            _ => false,
        },
        Key::ArrowLeft | Key::ArrowRight | Key::Backspace | Key::Delete => true,
        _ => false,
    }
}

/// The composer card. `text` is the session's; `selection` whether the list has text selected;
/// `focus` the request that puts the caret in the field, taken as it mounts when `take` says so.
#[component]
#[allow(non_snake_case)]
pub fn ComposerCard(
    style: String,
    /// The grid's family as CSS: a command reads as it will be run.
    font: String,
    text: String,
    folder: Option<String>,
    selection: bool,
    focus: FocusRequest,
    take: bool,
    actions: ComposerActions,
) -> Element {
    let field_focus = match take {
        true => FieldFocus::Controlled(focus),
        false => FieldFocus::Manual,
    };
    let ComposerActions {
        input,
        submit,
        history,
        chord,
        gone,
    } = actions;
    // Whether the caret is in the field, for the keyboard to go back when the card goes.
    let mut holds = use_hook(|| CopyValue::new(false));
    use_drop(move || {
        if *holds.peek() {
            gone.call(());
        }
    });
    let on_key = move |event: KeyboardEvent| {
        let mods = event.modifiers();
        let command = mods.contains(Modifiers::META);
        let control = mods.contains(Modifiers::CONTROL);
        match event.key() {
            Key::Enter if !command && !control => {
                event.prevent_default();
                submit.call(());
            }
            Key::ArrowUp | Key::ArrowDown if mods.is_empty() => {
                event.prevent_default();
                history.call(match event.key() {
                    Key::ArrowUp => HistoryStep::Older,
                    _ => HistoryStep::Newer,
                });
            }
            Key::Character(c) if control && !command && c.eq_ignore_ascii_case("c") => {
                event.prevent_default();
                input.call(String::new());
            }
            _ if command && !field_takes(&event, selection) => {
                event.prevent_default();
                let phase = match event.is_auto_repeating() {
                    true => KeyPhase::Repeat,
                    false => KeyPhase::Press,
                };
                chord.call(RawKey::new(
                    phase,
                    (event.key(), event.code()),
                    (mods, event.location()),
                ));
            }
            _ => {}
        }
    };
    rsx! {
        div {
            class: "temor-bseg temor-blk-composer",
            style: "{style}",
            onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
            onkeyup: move |event: KeyboardEvent| event.stop_propagation(),
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            onpointerup: move |event: PointerEvent| event.stop_propagation(),
            if let Some(folder) = folder {
                span { class: "temor-blk-where", "{folder}" }
            }
            div { class: "temor-blk-field", style: "{font}",
                TextField {
                    label: "Command".to_owned(),
                    value: text,
                    placeholder: "Type a command".to_owned(),
                    bezel: FieldBezel::Plain,
                    size: ControlSize::Regular,
                    focus: field_focus,
                    oninput: move |text: String| input.call(text),
                    onfocus: move |()| holds.set(true),
                    onblur: move |()| holds.set(false),
                    onkey: on_key,
                }
            }
        }
    }
}
