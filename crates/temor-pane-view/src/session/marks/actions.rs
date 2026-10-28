//! The block actions the bar offers (PLAN §9): copy the command or the output, save the output
//! to Downloads, and type the command again. None of them runs a command.

use std::io::Write;
use std::path::Path;

use temor_client::model::vocab::ids::BlockId;
use temor_client::model::vocab::text::CommandText;
use temor_client::shelf::transfer::FileName;
use temor_client::shelf::transfer::shelf::ShelfStore;
use temor_panes::pane::paste;
use temor_panes::vt::ViewportScroll;

use super::super::Session;
use super::super::osc52::Copied;
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

    /// Copies a finished block's output to the clipboard.
    pub fn copy_output(&mut self, id: BlockId) {
        match self.block_output(id) {
            Some(text) => self.copy_text(&text),
            None => self.notice = Some(notices::OUTPUT_NOT_FINISHED.to_owned()),
        }
    }

    pub(super) fn copy_text(&mut self, text: &str) {
        if self.put_on_clipboard(text) == Copied::No {
            self.notice = Some(notices::TEXT_NOT_COPIED.to_owned());
        }
    }

    /// Saves a finished block's output to the downloads shelf (PLAN §9): written to a fresh
    /// temporary file in the shelf folder, then placed under its own name by the shelf store.
    pub fn save_output(&mut self, id: BlockId) -> Result<(), ViewError> {
        let Some(text) = self.block_output(id) else {
            self.notice = Some(notices::OUTPUT_NOT_FINISHED.to_owned());
            return Ok(());
        };
        let command = self.block_command(id).unwrap_or_default().to_owned();
        let store = ShelfStore::new(self.core().shelf_dir.clone());
        let name = FileName::sanitise(&output_name(&command));
        let saved = write_temp(store.dir(), &text).and_then(|temp| {
            store
                .place(temp.as_ref(), &name)
                .map_err(|err| ViewError::Save(err.to_string()))
        });
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

/// Writes `text` to a new, uniquely named file in `dir`. The file is created exclusively, so a
/// planted link at its name is never followed. The returned path removes the file when dropped
/// (the shelf store has already moved it by then).
fn write_temp(dir: &Path, text: &str) -> Result<tempfile::TempPath, ViewError> {
    let io = |err: std::io::Error| ViewError::Save(err.to_string());
    std::fs::create_dir_all(dir).map_err(io)?;
    let mut file = tempfile::Builder::new()
        .prefix(".temor-output-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(io)?;
    file.write_all(text.as_bytes()).map_err(io)?;
    file.flush().map_err(io)?;
    Ok(file.into_temp_path())
}
