//! A pane's shell marks (OSC 133) and the blocks they make. Each mark arrives in three parts:
//! its number (`Begin`), its own event (`Event`), then its anchor (`Anchored`) with the same
//! number. The anchor is the terminal's tracked reference to the mark's cell; this module knows
//! it only as an opaque [`Anchor`] the edge can resolve, and says which anchors to give back.
//! A mark's anchor is kept only when the block machine accepts the mark, so an ignored mark
//! leaves nothing behind.
//!
//! Everything the edge reads (the clock, whether the anchor resolved, the typed text) comes in
//! on the input; everything it must do (give anchors back, announce a finished block) comes out.

use std::collections::BTreeMap;

use ds_core::time::stamp::Stamp;

use crate::block::{Author, Block, BlockIn, BlockOut, BlockState, BlockStore, block_step};
use temor_core::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use temor_core::vocab::text::CommandText;

/// One tracked reference to a cell of the terminal, as the edge numbers them. Ids are never
/// reused within a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Anchor(pub u64);

/// The anchors of one block: where its prompt, its typed command, its output and its end are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockAnchors {
    pub prompt: Option<Anchor>,
    pub input: Option<Anchor>,
    pub output: Option<Anchor>,
    pub end: Option<Anchor>,
}

impl BlockAnchors {
    /// Every anchor the block holds.
    pub fn all(self) -> impl Iterator<Item = Anchor> {
        [self.prompt, self.input, self.output, self.end]
            .into_iter()
            .flatten()
    }
}

/// What one mark says.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarkKind {
    /// A: a prompt starts. `primary` is false for a continuation prompt.
    PromptStart { primary: bool },
    /// B: the person's input starts.
    InputStart,
    /// C: the command's output starts. `command` is the command line when the shell sent it.
    OutputStart { command: String },
    /// D: the command ended. `exit` is none when the shell did not say.
    CommandEnd { exit: Option<ExitCode> },
    /// A mark this build does not know: no block is opened or closed for it.
    Other,
}

/// What the pane's marks are doing: the block machine, the blocks, and the anchors that find
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkLog {
    machine: BlockState,
    store: BlockStore,
    anchors: BTreeMap<BlockId, BlockAnchors>,
    /// The number of the mark in progress.
    pending_seq: Option<u64>,
    /// The mark's own event, waiting for its anchor.
    pending: Option<MarkKind>,
    /// The cursor where the person last pressed Enter: the end of the typed command.
    enter: Option<Anchor>,
    /// The prompt and typed-command anchors of the block being typed, before its id exists.
    prompt: Option<Anchor>,
    input: Option<Anchor>,
    /// The id the next command will get.
    next_id: u64,
    /// How many primary prompts the shell has marked.
    prompts: usize,
    /// How many commands have finished.
    finished: usize,
}

impl Default for MarkLog {
    fn default() -> Self {
        Self {
            machine: BlockState::Idle,
            store: BlockStore::default(),
            anchors: BTreeMap::new(),
            pending_seq: None,
            pending: None,
            enter: None,
            prompt: None,
            input: None,
            next_id: 1,
            prompts: 0,
            finished: 0,
        }
    }
}

impl MarkLog {
    /// Where the block machine stands.
    pub fn machine(&self) -> &BlockState {
        &self.machine
    }

    /// The blocks, oldest first.
    pub fn store(&self) -> &BlockStore {
        &self.store
    }

    /// The anchors of a block the log holds.
    pub fn anchors_of(&self, id: BlockId) -> Option<BlockAnchors> {
        self.anchors.get(&id).copied()
    }

    /// Every block the log holds with its anchors.
    pub fn anchor_table(&self) -> impl Iterator<Item = (BlockId, BlockAnchors)> + '_ {
        self.anchors.iter().map(|(&id, &anchors)| (id, anchors))
    }

    /// The anchor of the prompt being typed at.
    pub fn prompt(&self) -> Option<Anchor> {
        self.prompt
    }

    /// The anchor of the start of the typed command being typed.
    pub fn input(&self) -> Option<Anchor> {
        self.input
    }

    /// The anchor where Enter was last pressed.
    pub fn enter(&self) -> Option<Anchor> {
        self.enter
    }

    /// What the mark in progress is, once its event has come.
    pub fn pending(&self) -> Option<&MarkKind> {
        self.pending.as_ref()
    }

    /// The id the next command will get.
    pub fn next_id(&self) -> BlockId {
        BlockId(self.next_id)
    }

    /// How many primary prompts the shell has marked.
    pub fn prompts(&self) -> usize {
        self.prompts
    }

    /// How many commands have finished.
    pub fn finished(&self) -> usize {
        self.finished
    }
}

/// A finished block found again on a restored screen, with its anchors (see `found`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptBlock {
    pub anchors: BlockAnchors,
    pub prompt: GridSpan,
    pub output: GridSpan,
    pub command: String,
    pub exit: Option<ExitCode>,
    /// How long it ran, when that was kept.
    pub took_ms: Option<u64>,
}

/// What reaches the log.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarkIn {
    /// A mark starts, numbered `seq`. Whatever an earlier mark left pending is dropped.
    Begin { seq: u64 },
    /// A mark's own event. Kept only while its mark is the current one.
    Event(MarkKind),
    /// A mark's anchor arrived. It pairs with the event of the same number; anything else is
    /// given back, so a missed anchor cannot pair with the next mark's event. `row` is where the
    /// edge found the anchor (`None`: it could not). `typed` is the text the person typed, as the
    /// edge read it between the input anchor and Enter's, when it could.
    Anchored {
        seq: u64,
        anchor: Anchor,
        row: Option<GridRow>,
        at: Stamp,
        typed: Option<String>,
    },
    /// The person pressed Enter at the shell, and the cursor sits at the end of what was typed
    /// (`None`: the edge could not anchor it).
    Entered { anchor: Option<Anchor> },
    /// The shell is gone: a running command ends unreported.
    ShellGone { at: Stamp },
    /// The terminal was reset: a running command is no longer on screen.
    Reset { at: Stamp },
    /// The scrollback no longer holds these blocks: they are dropped.
    Gone { ids: Vec<BlockId> },
    /// Finished blocks found again on a screen restored from a snapshot, oldest first. They join
    /// the log as finished blocks, after any it holds; nothing is announced for them.
    Restored { blocks: Vec<KeptBlock>, at: Stamp },
}

/// What the edge does after a step.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarkOut {
    /// Give this anchor back to the terminal.
    Release(Anchor),
    /// A command finished: it may be worth announcing.
    Finished(Block),
    /// These blocks left the log (to make room, or because the scrollback dropped them); what
    /// is kept for them elsewhere can go.
    Dropped(Vec<BlockId>),
}

/// The log after `input`, and what to do about it.
pub fn mark_step(mut log: MarkLog, input: MarkIn) -> (MarkLog, Vec<MarkOut>) {
    let mut outs = Vec::new();
    match input {
        MarkIn::Begin { seq } => {
            log.pending = None;
            log.pending_seq = Some(seq);
        }
        MarkIn::Event(kind) => {
            if log.pending_seq.is_some() {
                log.pending = Some(kind);
            }
        }
        MarkIn::Anchored {
            seq,
            anchor,
            row,
            at,
            typed,
        } => log.anchored(seq, anchor, row, at, typed, &mut outs),
        MarkIn::Entered { anchor } => {
            log.clear_enter(&mut outs);
            log.enter = anchor;
        }
        MarkIn::ShellGone { at } => {
            log.run(BlockIn::ShellExited { at }, &mut outs);
        }
        MarkIn::Reset { at } => {
            log.run(BlockIn::Reset { at }, &mut outs);
        }
        MarkIn::Gone { ids } => {
            let gone = log.store.retain_live(|id| !ids.contains(&id));
            log.forget(gone, &mut outs);
        }
        MarkIn::Restored { blocks, at } => {
            for kept in blocks {
                let id = BlockId(log.next_id);
                log.next_id += 1;
                let took = kept.took_ms.unwrap_or(0);
                let block = Block {
                    id,
                    prompt: kept.prompt,
                    command: CommandText::from_shell(&kept.command),
                    output: kept.output,
                    exit: kept.exit,
                    by: Author::Person,
                    started: Stamp(at.0.saturating_sub(took)),
                    ended: Some(at),
                };
                log.prompts += 1;
                log.finished += 1;
                log.anchors.insert(id, kept.anchors);
                let gone = log.store.apply(&BlockOut::BlockDone(block));
                log.forget(gone, &mut outs);
            }
        }
    }
    (log, outs)
}

impl MarkLog {
    fn clear_enter(&mut self, outs: &mut Vec<MarkOut>) {
        if let Some(old) = self.enter.take() {
            outs.push(MarkOut::Release(old));
        }
    }

    /// Gives back the anchors of blocks that left the store, and says so.
    fn forget(&mut self, gone: Vec<BlockId>, outs: &mut Vec<MarkOut>) {
        for id in &gone {
            if let Some(held) = self.anchors.remove(id) {
                outs.extend(held.all().map(MarkOut::Release));
            }
        }
        if !gone.is_empty() {
            outs.push(MarkOut::Dropped(gone));
        }
    }

    /// Runs the block machine on one input and applies what it says. Returns whether the machine
    /// took the input (changed state): an ignored input leaves nothing to keep.
    fn run(&mut self, input: BlockIn, outs: &mut Vec<MarkOut>) -> bool {
        let machine = std::mem::replace(&mut self.machine, BlockState::Idle);
        let before = machine.clone();
        let (next, made) = block_step(machine, input);
        let taken = next != before;
        self.machine = next;
        for out in &made {
            if let BlockOut::BlockDone(block) = out {
                self.finished += 1;
                outs.push(MarkOut::Finished(block.clone()));
            }
            let gone = self.store.apply(out);
            self.forget(gone, outs);
        }
        taken
    }

    fn anchored(
        &mut self,
        seq: u64,
        anchor: Anchor,
        row: Option<GridRow>,
        at: Stamp,
        typed: Option<String>,
        outs: &mut Vec<MarkOut>,
    ) {
        let current = self.pending_seq.take();
        let mark = self.pending.take();
        let (Some(current), Some(mark)) = (current, mark) else {
            return outs.push(MarkOut::Release(anchor));
        };
        let Some(row) = row.filter(|_| current == seq) else {
            return outs.push(MarkOut::Release(anchor));
        };
        match mark {
            MarkKind::PromptStart { primary: true } => {
                if self.run(BlockIn::PromptStart { row, at }, outs) {
                    self.prompts += 1;
                    if let Some(old) = self.prompt.replace(anchor) {
                        outs.push(MarkOut::Release(old));
                    }
                    if let Some(old) = self.input.take() {
                        outs.push(MarkOut::Release(old));
                    }
                    self.clear_enter(outs);
                } else {
                    outs.push(MarkOut::Release(anchor));
                }
            }
            MarkKind::InputStart => {
                if self.run(BlockIn::CommandStart { row }, outs) {
                    if let Some(old) = self.input.replace(anchor) {
                        outs.push(MarkOut::Release(old));
                    }
                } else {
                    outs.push(MarkOut::Release(anchor));
                }
            }
            MarkKind::OutputStart { command } => {
                self.output_start(anchor, row, at, &command, typed, outs)
            }
            MarkKind::CommandEnd { exit } => {
                let running = match &self.machine {
                    BlockState::Running(block) => Some(block.id),
                    _ => None,
                };
                let taken = self.run(BlockIn::Finished { row, exit, at }, outs);
                match (taken, running) {
                    (true, Some(block)) => {
                        self.anchors.entry(block).or_default().end = Some(anchor)
                    }
                    _ => outs.push(MarkOut::Release(anchor)),
                }
            }
            // A secondary prompt, or a mark this build does not know.
            MarkKind::PromptStart { primary: false } | MarkKind::Other => {
                outs.push(MarkOut::Release(anchor))
            }
        }
    }

    /// The command started: its block keeps the prompt, input and output anchors, if accepted.
    fn output_start(
        &mut self,
        anchor: Anchor,
        row: GridRow,
        at: Stamp,
        command: &str,
        typed: Option<String>,
        outs: &mut Vec<MarkOut>,
    ) {
        let input = self.input.take();
        if let Some(enter) = self.enter.take() {
            outs.push(MarkOut::Release(enter));
        }
        let text = match command.is_empty() {
            true => typed.unwrap_or_default(),
            false => command.to_owned(),
        };
        let block = BlockId(self.next_id);
        let accepted = self.run(
            BlockIn::OutputStart {
                id: block,
                row,
                command: CommandText::from_shell(&text),
                by: Author::Person,
                at,
            },
            outs,
        );
        if accepted {
            self.next_id += 1;
            // The input anchor stays: the card's command rows (drawn bold) run from it to the
            // output anchor.
            self.anchors.insert(
                block,
                BlockAnchors {
                    prompt: self.prompt.take(),
                    input,
                    output: Some(anchor),
                    end: None,
                },
            );
        } else {
            // An ignored command keeps nothing: its anchors go back to the terminal.
            outs.push(MarkOut::Release(anchor));
            outs.extend(self.prompt.take().map(MarkOut::Release));
            outs.extend(input.map(MarkOut::Release));
        }
    }
}

#[cfg(test)]
#[path = "marks_tests.rs"]
mod tests;
