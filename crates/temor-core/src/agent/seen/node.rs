//! The script a `node` program word runs (an agent installed as a JavaScript package runs as
//! `node …/cli.js`), read by node's own options so an option's value is never the script.

use super::wrapper::is_option;

/// `node` options that take their value as the next word (`--title x`); each also takes it
/// after `=` (`--title=x`), which is one word.
const NODE_VALUED: &[&str] = &[
    "-r",
    "--require",
    "--import",
    "--loader",
    "--experimental-loader",
    "-C",
    "--conditions",
    "--title",
    "--env-file",
    "--env-file-if-exists",
    "--input-type",
    "--inspect-port",
    "--debug-port",
    "--stack-size",
    "--max-old-space-size",
    "--max-semi-space-size",
    "--redirect-warnings",
    "--disable-warning",
    "--unhandled-rejections",
    "--dns-result-order",
    "--openssl-config",
    "--icu-data-dir",
    "--diagnostic-dir",
    "--report-dir",
    "--report-directory",
    "--report-filename",
    "--report-signal",
    "--heapsnapshot-signal",
    "--heapsnapshot-near-heap-limit",
    "--cpu-prof-dir",
    "--cpu-prof-name",
    "--cpu-prof-interval",
    "--heap-prof-dir",
    "--heap-prof-name",
    "--heap-prof-interval",
    "--secure-heap",
    "--secure-heap-min",
    "--tls-cipher-list",
    "--tls-keylog",
    "--trace-event-categories",
    "--trace-event-file-pattern",
    "--watch-path",
    "--localstorage-file",
    "--use-largepages",
    "--experimental-policy",
    "--policy-integrity",
];

/// `node` options that take no value; with `--no-…`, `--experimental-…` and `--trace-…`.
const NODE_FLAGS: &[&str] = &[
    "--enable-source-maps",
    "--expose-gc",
    "--inspect",
    "--inspect-brk",
    "--inspect-wait",
    "--preserve-symlinks",
    "--preserve-symlinks-main",
    "--pending-deprecation",
    "--throw-deprecation",
    "--abort-on-uncaught-exception",
    "--frozen-intrinsics",
    "--jitless",
    "--watch",
    "--watch-preserve-output",
    "--use-openssl-ca",
    "--use-bundled-ca",
    "--use-system-ca",
    "--zero-fill-buffers",
    "--cpu-prof",
    "--heap-prof",
    "--report-on-signal",
    "--report-uncaught-exception",
    "--report-on-fatalerror",
    "--report-compact",
];

/// How many words a `node` option is, its value included; `None` when it is not known (a
/// value-taking option read as a flag would take its value for the script).
fn node_option_words(word: &str) -> Option<usize> {
    let flag = NODE_FLAGS.contains(&word)
        || ["--no-", "--experimental-", "--trace-"]
            .iter()
            .any(|p| word.starts_with(p));
    match word {
        _ if NODE_VALUED.contains(&word) => Some(2),
        _ if word.starts_with("--") && word.contains('=') => Some(1),
        _ if flag => Some(1),
        _ => None,
    }
}

/// The script `node` runs: its first argument that is not an option. `None` when it runs code
/// given on the command line, or an option is not known.
pub(super) fn node_script(tokens: &[String], node_at: usize) -> Option<&str> {
    let mut at = node_at + 1;
    loop {
        let word = tokens.get(at)?.as_str();
        at += match word {
            "--" => return tokens.get(at + 1).map(String::as_str),
            "-e" | "--eval" | "-p" | "--print" | "-c" | "--check" | "-i" | "--interactive" => {
                return None;
            }
            _ if is_option(word) => node_option_words(word)?,
            _ => return Some(word),
        };
    }
}
