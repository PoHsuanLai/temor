//! The working directory a program reports (OSC 7) as a path.

/// The path in a `file://host/path` URL, percent-decoded. `None` for anything else, or a path
/// that is not UTF-8.
pub(super) fn parse_cwd_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    let mut bytes = Vec::with_capacity(path.len());
    let mut chars = path.bytes();
    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let high = hex(chars.next()?)?;
            let low = hex(chars.next()?)?;
            bytes.push((high << 4) | low);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_become_paths_or_nothing() {
        let cases = [
            ("file://host/home/me", Some("/home/me")),
            ("file:///srv/a%20b/%C3%A9", Some("/srv/a b/é")),
            ("file://host", None),
            ("http://host/x", None),
            ("file://host/bad%2", None),
            ("file://host/bad%zz", None),
            ("file://host/%FF", None),
        ];
        for (url, want) in cases {
            assert_eq!(parse_cwd_url(url).as_deref(), want, "{url}");
        }
    }
}
