//! The text "Edit locally" works on (PLAN §9b): what the person selected in the pane, or else the
//! word under the pointer. The window hands it to the host-file editor, which resolves it against
//! the pane's folder on the host.

use temor_core::viewer::{candidates, token_at};
use temor_pane::text::CellPos;

use super::Session;
use super::pointer::screen_cell;

impl Session {
    /// The selected text, one line of it, if there is any.
    fn selected_line(&self) -> Option<String> {
        let selection = self.selection?;
        let text = self
            .pane
            .with_term(|term| selection.text(|row| term.row_text(row).unwrap_or_default()));
        let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
        Some(line.to_owned())
    }

    /// The word under the pointer, as the path it most likely names.
    fn word_under_pointer(&self) -> Option<String> {
        let at = self.pointer_over?;
        let fit = self.placed()?;
        let hit = fit.cell_at(at);
        let top = self.viewport_top().ok()?;
        let CellPos { row, col } = screen_cell(top, hit.pos.row.0, hit.pos.col.0);
        let token = token_at(&self.line(row).ok()?, col)?;
        candidates(&token).into_iter().next()
    }

    /// What to edit: the selection, else the word under the pointer.
    pub fn text_to_edit(&self) -> Option<String> {
        self.selected_line().or_else(|| self.word_under_pointer())
    }

    /// What the block menu's "Edit Locally" edits: the selection when there is one, else the
    /// last path-looking argument of the command of the block the menu is on (the active picked
    /// block, else the last picked). The pointer is on the menu by then, so the word under it
    /// is not consulted.
    pub fn block_text_to_edit(&self) -> Option<String> {
        self.selected_line().or_else(|| {
            let picked = self.picked_blocks();
            let id = self.selected_block().or_else(|| picked.last().copied())?;
            last_path_argument(self.block_command(id)?)
        })
    }
}

/// The last argument of `command` that looks like a path: it has a `/`, starts with `~` or `.`,
/// or ends in an extension (`notes.txt`), and is not an option (`-x`), a web address or after the
/// first pipe or redirect (`|`, `>`, `&&`). The command's own first word never counts. Quotes
/// around an argument are dropped. Words are split at spaces, so a name with a space in it is
/// not found: select it instead.
pub fn last_path_argument(command: &str) -> Option<String> {
    command
        .split_whitespace()
        .skip(1)
        .map(|word| word.trim_matches(|c| c == '\'' || c == '"'))
        .take_while(|word| !word.contains(['|', '&', ';', '<', '>']))
        .filter(|word| !word.starts_with('-') && !word.contains("://"))
        .filter(|word| looks_like_path(word))
        .last()
        .map(str::to_owned)
}

fn looks_like_path(word: &str) -> bool {
    let extension = word
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && !ext.is_empty());
    word.contains('/') || word.starts_with(['~', '.']) || extension
}

#[cfg(test)]
mod tests {
    use super::last_path_argument;

    #[test]
    fn the_last_path_looking_argument_of_a_command_is_the_file_to_edit() {
        let cases = [
            ("cat src/main.rs", Some("src/main.rs")),
            ("vim notes.txt todo.md", Some("todo.md")),
            ("ls -la /etc/hosts | head -n 5", Some("/etc/hosts")),
            ("cat 'my.txt' > out.log 2>&1", Some("my.txt")),
            ("less ~/x", Some("~/x")),
            ("make test", None),
            ("curl https://example.com/a.html", None),
            ("ls", None),
            ("./run.sh", None),
            ("tar xf a.tar -C ./dest", Some("./dest")),
            ("echo .hidden", Some(".hidden")),
        ];
        for (command, want) in cases {
            assert_eq!(last_path_argument(command).as_deref(), want, "{command}");
        }
    }
}
