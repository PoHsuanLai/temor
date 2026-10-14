//! Where the desktop's own shortcut settings come from (PLAN §6): keycap on our own desktop (its
//! daemon, else the machine's and the person's keymap files), kdeglobals on Plasma, a dump of
//! dconf on GNOME, the platform's conventions alone everywhere else.
//!
//! Each file source also watches its file, as chordkit's `KeymapSource::watch` asks of a source
//! that can: a thread looks at the file's modified time and calls back when it moves, and the
//! window loads the keymap again (`hub::keys`). Dropping the `Watch` stops the thread.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, SystemTime};

use chordkit::{
    ChangeCallback, ConventionSource, Desktop, Keymap, KeymapSource, Platform, SourceError, Watch,
};
use chordkit_gnome::GnomeSource;
use chordkit_kde::KdeSource;
use keycap_client::KeycapSource;

use crate::platform::{ensure_private_dir, write_private_file};

/// How often the watched file's modified time is looked at.
pub const LOOK_EVERY: Duration = Duration::from_secs(2);

/// A source that can be shared with the thread that loads the keymap again.
pub type SharedSource = Arc<dyn KeymapSource + Send + Sync>;

/// Which of the desktop's settings stores feeds the keymap on a platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemKeys {
    /// The platform has no settings of its own that temor reads: its conventions alone.
    Conventions,
    /// Our desktop: keycap's keymap, with the person's file under `config_home`. keycap's source
    /// watches for itself (the daemon says when the keymap changes).
    Keycap { config_home: PathBuf },
    /// Plasma: the shortcuts the person changed are in `kdeglobals`.
    Kde { kdeglobals: PathBuf },
    /// GNOME: the shortcuts live in the dconf database; `dconf dump /` is saved to `dump` for
    /// chordkit to read, and the database file is what is watched.
    Gnome { database: PathBuf, dump: PathBuf },
}

impl SystemKeys {
    /// The store for `platform`, with the config home (`~/.config`) and the runtime folder
    /// (where the dump is kept, for this user alone) named by the caller.
    pub fn choose(platform: Platform, config_home: &Path, runtime_dir: &Path) -> Self {
        match platform {
            Platform::Linux {
                desktop: Desktop::Ours,
            } => Self::Keycap {
                config_home: config_home.to_owned(),
            },
            Platform::Linux {
                desktop: Desktop::Kde,
            } => Self::Kde {
                kdeglobals: config_home.join("kdeglobals"),
            },
            Platform::Linux {
                desktop: Desktop::Gnome,
            } => Self::Gnome {
                database: config_home.join("dconf").join("user"),
                dump: runtime_dir.join("keys").join("dconf.dump"),
            },
            _ => Self::Conventions,
        }
    }

    /// The source that loads (and watches) this store.
    pub fn source(&self) -> SharedSource {
        match self {
            Self::Conventions => Arc::new(ConventionSource),
            Self::Keycap { config_home } => {
                let config_home = config_home.display().to_string();
                // The socket and the machine's file as keycap names them (its own variables, else
                // its defaults); the person's file under the config home temor was given.
                Arc::new(KeycapSource::from_lookup(move |name| match name {
                    "XDG_CONFIG_HOME" => Some(config_home.clone()),
                    _ => std::env::var(name).ok(),
                }))
            }
            Self::Kde { kdeglobals } => Arc::new(Polled {
                inner: KdeSource::new(kdeglobals),
                watched: kdeglobals.clone(),
                every: LOOK_EVERY,
            }),
            Self::Gnome { database, dump } => Arc::new(Polled {
                inner: Dconf {
                    gnome: GnomeSource::new(dump),
                    dump: dump.clone(),
                },
                watched: database.clone(),
                every: LOOK_EVERY,
            }),
        }
    }
}

/// GNOME's source: the dump is made again before every load, so a change is read as it is.
struct Dconf {
    gnome: GnomeSource,
    dump: PathBuf,
}

impl Dconf {
    /// Saves `dconf dump /` to the dump file. No `dconf` program, or no database yet, leaves
    /// GNOME's conventions as they are.
    fn refresh(&self) -> Result<(), SourceError> {
        let read = |reason: String| SourceError::Read {
            place: self.dump.display().to_string(),
            reason,
        };
        let out = match Command::new("dconf")
            .args(["dump", "/"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        {
            Ok(out) if out.status.success() => out,
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(read(err.to_string())),
        };
        if let Some(dir) = self.dump.parent() {
            ensure_private_dir(dir).map_err(|err| read(err.to_string()))?;
        }
        write_private_file(&self.dump, &out.stdout).map_err(|err| read(err.to_string()))
    }
}

impl KeymapSource for Dconf {
    fn load(&self, platform: Platform) -> Result<Keymap, SourceError> {
        self.refresh()?;
        self.gnome.load(platform)
    }
}

/// A source whose settings are one file: loads through `inner`, and watches by looking at the
/// file's modified time every `every`.
struct Polled<S> {
    inner: S,
    watched: PathBuf,
    every: Duration,
}

impl<S: KeymapSource> KeymapSource for Polled<S> {
    fn load(&self, platform: Platform) -> Result<Keymap, SourceError> {
        self.inner.load(platform)
    }

    fn watch(&self, on_change: ChangeCallback) -> Result<Watch, SourceError> {
        watch_file(&self.watched, self.every, on_change)
    }
}

/// Calls `on_change` whenever the file at `path` is changed, made or removed, looking at its
/// modified time every `every`, until the returned `Watch` is dropped. The first look is taken
/// now, not on the thread, so a change made before the thread first runs still counts.
pub fn watch_file(
    path: &Path,
    every: Duration,
    on_change: ChangeCallback,
) -> Result<Watch, SourceError> {
    let (stop, alive) = channel::<()>();
    let path = path.to_path_buf();
    let seen = modified(&path);
    std::thread::Builder::new()
        .name("temor-keys-watch".to_owned())
        .spawn(move || look(&path, seen, every, &alive, &*on_change))
        .map_err(|err| {
            SourceError::Unsupported(format!(
                "couldn't watch the shortcut settings for changes: {err}"
            ))
        })?;
    // Dropping the sender ends the thread at its next look.
    Ok(Watch::holding(stop))
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// Calls `on_change` whenever `path`'s modified time (or its being there) differs from the last
/// look (`seen` at first), until `alive` is closed.
fn look(
    path: &Path,
    mut seen: Option<SystemTime>,
    every: Duration,
    alive: &Receiver<()>,
    on_change: &dyn Fn(),
) {
    loop {
        match alive.recv_timeout(every) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = modified(path);
        if now != seen {
            seen = now;
            on_change();
        }
    }
}

#[cfg(test)]
#[path = "keymap_source_tests.rs"]
mod tests;
