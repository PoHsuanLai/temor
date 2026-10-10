//! The text that loads temor's shell integration into a shell that is already running (PLAN
//! P17): typed into a pane after the person said Yes, when an inner `ssh`, `docker exec` or
//! `sudo -i` shell shows no blocks. It starts no new shell. The scripts travel inside the text,
//! base64-encoded, and a small `sh` program tells which shell asked and writes only that
//! shell's script to a file; the shell then sources the file, which deletes itself.
//!
//! Three limits shape the text:
//! - A shell with no line editor (dash, ash) cuts a typed line at 4096 bytes, so the payload is
//!   split into lines of at most [`CHUNK_BYTES`] inside one quoted command.
//! - Nothing between the quotes may be a quote, a backslash or a history mark (`!`): the same
//!   text must parse in bash, zsh, fish and nushell.
//! - The text should not stay in history. Each shell's script deletes the entries (see
//!   `inner_shell`); zsh has no way to delete one, so a short first line makes it ignore
//!   lines that start with a space.

use temor_shell::ShellScript;

use super::base64;
use super::inner_shell::InnerShell;
use crate::error::SshError;

/// The longest text built, all lines together.
pub const MAX_INNER_LINE_BYTES: usize = 64 * 1024;
/// The longest one typed line: a terminal in line mode cuts a line at 4096 bytes, newline included.
pub const MAX_TTY_LINE_BYTES: usize = 4096;
/// The longest run of payload text on one line.
const CHUNK_BYTES: usize = 3000;

/// Where the program writes the script, and the name that finds the entries in history. Random
/// per offer, so a name cannot be guessed from another host's user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadToken(String);

impl LoadToken {
    /// A token from two random words.
    pub fn from_words(high: u64, low: u64) -> Self {
        Self(format!("{high:016x}{low:016x}"))
    }

    /// The file the program writes on the host and the shell sources, then deletes.
    pub fn path(&self) -> String {
        format!("/tmp/{}", self.marker())
    }

    /// The text every typed line of this offer holds, for finding it in history.
    pub fn marker(&self) -> String {
        format!("temor-load-{}", self.0)
    }
}

/// What zsh reads first: it makes lines that start with a space leave history, unless the
/// person already had that set (then nothing changes). The `[` and `&&` forms parse the same
/// in bash, zsh and fish, and do nothing there.
pub(super) const ZSH_PRELUDE: &str = " [ -n \"$ZSH_VERSION\" ] && [ ! -o histignorespace ] && setopt histignorespace && _temor_hs_off=1\n";

/// The `sh` program, up to the per-shell parts. `@MARK@` and `@PATH@` are the token's.
const HEAD: &str = " sh -c '#@MARK@
f=@PATH@
umask 077
c=$(cat /proc/$PPID/comm 2>/dev/null || ps -p $PPID -o comm= 2>/dev/null)
c=${c##*/}
c=${c#-}
if command -v base64 >/dev/null 2>&1 && printf QQ== | base64 -d >/dev/null 2>&1; then
dec() { base64 -d; }
else
dec() { openssl base64 -d; }
fi
set -C
case $c in
";

/// The rest of the program: any other shell gets an empty file, which is then removed.
const TAIL: &str = "*) : >$f ;;
esac
( sleep 120; rm -f $f ) </dev/null >/dev/null 2>&1 &
' sh
";

/// The text to type, in order: a first line for zsh, the program, and the line that sources
/// the file it wrote. Every line ends in a newline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerLoadLine(String);

impl InnerLoadLine {
    /// Builds the text from `integration` (`temor_shell::integration_scripts()`); only
    /// `temor.bash`, `temor.zsh`, `temor.fish` and `temor.nu` are used.
    pub fn build(integration: &[ShellScript], token: &LoadToken) -> Result<Self, SshError> {
        let mut text = String::from(ZSH_PRELUDE);
        text.push_str(
            &HEAD
                .replace("@MARK@", &token.marker())
                .replace("@PATH@", &token.path()),
        );
        for shell in InnerShell::ALL {
            let Some(payload) = shell.payload(integration, token) else {
                continue;
            };
            text.push_str(shell.pattern());
            text.push_str(") dec >$f <<E\n");
            for line in chunk_lines(&base64::encode(payload.as_bytes()), CHUNK_BYTES) {
                text.push_str(&line);
                text.push('\n');
            }
            text.push_str("E\n;;\n");
        }
        text.push_str(TAIL);
        text.push_str(" source ");
        text.push_str(&token.path());
        text.push('\n');
        check_sizes(&text)?;
        Ok(Self(text))
    }

    /// The text to type, newlines included.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The lines, without their newlines.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.0.lines()
    }
}

/// `text` cut into runs of at most `width` characters (the text is ASCII).
pub(super) fn chunk_lines(text: &str, width: usize) -> Vec<String> {
    text.as_bytes()
        .chunks(width.max(1))
        .map(|run| run.iter().map(|&byte| char::from(byte)).collect())
        .collect()
}

fn check_sizes(text: &str) -> Result<(), SshError> {
    if text.len() > MAX_INNER_LINE_BYTES {
        return Err(SshError::ScriptTooLarge {
            bytes: text.len(),
            limit: MAX_INNER_LINE_BYTES,
        });
    }
    match text.lines().map(|line| line.len() + 1).max() {
        Some(bytes) if bytes > MAX_TTY_LINE_BYTES => Err(SshError::ScriptTooLarge {
            bytes,
            limit: MAX_TTY_LINE_BYTES,
        }),
        _ => Ok(()),
    }
}
