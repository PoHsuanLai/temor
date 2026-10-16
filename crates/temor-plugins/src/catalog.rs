//! The plugins that are installed, and which of them serve a request.

use std::path::{Path, PathBuf};

use bayonet::registry::{Fit, Installed, Registry};
use bayonet::{Search, discover};

use crate::capability::{Cap, Provision};

/// The newest protocol version temor speaks.
pub const PROTOCOL: u32 = 1;

/// The name plugins are installed under: `<data dir>/temor/plugins/<id>.toml`.
pub const APP: &str = "temor";

/// Where manifests are looked for: the person's data directory, then the system's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirs {
    /// `$XDG_DATA_HOME`, else `~/.local/share`.
    pub user: PathBuf,
    /// `$XDG_DATA_DIRS`, else `/usr/local/share:/usr/share`; absolute entries only.
    pub system: Vec<PathBuf>,
}

impl DataDirs {
    /// The directories for a person at `home` whose environment `get` reads. Pure: the caller
    /// passes the environment in.
    pub fn from_env(home: &Path, get: &dyn Fn(&str) -> Option<String>) -> DataDirs {
        let set = |key: &str| get(key).filter(|value| !value.is_empty());
        let user = set("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .unwrap_or_else(|| home.join(".local").join("share"));
        let system = set("XDG_DATA_DIRS")
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned())
            .split(':')
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .collect();
        DataDirs { user, system }
    }
}

/// A plugin that can be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    installed: Installed<Provision>,
}

impl Plugin {
    /// Its id (`kubectx`).
    pub fn id(&self) -> &str {
        self.installed.manifest.id.as_str()
    }

    /// What a person calls it.
    pub fn name(&self) -> &str {
        &self.installed.manifest.name
    }

    pub(crate) fn installed(&self) -> &Installed<Provision> {
        &self.installed
    }
}

/// A provider and the query it was sent to: the typed text without the provider's trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    /// The provider.
    pub plugin: Plugin,
    /// What it is asked.
    pub text: String,
}

/// Every installed plugin, and a line for each manifest or program that could not be used.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    registry: Registry<Provision>,
    problems: Vec<String>,
}

impl Catalog {
    /// No plugins.
    pub fn empty() -> Catalog {
        Catalog::default()
    }

    /// Finds the plugins in `dirs`. Nothing here fails: a manifest that is not usable is a line
    /// in [`Catalog::problems`].
    pub fn discover(dirs: &DataDirs) -> Catalog {
        let search = Search::new(APP, &dirs.user, &dirs.system, PROTOCOL);
        let found = discover::<Provision>(&search);
        let mut problems: Vec<String> = found
            .rejected
            .iter()
            .map(|rejected| format!("{}: {}", rejected.file.display(), rejected.error))
            .collect();
        problems.extend(
            found
                .registry
                .unusable()
                .iter()
                .map(|unusable| format!("{}: {}", unusable.id.as_str(), unusable.reason)),
        );
        Catalog {
            registry: found.registry,
            problems,
        }
    }

    /// Lines saying why a plugin was set aside, for the log.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Every usable plugin, in serving order.
    pub fn plugins(&self) -> Vec<Plugin> {
        self.registry.installed().iter().map(wrap).collect()
    }

    /// The plugins that know `table`'s rows (`row.facts`).
    pub fn facts_sources(&self, table: &str) -> Vec<Plugin> {
        self.with(|entry| match entry {
            Provision::RowFacts(tables) if tables.iter().any(|t| t == table) => Some(()),
            Provision::RowFacts(_)
            | Provision::BlockLens(_)
            | Provision::OpenProvider(_)
            | Provision::AgentAdapter(_) => None,
        })
        .into_iter()
        .map(|(plugin, ())| plugin)
        .collect()
    }

    /// The lenses that draw the block of `command` (`block.lens`).
    pub fn lenses(&self, command: &str) -> Vec<Plugin> {
        self.with(|entry| match entry {
            Provision::BlockLens(trigger) if trigger.starts(command) => Some(()),
            Provision::BlockLens(_)
            | Provision::OpenProvider(_)
            | Provision::AgentAdapter(_)
            | Provision::RowFacts(_) => None,
        })
        .into_iter()
        .map(|(plugin, ())| plugin)
        .collect()
    }

    /// The providers a query goes to, each with the text it is asked (`open.provider`). A
    /// provider with a trigger gets only the queries that start with it.
    pub fn providers(&self, typed: &str) -> Vec<Routed> {
        self.with(|entry| match entry {
            Provision::OpenProvider(trigger) => trigger.route(typed),
            Provision::BlockLens(_) | Provision::AgentAdapter(_) | Provision::RowFacts(_) => None,
        })
        .into_iter()
        .map(|(plugin, text)| Routed { plugin, text })
        .collect()
    }

    /// The adapter for `agent` (`agent.adapter`): the first in serving order that names it.
    pub fn adapter(&self, agent: &str) -> Option<Plugin> {
        self.registry
            .serving(Cap::AgentAdapter, |entry| match entry {
                Provision::AgentAdapter(named) if named == agent => Fit::Exact,
                Provision::AgentAdapter(_)
                | Provision::BlockLens(_)
                | Provision::OpenProvider(_)
                | Provision::RowFacts(_) => Fit::Miss,
            })
            .map(wrap)
    }

    /// The plugins in serving order with what `pick` takes from their entries; a plugin has at
    /// most one entry for each capability, so each appears once.
    fn with<T>(&self, pick: impl Fn(&Provision) -> Option<T>) -> Vec<(Plugin, T)> {
        self.registry
            .installed()
            .iter()
            .filter_map(|installed| {
                let taken = installed.manifest.provides.iter().find_map(&pick)?;
                Some((wrap(installed), taken))
            })
            .collect()
    }
}

fn wrap(installed: &Installed<Provision>) -> Plugin {
    Plugin {
        installed: installed.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_directories_follow_the_xdg_rules_and_ignore_what_is_not_absolute() {
        let home = Path::new("/home/me");
        type Env = &'static [(&'static str, &'static str)];
        let table: &[(&str, Env, &str, &[&str])] = &[
            (
                "defaults",
                &[],
                "/home/me/.local/share",
                &["/usr/local/share", "/usr/share"],
            ),
            (
                "set",
                &[("XDG_DATA_HOME", "/d/home"), ("XDG_DATA_DIRS", "/a:/b")],
                "/d/home",
                &["/a", "/b"],
            ),
            (
                "relative entries are dropped",
                &[("XDG_DATA_HOME", "rel"), ("XDG_DATA_DIRS", "x:/b::")],
                "/home/me/.local/share",
                &["/b"],
            ),
            (
                "empty means unset",
                &[("XDG_DATA_HOME", ""), ("XDG_DATA_DIRS", "")],
                "/home/me/.local/share",
                &["/usr/local/share", "/usr/share"],
            ),
        ];
        for (why, env, user, system) in table {
            let get = |key: &str| {
                env.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| (*v).to_owned())
            };
            let dirs = DataDirs::from_env(home, &get);
            assert_eq!(dirs.user, PathBuf::from(user), "{why}: user");
            let want: Vec<PathBuf> = system.iter().map(PathBuf::from).collect();
            assert_eq!(dirs.system, want, "{why}: system");
        }
    }
}
