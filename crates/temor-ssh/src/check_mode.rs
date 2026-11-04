//! Tailscale SSH check mode (PLAN §9b, D22). Before the shell starts, a host that wants the
//! person to confirm in a browser prints a line with a `https://login.tailscale.com/…` link.
//! [`CheckScan`] finds it in the pre-shell output, as data only: the window shows a "Confirm
//! it's you in your browser" row with an Open button. Nothing here opens the link.

/// The only place a check link may point.
const PREFIX: &str = "https://login.tailscale.com/";
/// Longest link accepted.
const MAX_URL_BYTES: usize = 2048;
/// Longest unfinished line kept between chunks.
const MAX_PENDING_BYTES: usize = 4096;

/// A check link, always `https://login.tailscale.com/…` with only URL characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CheckUrl(String);

impl CheckUrl {
    /// Validates a link: the exact Tailscale login host, a path of URL characters, and nothing
    /// a terminal or shell would treat specially.
    pub fn new(text: &str) -> Option<Self> {
        let rest = text.strip_prefix(PREFIX)?;
        let url_char = |c: char| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '-' | '.' | '_' | '~' | '/' | '?' | '&' | '=' | '%' | '+' | '#'
                )
        };
        let valid = text.len() <= MAX_URL_BYTES && !rest.is_empty() && rest.chars().all(url_char);
        valid.then(|| Self(text.to_owned()))
    }

    /// The link.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Scans a connection's output until the shell starts, a chunk at a time (a link may be split
/// across chunks). Stop feeding it at the first prompt mark or working-directory report.
/// A scan belongs to one thread; the caller drives it.
#[derive(Debug, Clone, Default)]
pub struct CheckScan {
    pending: String,
    last: Option<CheckUrl>,
}

impl CheckScan {
    /// A fresh scan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds output bytes and returns the links the lines they completed hold: the host wants the
    /// person to confirm in a browser first. The same link repeated right after itself (a redraw)
    /// is reported once.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<CheckUrl> {
        self.pending.push_str(&String::from_utf8_lossy(bytes));
        let Some(end) = self.pending.rfind(['\n', '\r']) else {
            if self.pending.len() > MAX_PENDING_BYTES {
                self.pending.clear();
            }
            return Vec::new();
        };
        let rest = self.pending.split_off(end + 1);
        let complete = std::mem::replace(&mut self.pending, rest);
        let mut urls = Vec::new();
        for url in complete.lines().filter_map(url_in_line) {
            if self.last.as_ref() != Some(&url) {
                self.last = Some(url.clone());
                urls.push(url);
            }
        }
        urls
    }

    /// Reports a link on a last line the host never ended with a newline, then resets.
    pub fn finish(&mut self) -> Vec<CheckUrl> {
        let mut line = std::mem::take(&mut self.pending);
        line.push('\n');
        self.feed(line.as_bytes())
    }
}

/// The check link in one line of output: the first word that starts with the Tailscale login
/// address, after escape sequences are dropped and closing punctuation trimmed.
fn url_in_line(line: &str) -> Option<CheckUrl> {
    let plain = strip_escapes(line);
    plain
        .split_whitespace()
        .filter_map(|word| word.find(PREFIX).map(|at| &word[at..]))
        .find_map(|word| CheckUrl::new(word.trim_end_matches(['.', ',', ')', ']', '>', '"', '\''])))
}

/// `text` without CSI and OSC escape sequences (colours, cursor moves, hyperlinks).
fn strip_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                // A CSI sequence ends at a byte in 0x40..=0x7e.
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                // An OSC sequence ends at BEL or ESC \.
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{CheckScan, CheckUrl};

    fn urls(found: Vec<CheckUrl>) -> Vec<String> {
        found.into_iter().map(|u| u.as_str().to_owned()).collect()
    }

    #[test]
    fn the_link_is_found_in_the_lines_tailscale_prints() {
        // Name, output, links found.
        let cases = [
            (
                "the usual two lines",
                "# Tailscale SSH requires an additional check.\r\n# To authenticate, visit: https://login.tailscale.com/a/abc123\r\n",
                vec!["https://login.tailscale.com/a/abc123"],
            ),
            (
                "colour codes and a full stop around it",
                "\x1b[1mvisit: \x1b[4mhttps://login.tailscale.com/a/x_y-1?z=2\x1b[0m.\n",
                vec!["https://login.tailscale.com/a/x_y-1?z=2"],
            ),
            (
                "a hyperlink escape wrapping the text",
                "\x1b]8;;https://login.tailscale.com/a/h\x1b\\open\x1b]8;;\x1b\\ https://login.tailscale.com/a/h\n",
                vec!["https://login.tailscale.com/a/h"],
            ),
            (
                "a look-alike host is not a check",
                "visit https://login.tailscale.com.evil.example/a/x or http://login.tailscale.com/a/y\n",
                vec![],
            ),
            ("no link, no event", "Welcome to Ubuntu\n", vec![]),
        ];
        for (name, output, want) in cases {
            let mut scan = CheckScan::new();
            assert_eq!(urls(scan.feed(output.as_bytes())), want, "{name}");
        }
    }

    #[test]
    fn a_link_split_across_chunks_and_redrawn_is_reported_once() {
        let mut scan = CheckScan::new();
        assert!(scan.feed(b"visit: https://login.tail").is_empty());
        assert!(scan.feed(b"scale.com/a/split").is_empty());
        let got = urls(scan.feed(b"1\nvisit: https://login.tailscale.com/a/split1\n"));
        assert_eq!(got, vec!["https://login.tailscale.com/a/split1"]);
        let last_line_unfinished = urls({
            let mut scan = CheckScan::new();
            scan.feed(b"go to https://login.tailscale.com/a/tail");
            scan.finish()
        });
        assert_eq!(
            last_line_unfinished,
            vec!["https://login.tailscale.com/a/tail"]
        );
    }

    #[test]
    fn urls_with_anything_unusual_are_refused() {
        for text in [
            "https://login.tailscale.com/",
            "https://login.tailscale.com/a/\u{7}b",
            "https://login.tailscale.com/a/b c",
            "https://login.tailscale.com/a/$(x)",
            "https://login.tailscale.com@evil.example/a",
        ] {
            assert!(CheckUrl::new(text).is_none(), "{text}");
        }
    }
}
