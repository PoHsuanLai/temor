//! A directory only this user can write (PLAN §6, shell integration): made 0700, and refused
//! when an existing one belongs to someone else or is a link.

use std::fs;
use std::io;
use std::path::Path;

/// Makes `dir` (and the parents it lacks) for this user alone and checks it: it must be a real
/// directory owned by the current user. Its mode is set to 0700. The check is temor-shell's; a
/// system it has no check for yet falls back to [`unchecked_private_dir`].
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    match temor_shell::ensure_private_dir_all(dir) {
        Err(error) if error.kind() == io::ErrorKind::Unsupported => unchecked_private_dir(dir),
        done => done,
    }
}

/// Where temor-shell has no private-folder check yet: the folder is made 0700 and must be this
/// user's own. Other systems than unix have no ownership check.
// TODO(platform:windows): move onto temor-shell's check when it has one.
fn unchecked_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        if !dir.exists() {
            fs::DirBuilder::new().mode(0o700).create(dir)?;
        }
        let meta = fs::symlink_metadata(dir)?;
        let owner = rustix::process::getuid().as_raw();
        if !meta.is_dir() || meta.uid() != owner {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the folder is not this user's own",
            ));
        }
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

/// Writes `bytes` to `path` so that a reader sees the old file or the new one, never a part:
/// a temporary file beside it, made owner-only (0600), synced, then renamed over `path`.
// TODO(platform:windows): owner-only file creation there.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("json.tmp");
    let mut file = create_private(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

#[cfg(unix)]
fn create_private(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> io::Result<fs::File> {
    fs::File::create(path)
}
