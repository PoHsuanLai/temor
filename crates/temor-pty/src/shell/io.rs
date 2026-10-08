//! Writing the shims, and the one-call path from a shell to a [`SpawnSpec`].

use std::fs;
use std::io;
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
    fs::create_dir_all(dir)?;
    let name = file.path.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&temp, file.contents)?;
    fs::rename(&temp, &file.path)
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
