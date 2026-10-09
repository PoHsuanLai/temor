//! Keys (PLAN §6): a routed key runs its action, or goes to the program encoded by the
//! terminal's modes. Typing returns the view to the live screen.

use temor_core::terminal::action::Action;
use temor_vt::KeyAction;

use super::{FindStep, PromptStep, Session};
use crate::error::UiError;
use crate::input::{FindBar, Routed, Stroke, route};

impl Session {
    /// Routes one key event.
    pub fn key(&mut self, stroke: Stroke) -> Result<(), UiError> {
        let find = match self.finder.is_some() {
            true => FindBar::Open,
            false => FindBar::Closed,
        };
        match route(stroke, &self.keymap, find) {
            Routed::Action(action) => self.run(action),
            Routed::Shell(key) => {
                if key.action != KeyAction::Release {
                    self.pane
                        .with_term(|term| term.scroll(temor_vt::ViewportScroll::Bottom));
                }
                self.pane.key(&key)
            }
            Routed::Ignored => Ok(()),
        }
    }

    /// Runs a pane action.
    pub fn run(&mut self, action: Action) -> Result<(), UiError> {
        match action {
            Action::Copy => self.copy(),
            Action::Paste => self.paste(),
            Action::Find => {
                if self.finder.is_none() {
                    self.finder = Some(temor_core::terminal::search::Finder::new("", Vec::new()));
                }
                Ok(())
            }
            Action::FindNext => {
                self.step_find(FindStep::Next);
                Ok(())
            }
            Action::FindPrevious => {
                self.step_find(FindStep::Previous);
                Ok(())
            }
            Action::CloseFind => {
                self.finder = None;
                Ok(())
            }
            Action::PreviousPrompt => self.jump_prompt(PromptStep::Previous),
            Action::NextPrompt => self.jump_prompt(PromptStep::Next),
        }
    }
}
