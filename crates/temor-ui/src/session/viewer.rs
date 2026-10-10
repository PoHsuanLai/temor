//! Asking for a file to be shown (PLAN §10): ⌘-click on a path in the text, and the block
//! toolbar's View. The session finds the file and the files printed around it; the window opens
//! the viewer.

use std::path::{Path, PathBuf};

use temor_core::shelf::PaneSite;
use temor_core::viewer::{candidates, output_paths, resolve, token_at};
use temor_core::vocab::ids::BlockId;
use temor_pane::text::CellPos;

use super::Session;
use crate::error::UiError;

/// A file to show, with the files the same output lists (the viewer's arrows walk them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewAsk {
    /// The file to show first.
    pub file: PathBuf,
    /// The files its output lists, in the order they were printed; empty when it had none.
    pub around: Vec<PathBuf>,
}

/// Whether `path` is a file on this computer: the check the pure path rules are given.
fn is_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

impl Session {
    /// The file the last ⌘-click or View asked for, if one is waiting; taking it clears it.
    pub fn take_view_ask(&mut self) -> Option<ViewAsk> {
        self.view_ask.take()
    }

    /// ⌘-click at `cell`: the word there, if it names a file, is asked for. A pane on another
    /// computer has no files of this one's.
    pub(super) fn view_path_at(&mut self, cell: CellPos) -> Result<(), UiError> {
        if self.site == PaneSite::Remote {
            return Ok(());
        }
        let row = self.line(cell.row)?;
        let Some(token) = token_at(&row, cell.col) else {
            return Ok(());
        };
        let cwd = self.cwd.clone();
        let file = candidates(&token)
            .iter()
            .filter_map(|candidate| resolve(cwd.as_deref(), candidate))
            .find(|path| is_file(path));
        let Some(file) = file else {
            return Ok(());
        };
        let around = self
            .output_at_row(cell.row)
            .map(|output| output_paths(&output, cwd.as_deref(), is_file))
            .unwrap_or_default();
        self.view_ask = Some(ViewAsk { file, around });
        Ok(())
    }

    /// The block toolbar's View: the first file the block's output lists, with the rest around it.
    pub fn view_block(&mut self, id: BlockId) {
        if self.site == PaneSite::Remote {
            self.notice = Some("Files on another computer cannot be shown here yet.".to_owned());
            return;
        }
        let Some(output) = self.block_output(id) else {
            self.notice = Some("The output is not finished yet.".to_owned());
            return;
        };
        let around = output_paths(&output, self.cwd.as_deref(), is_file);
        match around.first().cloned() {
            Some(file) => self.view_ask = Some(ViewAsk { file, around }),
            None => self.notice = Some("This output lists no files to show.".to_owned()),
        }
    }
}
