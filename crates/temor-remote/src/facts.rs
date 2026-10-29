//! Reading a session's folder for its facts (PLAN P16, §9b): the version files in it and above
//! it. The helper only reads and sends text; the rules that turn the text into facts are temor's
//! own (`temor_core::facts`), so a pane on a host and a pane on this computer follow the same
//! ones. It runs no git. Anything that cannot be read is left out.

use std::io::Read;
use std::path::Path;

use temor_wire::message::{
    FACT_FILE_NAMES, FactFile, FolderFacts, FolderFiles, MOST_FACT_BYTES, MOST_FACT_PARENTS,
    RemotePath,
};

/// What there is to say about the folder `dir`; `home` is the helper's home folder, if it has one.
pub fn read_folder(dir: &Path, home: Option<&Path>) -> FolderFacts {
    let (folders, root) = read_versions(dir);
    let venv = dir.join(".venv");
    FolderFacts {
        dir: RemotePath(dir.to_string_lossy().into_owned()),
        venv: venv
            .is_dir()
            .then(|| RemotePath(venv.to_string_lossy().into_owned())),
        home: home.map(path_text),
        root,
        folders,
    }
}

fn path_text(path: &Path) -> RemotePath {
    RemotePath(path.to_string_lossy().into_owned())
}

/// The version files of `dir` and each folder above it, nearest first, up to the folder that
/// holds `.git` (which is returned too).
fn read_versions(dir: &Path) -> (Vec<FolderFiles>, Option<RemotePath>) {
    let mut folders = Vec::new();
    for folder in dir.ancestors().take(MOST_FACT_PARENTS) {
        let files: Vec<FactFile> = FACT_FILE_NAMES
            .iter()
            .filter_map(|name| {
                read_small(&folder.join(name)).map(|text| FactFile {
                    name: (*name).to_owned(),
                    text,
                })
            })
            .collect();
        folders.push(FolderFiles {
            dir: path_text(folder),
            files,
        });
        if folder.join(".git").exists() {
            return (folders, Some(path_text(folder)));
        }
    }
    (folders, None)
}

/// The text of a file, if it is there, is text, and is not longer than [`MOST_FACT_BYTES`].
fn read_small(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(MOST_FACT_BYTES + 1)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() as u64 <= MOST_FACT_BYTES).then_some(text)
}
