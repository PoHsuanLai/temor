//! The block machine and the store together: marks in odd orders, nested prompts, a reset,
//! scrollback trimming and the bound on the store.

use super::{Author, Block, BlockIn, BlockOut, BlockState, BlockStore, Outcome, block_step};
use crate::vocab::ids::{BlockId, ExitCode, GridRow};
use crate::vocab::text::CommandText;
use ds_core::time::stamp::Stamp;

/// Feeds marks through the machine into `store`, returning the final state.
struct Shell {
    state: BlockState,
    store: BlockStore,
    next_id: u64,
    clock: u64,
}

impl Shell {
    fn new(cap: usize) -> Self {
        Self {
            state: BlockState::Idle,
            store: BlockStore::with_cap(cap),
            next_id: 1,
            clock: 0,
        }
    }

    fn tick(&mut self) -> Stamp {
        self.clock += 100;
        Stamp(self.clock)
    }

    fn feed(&mut self, input: BlockIn) -> Vec<BlockId> {
        let (next, outs) = block_step(std::mem::replace(&mut self.state, BlockState::Idle), input);
        self.state = next;
        outs.iter().flat_map(|out| self.store.apply(out)).collect()
    }

    fn prompt(&mut self, row: u64) {
        let at = self.tick();
        self.feed(BlockIn::PromptStart {
            row: GridRow(row),
            at,
        });
    }

    fn typed(&mut self, row: u64) {
        self.feed(BlockIn::CommandStart { row: GridRow(row) });
    }

    fn run(&mut self, row: u64, command: &str) -> BlockId {
        let id = BlockId(self.next_id);
        self.next_id += 1;
        let at = self.tick();
        self.feed(BlockIn::OutputStart {
            id,
            row: GridRow(row),
            command: CommandText(command.into()),
            by: Author::Person,
            at,
        });
        id
    }

    fn finish(&mut self, row: u64, exit: Option<i32>) {
        let at = self.tick();
        self.feed(BlockIn::Finished {
            row: GridRow(row),
            exit: exit.map(ExitCode),
            at,
        });
    }

    fn outcomes(&self) -> Vec<Outcome> {
        self.store.iter().map(|b| b.outcome()).collect()
    }
}

#[test]
fn a_normal_command_makes_one_block_with_its_exit_and_time() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "make");
    sh.finish(3, Some(0));
    let block = sh.store.newest().expect("a block");
    assert_eq!(block.outcome(), Outcome::Succeeded);
    assert_eq!(block.command.0, "make");
    assert_eq!(block.elapsed_ms(), Some(100));
    assert_eq!(sh.state, BlockState::Finished(block.clone()));
}

#[test]
fn a_missing_b_still_makes_a_block_and_a_failure_keeps_its_code() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.run(1, "false");
    sh.finish(1, Some(1));
    assert_eq!(sh.outcomes(), vec![Outcome::Failed(ExitCode(1))]);
}

#[test]
fn a_missing_d_is_closed_by_the_next_prompt_as_unreported() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "sleep 9");
    sh.prompt(2);
    assert_eq!(sh.outcomes(), vec![Outcome::Unreported]);
    sh.typed(2);
    sh.run(3, "ls");
    sh.finish(4, Some(0));
    assert_eq!(sh.outcomes(), vec![Outcome::Unreported, Outcome::Succeeded]);
}

#[test]
fn an_empty_line_or_ctrl_c_at_the_prompt_makes_no_block() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.finish(1, Some(130));
    sh.prompt(1);
    sh.typed(1);
    sh.finish(2, Some(0));
    assert!(sh.store.is_empty());
}

#[test]
fn a_nested_shell_keeps_the_outer_block_open_until_the_inner_one_is_done() {
    // The outer shell runs `bash`. The inner shell's prompt ends the outer block as unknown;
    // when the outer shell's own D arrives later it has no open block and is ignored.
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "bash");
    sh.prompt(2);
    sh.typed(2);
    sh.run(3, "ls");
    sh.finish(4, Some(0));
    sh.finish(5, Some(0));
    sh.prompt(6);
    assert_eq!(sh.outcomes(), vec![Outcome::Unreported, Outcome::Succeeded]);
    assert_eq!(sh.store.len(), 2);
}

#[test]
fn marks_after_a_clear_screen_keep_their_blocks() {
    // A clear sends no mark: the blocks on either side of it are unaffected.
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "one");
    sh.finish(1, Some(0));
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "two");
    sh.finish(1, Some(2));
    assert_eq!(
        sh.outcomes(),
        vec![Outcome::Succeeded, Outcome::Failed(ExitCode(2))]
    );
}

#[test]
fn a_reset_mid_command_ends_it_unreported_and_the_next_prompt_starts_cleanly() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "vim");
    let at = sh.tick();
    sh.feed(BlockIn::Reset { at });
    sh.prompt(0);
    assert_eq!(sh.outcomes(), vec![Outcome::Unreported]);
    assert!(matches!(sh.state, BlockState::Prompting { .. }));
}

#[test]
fn a_stray_d_after_the_block_is_closed_is_ignored() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "x");
    sh.finish(1, Some(0));
    sh.finish(1, Some(9));
    assert_eq!(sh.outcomes(), vec![Outcome::Succeeded]);
}

#[test]
fn the_store_drops_the_oldest_finished_block_when_full() {
    let mut sh = Shell::new(2);
    for code in [0, 1, 2] {
        sh.prompt(0);
        sh.typed(0);
        sh.run(1, "c");
        sh.finish(1, Some(code));
    }
    assert_eq!(
        sh.outcomes(),
        vec![Outcome::Failed(ExitCode(1)), Outcome::Failed(ExitCode(2))]
    );
}

#[test]
fn trimmed_scrollback_takes_its_blocks_with_it() {
    let mut sh = Shell::new(10);
    for _ in 0..3 {
        sh.prompt(0);
        sh.typed(0);
        sh.run(1, "c");
        sh.finish(1, Some(0));
    }
    let gone = sh.store.retain_live(|id| id.0 >= 2);
    assert_eq!(gone, vec![BlockId(1)]);
    assert_eq!(sh.store.len(), 2);
    assert!(sh.store.get(BlockId(1)).is_none());
    assert!(sh.store.get(BlockId(3)).is_some());
}

#[test]
fn a_shell_exit_mid_command_closes_the_block_and_nothing_follows() {
    let mut sh = Shell::new(10);
    sh.prompt(0);
    sh.typed(0);
    sh.run(1, "exit");
    let at = sh.tick();
    sh.feed(BlockIn::ShellExited { at });
    assert_eq!(sh.outcomes(), vec![Outcome::Unreported]);
    sh.prompt(0);
    assert_eq!(sh.state, BlockState::Exited);
}

#[test]
fn a_full_store_drops_the_oldest_finished_block_before_a_running_one() {
    let mut sh = Shell::new(2);
    sh.prompt(0);
    sh.typed(0);
    let running = sh.run(1, "sleep");
    sh.prompt(2);
    sh.typed(2);
    sh.run(3, "a");
    sh.finish(3, Some(0));
    // The third block evicts the finished one, not the older running one.
    sh.prompt(4);
    sh.typed(4);
    sh.run(5, "b");
    assert!(sh.store.get(running).is_some(), "the running block is kept");
    assert_eq!(sh.store.len(), 2);
}

#[test]
fn a_full_store_of_running_blocks_drops_the_oldest() {
    let mut store = BlockStore::with_cap(1);
    let mut sh = Shell::new(1);
    sh.prompt(0);
    sh.typed(0);
    let first = sh.run(1, "one");
    let gone = {
        let block = sh.store.get(first).expect("block").clone();
        store.apply(&BlockOut::CommandStarted(block.clone()));
        store.apply(&BlockOut::CommandStarted(Block {
            id: BlockId(9),
            ..block
        }))
    };
    assert_eq!(gone, vec![first]);
}
