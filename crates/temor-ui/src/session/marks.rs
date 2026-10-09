//! Shell marks and blocks in the session (PLAN §9 blocks). Each OSC 133 mark arrives as a
//! `Prompt` event and then its `Anchor`; the anchor's row feeds the block machine. Finished blocks
//! may notify, and the gutter, the prompt jumps and the block actions read the anchors back.

use temor_core::block::{
    Author, BlockIn, BlockOut, BlockState, Outcome, block_step, format_elapsed, next_prompt,
    previous_prompt,
};
use temor_core::notify::{Focus, Notice as Desk, NotifyAfter, finished_notice};
use temor_core::terminal::selection::{CellPos, Granularity, Selection, Shape};
use temor_core::vocab::ids::{BlockId, ExitCode, GridRow};
use temor_core::vocab::text::CommandText;
use temor_transfer::FileName;
use temor_transfer::shelf::ShelfStore;
use temor_vt::{
    AnchorId, ExitStatus, GridPoint, PointSpace, PromptEvent, PromptKind, RowCount, ViewportScroll,
};

use super::blocks::BlockAnchors;
use super::model::PADDING_LOGICAL;
use super::{Notice, Session};
use crate::edge::DesktopNote;
use crate::error::UiError;

/// Which way a jump between prompts goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptStep {
    /// Towards older output (⌘↑).
    Previous,
    /// Towards the live prompt (⌘↓).
    Next,
}

/// What the gutter shows for one block whose prompt is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockMark {
    pub id: BlockId,
    /// The prompt's row in the viewport.
    pub row: u32,
    pub outcome: Outcome,
    /// How long it ran; `None` while it runs.
    pub took_ms: Option<u64>,
}

impl Session {
    /// A shell mark came in. It waits for its anchor, which the terminal sends next.
    pub(super) fn mark(&mut self, mark: PromptEvent) {
        self.blocks.pending = Some(mark);
    }

    /// The anchor of the pending mark arrived: the block machine takes the mark at its row.
    pub(super) fn anchored(&mut self, id: AnchorId) -> Notice {
        let row = self.screen_row(id);
        let mark = self.blocks.pending.take();
        let (Some(mark), Some(row)) = (mark, row) else {
            self.drop_anchor(id);
            return Notice::Chrome;
        };
        let row = GridRow(u64::from(row));
        let at = self.clock.now();
        match mark {
            PromptEvent::PromptStart(PromptKind::Primary) => {
                self.blocks.prompts += 1;
                // A new prompt: anchors left from an empty line are no longer needed.
                if let Some(old) = self.blocks.prompt.replace(id) {
                    self.drop_anchor(old);
                }
                if let Some(old) = self.blocks.input.take() {
                    self.drop_anchor(old);
                }
                self.step(BlockIn::PromptStart { row, at });
            }
            PromptEvent::PromptStart(_) => self.drop_anchor(id),
            PromptEvent::InputStart => {
                if let Some(old) = self.blocks.input.replace(id) {
                    self.drop_anchor(old);
                }
                self.step(BlockIn::CommandStart { row });
            }
            PromptEvent::OutputStart { command } => {
                let block = self.blocks.allocate();
                let typed = self
                    .blocks
                    .input
                    .and_then(|input| self.text_between(input, id));
                let command = match command.is_empty() {
                    true => typed.unwrap_or_default(),
                    false => command,
                };
                let anchors = BlockAnchors {
                    prompt: self.blocks.prompt.take(),
                    input: self.blocks.input.take(),
                    output: Some(id),
                    end: None,
                };
                self.blocks.anchors.insert(block, anchors);
                self.step(BlockIn::OutputStart {
                    id: block,
                    row,
                    command: CommandText(command),
                    by: Author::Person,
                    at,
                });
            }
            PromptEvent::CommandEnd { exit, .. } => {
                let running = match &self.blocks.machine {
                    BlockState::Running(block) => Some(block.id),
                    _ => None,
                };
                match running {
                    Some(block) => self.blocks.anchors.entry(block).or_default().end = Some(id),
                    None => self.drop_anchor(id),
                }
                let exit = match exit {
                    ExitStatus::Code(code) => Some(ExitCode(code.0)),
                    ExitStatus::Unreported => None,
                };
                self.step(BlockIn::Finished { row, exit, at });
            }
        }
        self.sweep();
        Notice::Chrome
    }

    /// The shell is gone: a running command ends unreported.
    pub(super) fn shell_gone(&mut self) {
        let at = self.clock.now();
        self.step(BlockIn::ShellExited { at });
        self.sweep();
    }

    /// The terminal was reset: a running command is no longer on screen.
    pub(super) fn terminal_reset(&mut self) {
        let at = self.clock.now();
        self.step(BlockIn::Reset { at });
    }

    /// Runs the block machine on one input and applies what it says: the store, the anchors of
    /// blocks it removed, and a notice for a block that finished.
    fn step(&mut self, input: BlockIn) {
        let machine = std::mem::replace(&mut self.blocks.machine, BlockState::Idle);
        let (next, outs) = block_step(machine, input);
        self.blocks.machine = next;
        for out in &outs {
            if let BlockOut::BlockDone(block) = out
                && let Some(note) = finished_notice(block, self.notify_after, self.focus)
            {
                self.announce(note);
            }
            let gone = self.blocks.store.apply(out);
            for anchor in self.blocks.drop_anchors(&gone) {
                self.drop_anchor(anchor);
            }
        }
    }

    /// Drops the blocks whose prompt and output the scrollback no longer holds.
    fn sweep(&mut self) {
        let pane = &self.pane;
        let anchors = &self.blocks.anchors;
        let alive = |id: BlockId| {
            anchors.get(&id).is_some_and(|held| {
                [held.prompt, held.output]
                    .into_iter()
                    .flatten()
                    .any(|anchor| {
                        pane.with_term(|term| term.anchor_point(anchor, PointSpace::Screen))
                            .is_some()
                    })
            })
        };
        let gone = self.blocks.store.retain_live(alive);
        for anchor in self.blocks.drop_anchors(&gone) {
            self.drop_anchor(anchor);
        }
    }

    /// The row of an anchor in the screen (0 is the oldest kept row); `None` once it is gone.
    fn screen_row(&self, id: AnchorId) -> Option<u32> {
        self.pane
            .with_term(|term| term.anchor_point(id, PointSpace::Screen))
            .map(|at| at.row)
    }

    fn drop_anchor(&self, id: AnchorId) {
        self.pane.with_term(|term| term.release_anchor(id));
    }

    /// The text from one anchor up to (not including) another, trimmed of the line ends around
    /// it. The end cell is excluded: an anchor at a line's start marks the end of the line above.
    fn text_between(&self, from: AnchorId, to: AnchorId) -> Option<String> {
        self.pane.with_term(|term| {
            let start = term.anchor_point(from, PointSpace::Screen)?;
            let end = term.anchor_point(to, PointSpace::Screen)?;
            let last_col = term.size().cols.0.saturating_sub(1);
            let stop = match (end.col.0, end.row.checked_sub(1)) {
                (0, Some(row)) if end.row > start.row => {
                    GridPoint::new(PointSpace::Screen, last_col, row)
                }
                (col, _) if col > 0 && end.row >= start.row => {
                    GridPoint::new(PointSpace::Screen, col - 1, end.row)
                }
                _ => return Some(String::new()),
            };
            let text = term.text(start, stop).ok()?;
            Some(text.trim().to_owned())
        })
    }

    /// The gutter's marks for the blocks whose prompts are on screen.
    pub fn block_marks(&self) -> Vec<BlockMark> {
        self.pane.with_term(|term| {
            self.blocks
                .store
                .iter()
                .filter_map(|block| {
                    let prompt = self.blocks.anchors.get(&block.id)?.prompt?;
                    let at = term.anchor_point(prompt, PointSpace::Viewport)?;
                    Some(BlockMark {
                        id: block.id,
                        row: at.row,
                        outcome: block.outcome(),
                        took_ms: block.elapsed_ms(),
                    })
                })
                .collect()
        })
    }

    /// Clears the block the gutter selected.
    pub fn clear_block_selection(&mut self) {
        self.selected_block = None;
    }

    /// The block the gutter has selected (PLAN §9: a click on its gutter).
    pub fn selected_block(&self) -> Option<BlockId> {
        self.selected_block
    }

    /// The command a block ran, as typed.
    pub fn block_command(&self, id: BlockId) -> Option<&str> {
        self.blocks.store.get(id).map(|b| b.command.0.as_str())
    }

    /// A block's output as text, once the command has finished.
    pub fn block_output(&self, id: BlockId) -> Option<String> {
        let anchors = self.blocks.anchors.get(&id)?;
        self.text_between(anchors.output?, anchors.end?)
    }

    /// How many primary prompts the shell has marked so far.
    pub fn prompts_seen(&self) -> usize {
        self.blocks.prompts
    }

    /// Every block the session holds, oldest first, with how each stands.
    pub fn block_outcomes(&self) -> Vec<(BlockId, Outcome)> {
        self.blocks
            .store
            .iter()
            .map(|block| (block.id, block.outcome()))
            .collect()
    }

    /// Whether a block's command has finished (its output is complete).
    pub fn block_finished(&self, id: BlockId) -> bool {
        self.blocks
            .store
            .get(id)
            .is_some_and(|block| block.outcome() != Outcome::Running)
    }

    /// How a block ended and how long it ran, for its hover and its notices.
    pub fn block_summary(&self, id: BlockId) -> Option<String> {
        let block = self.blocks.store.get(id)?;
        let took = block.elapsed_ms().map(format_elapsed);
        Some(match (block.outcome(), took) {
            (Outcome::Running, _) => "Still running".to_owned(),
            (Outcome::Succeeded, Some(took)) => format!("Succeeded in {took}"),
            (Outcome::Failed(code), Some(took)) => {
                format!("Failed with status {} after {took}", code.0)
            }
            (Outcome::Unreported, Some(took)) => format!("Ended after {took}"),
            (_, None) => "Ended".to_owned(),
        })
    }

    /// Moves the view to the previous or next prompt (⌘↑ and ⌘↓).
    pub(super) fn jump_prompt(&mut self, step: PromptStep) -> Result<(), UiError> {
        let (prompts, top) = self.pane.with_term(|term| {
            let prompts: Vec<(BlockId, GridRow)> = self
                .blocks
                .store
                .iter()
                .filter_map(|block| {
                    let prompt = self.blocks.anchors.get(&block.id)?.prompt?;
                    let at = term.anchor_point(prompt, PointSpace::Screen)?;
                    Some((block.id, GridRow(u64::from(at.row))))
                })
                .collect();
            let top = term.viewport().map_or(0, |(offset, _)| u64::from(offset));
            (prompts, GridRow(top))
        });
        let target = match step {
            PromptStep::Previous => previous_prompt(&prompts, top),
            PromptStep::Next => next_prompt(&prompts, top),
        };
        let row = target.and_then(|id| prompts.iter().find(|(b, _)| *b == id));
        if let Some((_, row)) = row {
            self.pane
                .with_term(|term| term.scroll(ViewportScroll::Row(RowCount(row.0))));
        }
        Ok(())
    }

    /// The block whose gutter is under `at` (device pixels in the pane), if the point is in the
    /// left margin, on a row that belongs to a block.
    pub fn gutter_block(&self, at: temor_vt::SurfacePoint) -> Option<BlockId> {
        let fit = self.fit?;
        let margin = i64::from(fit.padding.left.0);
        if at.x < 0 || i64::from(at.x) >= margin {
            return None;
        }
        let hit = fit.cell_at(at);
        let top = self.viewport_top().ok()?;
        let row = top.saturating_add(u32::from(hit.pos.row.0));
        self.block_at_row(row)
    }

    /// A press on the gutter selects the block at that row, its output included. Returns whether
    /// the press was a gutter press.
    pub fn gutter_press(&mut self, at: temor_vt::SurfacePoint) -> Result<bool, UiError> {
        let Some(id) = self.gutter_block(at) else {
            return Ok(false);
        };
        self.select_block(id)?;
        self.selected_block = Some(id);
        Ok(true)
    }

    /// The pane's logical padding and cell height, for placing things beside grid rows.
    pub fn gutter_geometry(&self) -> Option<(f32, f32)> {
        let fit = self.fit?;
        let factor = self.scale.factor();
        Some((PADDING_LOGICAL, fit.cell.height.0 as f32 / factor))
    }

    /// The newest block whose prompt..end span holds screen row `row`.
    fn block_at_row(&self, row: u32) -> Option<BlockId> {
        self.blocks.store.iter().rev().find_map(|block| {
            let (first, last) = self.block_rows(block.id)?;
            (first..=last).contains(&row).then_some(block.id)
        })
    }

    /// The first and last screen rows of a block (a running block runs to the bottom).
    fn block_rows(&self, id: BlockId) -> Option<(u32, u32)> {
        let anchors = self.blocks.anchors.get(&id)?;
        let first = self.screen_row(anchors.prompt.or(anchors.output)?)?;
        let last = match anchors.end {
            Some(end) => self.screen_row(end)?,
            None => u32::MAX,
        };
        Some((first, last))
    }

    /// Selects a finished block's output, the way a drag over it would.
    fn select_block(&mut self, id: BlockId) -> Result<(), UiError> {
        let Some(anchors) = self.blocks.anchors.get(&id).copied() else {
            return Ok(());
        };
        let (Some(output), Some(end)) = (anchors.output, anchors.end) else {
            return Ok(());
        };
        let (Some(first), Some(stop)) = (self.screen_row(output), self.screen_row(end)) else {
            return Ok(());
        };
        let end_at = self
            .pane
            .with_term(|term| term.anchor_point(end, PointSpace::Screen));
        // The end mark sits at the start of the line after the output, so stop one line short.
        let last = match end_at {
            Some(at) if at.col.0 == 0 && stop > first => stop - 1,
            _ => stop,
        };
        let line = self.line(first)?;
        let mut selection = Selection::begin(
            CellPos::new(first, 0),
            &line,
            Granularity::Cells,
            Shape::Linear,
        );
        let last_line = self.line(last)?;
        let width = u16::try_from(last_line.cells.len().saturating_sub(1)).unwrap_or(u16::MAX);
        selection.extend(CellPos::new(last, width), &last_line);
        self.selection = Some(selection);
        self.mirror_selection()
    }

    /// Puts a block's command in the prompt without running it: Enter stays the person's.
    pub fn rerun(&mut self, id: BlockId) {
        let Some(command) = self.block_command(id).map(str::to_owned) else {
            return;
        };
        if command.is_empty() {
            return;
        }
        self.pane.send(command.as_bytes());
        self.pane
            .with_term(|term| term.scroll(ViewportScroll::Bottom));
    }

    /// Copies a block's command to the clipboard.
    pub fn copy_command(&mut self, id: BlockId) {
        let Some(command) = self.block_command(id).map(str::to_owned) else {
            return;
        };
        self.copy_text(&command);
    }

    /// Copies a finished block's output to the clipboard.
    pub fn copy_output(&mut self, id: BlockId) {
        match self.block_output(id) {
            Some(text) => self.copy_text(&text),
            None => self.notice = Some("The output is not finished yet.".to_owned()),
        }
    }

    fn copy_text(&mut self, text: &str) {
        if self.clipboard.write(text).is_err() {
            self.notice = Some("The text could not be copied.".to_owned());
        }
    }

    /// Saves a finished block's output to the downloads shelf (PLAN §9): written to a file in
    /// the shelf folder, then placed under its own name by the shelf store.
    pub fn save_output(&mut self, id: BlockId) -> Result<(), UiError> {
        let Some(text) = self.block_output(id) else {
            self.notice = Some("The output is not finished yet.".to_owned());
            return Ok(());
        };
        let command = self.block_command(id).unwrap_or_default().to_owned();
        let store = ShelfStore::new(self.shelf_dir.clone());
        let name = FileName::sanitise(&output_name(&command));
        let saved = write_temp(&store, &text).and_then(|temp| {
            store
                .place(&temp, &name)
                .map_err(|err| UiError::Save(err.to_string()))
        });
        match saved {
            Ok(_) => self.notice = Some("Saved to Downloads.".to_owned()),
            Err(err) => self.notice = Some(err.to_string()),
        }
        Ok(())
    }

    /// Makes a desktop notice of a core notice, when the limiter lets one through.
    pub(super) fn announce(&mut self, notice: Desk) {
        let now = self.clock.now();
        if !self.limiter.allow(now) {
            return;
        }
        let note = desktop_note(&notice);
        if let Err(err) = self.notifier.notify(&note) {
            self.notice = Some(err.to_string());
        }
    }

    /// The focus the window reports (a pane that is not focused notifies).
    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    /// The notify setting the session uses.
    pub fn notify_after(&self) -> NotifyAfter {
        self.notify_after
    }
}

/// The text of a block's output file name: the command's start, then " output.txt".
fn output_name(command: &str) -> String {
    let head: String = command.chars().take(40).collect();
    let head = head.trim();
    match head.is_empty() {
        true => "Command output.txt".to_owned(),
        false => format!("{head} output.txt"),
    }
}

/// Writes `text` to a new file in the shelf folder, to be placed by the shelf store.
fn write_temp(store: &ShelfStore, text: &str) -> Result<std::path::PathBuf, UiError> {
    let dir = store.dir();
    std::fs::create_dir_all(dir).map_err(|err| UiError::Save(err.to_string()))?;
    let path = dir.join(format!(".temor-output-{}.tmp", std::process::id()));
    std::fs::write(&path, text.as_bytes()).map_err(|err| UiError::Save(err.to_string()))?;
    Ok(path)
}

/// The desktop's words for a core notice (plain text, no developer terms).
pub fn desktop_note(notice: &Desk) -> DesktopNote {
    match notice {
        Desk::CommandFinished {
            outcome,
            took_ms,
            command,
        } => {
            let what = match command.is_empty() {
                true => "The command".to_owned(),
                false => command.clone(),
            };
            let took = format_elapsed(*took_ms);
            match outcome {
                Outcome::Failed(code) => DesktopNote {
                    title: "Command failed".to_owned(),
                    body: format!("{what} failed with status {} after {took}", code.0),
                },
                _ => DesktopNote {
                    title: "Command finished".to_owned(),
                    body: format!("{what} finished after {took}"),
                },
            }
        }
        Desk::Program { title, body } => DesktopNote {
            title: match title.is_empty() {
                true => "Terminal".to_owned(),
                false => title.clone(),
            },
            body: body.clone(),
        },
        Desk::Bell => DesktopNote {
            title: "Terminal".to_owned(),
            body: "A bell rang in a terminal pane".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::output_name;

    #[test]
    fn output_names_come_from_the_command_or_a_fallback() {
        assert_eq!(output_name("make all"), "make all output.txt");
        assert_eq!(output_name("  "), "Command output.txt");
        assert_eq!(output_name(&"x".repeat(60)).len(), 40 + " output.txt".len());
    }
}
