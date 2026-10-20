//! The fake plugin's fixtures: where its example programs are, and the manifests written for
//! them. Strings and paths only; the plugin host itself is the tests' own business.

use std::path::{Path, PathBuf};

/// Every capability of the fake plugin, as a manifest lists them.
pub const ALL_CAPABILITIES: &str = r#"
[[provides]]
capability = "block.lens"
trigger = "kubectl get"

[[provides]]
capability = "open.provider"
trigger = "gh"

[[provides]]
capability = "agent.adapter"
agent = "claude"

[[provides]]
capability = "row.facts"
tables = ["folder"]
"#;

/// The example program `name`, built beside the running test (`target/<profile>/examples`).
/// Panics when it was not built.
pub fn example_program(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("the test's own path");
    let program = exe
        .parent()
        .and_then(Path::parent)
        .map(|profile| profile.join("examples").join(name))
        .expect("a target folder");
    assert!(
        program.is_file(),
        "{} is missing: build the examples (`cargo test --workspace` does)",
        program.display()
    );
    program
}

/// A manifest for plugin `id` that starts `program` with `args`, followed by `provides`.
pub fn manifest_for(id: &str, program: &Path, args: &[&str], provides: &str) -> String {
    format!(
        "id = \"{id}\"\nname = \"Plugin {id}\"\nprotocol = 1\n[program]\npath = {:?}\nargs = {:?}\n{provides}",
        program.display().to_string(),
        args
    )
}

/// The shipped manifest `shipped` with its `path =` line pointing at `program`.
pub fn with_program(shipped: &str, program: &Path) -> String {
    shipped
        .lines()
        .map(|line| match line.starts_with("path = ") {
            true => format!("path = {:?}", program.display().to_string()),
            false => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
