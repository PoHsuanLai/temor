//! The working directory a shell reports with OSC 7: a `file://host/path` URL (PLAN §7). The
//! scripts in `shell/` write it; the panes, here and on the other side of an SSH connection, read
//! it with [`cwd_from_url`].

use percent_encoding::percent_decode_str;

/// The path in a `file://` URL, percent-decoded. The host is dropped: the pane's shell is local
/// or remote, and the path is what the person reads. `None` for another scheme, a URL with no
/// path, a broken escape, or bytes that are not UTF-8 once decoded.
pub fn cwd_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    String::from_utf8(percent_decode_strict(path)?).ok()
}

/// The bytes `text` stands for, or `None` unless every `%` is followed by two hex digits.
/// (`percent_encoding` leaves a broken escape in place, and `u8::from_str_radix` takes a sign
/// as a digit, which would turn `%+A` into a newline.)
pub fn percent_decode_strict(text: &str) -> Option<Vec<u8>> {
    let mut rest = text.as_bytes();
    while let Some(at) = rest.iter().position(|&b| b == b'%') {
        let digits = rest.get(at + 1..at + 3)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        rest = &rest[at + 3..];
    }
    Some(percent_decode_str(text).collect())
}

#[cfg(test)]
mod tests {
    use super::{cwd_from_url, percent_decode_strict};

    #[test]
    fn urls_become_paths_or_nothing() {
        let cases = [
            ("file://host/home/me", Some("/home/me")),
            ("file:///tmp/a%20b", Some("/tmp/a b")),
            ("file://h/%E2%9C%93", Some("/✓")),
            ("file:///srv/a%20b/%c3%a9", Some("/srv/a b/é")),
            ("https://example.com/", None),
            ("file://host", None),
            ("file://h/bad%2", None),
            ("file://h/%zz", None),
            ("file://h/%FF", None),
            ("file:///a%+A", None),
            ("file:///a%-1", None),
        ];
        for (url, want) in cases {
            assert_eq!(cwd_from_url(url).as_deref(), want, "{url}");
        }
    }

    #[test]
    fn a_broken_escape_is_refused_wherever_it_stands() {
        assert_eq!(percent_decode_strict("a%41b"), Some(b"aAb".to_vec()));
        for bad in ["%", "%4", "%+A", "%G1", "ok%20%"] {
            assert_eq!(percent_decode_strict(bad), None, "{bad}");
        }
    }
}
