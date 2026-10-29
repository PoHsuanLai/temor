//! Typing a command line into a session: the one rule for a pinned tab's command, a command the
//! bar picked, and a line an agent asks to type. Text from anywhere else is the program's, and
//! a control character in it is never sent.

use temor_client::model::vocab::text::CommandText;

use crate::session::Session;

/// Whether a command typed into a prompt is submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Submit {
    /// Enter is sent: the person (or the row they picked) said it runs.
    Enter,
    /// The command waits at the prompt for the person's Enter.
    Wait,
}

/// The bytes that type `command` and, with [`Submit::Enter`], run it; `None` for a command that
/// is empty or has a control character.
pub(super) fn typed_bytes(command: &CommandText, submit: Submit) -> Option<Vec<u8>> {
    if !command.typable() {
        return None;
    }
    let tail = match submit {
        Submit::Enter => "\n",
        Submit::Wait => "",
    };
    Some(format!("{}{tail}", command.as_str()).into_bytes())
}

/// Types `command` into `session`; whether it was typable.
pub(super) fn type_command(session: &Session, command: &CommandText, submit: Submit) -> bool {
    match typed_bytes(command, submit) {
        Some(bytes) => {
            session.pane().send(&bytes);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_typed_with_enter_only_when_asked_and_never_with_a_control_character() {
        #[rustfmt::skip]
        let rows: &[(&str, Submit, Option<&str>)] = &[
            ("ls -l", Submit::Enter, Some("ls -l\n")),
            ("ls -l", Submit::Wait, Some("ls -l")),
            ("", Submit::Enter, None),
            ("ls\nrm x", Submit::Enter, None),
            ("ls\u{1b}[2J", Submit::Wait, None),
        ];
        for (text, submit, want) in rows {
            let got = typed_bytes(&CommandText((*text).to_owned()), *submit);
            assert_eq!(got.as_deref(), want.map(str::as_bytes), "{text:?}");
        }
    }
}
