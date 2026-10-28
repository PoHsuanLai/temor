//! Keys (PLAN §6): a routed key runs its action, or goes to the program encoded by the
//! terminal's modes. Typing returns the view to the live screen.

use temor_panes::pane::keys::Action;
use temor_panes::vt::{KeyAction, KeyCode};

use temor_client::model::block::HistoryStep;

use super::{FindStep, Session};
use crate::error::ViewError;
use crate::input::{Routed, Stroke, is_altgr_text, kitty_keys, route_with};

impl Session {
    /// Routes one key event.
    pub fn key(&mut self, stroke: Stroke) -> Result<(), ViewError> {
        let find = self.finder.is_some();
        let kitty = self
            .pane
            .with_term(|term| kitty_keys(term.kitty_keyboard_flags()));
        if !find && self.pick_key(&stroke.key) {
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
                        .with_term(|term| term.scroll(temor_panes::vt::ViewportScroll::Bottom));
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

    /// A chord typed while the composer has the keys: the action the keymap binds it to runs
    /// (the window's own reach the window through [`Session::take_asked`]), Ctrl+C empties the
    /// composer, and any other chord does nothing: the shell's own line editor, which the
    /// composer stands for, never hears it.
    pub fn composer_chord(&mut self, stroke: Stroke) -> Result<(), ViewError> {
        let find = self.finder.is_some();
        let kitty = self
            .pane
            .with_term(|term| kitty_keys(term.kitty_keyboard_flags()));
        let clear = stroke.key.mods == temor_panes::vt::Modifiers::CTRL
            && stroke.key.unshifted == Some('c')
            && stroke.key.action != KeyAction::Release;
        match route_with(stroke, &self.keymap, find, kitty) {
            Routed::Action(action) => self.run(action),
            _ if clear => {
                self.set_composer_text(String::new());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Runs a pane action.
    pub fn run(&mut self, action: Action) -> Result<(), ViewError> {
        match action {
            Action::Copy => self.copy(),
            Action::Paste => self.paste(),
            Action::Find => {
                if self.finder.is_none() {
                    self.finder = Some(temor_panes::pane::text::Finder::new("", Vec::new()));
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
                self.step_pick(HistoryStep::Older);
                Ok(())
            }
            Action::NextPrompt => {
                self.step_pick(HistoryStep::Newer);
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
            Action::ToggleFold => {
                self.toggle_folds();
                Ok(())
            }
            Action::PreviousBookmark => {
                self.step_bookmark(HistoryStep::Older);
                Ok(())
            }
            Action::NextBookmark => {
                self.step_bookmark(HistoryStep::Newer);
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
            | Action::Back
            | Action::Forward
            | Action::History
            | Action::PopOut
            | Action::ShowStage
            | Action::ShowConsole
            | Action::ShowStageAndConsole
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
