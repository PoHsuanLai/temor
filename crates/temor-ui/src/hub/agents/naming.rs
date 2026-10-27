//! The known agent a pane was seen to run, and how it is told when it is seen again.

use temor_client::model::agent::seen::CommandLine;

use super::ending::Via;

/// A known agent seen in a pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Named {
    /// The program's name, lowercase.
    pub(super) program: String,
    pub(super) via: Via,
    /// What its process was started with, from the program word on, when its process was seen and
    /// the line could be read.
    pub(super) command: Option<CommandLine>,
}

impl Named {
    /// `slot` after the known agent program `app` was seen `via` a way, with `command` as its
    /// command line if that is known. The process in front is the stronger evidence (its leaving
    /// ends the agent), so a report does not replace it. A report says nothing of the command
    /// line, so it never takes away one already known.
    pub(super) fn learn(
        slot: &mut Option<Named>,
        app: &str,
        via: Via,
        command: Option<&CommandLine>,
    ) {
        let program = app.to_ascii_lowercase();
        match slot {
            Some(named) if named.program == program && named.via == Via::Process => {
                if let Some(command) = command {
                    named.command = Some(command.clone());
                }
            }
            _ => {
                *slot = Some(Named {
                    program,
                    via,
                    command: command.cloned(),
                });
            }
        }
    }
}
