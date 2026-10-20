//! The process's environment, read once at start (never `set_var`), and the directories temor
//! reads and writes: the settings files under the config directory, and shell shims under the
//! runtime directory (never under HOME).

use std::ffi::OsString;
use std::path::PathBuf;

use ds_settings::Store;
use temor_plugins::DataDirs;
use temor_shell::ChildEnv;

use super::session_file::resolve_state_dir;
use crate::settings::{APP, root_for};

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
        let config_home = xdg_dir(&env, "XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        // Never the shared temporary directory: shims written there could be planted by others.
        // Without a runtime directory, temor keeps its own under the cache directory.
        let runtime_dir = match xdg_dir(&env, "XDG_RUNTIME_DIR") {
            Some(runtime) => runtime.join("temor"),
            None => xdg_dir(&env, "XDG_CACHE_HOME")
                .unwrap_or_else(|| home.join(".cache"))
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

    /// The config home (`XDG_CONFIG_HOME`, else `~/.config`): where the desktop's own settings
    /// are, such as KDE's `kdeglobals`.
    pub fn config_home(&self) -> PathBuf {
        self.config_dir
            .parent()
            .map_or_else(|| self.home.join(".config"), std::path::Path::to_path_buf)
    }

    /// The old settings file's path (`terminal.conf`), which is converted once and kept as
    /// `terminal.conf.old`.
    pub fn old_settings_path(&self) -> PathBuf {
        self.config_dir.join("terminal.conf")
    }

    /// The store `settings.toml` lives in.
    pub fn settings_store(&self) -> Store {
        Store::new(root_for(&self.config_dir), APP)
    }

    /// The folder `settings.toml` and `keys.conf` are in: temor's config folder.
    pub fn settings_dir(&self) -> PathBuf {
        self.settings_store()
            .dir()
            .unwrap_or_else(|| self.config_dir.clone())
    }

    /// The key changes' file (`keys.conf`).
    pub fn keys_path(&self) -> PathBuf {
        self.settings_dir().join("keys.conf")
    }

    /// `XDG_DATA_HOME`, else `~/.local/share`.
    fn data_home(&self) -> PathBuf {
        xdg_dir(&self.env, "XDG_DATA_HOME")
            .unwrap_or_else(|| self.home.join(".local").join("share"))
    }

    /// The folder the session file and its snapshots live in: `$XDG_STATE_HOME/temor`, else
    /// `~/.local/state/temor`. `None` only when there is no home (no saving then).
    pub fn state_dir(&self) -> Option<PathBuf> {
        let state_home = xdg_dir(&self.env, "XDG_STATE_HOME");
        resolve_state_dir(state_home.as_deref(), Some(&self.home))
    }

    /// Where plugin manifests are looked for: the person's data directory, then the system's.
    /// The one place the plugin folders read the environment.
    pub fn plugin_dirs(&self) -> DataDirs {
        let get = |key: &str| {
            self.env
                .get(key)
                .and_then(|v| v.to_str())
                .map(str::to_owned)
        };
        DataDirs::from_env(&self.home, &get)
    }

    /// temor's own data directory: outside every repository, where agent worktrees live.
    pub fn data_dir(&self) -> PathBuf {
        self.data_home().join("temor")
    }

    /// The viewer's store, shared with anyview's own windows so both keep one history of where
    /// each file was left (anyview's `STORE_FOLDER` under the data home; the store locks across
    /// processes).
    pub fn viewer_store(&self) -> PathBuf {
        self.data_home().join(anyview_pane::STORE_FOLDER)
    }

    /// Where the downloads shelf keeps its files: temor's own data directory, never the home
    /// folder (PLAN §9).
    pub fn shelf_dir(&self) -> PathBuf {
        self.data_dir().join("shelf")
    }

    /// Where files that arrive from a program are saved: the person's Downloads folder. The
    /// `XDG_DOWNLOAD_DIR` variable wins, then the `user-dirs.dirs` file under the config
    /// directory (`user_dirs` is its text, read by the caller), then `~/Downloads`.
    pub fn downloads_dir(&self, user_dirs: Option<&str>) -> PathBuf {
        let from_env = xdg_dir(&self.env, "XDG_DOWNLOAD_DIR");
        let from_file = user_dirs.and_then(|text| user_dir(text, "XDG_DOWNLOAD_DIR", &self.home));
        match (from_env, from_file) {
            (Some(dir), _) => dir,
            (None, Some(dir)) => dir,
            (None, None) => self.home.join("Downloads"),
        }
    }

    /// The `user-dirs.dirs` file the Downloads folder may be named in.
    pub fn user_dirs_path(&self) -> PathBuf {
        self.config_dir
            .parent()
            .map_or_else(|| self.home.join(".config"), PathBuf::from)
            .join("user-dirs.dirs")
    }

    /// The folder a pane stages the files a program sends before they are shelved.
    pub fn staging_dir(&self, pane: u64) -> PathBuf {
        self.shelf_dir()
            .join("incoming")
            .join(format!("pane-{pane}"))
    }
}

/// A folder named by an XDG variable: set, not empty and absolute, as the XDG spec says. A
/// relative value is ignored, so every folder temor derives from the environment agrees on it.
fn xdg_dir(env: &ChildEnv, key: &str) -> Option<PathBuf> {
    env.get(key)
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
}

/// One `NAME="$HOME/Downloads"` line of `user-dirs.dirs`: the folder, with `$HOME` filled in.
/// Only an absolute result counts.
fn user_dir(text: &str, name: &str, home: &std::path::Path) -> Option<PathBuf> {
    let line = text.lines().map(str::trim).find(|line| {
        line.strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('='))
    })?;
    let value = line.split_once('=')?.1.trim().trim_matches('"');
    let path = match value.strip_prefix("$HOME") {
        Some(rest) => PathBuf::from(format!("{}{rest}", home.display())),
        None => PathBuf::from(value),
    };
    path.is_absolute().then_some(path)
}
