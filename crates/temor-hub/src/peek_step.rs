//! When the peek card shows and goes (PLAN §10): a pure step. Holding ⌘ while the pointer rests on
//! a file path shows the card; releasing ⌘, moving off the path, a key typed or a press hides it.
//! After a key is typed the card stays away until ⌘ is let go or the pointer moves to another
//! path, so the card never comes back under a person who is typing.

use std::path::PathBuf;

use temor_panes::vt::Modifiers;

/// Whether the ⌘ key is down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Held,
    Up,
}

impl Command {
    /// ⌘ is the chord the ⌘-click uses: Ctrl and Shift together.
    // TODO(macos): ⌘ is the Super key there; the ⌘-click in `session/pointer.rs` moves with it.
    pub fn of(mods: Modifiers) -> Command {
        match mods.contains(Modifiers::CTRL) && mods.contains(Modifiers::SHIFT) {
            true => Command::Held,
            false => Command::Up,
        }
    }
}

/// A file path under the pointer, and where the pointer is (logical pixels in the pane).
#[derive(Debug, Clone, PartialEq)]
pub struct Under {
    pub file: PathBuf,
    pub x: f32,
    pub y: f32,
}

/// Where the peek is.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Peek {
    #[default]
    Idle,
    /// The card for this file is shown or on its way.
    Showing(PathBuf),
    /// A key was typed with the pointer over this file (or over none): no card until something
    /// else changes.
    Spent(Option<PathBuf>),
}

/// What happened. `under` is looked up only while ⌘ is held.
#[derive(Debug, Clone, PartialEq)]
pub enum PeekEvent {
    /// The pointer moved or ⌘ went down or up.
    Looked {
        command: Command,
        under: Option<Under>,
    },
    /// A key other than a modifier was pressed; `over` is the file under the pointer with ⌘ held.
    Typed { over: Option<PathBuf> },
    /// The pointer left the pane or pressed.
    Gone,
}

/// What the view does about it.
#[derive(Debug, Clone, PartialEq)]
pub enum PeekEffect {
    Nothing,
    /// Look at this file and show its card (replacing any card shown).
    Show(Under),
    Hide,
}

fn hide(state: &Peek) -> PeekEffect {
    match state {
        Peek::Showing(_) => PeekEffect::Hide,
        Peek::Idle | Peek::Spent(_) => PeekEffect::Nothing,
    }
}

/// The next state and what to do.
pub fn step(state: &Peek, event: PeekEvent) -> (Peek, PeekEffect) {
    match event {
        PeekEvent::Gone => (Peek::Idle, hide(state)),
        PeekEvent::Typed { over } => match (state, over) {
            (Peek::Showing(file), _) => (Peek::Spent(Some(file.clone())), PeekEffect::Hide),
            (_, Some(file)) => (Peek::Spent(Some(file)), PeekEffect::Nothing),
            (other, None) => (other.clone(), PeekEffect::Nothing),
        },
        PeekEvent::Looked {
            command: Command::Up,
            ..
        }
        | PeekEvent::Looked { under: None, .. } => (Peek::Idle, hide(state)),
        PeekEvent::Looked {
            under: Some(under), ..
        } => match state {
            Peek::Showing(file) | Peek::Spent(Some(file)) if *file == under.file => {
                (state.clone(), PeekEffect::Nothing)
            }
            Peek::Idle | Peek::Showing(_) | Peek::Spent(_) => {
                (Peek::Showing(under.file.clone()), PeekEffect::Show(under))
            }
        },
    }
}

#[cfg(test)]
#[path = "peek_step_tests.rs"]
mod tests;
