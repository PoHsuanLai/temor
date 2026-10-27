//! The facts of a pane on a host (PLAN P16, §9b Tier B): the helper reads the version files of the
//! session's folder and sends their text; the rules that make facts of it are the same ones a
//! pane on this computer follows (`temor_core::facts`, `facts_files`). What an older helper says
//! of git is not read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use temor_core::facts::{Facts, FolderSource, runtimes_in, venv_name};
use temor_wire::message::{FolderFacts, MOST_FACT_PARENTS};

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
    let runtimes = runtimes_in(Path::new(&sent.dir.0), &files, MOST_FACT_PARENTS);
    Facts {
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
    use temor_core::facts::{EnvName, NodeVersion, ToolVersion};
    use temor_wire::message::{FactFile, FolderFiles, RemotePath};

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

    fn sent(folders: Vec<FolderFiles>) -> FolderFacts {
        FolderFacts {
            dir: path("/w/app/src"),
            venv: Some(path("/w/app/src/.venv")),
            home: Some(path("/w")),
            root: Some(path("/w/app")),
            folders,
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
        let facts = facts_from_host(&sent(folders));
        assert_eq!(facts.venv, Some(EnvName("src".to_owned())));
        assert_eq!(facts.node, Some(NodeVersion("v20".to_owned())));
        assert_eq!(facts.python, Some(ToolVersion("3.12".to_owned())));
        assert_eq!(facts.ruby, Some(ToolVersion("3.3.0".to_owned())));
        assert_eq!(facts.go, None);
    }
}
