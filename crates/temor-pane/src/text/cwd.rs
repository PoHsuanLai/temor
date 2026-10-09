//! The working directory a shell reports with OSC 7: a `file://host/path` URL (PLAN §7).

/// The path in a `file://` URL, percent-decoded. The host is dropped: the pane's shell is local
/// or remote, and the path is what the person reads. `None` for another scheme, a URL with no
/// path, or bytes that are not UTF-8 once decoded.
pub fn cwd_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    percent_decode(path)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::cwd_from_url;

    #[test]
    fn a_file_url_gives_its_path_decoded() {
        assert_eq!(cwd_from_url("file://host/home/me"), Some("/home/me".into()));
        assert_eq!(cwd_from_url("file:///tmp/a%20b"), Some("/tmp/a b".into()));
        assert_eq!(cwd_from_url("file://h/%E2%9C%93"), Some("/✓".into()));
    }

    #[test]
    fn other_urls_and_broken_escapes_give_nothing() {
        assert_eq!(cwd_from_url("https://example.com/"), None);
        assert_eq!(cwd_from_url("file://host"), None);
        assert_eq!(cwd_from_url("file://h/%zz"), None);
        assert_eq!(cwd_from_url("file://h/%FF"), None);
    }
}
