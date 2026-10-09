//! Writing the shims, and the one-call path from a shell to a [`SpawnSpec`].

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::plan::{ShellStart, ShimFile, plan_integration};
use super::resolve::Shell;
use crate::env::ChildEnv;
use crate::error::PtyError;
use crate::pty::SpawnSpec;
use crate::size::GridSize;

/// Writes `files`, creating their directories. A file already holding the same contents is
/// left alone; a changed one is replaced through a rename, so a shell starting in another pane
/// never reads half a file.
pub fn write_shims(files: &[ShimFile]) -> Result<(), PtyError> {
    files.iter().try_for_each(|file| {
        write_one(file).map_err(|source| PtyError::Shims {
            dir: file
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            source,
        })
    })
}

fn write_one(file: &ShimFile) -> io::Result<()> {
    if fs::read(&file.path).is_ok_and(|old| old == file.contents.as_bytes()) {
        return Ok(());
    }
    let dir = file.path.parent().unwrap_or(Path::new("."));
    create_private_dir(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(file.contents.as_bytes())?;
    temp.persist(&file.path).map_err(|error| error.error)?;
    Ok(())
}

/// Creates `dir` and its parents for this user alone: mode 0700 on Unix, so no other user can
/// plant a shim in it. A directory that exists keeps its mode.
fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// Plans the integration for `shell`, writes its shims under `runtime_dir`, and returns what
/// to spawn in `cwd` at `size`.
pub fn prepare_shell(
    shell: &Shell,
    start: ShellStart,
    env: ChildEnv,
    runtime_dir: &Path,
    cwd: PathBuf,
    size: GridSize,
) -> Result<SpawnSpec, PtyError> {
    let plan = plan_integration(shell, start, env, runtime_dir);
    write_shims(&plan.files)?;
    Ok(SpawnSpec {
        program: plan.program,
        args: plan.args,
        cwd,
        env: plan.env,
        size,
    })
}
