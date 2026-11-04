//! Picking whole blocks, as Warp does: a click on a block picks it, Shift+click picks the run
//! from the last pick, Ctrl+click (⌘ on a Mac) adds or drops one, and a drag still selects text.
//! While blocks are picked, ↑ and ↓ move the pick, Shift+↑ and Shift+↓ grow it, Escape lets go,
//! and copy takes each picked block whole: its command, then its output. ⌘↑ picks the newest
//! block (or the one above the pick) and ⌘↓ the one below. ⌘A picks every block, ⌘⇧C and ⌘⇧O
//! copy only the commands or only the outputs, ⌘I puts the commands back in the prompt, and ⌘B
//! bookmarks the picked blocks; Alt+↑ and Alt+↓ step between bookmarks while the pane has one
//! and is at its prompt. Any other key goes to the shell and lets go of the pick. The rules are
//! `temor_client::model::block::Picks`; this reads the blocks and moves the view.

use std::collections::BTreeSet;

use temor_client::model::block::{BlockState, HistoryStep, PickClick};
use temor_client::model::session::{AnnotateIn, AnnotateOut, annotate_step};
use temor_client::model::vocab::ids::BlockId;
use temor_client::model::vocab::text::CommandText;
use temor_panes::vt::{KeyAction, KeyCode, KeyInput, Modifiers, SurfacePoint};

use super::super::Session;
use crate::error::ViewError;

impl Session {
    /// The blocks that can be picked, oldest first: those the scrollback still holds.
    fn pick_order(&self) -> Vec<BlockId> {
        self.frame()
            .map(|frame| frame.pick_order())
            .unwrap_or_default()
    }

    /// The picked blocks, oldest first.
    pub fn picked_blocks(&self) -> Vec<BlockId> {
        self.ann.picked_in(&self.pick_order())
    }

    /// Changes the picks, folds and bookmarks by `input`, and does what the step asks for.
    pub(in crate::session) fn annotate(&mut self, input: AnnotateIn) {
        let (ann, outs) = annotate_step(std::mem::take(&mut self.ann), input);
        self.ann = ann;
        if outs.contains(&AnnotateOut::FoldsChanged) {
            self.relayout();
        }
    }

    /// A click on block `id`, picking it as `how` says.
    pub(in crate::session) fn pick_block(&mut self, id: BlockId, how: PickClick) {
        let order = self.pick_order();
        self.annotate(AnnotateIn::Click { id, how, order });
    }

    /// A click that never left its cell, at `at`: it picks the block there (Shift: the run from
    /// the last pick; Ctrl or ⌘: in or out), or lets go of the pick where there is no block.
    pub(in crate::session) fn pick_at(&mut self, at: SurfacePoint, mods: Modifiers) {
        let Some(hit) = self.point_hit(at) else {
            return;
        };
        let alternate = self.pane.with_term(|term| term.on_alternate_screen());
        match self.hit_block(&hit).filter(|_| !alternate) {
            Some(id) => {
                let how = match () {
                    _ if mods.contains(Modifiers::SHIFT) => PickClick::Range,
                    _ if mods.contains(Modifiers::CTRL) || mods.contains(Modifiers::SUPER) => {
                        PickClick::Toggle
                    }
                    _ => PickClick::Only,
                };
                self.pick_block(id, how);
            }
            None => self.annotate(AnnotateIn::ClearPicks),
        }
    }

    /// ⌘↑ or ⌘↓, or an arrow while blocks are picked: the pick moves that way (with nothing
    /// picked, ⌘↑ picks the newest block) and the view follows it.
    pub(in crate::session) fn step_pick(&mut self, step: HistoryStep) {
        let order = self.pick_order();
        self.annotate(AnnotateIn::Step { step, order });
    }

    /// A key while blocks are picked: the arrows and Escape act on the pick and are used up.
    /// Alt+↑ and Alt+↓ step between bookmarks while there is one and the shell is at its prompt,
    /// picked or not. Returns whether the key was the pick's.
    pub(in crate::session) fn pick_key(&mut self, key: &KeyInput) -> bool {
        if key.action == KeyAction::Release {
            return false;
        }
        if key.mods == Modifiers::ALT && self.bookmark_keys_live() {
            match key.code {
                KeyCode::ArrowUp => {
                    self.step_bookmark(HistoryStep::Older);
                    return true;
                }
                KeyCode::ArrowDown => {
                    self.step_bookmark(HistoryStep::Newer);
                    return true;
                }
                _ => {}
            }
        }
        if self.ann.nothing_picked() {
            return false;
        }
        let order = self.pick_order();
        let shift = key.mods == Modifiers::SHIFT;
        let plain = key.mods == Modifiers::NONE;
        match key.code {
            KeyCode::ArrowUp if plain => self.step_pick(HistoryStep::Older),
            KeyCode::ArrowDown if plain => self.step_pick(HistoryStep::Newer),
            KeyCode::ArrowUp if shift => {
                self.annotate(AnnotateIn::Extend {
                    step: HistoryStep::Older,
                    order,
                });
            }
            KeyCode::ArrowDown if shift => {
                self.annotate(AnnotateIn::Extend {
                    step: HistoryStep::Newer,
                    order,
                });
            }
            KeyCode::Escape if plain => self.annotate(AnnotateIn::ClearPicks),
            _ => return false,
        }
        true
    }

    /// A key that goes on to the shell lets go of the pick (a modifier alone does not).
    pub(in crate::session) fn key_leaves_pick(&mut self, key: &KeyInput) {
        let modifier = matches!(
            key.code,
            KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::ControlLeft
                | KeyCode::ControlRight
                | KeyCode::AltLeft
                | KeyCode::AltRight
                | KeyCode::MetaLeft
                | KeyCode::MetaRight
        );
        if key.action != KeyAction::Release && !modifier {
            self.annotate(AnnotateIn::ClearPicks);
        }
    }

    /// The picked blocks as copy takes them: each block's command, then its output, with a
    /// blank line between blocks. None when nothing is picked.
    pub(in crate::session) fn picked_text(&self) -> Option<String> {
        let blocks: Vec<String> = self
            .picked_blocks()
            .into_iter()
            .map(|id| {
                let command = self.block_command(id).unwrap_or_default();
                let command = command.trim();
                match self.block_output(id).filter(|out| !out.is_empty()) {
                    Some(output) => format!("{command}\n{output}"),
                    None => command.to_owned(),
                }
            })
            .collect();
        (!blocks.is_empty()).then(|| blocks.join("\n\n"))
    }

    /// Whether Alt+↑ and Alt+↓ are the bookmarks': some block is bookmarked and the shell is at
    /// its prompt, on the main screen.
    fn bookmark_keys_live(&self) -> bool {
        let at_prompt = matches!(
            *self.core().blocks.machine(),
            BlockState::Prompting { .. } | BlockState::Typing { .. }
        );
        self.ann.any_bookmarked()
            && at_prompt
            && !self.pane.with_term(|term| term.on_alternate_screen())
    }

    /// Picks the nearest bookmark `step`'s way from the pick; none that way leaves the pick.
    pub(in crate::session) fn step_bookmark(&mut self, step: HistoryStep) {
        let order = self.pick_order();
        self.annotate(AnnotateIn::StepBookmark { step, order });
    }

    /// Whether `id` is bookmarked.
    pub fn block_bookmarked(&self, id: BlockId) -> bool {
        self.ann.is_bookmarked(id)
    }

    /// The picked blocks' commands, as typed, oldest first.
    fn picked_commands(&self) -> Vec<String> {
        self.picked_blocks()
            .into_iter()
            .filter_map(|id| self.block_command(id))
            .map(|command| command.trim().to_owned())
            .filter(|command| !command.is_empty())
            .collect()
    }

    /// ⌘⇧C: the picked commands, one per line.
    pub(in crate::session) fn copy_picked_commands(&mut self) {
        let commands = self.picked_commands();
        if !commands.is_empty() {
            self.copy_text(&commands.join("\n"));
        }
    }

    /// ⌘⇧O: the picked blocks' outputs, a blank line between them; a block still running has
    /// none yet.
    pub(in crate::session) fn copy_picked_outputs(&mut self) {
        let outputs: Vec<String> = self
            .picked_blocks()
            .into_iter()
            .filter_map(|id| self.block_output(id))
            .filter(|output| !output.is_empty())
            .collect();
        if !outputs.is_empty() {
            self.copy_text(&outputs.join("\n\n"));
        }
    }

    /// ⌘A: every block.
    pub(in crate::session) fn pick_all(&mut self) {
        let order = self.pick_order();
        self.annotate(AnnotateIn::SelectAll { order });
    }

    /// ⌘B: bookmarks the picked blocks, or, when every one of them is bookmarked already, takes
    /// their bookmarks away.
    pub(in crate::session) fn toggle_bookmarks(&mut self) {
        let order = self.pick_order();
        self.annotate(AnnotateIn::ToggleBookmarks { order });
    }

    /// ⌘I: the picked commands in the prompt, joined with `; ` (never a line break, so nothing
    /// runs), in the composer while it shows, else through the paste path; the pick is let go.
    /// Refused with a notice when one of them cannot be typed.
    pub(in crate::session) fn reinput_picked(&mut self) {
        let commands = self.picked_commands();
        if commands.is_empty() {
            return;
        }
        if !self.put_in_prompt(CommandText(commands.join("; "))) {
            self.notice = Some("These commands cannot be typed again.".to_owned());
            return;
        }
        self.annotate(AnnotateIn::ClearPicks);
    }

    /// A right-click at `at`: the block there, for its menu. It is picked, alone, unless it is
    /// in the pick already (then the pick stays, so the menu acts on all of it), and no text
    /// stays selected. None where there is no block, on a full-screen program's screen, or while
    /// the program has the mouse: the click is the program's then.
    pub fn block_menu_press(&mut self, at: SurfacePoint) -> Result<Option<BlockId>, ViewError> {
        let Some(hit) = self.point_hit(at) else {
            return Ok(None);
        };
        let (alternate, tracking) = self
            .pane
            .with_term(|term| (term.on_alternate_screen(), term.mouse_tracking()));
        if alternate || tracking {
            return Ok(None);
        }
        let Some(id) = self.hit_block(&hit) else {
            return Ok(None);
        };
        if !self.ann.is_picked(id) {
            self.pick_block(id, PickClick::Only);
        }
        self.selection = None;
        self.mirror_selection()?;
        Ok(Some(id))
    }

    /// The card `id`'s "…": the block, for its menu, picked as a right-click on it picks it.
    pub fn block_menu_for(&mut self, id: BlockId) -> Result<(), ViewError> {
        if !self.ann.is_picked(id) {
            self.pick_block(id, PickClick::Only);
        }
        self.selection = None;
        self.mirror_selection()
    }

    /// Whether `id`'s output is folded away.
    pub fn block_folded(&self, id: BlockId) -> bool {
        self.ann.is_folded(id)
    }

    /// The blocks in `order` that have output to fold: finished, and with a row below the command.
    fn foldable_blocks(&self, order: &[BlockId]) -> BTreeSet<BlockId> {
        let frame = self.frame();
        order
            .iter()
            .copied()
            .filter(|&id| {
                self.block_finished(id) && frame.as_ref().and_then(|f| f.output_rows(id)).is_some()
            })
            .collect()
    }

    /// Whether every picked block that has output is folded (and some has).
    pub fn picks_folded(&self) -> bool {
        let order = self.pick_order();
        self.ann.picks_folded(&order, &self.foldable_blocks(&order))
    }

    /// Folds the picked blocks' output away, or, when every one that has output is folded
    /// already, brings it back.
    pub(in crate::session) fn toggle_folds(&mut self) {
        let order = self.pick_order();
        let foldable = self.foldable_blocks(&order);
        self.annotate(AnnotateIn::ToggleFolds { order, foldable });
    }

    /// Whether `id` has output to fold: it is finished and has a row below its command.
    pub fn block_foldable(&self, id: BlockId) -> bool {
        self.block_finished(id)
            && self
                .frame()
                .and_then(|frame| frame.output_rows(id))
                .is_some()
    }

    /// Folds `id`'s output away, or brings it back. Nothing for a block with no output to fold.
    pub fn toggle_fold_of(&mut self, id: BlockId) {
        let foldable = self.block_foldable(id);
        self.annotate(AnnotateIn::ToggleFold { id, foldable });
    }

    /// Whether every picked block is bookmarked (and some block is picked).
    pub fn picks_bookmarked(&self) -> bool {
        self.ann.picks_bookmarked(&self.pick_order())
    }
}
