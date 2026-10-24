//! The composer card at the list's end (PLAN: block list): the folder, and a field where the
//! next command is typed while the shell waits at its prompt. Enter sends it, ⇧Enter starts a new
//! line (the field grows to hold its lines), the arrows step through the pane's commands while it
//! holds one line, Ctrl+C empties it, and a ⌘ chord the field does not take (a block's, the
//! window's) goes to the terminal's keys as if the field were not there.
//!
//! Tab asks the shell to complete the line. Words it offers to pick from are listed above the
//! field: the arrows and ⇧Tab move the pick, Tab or Enter puts it in, a press puts in the one
//! pressed, and Escape puts the list away.
//!
//! The field lies inside the terminal's key surface: its keys and presses stop at the card, so
//! the surface neither hears them a second time nor starts a selection under the card.

use dioxus::prelude::*;
use ds::components::fields::text_field_model::FieldRows;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::focus::request::FocusRequest;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;

use crate::session::HistoryStep;

/// The card's height with one line, in logical pixels.
const ONE_LINE: f32 = 44.0;
/// One more line of the field: the regular control's 13 px at the field's 1.55 leading.
const LINE: f32 = 13.0 * 1.55;
/// One word offered, in logical pixels.
const CHOICE: f32 = 22.0;
/// The most words listed at once; past that the list follows the pick and says how many more.
const MOST_CHOICES: usize = 8;
/// The list's space under its words: a gap, the rule, a gap.
const CHOICES_END: f32 = 13.0;

/// The rows the offered words take: each word, at most [`MOST_CHOICES`], and one saying how many
/// more there are.
fn choice_rows(count: usize) -> usize {
    match count {
        0 => 0,
        n if n <= MOST_CHOICES => n,
        _ => MOST_CHOICES + 1,
    }
}

/// The words in view: [`MOST_CHOICES`] of them, from the first unless the pick is further down.
fn in_view(count: usize, picked: usize) -> std::ops::Range<usize> {
    if count == 0 {
        return 0..0;
    }
    let shown = count.min(MOST_CHOICES);
    let start = picked.saturating_sub(shown - 1).min(count - shown);
    start..start + shown
}

/// How many lines the field shows for `text`: none past one line (a single-line field), else the
/// least of quire's heights that holds them, at most ten (the text scrolls past that).
fn shown_rows(text: &str) -> Option<(FieldRows, u8)> {
    match text.matches('\n').count() + 1 {
        0 | 1 => None,
        2 => Some((FieldRows::Two, 2)),
        3 => Some((FieldRows::Three, 3)),
        4 => Some((FieldRows::Four, 4)),
        5 | 6 => Some((FieldRows::Six, 6)),
        _ => Some((FieldRows::Ten, 10)),
    }
}

/// The card's height for `text` with `choices` words offered, in logical pixels: one line's, a
/// line more for each line the field shows past the first, and the list of words.
pub fn composer_height(text: &str, choices: usize) -> f32 {
    let more = shown_rows(text).map_or(0, |(_, n)| n - 1);
    let listed = match choice_rows(choices) {
        0 => 0.0,
        rows => rows as f32 * CHOICE + CHOICES_END,
    };
    ONE_LINE + f32::from(more) * LINE + listed
}

/// What the composer card does with what is typed in it.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerActions {
    /// The text changed.
    pub input: EventHandler<String>,
    /// Enter: send the text to the shell.
    pub submit: EventHandler<()>,
    /// ↑ or ↓: the command one older or newer.
    pub history: EventHandler<HistoryStep>,
    /// A chord the field does not take: the keymap's (the terminal's keys hear it).
    pub chord: EventHandler<RawKey>,
    /// The card went away with the caret in its field: the keyboard goes back to the terminal.
    pub gone: EventHandler<()>,
    /// Every key pressed in the field, before the field acts on it: the ⌘-hover card hears the
    /// keys typed here as it hears the terminal's.
    pub key: EventHandler<RawKey>,
    /// Tab: ask the shell to complete the line.
    pub complete: EventHandler<()>,
    /// The pick among the offered words moves up (`Older`) or down (`Newer`).
    pub pick: EventHandler<HistoryStep>,
    /// The picked word goes in, or the one at this index.
    pub take: EventHandler<Option<usize>>,
    /// The offered words are put away.
    pub close: EventHandler<()>,
}

/// The words the shell offered for the line's end, and the one picked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChoicesView {
    pub words: Vec<String>,
    pub picked: usize,
}

/// Whether a chord is the field's own. With ⌘: select all, copy, cut, paste, undo and redo, and
/// the caret's moves (to the line's ends, by word). With Ctrl alone, the line editing a Mac
/// field does (to the line's start and end, a character back and on, cutting to the end, and
/// deleting); Ctrl+C is the composer's, which empties it.
fn field_takes(event: &KeyboardEvent, selection: bool) -> bool {
    let mods = event.modifiers();
    let command = mods.intersects(Modifiers::SUPER | Modifiers::META);
    match event.key() {
        Key::ArrowLeft | Key::ArrowRight | Key::Backspace | Key::Delete => true,
        Key::Character(c) if command => match c.to_lowercase().as_str() {
            // With text selected in the list, ⌘C copies that.
            "c" => !selection,
            "a" | "x" | "v" | "z" => true,
            _ => false,
        },
        Key::Character(c) if mods == Modifiers::CONTROL => {
            matches!(
                c.to_lowercase().as_str(),
                "a" | "e" | "b" | "f" | "k" | "d" | "h"
            )
        }
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
    /// The words offered to pick from, if any.
    choices: Option<ChoicesView>,
) -> Element {
    let rows = shown_rows(&text);
    let choosing = choices.is_some();
    let one_line = rows.is_none();
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
        key,
        complete,
        pick,
        take: take_word,
        close,
    } = actions;
    // Whether the caret is in the field, for the keyboard to go back when the card goes.
    let mut holds = use_hook(|| CopyValue::new(false));
    use_drop(move || {
        if *holds.peek() {
            gone.call(());
        }
    });
    let typed = text.clone();
    let on_key = move |event: KeyboardEvent| {
        let mods = event.modifiers();
        // ⌘ is Super where the platform says so, Meta where it says that.
        let command = mods.intersects(Modifiers::SUPER | Modifiers::META);
        let control = mods.contains(Modifiers::CONTROL);
        let shift = mods.contains(Modifiers::SHIFT);
        let phase = match event.is_auto_repeating() {
            true => KeyPhase::Repeat,
            false => KeyPhase::Press,
        };
        let raw = RawKey::new(phase, (event.key(), event.code()), (mods, event.location()));
        key.call(raw.clone());
        let plain = !command && !control && !mods.contains(Modifiers::ALT);
        match event.key() {
            Key::Tab if plain && shift && choosing => {
                event.prevent_default();
                pick.call(HistoryStep::Older);
            }
            Key::Tab if plain && !shift => {
                event.prevent_default();
                match choosing {
                    true => take_word.call(None),
                    false => complete.call(()),
                }
            }
            Key::ArrowUp | Key::ArrowDown if mods.is_empty() && choosing => {
                event.prevent_default();
                pick.call(match event.key() {
                    Key::ArrowUp => HistoryStep::Older,
                    _ => HistoryStep::Newer,
                });
            }
            Key::Enter if plain && !shift && choosing => {
                event.prevent_default();
                take_word.call(None);
            }
            Key::Escape if choosing => {
                event.prevent_default();
                close.call(());
            }
            // A new line: a multi-line field adds it itself; a one-line field becomes one that
            // holds it, and the caret goes on at its end.
            Key::Enter if shift && !command && !control => {
                if one_line {
                    event.prevent_default();
                    input.call(format!("{typed}\n"));
                }
            }
            Key::Enter if !command && !control => {
                event.prevent_default();
                submit.call(());
            }
            Key::ArrowUp | Key::ArrowDown if mods.is_empty() && one_line => {
                event.prevent_default();
                history.call(match event.key() {
                    Key::ArrowUp => HistoryStep::Older,
                    _ => HistoryStep::Newer,
                });
            }
            _ if (command || control) && !field_takes(&event, selection) => {
                event.prevent_default();
                chord.call(raw);
            }
            _ => {}
        }
    };
    rsx! {
        div {
            class: "temor-bseg temor-blk-composer",
            style: "{style}",
            "data-lines": (!one_line).then_some("many"),
            "data-choices": choosing.then_some("true"),
            onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
            onkeyup: move |event: KeyboardEvent| event.stop_propagation(),
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            onpointerup: move |event: PointerEvent| event.stop_propagation(),
            if let Some(choices) = choices {
                ChoiceList { choices, font: font.clone(), take: take_word }
            }
            div { class: "temor-blk-line",
            if let Some(folder) = folder {
                span { class: "temor-blk-where", "{folder}" }
            }
            div { class: "temor-blk-field", style: "{font}",
                TextField {
                    label: "Command".to_owned(),
                    value: text,
                    placeholder: "Type a command".to_owned(),
                    kind: match one_line {
                        true => FieldKind::Plain,
                        false => FieldKind::Multiline,
                    },
                    rows: rows.map_or(FieldRows::Two, |(rows, _)| rows),
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
}

/// The words offered for the line's end, the picked one marked; a press puts one in.
#[component]
#[allow(non_snake_case)]
fn ChoiceList(choices: ChoicesView, font: String, take: EventHandler<Option<usize>>) -> Element {
    let count = choices.words.len();
    let view = in_view(count, choices.picked);
    let more = count - view.len();
    rsx! {
        div { class: "temor-blk-choices", style: "{font}",
            for index in view {
                div {
                    key: "{index}",
                    class: "temor-blk-choice",
                    style: "height:{CHOICE}px",
                    "data-picked": (index == choices.picked).then_some("true"),
                    onpointerdown: move |event: PointerEvent| {
                        event.prevent_default();
                        take.call(Some(index));
                    },
                    "{choices.words[index]}"
                }
            }
            if more > 0 {
                div { class: "temor-blk-choice", style: "height:{CHOICE}px", "data-more": "true",
                    "{more} more"
                }
            }
        }
    }
}
