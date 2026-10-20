//! Writing the shims into the runtime directory.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use super::plan::ShimFile;
use super::resolve::UserId;
use crate::error::ShellError;
use crate::platform::ensure_private_dir;

/// Writes `files`, creating their directories for this user alone. A file already holding the
/// same contents is left alone; a changed one is replaced through a rename, so a shell starting
/// in another pane never reads half a file. The folder must be this user's own: where the system
/// has the full check that is the process's own user, elsewhere `owner`, when given.
pub fn write_shims(files: &[ShimFile], owner: Option<UserId>) -> Result<(), ShellError> {
    files.iter().try_for_each(|file| write_one(file, owner))
}

fn write_one(file: &ShimFile, owner: Option<UserId>) -> Result<(), ShellError> {
    if fs::read(&file.path).is_ok_and(|old| old == file.contents.as_bytes()) {
        return Ok(());
    }
    let dir = file.path.parent().unwrap_or(Path::new("."));
    create_private_dirs(dir, owner)?;
    let shims = |source| ShellError::Shims {
        dir: dir.to_path_buf(),
        source,
    };
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(shims)?;
    temp.write_all(file.contents.as_bytes()).map_err(shims)?;
    temp.persist(&file.path)
        .map_err(|error| shims(error.error))?;
    Ok(())
}

/// Creates `dir` and the parents it lacks for this user alone (so no other user can plant a shim
/// in them), and checks and tightens `dir` itself if it was already there.
fn create_private_dirs(dir: &Path, owner: Option<UserId>) -> Result<(), ShellError> {
    let mut chain: Vec<&Path> = dir
        .ancestors()
        .take_while(|path| *path == dir || fs::symlink_metadata(path).is_err())
        .filter(|path| !path.as_os_str().is_empty())
        .collect();
    chain.reverse();
    chain
        .into_iter()
        .try_for_each(|path| match ensure_private_dir(path) {
            Err(ShellError::Unsupported { .. }) => {
                plain_private_dir(path, owner).map_err(|source| ShellError::PrivateDir {
                    path: path.to_path_buf(),
                    source,
                })
            }
            other => other,
        })
}

/// Where the system has no full check yet (see `platform`): the folder is made 0700, must be a
/// folder and, when `owner` is known, theirs, and is tightened to 0700. Shims still work there.
fn plain_private_dir(dir: &Path, owner: Option<UserId>) -> io::Result<()> {
    if fs::symlink_metadata(dir).is_err() {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(dir)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta = fs::symlink_metadata(dir)?;
        if !meta.is_dir() || owner.is_some_and(|user| user.0 != meta.uid()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the shim folder is not this user's own",
            ));
        }
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = owner;
    Ok(())
}
