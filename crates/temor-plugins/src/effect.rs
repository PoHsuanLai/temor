//! What choosing a plugin's row does. The bar's own actions are the only ones: copy, open and
//! nothing (PLAN D14, plugins.md §4). A plugin cannot ask for anything else, and what it does ask
//! for is checked here before the app carries it out.

use crate::message::{Activation, OpenTarget};

/// The most text a plugin may put on the clipboard.
pub const MOST_COPIED_BYTES: usize = 64 * 1024;

/// What the app does for a chosen row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Put this text on the clipboard.
    Copy(String),
    /// Open this file or folder (an absolute path).
    OpenFile(String),
    /// Open this address (http or https).
    OpenUrl(String),
    /// Nothing.
    Nothing,
}

/// Why an activation was refused. The app logs it and does nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// More text than a clipboard entry from a plugin may hold.
    #[error("the plugin asked to copy {0} bytes")]
    TooMuchText(usize),
    /// A path that is not absolute, or has a control character.
    #[error("the plugin asked to open a path that is not a plain absolute path")]
    BadPath,
    /// An address that is not http or https.
    #[error("the plugin asked to open an address that is not a web address")]
    BadUrl,
}

/// The effect of `activation`, or why it is refused.
pub fn effect_of(activation: Activation) -> Result<Effect, Refused> {
    match activation {
        Activation::Nothing => Ok(Effect::Nothing),
        Activation::Copy { text } if text.len() > MOST_COPIED_BYTES => {
            Err(Refused::TooMuchText(text.len()))
        }
        Activation::Copy { text } => Ok(Effect::Copy(text)),
        Activation::Open {
            target: OpenTarget::File(path),
        } => match path.starts_with('/') && !path.chars().any(char::is_control) {
            true => Ok(Effect::OpenFile(path)),
            false => Err(Refused::BadPath),
        },
        Activation::Open {
            target: OpenTarget::Url(url),
        } => match web_address(&url) {
            true => Ok(Effect::OpenUrl(url)),
            false => Err(Refused::BadUrl),
        },
    }
}

/// Whether `url` is an http or https address with a host and no control character or blank.
fn web_address(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        !rest.is_empty()
            && !rest.starts_with('/')
            && !url.chars().any(|c| c.is_control() || c.is_whitespace())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(text: &str) -> Activation {
        Activation::Copy {
            text: text.to_owned(),
        }
    }

    fn file(path: &str) -> Activation {
        Activation::Open {
            target: OpenTarget::File(path.to_owned()),
        }
    }

    fn url(address: &str) -> Activation {
        Activation::Open {
            target: OpenTarget::Url(address.to_owned()),
        }
    }

    #[test]
    fn a_plugin_may_copy_open_or_do_nothing_and_what_it_names_is_checked() {
        let huge = "x".repeat(MOST_COPIED_BYTES + 1);
        let table: Vec<(&str, Activation, Result<Effect, Refused>)> = vec![
            ("nothing", Activation::Nothing, Ok(Effect::Nothing)),
            ("copy", copy("42"), Ok(Effect::Copy("42".to_owned()))),
            (
                "copy too much",
                copy(&huge),
                Err(Refused::TooMuchText(huge.len())),
            ),
            (
                "a file",
                file("/tmp/a b.txt"),
                Ok(Effect::OpenFile("/tmp/a b.txt".to_owned())),
            ),
            ("a relative path", file("a.txt"), Err(Refused::BadPath)),
            (
                "a path with a newline",
                file("/tmp/a\nb"),
                Err(Refused::BadPath),
            ),
            (
                "https",
                url("https://example.org/x"),
                Ok(Effect::OpenUrl("https://example.org/x".to_owned())),
            ),
            (
                "http",
                url("http://example.org"),
                Ok(Effect::OpenUrl("http://example.org".to_owned())),
            ),
            (
                "a program's scheme",
                url("ssh://host"),
                Err(Refused::BadUrl),
            ),
            (
                "a file scheme",
                url("file:///etc/passwd"),
                Err(Refused::BadUrl),
            ),
            ("no host", url("https:///x"), Err(Refused::BadUrl)),
            ("a blank", url("https://a b"), Err(Refused::BadUrl)),
        ];
        for (why, activation, want) in table {
            assert_eq!(effect_of(activation), want, "{why}");
        }
    }
}
