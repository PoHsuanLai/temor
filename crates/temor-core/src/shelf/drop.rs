//! A file dropped on a pane: send it when the program there takes files, otherwise paste its
//! path the way the shell wants it. Pure; the window carries out the answer.

/// Whether the program in the pane has said it can take files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReceiveSupport {
    /// It announced file receive (it answered the transfer offer, or the shell integration says so).
    Announced,
    /// Nothing is known.
    Unknown,
}

/// Where the pane's shell runs. A path on this computer means nothing on another one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneSite {
    Local,
    Remote,
}

/// How the pane's shell quotes a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quoting {
    /// bash, zsh and other POSIX shells.
    Posix,
    Fish,
    Nushell,
}

/// What was dropped, and on what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub paths: Vec<String>,
    pub support: ReceiveSupport,
    pub site: PaneSite,
    pub quoting: Quoting,
}

/// What the window does with the drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropAction {
    /// Offer these files to the program with the file transfer protocol.
    Send(Vec<String>),
    /// Type this text into the pane as a paste (quoted paths, a space after each).
    Paste(String),
    /// Do nothing but tell the person why.
    Tell(&'static str),
    /// Nothing was dropped.
    Nothing,
}

/// The answer for `dropped`. Paths with control characters are never pasted (a newline in a
/// paste could run a command).
pub fn drop_action(dropped: &Dropped) -> DropAction {
    if dropped.paths.is_empty() {
        return DropAction::Nothing;
    }
    if dropped.support == ReceiveSupport::Announced {
        return DropAction::Send(dropped.paths.clone());
    }
    if dropped.site == PaneSite::Remote {
        return DropAction::Tell("This program can't receive dropped files");
    }
    let words: Vec<String> = dropped
        .paths
        .iter()
        .filter(|path| !path.chars().any(char::is_control))
        .map(|path| quote(path, dropped.quoting))
        .collect();
    if words.is_empty() {
        return DropAction::Tell("That file's name can't be typed into a terminal");
    }
    DropAction::Paste(words.iter().map(|word| format!("{word} ")).collect())
}

/// One word, quoted so the shell reads it back as the same text.
pub fn quote(text: &str, quoting: Quoting) -> String {
    match quoting {
        Quoting::Posix => temor_shell::sh_quote(text),
        Quoting::Fish => format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'")),
        Quoting::Nushell => {
            if !text.contains('\'') {
                format!("'{text}'")
            } else if !text.contains('`') {
                format!("`{text}`")
            } else {
                format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
            }
        }
    }
}
