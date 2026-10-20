//! Typing a command the person chose into the program, as they would at the prompt.

use temor_core::vocab::text::CommandText;

use super::Session;

/// Whether a command typed into a pane is submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submit {
    /// Return is sent too: the command runs.
    Enter,
    /// The command waits at the prompt for the person's Return.
    Wait,
}

/// Whether a command was typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    Sent,
    /// It was empty or had a control character (a line break would run more than the one command
    /// asked for): nothing was sent.
    Refused,
}

/// The bytes that type `command` and, with `submit`, press Return; `None` for a command that is
/// not typable (`CommandText::typable`).
fn keystrokes(command: &CommandText, submit: Submit) -> Option<String> {
    if !command.typable() {
        return None;
    }
    let tail = match submit {
        Submit::Enter => "\n",
        Submit::Wait => "",
    };
    Some(format!("{}{tail}", command.as_str()))
}

impl Session {
    /// Types `command` into the program. A command with a control character in it is never sent.
    pub fn type_command(&self, command: &CommandText, submit: Submit) -> Typed {
        match keystrokes(command, submit) {
            Some(text) => {
                self.pane.send(text.as_bytes());
                Typed::Sent
            }
            None => Typed::Refused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_line_is_typed_and_return_only_when_asked() {
        let table = [
            ("runs", "ls -la", Submit::Enter, Some("ls -la\n")),
            ("waits", "ls -la", Submit::Wait, Some("ls -la")),
            ("empty", "", Submit::Enter, None),
            ("a line break", "ls\nrm x", Submit::Enter, None),
            ("an escape", "ls\u{1b}[A", Submit::Wait, None),
        ];
        for (name, line, submit, want) in table {
            let got = keystrokes(&CommandText(line.to_owned()), submit);
            assert_eq!(got.as_deref(), want, "{name}");
        }
    }
}
