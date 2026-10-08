//! Text newtypes: each names what its string means, so a title is never passed where a path is
//! wanted. Paths are kept as text because the core does no I/O; the edges turn them into
//! `PathBuf`s.

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

/// Defines a transparent `String` newtype with the derives stored text carries.
macro_rules! text_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// The text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

text_newtype!(
    /// A tab's title: from OSC 0/2, else the command plus the cwd, else what the person typed.
    TabTitle
);
text_newtype!(
    /// The command line of a block, as the shell ran it.
    CommandText
);
text_newtype!(
    /// The value of one environment variable in a Space's profile.
    EnvValue
);
text_newtype!(
    /// A path to an SSH private key a Space's agent holds.
    KeyPath
);
text_newtype!(
    /// A path relative to the project root (a dotenv file).
    RelPath
);
text_newtype!(
    /// The absolute folder a Space's new tabs open in.
    ProjectRoot
);
text_newtype!(
    /// What a route matches: an `ssh prod-*` pattern or a path prefix.
    RoutePattern
);
text_newtype!(
    /// The command a pinned tab runs again when temor starts.
    Relaunch
);
text_newtype!(
    /// The name an agent is shown by (`agents.toml` labels).
    AgentLabel
);
text_newtype!(
    /// An agent's own short message about its state.
    Note
);
text_newtype!(
    /// The first error line a failed trial run printed.
    ErrorLine
);
text_newtype!(
    /// A plugin's id, as bayonet's discovery names it.
    PluginId
);
text_newtype!(
    /// A file a viewer pane shows.
    ViewerRef
);
text_newtype!(
    /// A host's alias: the `Host` name in `~/.ssh/config`, or what was typed in the bar.
    HostAlias
);
text_newtype!(
    /// What `ssh` connects to: `user@host:port` or an alias it resolves.
    SshTarget
);
text_newtype!(
    /// A host's name as OSC 7 reports it.
    HostName
);
text_newtype!(
    /// A path on a remote host.
    RemotePath
);
text_newtype!(
    /// Where a download sits on the shelf.
    ShelfPath
);
text_newtype!(
    /// The content hash that names a helper build.
    HelperHash
);
text_newtype!(
    /// The working directory a pane is in (OSC 7).
    Cwd
);

/// The name of one environment variable: non-empty, with no `=` and no NUL, so it can always be
/// written into an environment block.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EnvKey(String);

impl EnvKey {
    /// `name` as an environment key, or why it cannot be one.
    pub fn new(name: impl Into<String>) -> Result<EnvKey, CoreError> {
        let name = name.into();
        if name.is_empty() || name.contains(['=', '\0']) {
            return Err(CoreError::BadEnvKey(name));
        }
        Ok(EnvKey(name))
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EnvKey {
    type Error = CoreError;
    fn try_from(name: String) -> Result<EnvKey, CoreError> {
        EnvKey::new(name)
    }
}

impl From<EnvKey> for String {
    fn from(key: EnvKey) -> String {
        key.0
    }
}
