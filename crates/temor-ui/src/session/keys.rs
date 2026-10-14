//! Keys (PLAN §6): a routed key runs its action, or goes to the program encoded by the
//! terminal's modes. Typing returns the view to the live screen.

use temor_pane::keys::Action;
use temor_vt::{KeyAction, KeyCode};

use temor_core::block::PickStep;

use super::{FindStep, Session};
use crate::error::UiError;
use crate::input::{FindBar, KittyKeys, Routed, Stroke, is_altgr_text, route_with};

impl Session {
    /// Routes one key event.
    pub fn key(&mut self, stroke: Stroke) -> Result<(), UiError> {
        let find = match self.finder.is_some() {
            true => FindBar::Open,
            false => FindBar::Closed,
        };
        let kitty = self
            .pane
            .with_term(|term| KittyKeys::of(term.kitty_keyboard_flags()));
        if find == FindBar::Closed && self.pick_key(&stroke.key) {
            return Ok(());
        }
        match route_with(stroke, &self.keymap, find, kitty) {
            Routed::Action(action) => self.run(action),
            Routed::Shell(key) => {
                self.key_leaves_pick(&key);
                self.wake_cursor();
                if key.action == KeyAction::Press && key.code == KeyCode::Enter {
                    self.note_enter();
                }
                if key.action != KeyAction::Release {
                    self.pane
                        .with_term(|term| term.scroll(temor_vt::ViewportScroll::Bottom));
                }
                if is_altgr_text(&key) {
                    self.pane.send(key.text.as_bytes());
                    return Ok(());
                }
                Ok(self.pane.key(&key)?)
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
                    self.finder = Some(temor_pane::text::Finder::new("", Vec::new()));
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
            Action::PreviousPrompt => {
                self.step_pick(PickStep::Older);
                Ok(())
            }
            Action::NextPrompt => {
                self.step_pick(PickStep::Newer);
                Ok(())
            }
            Action::CopyCommands => {
                self.copy_picked_commands();
                Ok(())
            }
            Action::CopyOutputs => {
                self.copy_picked_outputs();
                Ok(())
            }
            Action::SelectAllBlocks => {
                self.pick_all();
                Ok(())
            }
            Action::ToggleBookmark => {
                self.toggle_bookmarks();
                Ok(())
            }
            Action::PreviousBookmark => {
                self.step_bookmark(PickStep::Older);
                Ok(())
            }
            Action::NextBookmark => {
                self.step_bookmark(PickStep::Newer);
                Ok(())
            }
            Action::ReinputCommands => {
                self.reinput_picked();
                Ok(())
            }
            // Tabs and splits belong to the window: the action waits here for it to take.
            Action::NewTab
            | Action::OpenBar
            | Action::ToggleQuick
            | Action::ClosePane
            | Action::SplitRight
            | Action::SplitDown
            | Action::FocusLeft
            | Action::FocusRight
            | Action::FocusUp
            | Action::FocusDown
            | Action::ToggleZoom
            | Action::Equalize
            | Action::ReopenTab
            | Action::JumpToWaiting
            | Action::ToggleSidebar
            | Action::PinTab
            | Action::UnpinTab
            | Action::ShowDownloads
            | Action::Settings
            | Action::SettingsAt(_)
            | Action::ZoomIn
            | Action::ZoomOut
            | Action::ActualSize
            | Action::SelectTab(_)
            | Action::SelectSpace(_) => {
                self.asked = Some(action);
                Ok(())
            }
        }
    }

    /// The tab or split action the last key asked for, if one is waiting; taking it clears it.
    pub fn take_asked(&mut self) -> Option<Action> {
        self.asked.take()
    }
}
