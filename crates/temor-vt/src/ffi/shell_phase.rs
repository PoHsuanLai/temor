//! Where the shell is, as its OSC 133 marks said: what a screen or scrollback erase keeps.

use super::callbacks::MarkKind;
use crate::AnchorId;

/// Where the shell is, with the anchors of the marks that say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ShellPhase {
    /// No mark yet, a command ended, or the person cleared the screen while one ran.
    #[default]
    Between,
    /// At a prompt, no command running: its prompt mark and, once typing starts, its input
    /// mark. A screen erase here is the shell's own (Ctrl+L), and it draws the prompt again at
    /// the top.
    Typing {
        prompt: Option<AnchorId>,
        input: Option<AnchorId>,
    },
    /// A command's output runs from this mark: a `CSI 3 J` erases the scrollback only from it
    /// down, so earlier commands stay.
    Running(AnchorId),
}

impl ShellPhase {
    /// The phase after a `kind` mark set at `anchor` (none when the anchor could not be set).
    pub(super) fn after_mark(self, kind: MarkKind, anchor: Option<AnchorId>) -> ShellPhase {
        match kind {
            MarkKind::Prompt => ShellPhase::Typing {
                prompt: anchor,
                input: None,
            },
            MarkKind::Input => ShellPhase::Typing {
                prompt: self.prompt(),
                input: anchor,
            },
            MarkKind::Output => anchor.map_or(ShellPhase::Between, ShellPhase::Running),
            MarkKind::End => ShellPhase::Between,
        }
    }

    /// The phase after the person's own `clear`: the running output's rows are gone.
    pub(super) fn after_person_clears(self) -> ShellPhase {
        match self {
            ShellPhase::Running(_) => ShellPhase::Between,
            typing => typing,
        }
    }

    /// The running command's output mark.
    pub(super) fn floor(self) -> Option<AnchorId> {
        match self {
            ShellPhase::Running(anchor) => Some(anchor),
            _ => None,
        }
    }

    /// The prompt mark of the prompt being typed at.
    pub(super) fn prompt(self) -> Option<AnchorId> {
        match self {
            ShellPhase::Typing { prompt, .. } => prompt,
            _ => None,
        }
    }

    /// Whether `anchor` marks the prompt being typed at (its prompt or input).
    pub(super) fn typing_at(self, anchor: AnchorId) -> bool {
        matches!(self, ShellPhase::Typing { prompt, input } if prompt == Some(anchor) || input == Some(anchor))
    }
}

#[cfg(test)]
mod tests {
    use super::ShellPhase;
    use crate::AnchorId;
    use crate::ffi::callbacks::MarkKind;

    const A: Option<AnchorId> = Some(AnchorId(1));
    const B: Option<AnchorId> = Some(AnchorId(2));

    #[test]
    fn a_command_runs_from_its_output_mark_until_it_ends() {
        let typing = ShellPhase::Between
            .after_mark(MarkKind::Prompt, A)
            .after_mark(MarkKind::Input, B);
        assert_eq!(
            typing,
            ShellPhase::Typing {
                prompt: A,
                input: B
            }
        );
        assert_eq!(typing.floor(), None);
        let running = typing.after_mark(MarkKind::Output, Some(AnchorId(3)));
        assert_eq!(running.floor(), Some(AnchorId(3)));
        assert_eq!(running.prompt(), None);
        assert_eq!(running.after_mark(MarkKind::End, None), ShellPhase::Between);
    }

    #[test]
    fn the_persons_clear_ends_the_floor_and_keeps_the_prompt() {
        let running = ShellPhase::Running(AnchorId(3));
        assert_eq!(running.after_person_clears(), ShellPhase::Between);
        let typing = ShellPhase::Typing {
            prompt: A,
            input: None,
        };
        assert_eq!(typing.after_person_clears(), typing);
        assert!(typing.typing_at(AnchorId(1)) && !typing.typing_at(AnchorId(2)));
    }

    #[test]
    fn an_output_mark_with_no_anchor_leaves_nothing_to_erase_from() {
        let phase = ShellPhase::Between.after_mark(MarkKind::Output, None);
        assert_eq!(phase.floor(), None);
    }
}
