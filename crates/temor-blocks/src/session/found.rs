//! Blocks found again on a screen restored from a snapshot. The snapshot keeps each cell's shell
//! integration kind (prompt, typed command, output) but none of the marks as events, so the edge
//! reads where each kind starts and this says which runs are blocks. What the shell said of each
//! command (its status, how long it took) comes from what was kept of the pane, matched by the
//! command's text from the newest back.

use super::checkpoint::CommandRecord;

/// What a run of cells is, as shell integration marked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Prompt,
    Input,
    Output,
}

/// A point on the screen: its row (0 is the oldest kept row) and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScreenPoint {
    pub row: u64,
    pub col: u16,
}

/// One block found on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub prompt: ScreenPoint,
    pub input: ScreenPoint,
    pub output: ScreenPoint,
    /// Where the next prompt starts, when there is one: the block ends before it.
    pub end: Option<ScreenPoint>,
}

/// The prompt being typed at, found on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Typing {
    pub prompt: ScreenPoint,
    pub input: Option<ScreenPoint>,
}

/// The blocks among `starts` (where each kind begins, top to bottom), and the prompt the last
/// run leaves open, if any. A prompt after a typed command with no output between is a
/// continuation prompt and belongs to the same command; typing with no prompt before it starts
/// a command of its own.
pub fn find_blocks(starts: &[(ScreenPoint, Kind)]) -> (Vec<Found>, Option<Typing>) {
    let mut blocks = Vec::new();
    let mut open: Option<(ScreenPoint, Option<ScreenPoint>, Option<ScreenPoint>)> = None;
    for &(at, kind) in starts {
        match (kind, open) {
            (Kind::Prompt, Some((_, Some(_), None))) => {}
            (Kind::Prompt, current) => {
                if let Some((prompt, Some(input), Some(output))) = current {
                    blocks.push(Found {
                        prompt,
                        input,
                        output,
                        end: Some(at),
                    });
                }
                open = Some((at, None, None));
            }
            (Kind::Input, Some((prompt, None, None))) => open = Some((prompt, Some(at), None)),
            // Typing with no prompt marked before it (a shell's first prompt can be drawn before
            // its mark): the command starts where the typing does.
            (Kind::Input, None) => open = Some((at, Some(at), None)),
            (Kind::Input, Some((prompt, Some(input), Some(output)))) => {
                blocks.push(Found {
                    prompt,
                    input,
                    output,
                    end: Some(at),
                });
                open = Some((at, Some(at), None));
            }
            (Kind::Output, Some((prompt, Some(input), None))) => {
                open = Some((prompt, Some(input), Some(at)))
            }
            _ => {}
        }
    }
    let typing = match open {
        Some((prompt, Some(input), Some(output))) => {
            blocks.push(Found {
                prompt,
                input,
                output,
                end: None,
            });
            None
        }
        Some((prompt, input, None)) => Some(Typing { prompt, input }),
        Some((_, None, Some(_))) | None => None,
    };
    (blocks, typing)
}

/// For each command (oldest first), the record kept for it: matched from the newest back by its
/// text, each record used once, a command with no match getting none. Also how many of the
/// newest records were matched past the earliest match: those are on screen now, not history.
pub fn match_history<'a>(
    commands: &[String],
    history: &'a [CommandRecord],
) -> (Vec<Option<&'a CommandRecord>>, usize) {
    let mut out = vec![None; commands.len()];
    let mut next = history.len();
    let mut earliest = history.len();
    for (n, command) in commands.iter().enumerate().rev() {
        let found = history[..next]
            .iter()
            .rposition(|record| record.command.trim() == command.trim());
        if let Some(at) = found {
            out[n] = Some(&history[at]);
            next = at;
            earliest = at;
        }
    }
    (out, history.len() - earliest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(row: u64, col: u16) -> ScreenPoint {
        ScreenPoint { row, col }
    }

    #[test]
    fn prompts_inputs_and_outputs_make_blocks_and_the_last_prompt_is_typed_at() {
        let starts = [
            (at(0, 0), Kind::Prompt),
            (at(1, 0), Kind::Input),
            (at(2, 0), Kind::Output),
            (at(4, 0), Kind::Prompt),
            (at(5, 0), Kind::Input),
            (at(6, 0), Kind::Output),
            (at(7, 0), Kind::Prompt),
            (at(8, 2), Kind::Input),
        ];
        let (blocks, typing) = find_blocks(&starts);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].end, Some(at(4, 0)));
        assert_eq!(blocks[1].output, at(6, 0));
        assert_eq!(
            typing,
            Some(Typing {
                prompt: at(7, 0),
                input: Some(at(8, 2))
            })
        );
    }

    #[test]
    fn a_continuation_prompt_stays_in_its_command_and_one_with_no_output_is_still_typed() {
        let starts = [
            (at(0, 0), Kind::Prompt),
            (at(0, 2), Kind::Input),
            (at(1, 0), Kind::Prompt),
            (at(1, 2), Kind::Input),
            (at(2, 0), Kind::Output),
            (at(3, 0), Kind::Prompt),
            (at(3, 2), Kind::Input),
            (at(4, 0), Kind::Prompt),
        ];
        let (blocks, typing) = find_blocks(&starts);
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].input, at(0, 2));
        assert_eq!(
            typing,
            Some(Typing {
                prompt: at(3, 0),
                input: Some(at(3, 2))
            }),
            "the last prompt continues the command being typed"
        );
    }

    #[test]
    fn a_last_block_with_output_and_no_prompt_after_still_counts() {
        let (blocks, typing) = find_blocks(&[
            (at(0, 0), Kind::Prompt),
            (at(0, 2), Kind::Input),
            (at(1, 0), Kind::Output),
        ]);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].end, None);
        assert_eq!(typing, None);
    }

    #[test]
    fn typing_with_no_prompt_marked_before_it_starts_a_command() {
        let (blocks, _) = find_blocks(&[
            (at(0, 0), Kind::Input),
            (at(1, 0), Kind::Output),
            (at(2, 0), Kind::Input),
            (at(3, 0), Kind::Output),
            (at(4, 0), Kind::Prompt),
        ]);
        assert_eq!(blocks.len(), 2, "{blocks:?}");
        assert_eq!(
            (blocks[0].prompt, blocks[0].end),
            (at(0, 0), Some(at(2, 0)))
        );
        assert_eq!(blocks[1].input, at(2, 0));
    }

    fn record(command: &str, exit: i32) -> CommandRecord {
        CommandRecord {
            command: command.to_owned(),
            exit: Some(exit),
            took_ms: Some(5),
        }
    }

    #[test]
    fn records_match_from_the_newest_back_once_each() {
        let history = [record("ls", 0), record("make", 2), record("ls", 1)];
        let commands = ["make".to_owned(), "ls".to_owned(), "vim".to_owned()];
        let (matched, on_screen) = match_history(&commands, &history);
        assert_eq!(matched[0].map(|r| r.exit), Some(Some(2)));
        assert_eq!(matched[1].map(|r| r.exit), Some(Some(1)));
        assert_eq!(matched[2], None);
        assert_eq!(on_screen, 2, "make and the last ls are on screen");
    }
}
