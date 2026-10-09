//! Quoting for the remote command and heredocs for the files it writes.

use crate::error::SshError;

/// The marker that ends each heredoc. The quoted form (`<<'TEMOR_EOF'`) stops the remote shell
/// from expanding anything in the body.
pub(super) const HEREDOC_END: &str = "TEMOR_EOF";

/// `text` as one POSIX single-quoted word: every `'` becomes `'\''`, so nothing in it is
/// interpreted by the shell that reads it.
pub(super) fn sh_single_quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for c in text.chars() {
        match c {
            '\'' => quoted.push_str("'\\''"),
            _ => quoted.push(c),
        }
    }
    quoted.push('\'');
    quoted
}

/// A command that writes `body` verbatim to `target_expr` (already shell text, such as
/// `"$temor_tmp/x"`). Fails when a body line would end the heredoc early.
pub(super) fn heredoc_write(target_expr: &str, body: &str) -> Result<String, SshError> {
    if body.lines().any(|line| line == HEREDOC_END) {
        return Err(SshError::HeredocConflict(HEREDOC_END.to_owned()));
    }
    Ok(format!(
        "cat > {target_expr} <<'{HEREDOC_END}'\n{}\n{HEREDOC_END}\n",
        body.trim_end_matches('\n')
    ))
}
