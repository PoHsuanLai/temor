//! Reading a session's folder for the chips (PLAN P16, §9b): what git says about it and the
//! version files in it and above it. The helper only reads and sends text; the rules that turn
//! the text into chips are temor's own (`temor_core::facts`), so a pane on a host and a pane on
//! this computer follow the same ones.
//!
//! Git runs with no locks and no prompts, and the untracked list is read as it streams and cut
//! once it is past what the chip shows, so a folder with a million new files costs about a
//! thousand lines. Anything that cannot be read is left out.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use temor_wire::message::{
    FACT_FILE_NAMES, FactFile, FolderFacts, FolderFiles, GitText, MOST_FACT_BYTES,
    MOST_FACT_PARENTS, RemotePath,
};

/// Untracked entries are counted up to this many, then git is stopped (the chip shows `999+`).
const UNTRACKED_PAST: usize = 1000;

/// What there is to say about the folder `dir`; `home` is the helper's home folder, if it has one.
pub fn read_folder(dir: &Path, home: Option<&Path>) -> FolderFacts {
    let (folders, root) = read_versions(dir);
    let venv = dir.join(".venv");
    FolderFacts {
        dir: RemotePath(dir.to_string_lossy().into_owned()),
        git: read_git(dir),
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

fn git(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

/// What git printed, when it ran and succeeded.
fn git_text(dir: &Path, args: &[&str]) -> Option<String> {
    let out = git(dir, args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `None` outside a repository, or where git is missing.
fn read_git(dir: &Path) -> Option<GitText> {
    let head = git_text(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        // A repository with no commit has no `HEAD` to resolve, but it has a branch name.
        .or_else(|| git_text(dir, &["symbolic-ref", "--short", "HEAD"]))?;
    // A detached `HEAD` has no branch, so there is nothing to compare or count either.
    if head.trim() == "HEAD" {
        return Some(GitText {
            head: Some(head),
            ..GitText::default()
        });
    }
    Some(GitText {
        head: Some(head),
        shortstat: git_text(dir, &["diff", "--shortstat", "HEAD"]),
        untracked: read_untracked(dir),
    })
}

/// The `?? ` lines of git's status, read as they arrive.
fn read_untracked(dir: &Path) -> Option<String> {
    let args = ["status", "--porcelain=v1", "--untracked-files=normal"];
    let mut child = git(dir, &args).stdout(Stdio::piped()).spawn().ok()?;
    let (text, cut) = match child.stdout.take() {
        Some(pipe) => take_untracked(BufReader::new(pipe)),
        None => (String::new(), false),
    };
    finish(child, cut).then_some(text)
}

/// The untracked lines of `reader`, and whether it was stopped past [`UNTRACKED_PAST`].
fn take_untracked(mut reader: impl BufRead) -> (String, bool) {
    let mut text = String::new();
    let mut seen = 0;
    let mut line = Vec::new();
    while seen < UNTRACKED_PAST {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) if line.starts_with(b"?? ") => {
                text.push_str(&String::from_utf8_lossy(&line));
                seen += 1;
            }
            Ok(_) => {}
        }
    }
    (text, seen >= UNTRACKED_PAST)
}

/// Stops git if it was cut off, waits for it, and says whether the text it gave can be used.
fn finish(mut child: Child, cut: bool) -> bool {
    if cut {
        // Past the cap git still has work to do on a huge folder: stop it now.
        let _ = child.kill();
    }
    let status = child.wait();
    cut || status.is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untracked_lines_are_kept_and_the_read_stops_past_the_cap() {
        let few = " M kept\n?? a\n?? b/\nA  c\n";
        assert_eq!(
            take_untracked(few.as_bytes()),
            ("?? a\n?? b/\n".to_owned(), false)
        );
        let many = "?? f\n".repeat(UNTRACKED_PAST + 50);
        let (text, cut) = take_untracked(many.as_bytes());
        assert_eq!(text.lines().count(), UNTRACKED_PAST);
        assert!(cut);
    }
}
