//! The block actions the bar offers (PLAN §9): copy the command or the output, save the output
//! to Downloads, and type the command again. None of them runs a command.

use temor_client::model::vocab::ids::BlockId;
use temor_client::model::vocab::text::CommandText;
use temor_client::shelf::transfer::FileName;
use temor_panes::pane::paste;
use temor_panes::vt::ViewportScroll;

use super::super::Session;
use crate::error::ViewError;
use crate::words::notices;

impl Session {
    /// Puts a block's command in the prompt without running it: in the composer while the list
    /// shows it, else through the paste path (framed when the shell asked for it), so no line
    /// break in it can submit anything. A command that is not safe to type is refused with a
    /// notice.
    pub fn rerun(&mut self, id: BlockId) {
        let Some(command) = self.block_command(id) else {
            return;
        };
        if !self.put_in_prompt(CommandText(command)) {
            self.notice = Some("This command cannot be typed again.".to_owned());
        }
    }

    /// Puts `command` in the prompt without running it: in the composer while the list shows it
    /// (lines and all: it sends nothing until Return), else through the paste path. False, with
    /// nothing put anywhere, when it is not safe there.
    pub(in crate::session) fn put_in_prompt(&mut self, command: CommandText) -> bool {
        if self.composer_shown() {
            if !command.composable() {
                return false;
            }
            self.set_composer_text(command.0);
            return true;
        }
        if !command.typable() {
            return false;
        }
        let mode = self.pane.with_term(|term| term.paste_mode());
        self.pane.send(&paste::encode(&command.0, mode));
        self.pane
            .with_term(|term| term.scroll(ViewportScroll::Bottom));
        true
    }

    /// Copies a block's command to the clipboard.
    pub fn copy_command(&mut self, id: BlockId) {
        let Some(command) = self.block_command(id) else {
            return;
        };
        self.copy_text(&command);
    }

    pub(super) fn copy_text(&mut self, text: &str) {
        if !self.put_on_clipboard(text) {
            self.notice = Some(notices::TEXT_NOT_COPIED.to_owned());
        }
    }

    /// Saves a finished block's output to the downloads shelf (PLAN §9), under its own name
    /// (`temor_client::shelf::save_text`).
    pub fn save_output(&mut self, id: BlockId) -> Result<(), ViewError> {
        let Some(text) = self.block_output(id) else {
            self.notice = Some(notices::OUTPUT_NOT_FINISHED.to_owned());
            return Ok(());
        };
        let command = self.block_command(id).unwrap_or_default().to_owned();
        let name = FileName::sanitise(&output_name(&command));
        let saved = temor_client::shelf::save_text(&self.core().shelf_dir, &text, &name)
            .map_err(|err| ViewError::Save(err.to_string()));
        self.notice = Some(match saved {
            Ok(_) => "Saved to Downloads.".to_owned(),
            Err(err) => err.to_string(),
        });
        Ok(())
    }
}

/// The name of a block's output file: the command's start, then " output.txt".
pub(super) fn output_name(command: &str) -> String {
    let head: String = command.chars().take(40).collect();
    let head = head.trim();
    match head.is_empty() {
        true => "Command output.txt".to_owned(),
        false => format!("{head} output.txt"),
    }
}
