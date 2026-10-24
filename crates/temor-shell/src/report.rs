//! What a shell tells temor about itself besides its prompt marks: the state a new shell would
//! need to look like this one (exported variables, aliases). The integration scripts send it as
//! one OSC after each prompt; this is the format, written once, pure, and read by both sides.
//!
//! ```text
//! ESC ] 7503 ; v2 ; k=KEY { ; e=NAME=VALUE | ; a=NAME=VALUE } ST
//! ```
//!
//! `k` is the report key: temor gives each shell it starts a random one in [`KEY_VAR`], the
//! script moves it into an unexported shell variable before the person's rc runs, and a report
//! whose key is not that shell's is dropped ([`read`]). So text a program prints (a file shown, a
//! download, a host's output) cannot pass for the shell, and a shell started inside the pane
//! (which has no key) does not report. `e` is an exported variable and `a` an alias. Every NAME
//! and VALUE is percent-encoded (so a `;`, `=` or newline in a value cannot break the line). A
//! tag this version does not know is skipped, so a newer script can say more than an older temor
//! reads. A whole report is a snapshot, not a delta: the receiver diffs it against the last one.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use crate::osc7::percent_decode_strict;

/// The OSC number the report travels in.
pub const OSC_NUMBER: &str = "7503";

/// The most bytes of a report that are read. A longer one is refused whole: a half-read
/// environment would be worse than none.
pub const MAX_REPORT_BYTES: usize = 64 * 1024;

const VERSION: &str = "v2";

/// The variable a shell finds its report key in. The scripts unset it at load, so programs the
/// shell runs never see it.
pub const KEY_VAR: &str = "TEMOR_REPORT_KEY";

/// What stays unescaped in a name or a value.
const KEEP: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'.')
    .remove(b'_')
    .remove(b'-')
    .remove(b'~')
    .remove(b'/');

/// The key one shell signs its reports with: 32 hex digits, made from two random words by the
/// caller (this crate reads no randomness).
#[derive(Clone, PartialEq, Eq)]
pub struct ReportKey(String);

impl ReportKey {
    /// The key made of two random words.
    pub fn from_words(high: u64, low: u64) -> Self {
        Self(format!("{high:016x}{low:016x}"))
    }

    /// A key read back (from a checkpoint or a spawn environment): 32 hex digits, else none.
    pub fn parse(text: &str) -> Option<Self> {
        (text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| Self(text.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `other` is this key. Every byte is compared, however early they differ.
    pub fn matches(&self, other: &str) -> bool {
        let (ours, theirs) = (self.0.as_bytes(), other.as_bytes());
        ours.len() == theirs.len()
            && ours
                .iter()
                .zip(theirs)
                .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                == 0
    }
}

impl std::fmt::Debug for ReportKey {
    // The key is not shown in logs or test output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReportKey(..)")
    }
}

/// One report: the shell's exported variables and aliases at a prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The key the report was sent with; empty when it had none.
    pub key: String,
    pub env: Vec<(String, String)>,
    pub aliases: Vec<(String, String)>,
}

/// Why a report was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportFault {
    /// Longer than [`MAX_REPORT_BYTES`].
    TooLarge,
    /// Not a version this build reads.
    Version,
    /// An entry is not `tag=NAME=VALUE` with valid escapes, or a name is not one a shell gives.
    Malformed,
    /// The report has no key, or not the key of the shell temor started.
    Key,
}

/// What a shell's script can say, so a conformance check asks no more of it than that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportCaps {
    pub env: bool,
    pub aliases: bool,
}

fn escape(text: &str) -> String {
    utf8_percent_encode(text, KEEP).to_string()
}

fn unescape(text: &str) -> Result<String, ReportFault> {
    let bytes = percent_decode_strict(text).ok_or(ReportFault::Malformed)?;
    String::from_utf8(bytes).map_err(|_| ReportFault::Malformed)
}

/// Whether `name` is a variable name: `[A-Za-z_][A-Za-z0-9_]*`.
fn variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Whether `name` can be an alias name: not empty, and no NUL, `=`, `/` or white space.
fn alias_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c == '\0' || c == '=' || c == '/' || c.is_whitespace())
}

/// The payload after `ESC ] 7503 ;`: what a script writes.
pub fn encode(report: &Report) -> String {
    let mut out = format!("{VERSION};k={}", escape(&report.key));
    for (tag, entries) in [('e', &report.env), ('a', &report.aliases)] {
        for (name, value) in entries {
            out.push_str(&format!(";{tag}={}={}", escape(name), escape(value)));
        }
    }
    out
}

/// The report in a payload (the text after `ESC ] 7503 ;`), if it carries `key`: what the window
/// takes from a shell.
pub fn read(payload: &str, key: &ReportKey) -> Result<Report, ReportFault> {
    let report = decode(payload)?;
    match key.matches(&report.key) {
        true => Ok(report),
        false => Err(ReportFault::Key),
    }
}

/// The report in a payload (the text after `ESC ] 7503 ;`), whatever key it has.
pub fn decode(payload: &str) -> Result<Report, ReportFault> {
    if payload.len() > MAX_REPORT_BYTES {
        return Err(ReportFault::TooLarge);
    }
    let mut parts = payload.split(';');
    if parts.next() != Some(VERSION) {
        return Err(ReportFault::Version);
    }
    let mut report = Report::default();
    for part in parts.filter(|part| !part.is_empty()) {
        let (tag, rest) = part.split_once('=').ok_or(ReportFault::Malformed)?;
        if tag == "k" {
            report.key = unescape(rest)?;
            continue;
        }
        let (list, good_name): (_, fn(&str) -> bool) = match tag {
            "e" => (&mut report.env, variable_name),
            "a" => (&mut report.aliases, alias_name),
            // A tag a newer script sends: skipped, not an error.
            _ => continue,
        };
        let (name, value) = rest.split_once('=').ok_or(ReportFault::Malformed)?;
        let (name, value) = (unescape(name)?, unescape(value)?);
        if !good_name(&name) || value.contains('\0') {
            return Err(ReportFault::Malformed);
        }
        list.push((name, value));
    }
    Ok(report)
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
