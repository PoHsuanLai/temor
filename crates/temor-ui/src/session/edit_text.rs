//! The text "Edit locally" works on (PLAN §9b): what the person selected in the pane, or else
//! the last file named in a block's command. The window hands it to the host-file editor, which
//! resolves it against the pane's folder on the host.

use temor_core::host::target::resolvable;
use temor_core::vocab::ids::BlockId;

use super::Session;

impl Session {
    /// The selected text, one line of it, if there is any.
    fn selected_line(&self) -> Option<String> {
        let selection = self.selection?;
        let text = self
            .core
            .pane
            .with_term(|term| selection.text(|row| term.row_text(row).unwrap_or_default()));
        let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
        Some(line.to_owned())
    }

    /// What "Edit Locally" edits, from the toolbar and the menu alike: the selection when there
    /// is one, else the last path-looking argument of the command of `block` (the active picked
    /// block, else the last picked, when `block` is `None`). The pointer is on the toolbar or the
    /// menu by then, so the word under it is never consulted.
    pub fn block_text_to_edit(&self, block: Option<BlockId>) -> Option<String> {
        self.selected_line().or_else(|| {
            let picked = self.picked_blocks();
            let id = block
                .or_else(|| self.selected_block())
                .or_else(|| picked.last().copied())?;
            last_path_argument(self.block_command(id)?)
        })
    }
}

/// The last argument of `command` that looks like a path: it has a `/`, starts with `~` or `.`,
/// or ends in an extension (`notes.txt`), and is not an option (`-x`), a web address or after the
/// first pipe or redirect (`|`, `>`, `&&`). The command's own first word never counts, and neither
/// does a path the host would have to expand (`~/x`, `../x`): core refuses those. Quotes around an
/// argument are dropped. Words are split at spaces, so a name with a space in it is not found:
/// select it instead.
pub fn last_path_argument(command: &str) -> Option<String> {
    command
        .split_whitespace()
        .skip(1)
        .map(|word| word.trim_matches(|c| c == '\'' || c == '"'))
        .take_while(|word| !word.contains(['|', '&', ';', '<', '>']))
        .filter(|word| !word.starts_with('-') && !word.contains("://"))
        .filter(|word| looks_like_path(word) && resolvable(word))
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
            ("less ~/x", None),
            ("cp notes.txt ~/backup/", Some("notes.txt")),
            ("cat ../up.txt", None),
            ("cat /etc/../hosts", Some("/etc/../hosts")),
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
