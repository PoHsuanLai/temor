//! IME events to composition steps: a copy of quire's `ds::edit::composition::{on_ime, settle}`
//! (quire v0.2.30, MIT OR Apache-2.0), which is `pub(crate)` in quire.
// TODO(quire 0.2.31): use quire's public composition machine once it is exported.
//
// winit clears the preedit (an empty `Preedit`) right before every commit, and an IME that
// cancels a composition clears it and commits nothing.

use ds::edit::input::{Composition, EditInput, PreeditCursor};
use ds::host::ime::ImeEvent;

/// Where a composition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Composing {
    #[default]
    Idle,
    Active,
    Cleared,
}

/// One IME event, as the composition steps it.
pub fn on_ime(state: Composing, event: ImeEvent) -> (Composing, Vec<EditInput>) {
    match (state, event) {
        (Composing::Idle, ImeEvent::Preedit { text, cursor }) if !text.is_empty() => (
            Composing::Active,
            vec![
                EditInput::Composition(Composition::Start),
                update(text, cursor),
            ],
        ),
        (Composing::Active | Composing::Cleared, ImeEvent::Preedit { text, cursor })
            if !text.is_empty() =>
        {
            (Composing::Active, vec![update(text, cursor)])
        }
        (Composing::Active, ImeEvent::Preedit { .. }) => {
            (Composing::Cleared, vec![update(String::new(), None)])
        }
        (Composing::Idle, ImeEvent::Commit(text)) if !text.is_empty() => {
            (Composing::Idle, vec![EditInput::Text(text)])
        }
        (Composing::Active | Composing::Cleared, ImeEvent::Commit(text)) => (
            Composing::Idle,
            vec![EditInput::Composition(Composition::End { text })],
        ),
        (Composing::Active | Composing::Cleared, ImeEvent::Disabled) => settle(state),
        (state, _) => (state, Vec::new()),
    }
}

/// Ends a composition the window is leaving (focus lost, the input method switched off).
pub fn settle(state: Composing) -> (Composing, Vec<EditInput>) {
    match state {
        Composing::Idle => (Composing::Idle, Vec::new()),
        Composing::Active | Composing::Cleared => (
            Composing::Idle,
            vec![EditInput::Composition(Composition::End {
                text: String::new(),
            })],
        ),
    }
}

fn update(text: String, cursor: Option<(usize, usize)>) -> EditInput {
    EditInput::Composition(Composition::Update {
        text,
        cursor: cursor.map(|(start, end)| PreeditCursor { start, end }),
    })
}
