//! Shell-style wildcards for `Host` patterns and `Include` paths: `*` and `?`, ASCII
//! case-insensitive (host names are case-insensitive).

/// Whether `pattern` has a wildcard.
pub(super) fn is_wild(pattern: &str) -> bool {
    pattern.contains(['*', '?'])
}

/// Whether `text` matches `pattern`. Iterative, so a long pattern cannot recurse deeply.
pub(super) fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let t: Vec<char> = text.to_ascii_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    // The last `*` seen, and the text position it has consumed up to.
    let mut star: Option<usize> = None;
    let mut resume = 0usize;
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            resume = ti;
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            resume += 1;
            ti = resume;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}
