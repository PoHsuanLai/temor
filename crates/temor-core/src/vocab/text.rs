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

impl CommandText {
    /// A command as a shell reported it, made safe to type into a prompt. A line break (LF or CR)
    /// refuses it: the result is empty, which `typable` rejects. Other control characters are
    /// dropped, a tab becomes a space, and trailing blanks are cut. Nothing from the shell can
    /// submit a command through this text (PLAN §6, D6).
    pub fn from_shell(raw: &str) -> Self {
        if raw.contains(['\n', '\r']) {
            return Self(String::new());
        }
        let text: String = raw
            .chars()
            .map(|c| if c == '\t' { ' ' } else { c })
            .filter(|c| !c.is_control() && !is_bidi_control(*c))
            .collect();
        Self(text.trim_end().to_owned())
    }

    /// Whether the command may be typed into a prompt: it has text and no control character.
    pub fn typable(&self) -> bool {
        !self.0.is_empty() && !self.0.chars().any(char::is_control)
    }
}

/// Text a program or a path may hand us for display: control characters and bidirectional
/// overrides removed, and at most `max` characters kept.
pub fn display_text(raw: &str, max: usize) -> String {
    raw.chars()
        .filter(|c| !c.is_control() && !is_bidi_control(*c))
        .take(max)
        .collect()
}

/// The Unicode Bidi_Control characters (the Arabic letter mark, the left and right marks, the
/// embeddings, overrides and isolates): they reorder what is shown. The one table for core.
pub(crate) fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::{CommandText, display_text};

    #[test]
    fn a_line_break_refuses_the_command() {
        assert!(!CommandText::from_shell("ls\nrm -rf x").typable());
        assert!(!CommandText::from_shell("ls\rrm").typable());
        assert_eq!(
            CommandText::from_shell("ls\nrm"),
            CommandText(String::new())
        );
    }

    #[test]
    fn other_control_characters_are_dropped_and_blanks_trimmed() {
        let text = CommandText::from_shell("echo\t\x03hi\x1b[0m  ");
        assert_eq!(text.0, "echo hi[0m");
        assert_eq!(CommandText::from_shell("a\u{061C}b").0, "ab");
        assert!(text.typable());
    }

    #[test]
    fn display_text_drops_controls_and_bidi_and_is_bounded() {
        assert_eq!(display_text("/a\u{202E}b\x07c", 100), "/abc");
        assert_eq!(display_text("abcdef", 3), "abc");
        assert_eq!(
            display_text("a\u{061C}b", 100),
            "ab",
            "the Arabic letter mark"
        );
    }
}
