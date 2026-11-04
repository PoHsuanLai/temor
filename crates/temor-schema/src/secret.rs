//! What is a secret, and so never kept (PLAN §2b): a variable whose name says so (`*_TOKEN`,
//! `*_KEY`, `*PASSWORD*`, credentials), a value that looks like one (a URL with a password in it,
//! a well-known token), and a line of shell that sets or holds one. A shell's state keeps such a
//! variable's name only, and history never keeps such a command.
//!
//! These are rules, not shapes, and live in the schema all the same: temor-store (L1) refuses to
//! write a command [`kept_in_history`] turns down, so the rule must sit below the store, and the
//! shell-state code above it uses the same table.

/// Whether a command may be kept in history: not one typed with a space first (the shells' way to
/// keep a command out of history), and not one that sets a secret or holds something that looks
/// like one.
pub fn kept_in_history(command: &str) -> bool {
    !command.starts_with(' ') && !holds_secret(command)
}

/// Words that make a name a secret wherever they stand in it.
const SECRET_WORDS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PASSPHRASE",
    "CREDENTIAL",
    "PRIVATE",
    "APIKEY",
    "API_KEY",
    "ACCESS_KEY",
];

/// Parts of a name (between underscores) that make it a secret: `DB_PASS`, `GITLAB_PAT`,
/// `SENTRY_DSN`, `NPM_AUTH`. As parts only, so `MY_PATH` and `AUTHOR` are not.
const SECRET_PARTS: &[&str] = &["PASS", "PWD", "PAT", "DSN", "AUTH"];

/// Whether a variable's name says its value is a secret.
pub fn is_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|word| upper.contains(word))
        || upper.split('_').any(|part| SECRET_PARTS.contains(&part))
        || upper.ends_with("_KEY")
}

/// Starts of well-known tokens (GitHub, GitLab, OpenAI and others, Slack); a word that starts
/// with one and goes on for 16 or more token characters is taken for a token.
const TOKEN_PREFIXES: &[&str] = &[
    "ghp_",
    "gho_",
    "ghs_",
    "ghu_",
    "github_pat_",
    "glpat-",
    "sk-",
    "xoxb-",
    "xoxp-",
];

/// Starts of AWS access key ids, which go on with 16 capitals or digits.
const AWS_PREFIXES: &[&str] = &["AKIA", "ASIA"];

fn token_like(word: &str) -> bool {
    let tail_is = |prefix: &str, ok: fn(u8) -> bool| {
        word.strip_prefix(prefix)
            .is_some_and(|rest| rest.len() >= 16 && rest.bytes().all(ok))
    };
    TOKEN_PREFIXES
        .iter()
        .any(|p| tail_is(p, |b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        || AWS_PREFIXES
            .iter()
            .any(|p| tail_is(p, |b| b.is_ascii_uppercase() || b.is_ascii_digit()))
}

/// Whether `text` has a URL with a password in it (`scheme://user:password@host`).
fn url_with_password(text: &str) -> bool {
    text.match_indices("://").any(|(at, _)| {
        let rest = &text[at + 3..];
        let authority = rest
            .split(|c: char| c == '/' || c == '?' || c == '#' || c.is_whitespace())
            .next()
            .unwrap_or("");
        authority
            .rsplit_once('@')
            .is_some_and(|(user, _)| user.contains(':'))
    })
}

/// Whether a value looks like a secret whatever its name: a URL with a password in it, or a
/// word that looks like a well-known token.
pub fn looks_secret(value: &str) -> bool {
    url_with_password(value)
        || value
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .any(token_like)
}

/// Whether a line of shell (an alias's value, a command) holds a secret: it sets a variable
/// whose name is a secret's (`export DB_PASSWORD=...`), or something in it looks like one.
pub fn holds_secret(line: &str) -> bool {
    looks_secret(line)
        || line
            .split(|c: char| c.is_whitespace() || ";&|()`".contains(c))
            .filter_map(|word| word.split_once('='))
            .any(|(name, _)| is_variable_name(name) && is_secret(name))
}

fn is_variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
