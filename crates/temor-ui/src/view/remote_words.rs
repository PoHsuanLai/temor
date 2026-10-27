//! What the helper views say, in plain words (no jargon): the question about installing it, the
//! state of a dropped connection, and what can be done about a port the host listens on.

use temor_remote::PortAction;

use super::host_words::{FORWARD_PORT, OPEN_DIRECT_TIP};
use crate::hub::RemoteStage;
use temor_client::remote::hold::Typing;

/// The question about putting the helper on a computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperAsk {
    pub allow: &'static str,
    pub refuse: &'static str,
}

/// The labels of the question's two answers.
pub const HELPER_ASK: HelperAsk = HelperAsk {
    allow: "Install helper",
    refuse: "No thanks",
};

/// The question's text for `host`.
pub fn ask_line(host: &str) -> String {
    format!("Keep your sessions on {host} running?")
}

/// What the question says more.
pub const ASK_DETAIL: &str = "temor can put one small program in a folder on this computer. \
    If the connection drops, your sessions keep running and come back just as you left them.";

/// The quiet line a pane shows while its session is not simply open.
pub fn stage_line(stage: RemoteStage, host: &str) -> Option<String> {
    match stage {
        RemoteStage::Preparing => Some(format!("Setting up {host}…")),
        RemoteStage::Reconnecting => Some("Reconnecting…".to_owned()),
        RemoteStage::Lost => Some("Connection lost".to_owned()),
        RemoteStage::Asking | RemoteStage::Live | RemoteStage::Ended => None,
    }
}

/// What a pane says when typing held back during a lost connection overflowed.
pub fn typing_line(typing: Typing) -> Option<&'static str> {
    match typing {
        Typing::Kept => None,
        Typing::Dropped => Some("Part of what you typed while disconnected was dropped"),
    }
}

/// A listening port's line.
pub fn port_line(program: &str, port: u16) -> String {
    match program.is_empty() {
        true => format!("Something is listening on port {port}"),
        false => format!("{program} is listening on port {port}"),
    }
}

/// A button's label and tooltip for what can be done about a port.
pub fn port_action_words(action: &PortAction) -> (String, &'static str) {
    match action {
        PortAction::OpenAt { name, port } => {
            (format!("Open at {name}:{}", port.0), OPEN_DIRECT_TIP)
        }
        PortAction::Forward { .. } => (FORWARD_PORT.label.to_owned(), FORWARD_PORT.tip),
    }
}

#[cfg(test)]
mod tests {
    use temor_wire::units::Port;

    use super::*;

    #[test]
    fn each_stage_has_its_line_and_none_uses_developer_words() {
        let cases = [
            (RemoteStage::Asking, None),
            (RemoteStage::Preparing, Some("Setting up box…")),
            (RemoteStage::Live, None),
            (RemoteStage::Reconnecting, Some("Reconnecting…")),
            (RemoteStage::Lost, Some("Connection lost")),
            (RemoteStage::Ended, None),
        ];
        for (stage, want) in cases {
            assert_eq!(stage_line(stage, "box").as_deref(), want, "{stage:?}");
        }
        let mut said = vec![ask_line("box"), ASK_DETAIL.to_owned()];
        said.push(HELPER_ASK.allow.to_owned());
        said.push(HELPER_ASK.refuse.to_owned());
        said.push(port_line("node", 3000));
        for action in [
            PortAction::OpenAt {
                name: "box.ts.net".to_owned(),
                port: Port(3000),
            },
            PortAction::Forward { port: Port(3000) },
        ] {
            let (label, tip) = port_action_words(&action);
            said.extend([label, tip.to_owned()]);
        }
        for text in said {
            for word in ["daemon", "ssh", "pty", "wire", "socket", "protocol"] {
                assert!(!text.to_lowercase().contains(word), "{word} in {text:?}");
            }
        }
    }

    #[test]
    fn only_dropped_typing_is_said() {
        assert_eq!(typing_line(Typing::Kept), None);
        let said = typing_line(Typing::Dropped).unwrap_or_default();
        assert!(
            said.contains("dropped") && !said.contains("buffer"),
            "{said}"
        );
    }

    #[test]
    fn a_port_without_a_program_name_is_still_described() {
        assert_eq!(port_line("", 22), "Something is listening on port 22");
        assert_eq!(port_line("sshd", 22), "sshd is listening on port 22");
    }
}
