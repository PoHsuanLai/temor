//! One config line into a keyword and its arguments. Lenient: the only failure is a quote that
//! never closes.

/// A keyword (lowercased, since keywords are case-insensitive) and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Line {
    pub keyword: String,
    pub args: Vec<String>,
}

/// A line that cannot be split into arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UnterminatedQuote;

/// Splits one line. `None` for blank lines and whole-line comments. The keyword ends at
/// whitespace or `=`; an optional `=` may follow it. Arguments split on whitespace, and a
/// double-quoted run keeps its spaces (inside quotes `\"` and `\\` are escapes).
pub(super) fn lex_line(line: &str) -> Result<Option<Line>, UnterminatedQuote> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }
    let keyword_end = trimmed
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(trimmed.len());
    let keyword = trimmed[..keyword_end].to_ascii_lowercase();
    let rest = trimmed[keyword_end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    Ok(Some(Line {
        keyword,
        args: split_args(rest)?,
    }))
}

fn split_args(rest: &str) -> Result<Vec<String>, UnterminatedQuote> {
    let mut args = Vec::new();
    let mut current: Option<String> = None;
    let mut quoted = false;
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' => quoted = false,
                '\\' => match chars.peek() {
                    Some(&escaped @ ('"' | '\\')) => {
                        current.get_or_insert_with(String::new).push(escaped);
                        chars.next();
                    }
                    _ => current.get_or_insert_with(String::new).push('\\'),
                },
                _ => current.get_or_insert_with(String::new).push(c),
            }
        } else if c == '"' {
            quoted = true;
            current.get_or_insert_with(String::new);
        } else if c.is_whitespace() {
            args.extend(current.take());
        } else {
            current.get_or_insert_with(String::new).push(c);
        }
    }
    if quoted {
        return Err(UnterminatedQuote);
    }
    args.extend(current);
    Ok(args)
}
