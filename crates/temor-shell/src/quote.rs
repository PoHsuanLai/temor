//! Quoting text for a POSIX shell.

/// `text` in single quotes for `sh`: a quote inside it closes the string, is written as `\'`,
/// and opens it again, so nothing in the text is ever read as syntax.
pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::sh_quote;

    #[test]
    fn text_is_quoted_so_that_nothing_in_it_is_syntax() {
        let cases = [
            ("plain", "'plain'"),
            ("", "''"),
            ("a b", "'a b'"),
            ("it's", "'it'\\''s'"),
            ("$HOME `x` \"y\" \\z", "'$HOME `x` \"y\" \\z'"),
            ("'", "''\\'''"),
        ];
        for (text, want) in cases {
            assert_eq!(sh_quote(text), want, "{text:?}");
        }
    }
}
