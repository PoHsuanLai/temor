//! A directory only this user can write (PLAN §6, shell integration): made 0700, and refused
//! when an existing one belongs to someone else or is a link.

use std::fs;
use std::io;
use std::path::Path;

/// Makes `dir` for this user alone and checks it: it must be a real directory owned by the
/// current user. Its mode is set to 0700. Other systems have no ownership check yet.
// TODO(platform:macos), TODO(platform:windows): ACL-based check for the owner-only directory.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        if !dir.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
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
// TODO(platform:macos), TODO(platform:windows): owner-only file creation on those systems.
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
