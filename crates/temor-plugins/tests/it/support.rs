//! A scratch data directory for the host called `temor`, and the fake plugin installed in it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use temor_plugins::{Catalog, DataDirs, Launcher, Plugin, Waits};

/// Every capability of the fake, as a manifest lists them.
pub const ALL: &str = r#"
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

/// The example `fake_plugin`, built beside the tests.
pub fn fake_program() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let program = exe
        .parent()
        .and_then(Path::parent)
        .map(|profile| profile.join("examples").join("fake_plugin"))
        .unwrap();
    assert!(
        program.is_file(),
        "{} is missing: run the tests with `cargo test`, which builds the examples",
        program.display()
    );
    program
}

/// Waits short enough for a test of a hang, long enough for a loaded machine to start a program.
pub fn quick() -> Waits {
    Waits {
        hello: Duration::from_secs(10),
        silence: Duration::from_millis(500),
        cancel_grace: Duration::from_millis(300),
    }
}

pub fn launcher() -> Launcher {
    Launcher::new(quick(), |_| {})
}

pub struct Host {
    root: tempfile::TempDir,
}

impl Host {
    pub fn new() -> Host {
        Host {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn folder(&self) -> PathBuf {
        self.root.path().join("user").join("temor").join("plugins")
    }

    /// Writes `<id>.toml` for the fake, started with `args`, providing `provides`.
    pub fn install(&self, id: &str, args: &[&str], provides: &str) {
        let text = format!(
            "id = \"{id}\"\nname = \"Plugin {id}\"\nprotocol = 1\n[program]\npath = {:?}\nargs = {:?}\n{provides}",
            fake_program().display().to_string(),
            args
        );
        self.write(&format!("{id}.toml"), &text);
    }

    pub fn write(&self, file: &str, text: &str) {
        std::fs::create_dir_all(self.folder()).unwrap();
        std::fs::write(self.folder().join(file), text).unwrap();
    }

    pub fn catalog(&self) -> Catalog {
        Catalog::discover(&DataDirs {
            user: self.root.path().join("user"),
            system: Vec::new(),
        })
    }

    /// The plugin `id`, installed and usable.
    pub fn plugin(&self, id: &str) -> Plugin {
        self.catalog()
            .plugins()
            .into_iter()
            .find(|plugin| plugin.id() == id)
            .unwrap()
    }
}
