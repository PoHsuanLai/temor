//! The script that gives a new shell the aliases of the one it replaces.

use crate::quote::sh_quote;

/// Whether `name` is an alias name written as itself: letters, digits and `_ . - : + @ ,`. Any
/// other name is left out rather than quoted, since each shell reads a quoted name its own way.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.-:+@,".contains(c))
}

/// A script for zsh (and bash) that defines `aliases`. Nothing in a value is read as syntax.
pub fn alias_script(aliases: &[(String, String)]) -> String {
    aliases
        .iter()
        .filter(|(name, _)| plain_name(name))
        .map(|(name, value)| format!("alias -- {name}={}\n", sh_quote(value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, value: &str) -> (String, String) {
        (name.to_owned(), value.to_owned())
    }

    #[test]
    fn values_are_quoted_and_names_that_are_not_plain_are_left_out() {
        let script = alias_script(&[
            pair("ll", "ls -la"),
            pair("sneaky", "x'; rm -rf ~; echo '"),
            pair("bad name", "x"),
            pair("$(boom)", "x"),
            pair("-o", "x"),
        ]);
        assert_eq!(
            script,
            "alias -- ll='ls -la'\nalias -- sneaky='x'\\''; rm -rf ~; echo '\\'''\n"
        );
    }
}
