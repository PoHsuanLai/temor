//! Writing a folder with `~` for the person's home. Pure: the edge passes the home in.

/// `text` with a leading `~` (alone, or before a `/`) written as `home`; any other text as it is.
/// `~user` is not a home to this, and stays.
pub fn expand_home(text: &str, home: &str) -> String {
    match text.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{home}{rest}"),
        _ => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::expand_home;

    #[test]
    fn only_a_leading_tilde_alone_or_before_a_slash_is_the_home() {
        let table = [
            ("~", "/h"),
            ("~/work", "/h/work"),
            ("~/", "/h/"),
            ("~bob/work", "~bob/work"),
            ("/a/~/b", "/a/~/b"),
            ("work", "work"),
            ("", ""),
        ];
        for (text, want) in table {
            assert_eq!(expand_home(text, "/h"), want, "{text}");
        }
    }
}
