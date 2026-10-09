//! Writing the shims into the runtime directory.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use super::plan::ShimFile;
use super::resolve::UserId;
use crate::error::ShellError;

/// Writes `files`, creating their directories. A file already holding the same contents is
/// left alone; a changed one is replaced through a rename, so a shell starting in another pane
/// never reads half a file. The folder must belong to `owner`; `None` skips that check.
pub fn write_shims(files: &[ShimFile], owner: Option<UserId>) -> Result<(), ShellError> {
    files.iter().try_for_each(|file| {
        write_one(file, owner).map_err(|source| ShellError::Shims {
            dir: file
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            source,
        })
    })
}

fn write_one(file: &ShimFile, owner: Option<UserId>) -> io::Result<()> {
    if fs::read(&file.path).is_ok_and(|old| old == file.contents.as_bytes()) {
        return Ok(());
    }
    let dir = file.path.parent().unwrap_or(Path::new("."));
    create_private_dir(dir, owner)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(file.contents.as_bytes())?;
    temp.persist(&file.path).map_err(|error| error.error)?;
    Ok(())
}

/// Creates `dir` and its parents for this user alone: mode 0700 on Unix, so no other user can
/// plant a shim in it. An existing directory must be a real directory owned by this user (it is
/// refused otherwise), and it is tightened to 0700. Where the owner is not known, the check is
/// skipped. // TODO(platform:macos), TODO(platform:windows): owner check there.
fn create_private_dir(dir: &Path, owner: Option<UserId>) -> io::Result<()> {
    if fs::symlink_metadata(dir).is_err() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
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
    Ok(())
}
