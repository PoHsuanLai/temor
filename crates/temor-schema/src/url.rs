//! `temor://` URLs (PLAN §2b): everything the window can show has one, as in a browser. A place
//! is a folder or a file on a computer, and may point into one session there, or one block of it:
//!
//! ```text
//! temor://local/Users/po/ml/            a folder on this computer
//! temor://box/srv/app/main.rs           a file on the host `box`
//! temor://local/Users/po/ml/#7f3a…      a session in that folder
//! temor://local/Users/po/ml/#7f3a…/12   one block of it
//! temor://history                       every session, by day
//! temor://history/7f3a…                 one session's commands
//! temor://history?in=/Users/po/ml/      the sessions in a folder
//! temor://downloads                     what was saved and dropped
//! ```
//!
//! A URL never holds a command or its output: a block is named by its number, so a link can be
//! passed on without what was typed. Paths are written as they are, with `%`, `#`, `?`, spaces,
//! control characters and text outside ASCII percent-encoded.

use std::fmt;
use std::str::FromStr;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

use crate::error::SchemaError;
use crate::ids::BlockId;
use crate::session::SessionId;

const SCHEME: &str = "temor://";
/// This computer's name in a URL: the same on every computer, so a link to a local folder means
/// the reader's own.
const LOCAL: &str = "local";
const HISTORY: &str = "history";
const DOWNLOADS: &str = "downloads";
/// Names a host cannot have in a URL: they name this computer and the pages.
const RESERVED: [&str; 3] = [LOCAL, HISTORY, DOWNLOADS];

/// What a path keeps encoded: what would end it or change what follows.
const IN_PATH: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');
/// What a query value keeps encoded: a path's, and what would start another value.
const IN_QUERY: &AsciiSet = &IN_PATH.add(b'&').add(b'=').add(b'+');

/// The computer a place is on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Host {
    /// This computer.
    Local,
    /// A host, by the name the person gave it.
    Named(String),
}

impl Host {
    /// A host by `name`: not empty, none of `/ # ? : @ %` or whitespace, and not a name the URL
    /// keeps for itself (`local`, `history`, `downloads`).
    pub fn named(name: &str) -> Result<Host, SchemaError> {
        let bad = name.is_empty()
            || RESERVED.contains(&name)
            || name
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "/#?:@%".contains(c));
        match bad {
            true => Err(SchemaError::BadUrl {
                why: "the host's name cannot be written",
                text: name.to_owned(),
            }),
            false => Ok(Host::Named(name.to_owned())),
        }
    }

    /// The host as a URL writes it: `local` for this computer, else its name.
    pub fn as_str(&self) -> &str {
        match self {
            Host::Local => LOCAL,
            Host::Named(name) => name,
        }
    }

    /// The host a URL wrote as `text`.
    pub fn parse(text: &str) -> Result<Host, SchemaError> {
        match text {
            LOCAL => Ok(Host::Local),
            name => Host::named(name),
        }
    }
}

/// Where in a place's sessions a URL points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Within {
    pub session: SessionId,
    /// One block of the session, by its number.
    pub block: Option<BlockId>,
}

/// A folder or file on a computer. A folder's path ends in `/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    pub host: Host,
    /// The absolute path, as the host writes it.
    pub path: String,
    pub within: Option<Within>,
}

impl Location {
    /// Whether the place is a folder.
    pub fn is_folder(&self) -> bool {
        self.path.ends_with('/')
    }
}

/// Which sessions the history page lists.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HistoryScope {
    /// Every session, by day.
    All,
    /// One session's commands.
    Session(SessionId),
    /// The sessions that ran in a folder (an absolute path) or under it.
    In(String),
}

/// Anything the window can show.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TemorUrl {
    Location(Location),
    History(HistoryScope),
    Downloads,
}

impl fmt::Display for TemorUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(SCHEME)?;
        match self {
            TemorUrl::Location(place) => {
                f.write_str(place.host.as_str())?;
                write!(f, "{}", utf8_percent_encode(&place.path, IN_PATH))?;
                if let Some(within) = place.within {
                    write!(f, "#{}", within.session)?;
                    if let Some(block) = within.block {
                        write!(f, "/{}", block.0)?;
                    }
                }
                Ok(())
            }
            TemorUrl::History(HistoryScope::All) => f.write_str(HISTORY),
            TemorUrl::History(HistoryScope::Session(id)) => write!(f, "{HISTORY}/{id}"),
            TemorUrl::History(HistoryScope::In(folder)) => {
                write!(f, "{HISTORY}?in={}", utf8_percent_encode(folder, IN_QUERY))
            }
            TemorUrl::Downloads => f.write_str(DOWNLOADS),
        }
    }
}

fn bad(why: &'static str, text: &str) -> SchemaError {
    SchemaError::BadUrl {
        why,
        text: text.to_owned(),
    }
}

/// `text` with its percent-encoding undone; it must decode to UTF-8.
fn decoded(part: &str, whole: &str) -> Result<String, SchemaError> {
    percent_decode_str(part)
        .decode_utf8()
        .map(|text| text.into_owned())
        .map_err(|_| bad("an encoded part is not UTF-8", whole))
}

fn parse_history(rest: &str, text: &str) -> Result<TemorUrl, SchemaError> {
    let scope = match rest {
        "" => HistoryScope::All,
        _ if rest.starts_with('/') => HistoryScope::Session(rest[1..].parse()?),
        _ => match rest.strip_prefix("?in=") {
            Some(folder) if folder.starts_with('/') => HistoryScope::In(decoded(folder, text)?),
            _ => {
                return Err(bad(
                    "the history page takes a session or ?in=<folder>",
                    text,
                ));
            }
        },
    };
    Ok(TemorUrl::History(scope))
}

fn parse_within(fragment: &str, text: &str) -> Result<Within, SchemaError> {
    let (session, block) = match fragment.split_once('/') {
        Some((session, block)) => {
            let block = block
                .parse::<u64>()
                .map_err(|_| bad("a block is named by its number", text))?;
            (session, Some(BlockId(block)))
        }
        None => (fragment, None),
    };
    Ok(Within {
        session: session.parse()?,
        block,
    })
}

impl FromStr for TemorUrl {
    type Err = SchemaError;

    fn from_str(text: &str) -> Result<TemorUrl, SchemaError> {
        let rest = text
            .strip_prefix(SCHEME)
            .ok_or_else(|| bad("it does not start with temor://", text))?;
        if let Some(after) = rest.strip_prefix(HISTORY)
            && (after.is_empty() || after.starts_with(['/', '?']))
        {
            return parse_history(after, text);
        }
        if rest == DOWNLOADS {
            return Ok(TemorUrl::Downloads);
        }
        let slash = rest
            .find('/')
            .ok_or_else(|| bad("a place needs a path after its host", text))?;
        let (host, rest) = rest.split_at(slash);
        let host = Host::parse(host).map_err(|_| bad("the host's name cannot be read", text))?;
        let (path, fragment) = match rest.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment)),
            None => (rest, None),
        };
        if path.contains('?') {
            return Err(bad("a place takes no query", text));
        }
        Ok(TemorUrl::Location(Location {
            host,
            path: decoded(path, text)?,
            within: fragment.map(|f| parse_within(f, text)).transpose()?,
        }))
    }
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
