//! The chips of a pane on a host (PLAN P16, §9b Tier B): the helper reads git and the version files
//! of the session's folder and sends their text; the rules that make chips of it are the same
//! ones a pane on this computer follows (`temor_core::facts`, `facts_files`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use temor_core::facts::{Facts, parse_branch, parse_shortstat, parse_untracked, venv_name};
use temor_wire::message::FolderFacts;

use super::facts_files::{FolderSource, runtimes_in};

/// The version files the helper sent, by folder.
struct Sent {
    files: BTreeMap<PathBuf, BTreeMap<String, String>>,
    root: Option<PathBuf>,
}

impl FolderSource for Sent {
    fn read(&self, folder: &Path, file: &str) -> Option<String> {
        self.files.get(folder)?.get(file).cloned()
    }

    fn is_root(&self, folder: &Path) -> bool {
        self.root.as_deref() == Some(folder)
    }
}

/// What the helper's text says about the folder.
pub fn facts_from_host(sent: &FolderFacts) -> Facts {
    let branch = sent
        .git
        .as_ref()
        .and_then(|git| git.head.as_deref())
        .and_then(parse_branch);
    // No branch (not a repository, or a detached head): nothing to compare or count either.
    let git = sent.git.as_ref().filter(|_| branch.is_some());
    let files = Sent {
        files: sent
            .folders
            .iter()
            .map(|folder| {
                let named = folder
                    .files
                    .iter()
                    .map(|file| (file.name.clone(), file.text.clone()))
                    .collect();
                (PathBuf::from(&folder.dir.0), named)
            })
            .collect(),
        root: sent.root.as_ref().map(|root| PathBuf::from(&root.0)),
    };
    let runtimes = runtimes_in(Path::new(&sent.dir.0), &files);
    Facts {
        branch,
        changes: git
            .and_then(|git| git.shortstat.as_deref())
            .and_then(parse_shortstat),
        untracked: git
            .and_then(|git| git.untracked.as_deref())
            .map(parse_untracked),
        venv: sent.venv.as_ref().and_then(|path| venv_name(&path.0)),
        node: runtimes.node,
        python: runtimes.python,
        ruby: runtimes.ruby,
        go: runtimes.go,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_core::facts::{BranchName, DiffStat, EnvName, NodeVersion, ToolVersion, Untracked};
    use temor_wire::message::{FactFile, FolderFiles, GitText, RemotePath};

    fn path(text: &str) -> RemotePath {
        RemotePath(text.to_owned())
    }

    fn folder(dir: &str, files: &[(&str, &str)]) -> FolderFiles {
        FolderFiles {
            dir: path(dir),
            files: files
                .iter()
                .map(|(name, text)| FactFile {
                    name: (*name).to_owned(),
                    text: (*text).to_owned(),
                })
                .collect(),
        }
    }

    fn sent(git: Option<GitText>, folders: Vec<FolderFiles>) -> FolderFacts {
        FolderFacts {
            dir: path("/w/app/src"),
            git,
            venv: Some(path("/w/app/src/.venv")),
            home: Some(path("/w")),
            root: Some(path("/w/app")),
            folders,
        }
    }

    fn git(head: &str) -> GitText {
        GitText {
            head: Some(head.to_owned()),
            shortstat: Some(" 2 files changed, 5 insertions(+), 1 deletion(-)\n".to_owned()),
            untracked: Some("?? a\n?? b/\n".to_owned()),
        }
    }

    #[test]
    fn the_helpers_text_makes_the_same_chips_a_local_folder_would() {
        let folders = vec![
            folder("/w/app/src", &[]),
            folder(
                "/w/app",
                &[
                    (".nvmrc", "v20\n"),
                    (".python-version", "3.12\n"),
                    (".tool-versions", "ruby 3.3.0\n"),
                ],
            ),
            // Above the repository root: never looked in.
            folder("/w", &[(".nvmrc", "v14\n")]),
        ];
        let facts = facts_from_host(&sent(Some(git("main\n")), folders));
        assert_eq!(facts.branch, Some(BranchName("main".to_owned())));
        assert_eq!(
            facts.changes,
            Some(DiffStat {
                files: 2,
                added: 5,
                removed: 1
            })
        );
        assert_eq!(facts.untracked, Some(Untracked(2)));
        assert_eq!(facts.venv, Some(EnvName("src".to_owned())));
        assert_eq!(facts.node, Some(NodeVersion("v20".to_owned())));
        assert_eq!(facts.python, Some(ToolVersion("3.12".to_owned())));
        assert_eq!(facts.ruby, Some(ToolVersion("3.3.0".to_owned())));
        assert_eq!(facts.go, None);
    }

    #[test]
    fn without_a_branch_there_are_no_changes_or_untracked_files_to_show() {
        let table = [
            ("detached", Some(git("HEAD\n"))),
            ("not a repository", None),
        ];
        for (why, git) in table {
            let facts = facts_from_host(&sent(git, vec![]));
            assert_eq!(facts.branch, None, "{why}");
            assert_eq!(facts.changes, None, "{why}");
            assert_eq!(facts.untracked, None, "{why}");
        }
    }
}
