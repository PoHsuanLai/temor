//! temor-ssh's integration tests: the bootstrap run by a real `sh` under a scratch HOME, and the
//! argv parsed by the system `ssh` with `-G` (which never connects). No test contacts a host and
//! none reads the real `~/.ssh`. The harness uses unix APIs (symlinks, file modes), so these
//! modules exist only on unix; the library itself has no such gate.

#[cfg(unix)]
mod bootstrap_run;
#[cfg(unix)]
mod edges;
#[cfg(unix)]
mod harness;
#[cfg(unix)]
mod shells;
#[cfg(unix)]
mod ssh_parse;

#[test]
fn every_module_in_this_directory_is_declared() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/it");
    let main = std::fs::read_to_string(dir.join("main.rs")).expect("main.rs");
    for entry in std::fs::read_dir(&dir).expect("tests/it") {
        let path = entry.expect("entry").path();
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "main" {
            continue;
        }
        let is_module = path.is_dir() || path.extension().is_some_and(|e| e == "rs");
        if is_module {
            let decl = format!("mod {name};");
            assert!(
                main.contains(&decl),
                "tests/it/{name} is not declared in main.rs"
            );
        }
    }
}
