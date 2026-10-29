//! What the host views say, in plain words (no jargon): the questions before a first connection,
//! the state of the connection, and what to do about a file that changed on the computer.

use temor_client::model::host::consent::BootstrapAction;

use crate::hub::{EditStatus, Stage};

/// A question about what temor may do on a computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ask {
    pub question: &'static str,
    pub detail: &'static str,
    pub allow: &'static str,
    pub refuse: &'static str,
}

/// A button's label and its hover hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonWords {
    pub label: &'static str,
    pub tip: &'static str,
}

impl ButtonWords {
    const fn new(label: &'static str, tip: &'static str) -> Self {
        Self { label, tip }
    }

    /// The hint, as a button takes it.
    pub fn title(self) -> Option<String> {
        Some(self.tip.to_owned())
    }
}

/// What a button that answers a question tells: the answer is kept for the computer.
pub const REMEMBER_TIP: &str = "Remember this for this computer";

/// Connecting again, from a view that stopped trying: a failed connection and a lost helper alike.
pub const TRY_AGAIN: ButtonWords = ButtonWords::new("Try again", "Connect again");

/// Connecting to a computer that looks offline.
pub const TRY_ANYWAY: ButtonWords =
    ButtonWords::new("Try anyway", "Connect even though it looks offline");

/// The browser page a connection waits on.
pub const OPEN_CHECK: ButtonWords = ButtonWords::new("Open", "Open the confirmation page");

/// Making a port of the computer reachable here, from the banner and from a listening port's line.
pub const FORWARD_PORT: ButtonWords = ButtonWords::new(
    "Forward port\u{2026}",
    "Reach this computer's port from here",
);

/// The tip of opening a port over the tailnet, whatever the button's label says.
pub const OPEN_DIRECT_TIP: &str = "Open the port over your tailnet, with no forward";

/// Opening the port typed in the banner over the tailnet.
pub const OPEN_DIRECTLY: ButtonWords = ButtonWords::new("Open directly", OPEN_DIRECT_TIP);

/// The edit field's button.
pub const EDIT_LOCALLY: ButtonWords =
    ButtonWords::new("Edit locally", "Open a copy here and save it back");

/// A forwarded port, from its line.
pub const OPEN_FORWARDED: ButtonWords = ButtonWords::new("Open", "Open it in your browser");

/// Ending a forward, from its line.
pub const CLOSE_FORWARD: ButtonWords = ButtonWords::new("Close", "Stop forwarding this port");

/// The ways out of a file that changed on the computer.
pub const SHOW_CHANGES: ButtonWords =
    ButtonWords::new("Show changes", "See what differs, line by line");
pub const REPLACE_THEIRS: ButtonWords =
    ButtonWords::new("Replace theirs", "Write your copy over the computer's file");
pub const USE_THEIRS: ButtonWords = ButtonWords::new(
    "Use theirs",
    "Discard your changes and fetch the computer's file",
);
pub const DECIDE_LATER: ButtonWords =
    ButtonWords::new("Decide later", "Keep editing; the next save asks again");

/// Ending an edit.
pub const DONE_EDITING: ButtonWords =
    ButtonWords::new("Done", "Stop watching and delete this copy");

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
    use temor_client::model::host::consent::BootstrapAction;

    use super::*;
    use crate::hub::{EditStatus, Stage};

    #[test]
    fn no_question_or_state_uses_developer_words() {
        let mut said = vec![];
        for action in [BootstrapAction::Integrate, BootstrapAction::Terminfo] {
            let ask = ask_words(action);
            said.extend([ask.question, ask.detail, ask.allow, ask.refuse]);
        }
        for words in [
            TRY_AGAIN,
            TRY_ANYWAY,
            OPEN_CHECK,
            FORWARD_PORT,
            OPEN_DIRECTLY,
            EDIT_LOCALLY,
            OPEN_FORWARDED,
            CLOSE_FORWARD,
            SHOW_CHANGES,
            REPLACE_THEIRS,
            USE_THEIRS,
            DECIDE_LATER,
            DONE_EDITING,
        ] {
            said.extend([words.label, words.tip]);
        }
        said.extend([REMEMBER_TIP, OPEN_DIRECT_TIP]);
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
