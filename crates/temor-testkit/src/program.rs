//! Finding a program, and the one rule for a program that is missing.

use std::path::PathBuf;

/// Whether a missing tool fails the test. The gate sets `TEMOR_REQUIRE_TOOLS=1`; any other value,
/// `0` included, lets the test skip.
pub fn tools_required() -> bool {
    std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|value| value == "1")
}

/// The first executable file called `name` on the test process's PATH.
pub fn find_program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

/// Says that `test` cannot run because `what` is missing: fails under `TEMOR_REQUIRE_TOOLS=1`,
/// else prints a `SKIPPED` line (the gate looks for that word).
pub fn skip(what: &str, test: &str) {
    assert!(
        !tools_required(),
        "{what} is required (TEMOR_REQUIRE_TOOLS=1) but is missing ({test})"
    );
    eprintln!("SKIPPED: {test}: {what} is not available");
}

/// The program `name`, or [`skip`] and `None`. Use it for everything a gate machine has.
pub fn program_or_skip(name: &str, test: &str) -> Option<PathBuf> {
    let found = find_program(name);
    if found.is_none() {
        skip(name, test);
    }
    found
}

/// Every program in `names`, in order, or [`skip`] (naming the first missing one) and `None`.
pub fn programs_or_skip(names: &[&str], test: &str) -> Option<Vec<PathBuf>> {
    let mut found = Vec::new();
    for name in names {
        found.push(program_or_skip(name, test)?);
    }
    Some(found)
}

/// The program `name`, or a printed `SKIPPED` line and `None`, even under `TEMOR_REQUIRE_TOOLS=1`.
/// Use it for extras the gate machine need not have, such as bash 3.2 or fish 3.
pub fn optional_program(name: &str, test: &str) -> Option<PathBuf> {
    let found = find_program(name);
    if found.is_none() {
        eprintln!("SKIPPED: {test}: {name} is not installed (optional)");
    }
    found
}
