//! What a shell tells temor about itself besides its prompt marks: the state a new shell would
//! need to look like this one (exported variables, aliases). The integration scripts send it as
//! one OSC after each prompt; this is the format, written once, pure, and read by both sides.
//!
//! ```text
//! ESC ] 7503 ; v1 { ; e=NAME=VALUE | ; a=NAME=VALUE } ST
//! ```
//!
//! `e` is an exported variable and `a` an alias. Every NAME and VALUE is percent-encoded (so a
//! `;`, `=` or newline in a value cannot break the line). A tag this version does not know is
//! skipped, so a newer script can say more than an older temor reads. A whole report is a
//! snapshot, not a delta: the receiver diffs it against the last one.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use crate::osc7::percent_decode_strict;

/// The OSC number the report travels in.
pub const OSC_NUMBER: &str = "7503";

/// The most bytes of a report that are read. A longer one is refused whole: a half-read
/// environment would be worse than none.
pub const MAX_REPORT_BYTES: usize = 64 * 1024;

const VERSION: &str = "v1";

/// What stays unescaped in a name or a value.
const KEEP: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'.')
    .remove(b'_')
    .remove(b'-')
    .remove(b'~')
    .remove(b'/');

/// One report: the shell's exported variables and aliases at a prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
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
    /// An entry is not `tag=NAME=VALUE` with valid escapes.
    Malformed,
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

/// The payload after `ESC ] 7503 ;`: what a script writes.
pub fn encode(report: &Report) -> String {
    let mut out = VERSION.to_owned();
    for (tag, entries) in [('e', &report.env), ('a', &report.aliases)] {
        for (name, value) in entries {
            out.push_str(&format!(";{tag}={}={}", escape(name), escape(value)));
        }
    }
    out
}

/// The report in a payload (the text after `ESC ] 7503 ;`).
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
        let list = match tag {
            "e" => &mut report.env,
            "a" => &mut report.aliases,
            // A tag a newer script sends: skipped, not an error.
            _ => continue,
        };
        let (name, value) = rest.split_once('=').ok_or(ReportFault::Malformed)?;
        let name = unescape(name)?;
        if name.is_empty() {
            return Err(ReportFault::Malformed);
        }
        list.push((name, unescape(value)?));
    }
    Ok(report)
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
