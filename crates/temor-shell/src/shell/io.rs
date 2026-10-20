//! Writing the shims into the runtime directory.

use std::fs;
use std::io::Write;
use std::path::Path;

use super::plan::ShimFile;
use super::resolve::UserId;
use crate::error::ShellError;
use crate::platform::ensure_private_dir;

/// Writes `files`, creating their directories for this user alone. A file already holding the
/// same contents is left alone; a changed one is replaced through a rename, so a shell starting
/// in another pane never reads half a file. The folder must be this user's own (the check
/// compares with the process's own user, so `_owner`, which callers still pass, is not read).
pub fn write_shims(files: &[ShimFile], _owner: Option<UserId>) -> Result<(), ShellError> {
    files.iter().try_for_each(write_one)
}

fn write_one(file: &ShimFile) -> Result<(), ShellError> {
    if fs::read(&file.path).is_ok_and(|old| old == file.contents.as_bytes()) {
        return Ok(());
    }
    let dir = file.path.parent().unwrap_or(Path::new("."));
    create_private_dirs(dir)?;
    let shims = |source| ShellError::Shims {
        dir: dir.to_path_buf(),
        source,
    };
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(shims)?;
    temp.write_all(file.contents.as_bytes()).map_err(shims)?;
    temp.persist(&file.path).map_err(|error| shims(error.error))?;
    Ok(())
}

/// Creates `dir` and the parents it lacks for this user alone (so no other user can plant a shim
/// in them), and checks and tightens `dir` itself if it was already there.
fn create_private_dirs(dir: &Path) -> Result<(), ShellError> {
    let mut chain: Vec<&Path> = dir
        .ancestors()
        .take_while(|path| *path == dir || fs::symlink_metadata(path).is_err())
        .filter(|path| !path.as_os_str().is_empty())
        .collect();
    chain.reverse();
    chain.into_iter().try_for_each(ensure_private_dir)
}
