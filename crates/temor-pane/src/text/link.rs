//! Which hyperlinks the pane may open (PLAN §6, §9): a program's OSC 8 target is only handed to
//! the desktop when its scheme is web or mail. Anything else (files, remote shells, custom
//! handlers) is refused with a reason the person can read. Pure.

/// A link the pane may open. Only the URL's text is kept, after the checks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LinkTarget(String);

impl LinkTarget {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a link was refused.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LinkRefusal {
    /// The scheme is not one the pane opens (`file`, `ssh`, a custom handler, or none).
    Scheme(String),
    /// The target has spaces or control characters, so it is not a plain URL.
    Malformed,
}

/// The verdict on a link's target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LinkVerdict {
    Open(LinkTarget),
    Refused(LinkRefusal),
}

/// The schemes the pane opens.
const ALLOWED: [&str; 3] = ["http", "https", "mailto"];

/// Checks a link target. The scheme is compared case-insensitively.
pub fn check_link(target: &str) -> LinkVerdict {
    if target.is_empty() || target.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return LinkVerdict::Refused(LinkRefusal::Malformed);
    }
    let scheme = match target.split_once(':') {
        Some((scheme, _)) if !scheme.is_empty() => scheme.to_ascii_lowercase(),
        _ => return LinkVerdict::Refused(LinkRefusal::Malformed),
    };
    if ALLOWED.contains(&scheme.as_str()) {
        LinkVerdict::Open(LinkTarget(target.to_owned()))
    } else {
        LinkVerdict::Refused(LinkRefusal::Scheme(scheme))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_links_open() {
        for target in [
            "https://example.com/a?b=1",
            "http://localhost:8080",
            "MAILTO:a@b.c",
        ] {
            assert!(
                matches!(check_link(target), LinkVerdict::Open(_)),
                "{target}"
            );
        }
    }

    #[test]
    fn other_schemes_are_refused_with_the_scheme_named() {
        assert_eq!(
            check_link("file:///etc/passwd"),
            LinkVerdict::Refused(LinkRefusal::Scheme("file".to_owned()))
        );
        assert_eq!(
            check_link("ssh://prod"),
            LinkVerdict::Refused(LinkRefusal::Scheme("ssh".to_owned()))
        );
        assert_eq!(
            check_link("x-handler:open"),
            LinkVerdict::Refused(LinkRefusal::Scheme("x-handler".to_owned()))
        );
    }

    #[test]
    fn malformed_targets_are_refused() {
        for target in [
            "",
            "example.com",
            "https://a b",
            "https://a\u{7}",
            ":x",
            "https:\n",
        ] {
            assert!(
                matches!(check_link(target), LinkVerdict::Refused(_)),
                "{target:?}"
            );
        }
    }
}
