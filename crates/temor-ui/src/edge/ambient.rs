//! The process's environment, read once at start (never `set_var`), and the directories temor
//! reads and writes: the settings file under the config directory, and shell shims under the
//! runtime directory (never under HOME).

use std::ffi::OsString;
use std::path::PathBuf;

use temor_pty::ChildEnv;

/// What the app needs from its surroundings.
#[derive(Debug, Clone, PartialEq)]
pub struct Ambient {
    /// The person's home directory, for the shell's start and the settings lookup.
    pub home: PathBuf,
    /// The environment a new shell starts from (before `base_env` drops what describes
    /// another terminal).
    pub env: ChildEnv,
    /// Where the settings file is looked for (`terminal.conf` inside it).
    pub config_dir: PathBuf,
    /// Where shell integration files are written.
    pub runtime_dir: PathBuf,
}

impl Ambient {
    /// Reads the process environment. `None` when there is no home directory to start in.
    pub fn from_process() -> Option<Self> {
        Self::from_vars(std::env::vars_os())
    }

    /// Builds the surroundings from an environment (the process's, or a test's).
    pub fn from_vars(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Option<Self> {
        let env: ChildEnv = vars.into_iter().collect();
        let home = PathBuf::from(env.get("HOME").filter(|h| !h.is_empty())?);
        let config_home = env
            .get("XDG_CONFIG_HOME")
            .filter(|d| !d.is_empty())
            .map_or_else(|| home.join(".config"), PathBuf::from);
        // Never the shared temporary directory: shims written there could be planted by others.
        // Without a runtime directory, temor keeps its own under the cache directory.
        let runtime_dir = match env.get("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
            Some(runtime) => PathBuf::from(runtime).join("temor"),
            None => env
                .get("XDG_CACHE_HOME")
                .filter(|d| !d.is_empty())
                .map_or_else(|| home.join(".cache"), PathBuf::from)
                .join("temor")
                .join("run"),
        };
        Some(Self {
            config_dir: config_home.join("temor"),
            runtime_dir,
            home,
            env,
        })
    }

    /// The settings file's path.
    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("terminal.conf")
    }

    /// Where the downloads shelf keeps its files: temor's own data directory, never the home
    /// folder (PLAN §9).
    pub fn shelf_dir(&self) -> PathBuf {
        let data_home = self
            .env
            .get("XDG_DATA_HOME")
            .filter(|d| !d.is_empty())
            .map_or_else(|| self.home.join(".local").join("share"), PathBuf::from);
        data_home.join("temor").join("shelf")
    }
}
