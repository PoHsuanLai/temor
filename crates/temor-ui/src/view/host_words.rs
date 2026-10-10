//! What the host views say, in plain words (no jargon): the questions before a first connection,
//! the state of the connection, and what to do about a file that changed on the computer.

use temor_core::host::consent::BootstrapAction;

use crate::hub::{EditStatus, Stage};

/// A question about what temor may do on a computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ask {
    pub question: &'static str,
    pub detail: &'static str,
    pub allow: &'static str,
    pub refuse: &'static str,
}

/// The words for each question the connection asks, one at a time.
pub fn ask_words(action: BootstrapAction) -> Ask {
    match action {
        BootstrapAction::Integrate => Ask {
            question: "Set up this computer's terminal?",
            detail: "temor can send a small setup with the connection, so folders, prompts and \
                     the terminal type work here. It lives in a temporary folder that is removed \
                     when you disconnect.",
            allow: "Set up",
            refuse: "Plain connection",
        },
        BootstrapAction::Terminfo => Ask {
            question: "Add the terminal description to this computer?",
            detail: "temor can add one small file to your home folder here, so full-screen \
                     programs draw correctly. Without it the computer uses a more basic terminal \
                     type.",
            allow: "Add it",
            refuse: "Don't add",
        },
    }
}

/// The line shown while a connection is not up or has ended; nothing once it is up.
pub fn stage_line(stage: Stage, host: &str) -> Option<String> {
    match stage {
        Stage::Asking(_) | Stage::Up => None,
        Stage::Connecting => Some(format!("Connecting to {host}")),
        Stage::Offline => Some(format!("{host} looks offline on your tailnet")),
        Stage::Ended => Some(format!("The connection to {host} has ended")),
        Stage::Failed => Some(format!("Couldn't connect to {host}")),
    }
}

/// What an edit is doing, after its name.
pub fn edit_line(name: &str, status: EditStatus) -> String {
    let said = match status {
        EditStatus::Opening => "opening",
        EditStatus::Open => "open in your editor",
        EditStatus::Saving => "saving",
        EditStatus::Changed => "changed on the computer since you opened it",
        EditStatus::Rewritten => {
            "rewritten on the computer since you opened it (same time and size, other contents)"
        }
        EditStatus::Deleted => "removed from the computer since you opened it",
        EditStatus::Stalled => "couldn't be saved; save again to retry",
        EditStatus::Broken => "couldn't be opened",
    };
    format!("{name}: {said}")
}

/// The forward's line: where it can be reached on this computer, or that it is opening.
pub fn forward_line(host: &str, remote: u16, local: Option<u16>) -> String {
    match local {
        Some(local) => format!("localhost:{local} is {host} port {remote}"),
        None => format!("Forwarding {host} port {remote}"),
    }
}

#[cfg(test)]
mod tests {
    use temor_core::host::consent::BootstrapAction;

    use super::{ask_words, edit_line, forward_line, stage_line};
    use crate::hub::{EditStatus, Stage};

    #[test]
    fn no_question_or_state_uses_developer_words() {
        let mut said = vec![];
        for action in [BootstrapAction::Integrate, BootstrapAction::Terminfo] {
            let ask = ask_words(action);
            said.extend([ask.question, ask.detail, ask.allow, ask.refuse]);
        }
        let owned: Vec<String> = [
            stage_line(Stage::Connecting, "prod"),
            stage_line(Stage::Offline, "prod"),
            stage_line(Stage::Ended, "prod"),
        ]
        .into_iter()
        .flatten()
        .chain([
            edit_line("prod:/a", EditStatus::Changed),
            edit_line("prod:/a", EditStatus::Rewritten),
            forward_line("prod", 3000, Some(3001)),
        ])
        .collect();
        for text in said.into_iter().chain(owned.iter().map(String::as_str)) {
            for jargon in ["OSC", "PTY", "ACP", "MCP", "CLI", "ssh", "scp"] {
                assert!(!text.contains(jargon), "{jargon} in {text}");
            }
        }
    }

    #[test]
    fn a_connection_that_is_up_or_being_asked_about_has_no_line() {
        assert_eq!(stage_line(Stage::Up, "prod"), None);
        assert_eq!(
            stage_line(Stage::Asking(BootstrapAction::Integrate), "prod"),
            None
        );
        assert_eq!(
            forward_line("prod", 3000, None),
            "Forwarding prod port 3000"
        );
    }
}
