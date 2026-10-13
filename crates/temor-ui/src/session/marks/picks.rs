//! Picking whole blocks, as Warp does: a click on a block picks it, Shift+click picks the run
//! from the last pick, Ctrl+click (⌘ on a Mac) adds or drops one, and a drag still selects text.
//! While blocks are picked, ↑ and ↓ move the pick, Shift+↑ and Shift+↓ grow it, Escape lets go,
//! and copy takes each picked block whole: its command, then its output. ⌘↑ picks the newest
//! block (or the one above the pick) and ⌘↓ the one below. Any other key goes to the shell and lets go of the pick. The rules are
//! `temor_core::block::Picks`; this reads the blocks and moves the view.

use temor_core::block::{PickClick, PickStep};
use temor_core::vocab::ids::BlockId;
use temor_vt::{KeyAction, KeyCode, KeyInput, Modifiers, RowCount, SurfacePoint, ViewportScroll};

use super::super::Session;

impl Session {
    /// The blocks that can be picked, oldest first: those the scrollback still holds.
    fn pick_order(&self) -> Vec<BlockId> {
        self.blocks
            .store
            .iter()
            .filter(|block| self.block_rows(block.id).is_some())
            .map(|block| block.id)
            .collect()
    }

    /// The picked blocks, oldest first.
    pub fn picked_blocks(&self) -> Vec<BlockId> {
        self.picks.in_order(&self.pick_order())
    }

    /// Whether `id` is picked.
    pub fn block_picked(&self, id: BlockId) -> bool {
        self.picks.contains(id)
    }

    /// A click on block `id`, picking it as `how` says.
    pub(in crate::session) fn pick_block(&mut self, id: BlockId, how: PickClick) {
        let order = self.pick_order();
        self.picks.click(id, how, &order);
    }

    /// A click that never left its cell, at `at`: it picks the block there (Shift: the run from
    /// the last pick; Ctrl or ⌘: in or out), or lets go of the pick where there is no block.
    pub(in crate::session) fn pick_at(&mut self, at: SurfacePoint, mods: Modifiers) {
        let Some(fit) = self.fit else {
            return;
        };
        let Ok(top) = self.viewport_top() else {
            return;
        };
        let row = top.saturating_add(u32::from(fit.cell_at(at).pos.row.0));
        let alternate = self.pane.with_term(|term| term.on_alternate_screen());
        match self.block_at_row(row).filter(|_| !alternate) {
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
            None => self.picks.clear(),
        }
    }

    /// ⌘↑ or ⌘↓, or an arrow while blocks are picked: the pick moves that way (with nothing
    /// picked, ⌘↑ picks the newest block) and the view follows it.
    pub(in crate::session) fn step_pick(&mut self, step: PickStep) {
        let order = self.pick_order();
        self.picks.step(step, &order);
        self.reveal_pick();
    }

    /// Brings the active pick's prompt into view (to the top, when it is out of view); with
    /// nothing picked, the live screen.
    fn reveal_pick(&mut self) {
        let first = self
            .picks
            .active()
            .and_then(|id| self.block_rows(id))
            .map(|(first, _)| first);
        self.pane.with_term(|term| {
            let Some(first) = first else {
                term.scroll(ViewportScroll::Bottom);
                return;
            };
            let shown = term
                .viewport()
                .is_ok_and(|(top, rows)| (top..top.saturating_add(rows)).contains(&first));
            if !shown {
                term.scroll(ViewportScroll::Row(RowCount(u64::from(first))));
            }
        });
    }

    /// A key while blocks are picked: the arrows and Escape act on the pick and are used up.
    /// Returns whether the key was the pick's.
    pub(in crate::session) fn pick_key(&mut self, key: &KeyInput) -> bool {
        if self.picks.is_empty() || key.action == KeyAction::Release {
            return false;
        }
        let order = self.pick_order();
        let shift = key.mods == Modifiers::SHIFT;
        let plain = key.mods == Modifiers::NONE;
        match key.code {
            KeyCode::ArrowUp if plain => self.step_pick(PickStep::Older),
            KeyCode::ArrowDown if plain => self.step_pick(PickStep::Newer),
            KeyCode::ArrowUp if shift => {
                self.picks.extend(PickStep::Older, &order);
                self.reveal_pick();
            }
            KeyCode::ArrowDown if shift => {
                self.picks.extend(PickStep::Newer, &order);
                self.reveal_pick();
            }
            KeyCode::Escape if plain => self.picks.clear(),
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
            self.picks.clear();
        }
    }

    /// The picked blocks as copy takes them: each block's command, then its output, with a
    /// blank line between blocks. None when nothing is picked.
    pub(in crate::session) fn picked_text(&self) -> Option<String> {
        let blocks: Vec<String> = self
            .picked_blocks()
            .into_iter()
            .map(|id| {
                let command = self.block_command(id).unwrap_or_default().trim();
                match self.block_output(id).filter(|out| !out.is_empty()) {
                    Some(output) => format!("{command}\n{output}"),
                    None => command.to_owned(),
                }
            })
            .collect();
        (!blocks.is_empty()).then(|| blocks.join("\n\n"))
    }
}
