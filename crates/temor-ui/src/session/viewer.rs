//! Asking for a file to be shown (PLAN §10): ⌘-click on a path in the text, and the block
//! toolbar's View. The session finds the file and the files printed around it; the window opens
//! the viewer.

use std::path::{Path, PathBuf};

use temor_core::shelf::PaneSite;
use temor_core::viewer::{candidates, output_paths, resolve, token_at};
use temor_core::vocab::ids::BlockId;
use temor_pane::text::{CellPos, cwd_from_url};
use temor_vt::{GridPoint, PointSpace, SurfacePoint};

use super::Session;
use super::pointer::screen_cell;
use crate::error::UiError;

/// A file to show, with the files the same output lists (the viewer's arrows walk them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewAsk {
    /// The file to show first.
    pub file: PathBuf,
    /// The files its output lists, in the order they were printed; empty when it had none.
    pub around: Vec<PathBuf>,
}

/// The path a `file://` link names when it names this computer (no host, or `localhost`).
fn local_file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let host = &rest[..rest.find('/')?];
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    cwd_from_url(url).map(PathBuf::from)
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
        let Some(file) = self.file_at(cell)? else {
            return Ok(());
        };
        let cwd = self.cwd.clone();
        let around = self
            .output_at_row(cell.row)
            .map(|output| output_paths(&output, cwd.as_deref(), is_file))
            .unwrap_or_default();
        self.view_ask = Some(ViewAsk { file, around });
        Ok(())
    }

    /// The file the word at `cell` names, if it names one that exists here. The rule is the
    /// ⌘-click's, and the peek's too.
    fn file_at(&self, cell: CellPos) -> Result<Option<PathBuf>, UiError> {
        if self.site == PaneSite::Remote {
            return Ok(None);
        }
        let row = self.line(cell.row)?;
        let Some(token) = token_at(&row, cell.col) else {
            return Ok(None);
        };
        let cwd = self.cwd.as_deref();
        Ok(candidates(&token)
            .iter()
            .filter_map(|candidate| resolve(cwd, candidate))
            .find(|path| is_file(path)))
    }

    /// The file a `file://` link (OSC 8) at `cell` names, if it is one that exists here.
    fn linked_file_at(&self, cell: CellPos) -> Option<PathBuf> {
        let target = self.pane.with_term(|term| {
            term.hyperlink_at(GridPoint::new(PointSpace::Screen, cell.col, cell.row))
        })?;
        local_file_url(&target).filter(|path| is_file(path))
    }

    /// The file under the pointer at `at`, for the peek: a `file://` link there, else a path in
    /// the text. None on a pane on another computer, while the program has the mouse (a ⌘-click
    /// would go to it), or where nothing names a file.
    pub fn file_under(&self, at: SurfacePoint) -> Option<PathBuf> {
        let fit = self.fit?;
        if self.pane.with_term(|term| term.mouse_tracking()) {
            return None;
        }
        let hit = fit.cell_at(at);
        let top = self.viewport_top().ok()?;
        let cell = screen_cell(top, hit.pos.row.0, hit.pos.col.0);
        if self.site == PaneSite::Remote {
            return None;
        }
        self.linked_file_at(cell)
            .or_else(|| self.file_at(cell).ok().flatten())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_link_to_a_file_on_this_computer_names_a_path() {
        let table = [
            ("file:///tmp/a%20b.txt", Some("/tmp/a b.txt")),
            ("file://localhost/tmp/a", Some("/tmp/a")),
            ("file://LOCALHOST/tmp/a", Some("/tmp/a")),
            ("file://other-host/tmp/a", None),
            ("file://", None),
            ("https://example.com/a", None),
        ];
        for (url, want) in table {
            assert_eq!(local_file_url(url), want.map(PathBuf::from), "{url}");
        }
    }
}
