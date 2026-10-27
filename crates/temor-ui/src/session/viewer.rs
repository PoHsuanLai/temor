//! Asking for a file to be shown (PLAN §10): ⌘-click on a path in the text, and the block
//! toolbar's View. The session finds the file and the files printed around it; the window opens
//! the viewer.

use std::path::{Path, PathBuf};

use temor_client::model::shelf::PaneSite;
use temor_client::model::viewer::{candidates, first_file, output_paths, resolve, token_at};
use temor_client::model::vocab::ids::BlockId;
use temor_panes::pane::text::{CellPos, cwd_from_url};
use temor_panes::vt::{GridPoint, PointSpace, SurfacePoint};

use super::Session;
use crate::error::UiError;
use crate::words::notices;

/// A file to show, with the files the same output lists (the viewer's arrows walk them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewAsk {
    /// The file to show first.
    pub file: PathBuf,
    /// The files its output lists, in the order they were printed; empty when it had none.
    pub around: Vec<PathBuf>,
    /// Where the file is: on this computer, or on the host the pane is a session on (then it is
    /// a path there, not checked, and nothing is listed around it).
    pub site: PaneSite,
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

/// The path a click on `token` most likely means on a host: the last of its candidates, which is
/// the token without wrapping, `:line:column` or `rg`'s trailing text. Nothing here can look at
/// the host's disk, so the fetch finds out whether it is a file.
fn host_path(cwd: Option<&str>, token: &str) -> Option<PathBuf> {
    candidates(token)
        .iter()
        .rev()
        .find_map(|candidate| resolve(cwd, candidate))
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

    /// ⌘-click at `cell`: the word there, if it names a file or a folder here, is asked for. On a
    /// pane on another computer the word is taken as a path there (the window fetches it).
    pub(super) fn view_path_at(&mut self, cell: CellPos) -> Result<(), UiError> {
        let Some(file) = self.file_at(cell)? else {
            if let Some(folder) = self.folder_at(cell)? {
                self.view_ask = Some(ViewAsk {
                    file: folder,
                    around: Vec::new(),
                    site: PaneSite::Local,
                });
            }
            return Ok(());
        };
        let (cwd, site) = {
            let core = self.core();
            (core.cwd.clone(), core.site)
        };
        let around = match site {
            PaneSite::Remote => Vec::new(),
            PaneSite::Local => self
                .output_at_row(cell.row)
                .map(|output| output_paths(&output, cwd.as_deref(), is_file))
                .unwrap_or_default(),
        };
        self.view_ask = Some(ViewAsk { file, around, site });
        Ok(())
    }

    /// The file the word at `cell` names, if it names one that exists here; on a pane on another
    /// computer, the path it most likely names there. The rule is the ⌘-click's, and the peek's
    /// too.
    fn file_at(&self, cell: CellPos) -> Result<Option<PathBuf>, UiError> {
        let row = self.line(cell.row)?;
        let Some(token) = token_at(&row, cell.col) else {
            return Ok(None);
        };
        let (cwd, site) = {
            let core = self.core();
            (core.cwd.clone(), core.site)
        };
        let cwd = cwd.as_deref();
        Ok(match site {
            PaneSite::Remote => host_path(cwd, &token),
            PaneSite::Local => first_file(&token, cwd, &is_file),
        })
    }

    /// The folder on this computer the word at `cell` names, by the ⌘-click's rule for files.
    fn folder_at(&self, cell: CellPos) -> Result<Option<PathBuf>, UiError> {
        if self.core().site == PaneSite::Remote {
            return Ok(None);
        }
        let row = self.line(cell.row)?;
        let Some(token) = token_at(&row, cell.col) else {
            return Ok(None);
        };
        let cwd = self.core().cwd.clone();
        Ok(first_file(&token, cwd.as_deref(), &|path: &Path| {
            path.is_dir()
        }))
    }

    /// The file a `file://` link names: on this computer, one that exists here; on a pane on
    /// another computer, the path in the link, whatever host it names (the program on the host
    /// wrote it).
    fn linked_file(&self, target: &str) -> Option<PathBuf> {
        match self.core().site {
            PaneSite::Remote => cwd_from_url(target).map(PathBuf::from),
            PaneSite::Local => local_file_url(target).filter(|path| is_file(path)),
        }
    }

    /// A `file://` link that names a file opens in the viewer, as the peek shows it. Whether it
    /// did.
    pub(super) fn view_link(&mut self, target: &str) -> bool {
        let Some(file) = self.linked_file(target) else {
            return false;
        };
        let site = self.core().site;
        self.view_ask = Some(ViewAsk {
            file,
            around: Vec::new(),
            site,
        });
        true
    }

    fn linked_file_at(&self, cell: CellPos) -> Option<PathBuf> {
        let target = self.pane.with_term(|term| {
            term.hyperlink_at(GridPoint::new(PointSpace::Screen, cell.col, cell.row))
        })?;
        self.linked_file(&target)
    }

    /// The file under the pointer at `at`, for the peek: a `file://` link there, else a path in
    /// the text (on a pane on another computer, a path there, unchecked). None while the program
    /// has the mouse (a ⌘-click would go to it), or where nothing names a file.
    pub fn file_under(&self, at: SurfacePoint) -> Option<PathBuf> {
        if self.pane.with_term(|term| term.mouse_tracking()) {
            return None;
        }
        let cell = self.point_hit(at)?.cell;
        self.linked_file_at(cell)
            .or_else(|| self.file_at(cell).ok().flatten())
    }

    /// The block toolbar's View: the first file the block's output lists, with the rest around it.
    pub fn view_block(&mut self, id: BlockId) {
        if self.core().site == PaneSite::Remote {
            self.notice =
                Some("To view a file on another computer, \u{2318}-click its name".to_owned());
            return;
        }
        let Some(output) = self.block_output(id) else {
            self.notice = Some(notices::OUTPUT_NOT_FINISHED.to_owned());
            return;
        };
        let around = output_paths(&output, self.core().cwd.as_deref(), is_file);
        match around.first().cloned() {
            Some(file) => {
                self.view_ask = Some(ViewAsk {
                    file,
                    around,
                    site: PaneSite::Local,
                })
            }
            None => self.notice = Some("This output lists no files to show.".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_on_a_host_takes_the_path_without_the_line_numbers_or_the_text_after_them() {
        let table = [
            ("src/a.rs", Some("/w/src/a.rs")),
            ("src/a.rs:12:5", Some("/w/src/a.rs")),
            ("(src/a.rs:12)", Some("/w/src/a.rs")),
            ("src/a.rs:12:let", Some("/w/src/a.rs")),
            ("/etc/hosts", Some("/etc/hosts")),
            ("", None),
        ];
        for (token, want) in table {
            assert_eq!(
                host_path(Some("/w"), token),
                want.map(PathBuf::from),
                "{token}"
            );
        }
        assert_eq!(host_path(None, "a.rs"), None, "no folder to start from");
    }

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
