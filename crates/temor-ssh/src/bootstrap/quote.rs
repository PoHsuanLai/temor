//! Heredocs for the files the bootstrap writes.

use crate::error::SshError;

/// The marker that ends each heredoc. The quoted form (`<<'TEMOR_EOF'`) stops the shell from
/// expanding anything in the body.
pub(super) const HEREDOC_END: &str = "TEMOR_EOF";

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
