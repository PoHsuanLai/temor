//! `Host` patterns and which blocks apply to an alias.

use super::glob::{glob_match, is_wild};

/// Whether a pattern selects or rules out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Polarity {
    Select,
    /// Written with a leading `!`: a match here excludes the whole block.
    Exclude,
}

/// One token of a `Host` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostPattern {
    pub polarity: Polarity,
    pub glob: String,
}

impl HostPattern {
    pub(super) fn parse(token: &str) -> Self {
        match token.strip_prefix('!') {
            Some(rest) => Self {
                polarity: Polarity::Exclude,
                glob: rest.to_owned(),
            },
            None => Self {
                polarity: Polarity::Select,
                glob: token.to_owned(),
            },
        }
    }

    /// The name this token stands for, when it names exactly one host: a plain selecting
    /// pattern with no wildcard.
    pub(super) fn concrete(&self) -> Option<&str> {
        let plain = self.polarity == Polarity::Select && !is_wild(&self.glob);
        plain.then_some(self.glob.as_str())
    }
}

/// Which alias a block applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Scope {
    /// Lines before the first `Host` line: they apply to every alias.
    Global,
    /// A `Host` line. With no selecting pattern it applies to nothing.
    Hosts(Vec<HostPattern>),
    /// A `Match` block, which is never evaluated.
    Skipped,
}

impl Scope {
    /// OpenSSH's rule: some pattern selects the alias and no `!` pattern matches it.
    pub(super) fn matches(&self, alias: &str) -> bool {
        match self {
            Self::Global => true,
            Self::Skipped => false,
            Self::Hosts(patterns) => {
                let mut selected = false;
                for pattern in patterns {
                    if glob_match(&pattern.glob, alias) {
                        match pattern.polarity {
                            Polarity::Exclude => return false,
                            Polarity::Select => selected = true,
                        }
                    }
                }
                selected
            }
        }
    }
}
