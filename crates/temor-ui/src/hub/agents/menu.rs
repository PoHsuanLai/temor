//! What the views show about agents, prepared as plain data: the choices in a tab's menu, the
//! sheet over the window, and the row's caption. The words are the person's, no jargon.

use temor_core::checkpoint::IGNORED_NOT_COVERED;
use temor_core::vocab::ids::{BlockId, TabId};
use temor_core::worktree::overlap::{Other, TEXT_OVERLAP_ONLY};

use super::state::Sheet;
use crate::hub::model::Hub;

/// What a tab's menu can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentPick {
    /// Put the files back to the checkpoint taken before this turn.
    Restore(BlockId),
    /// Open a worktree on a new branch for the pane's repository, in a new tab.
    RunInWorktree,
    /// Merge the tab's worktree back (Squash).
    MergeBack,
    /// Show the worktree's findings.
    Details,
    /// List the worktrees no tab uses.
    Leftovers,
}

/// Whether the tab works in the person's checkout or in a worktree of temor's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Checkout,
    Worktree,
}

/// The choices of one tab's menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMenu {
    /// The checkpoints offered, oldest first, with the words for each.
    pub checkpoints: Vec<(BlockId, String)>,
    pub place: Where,
}

/// One leftover worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverRow {
    pub path: String,
    pub branch: String,
}

/// The sheet over the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetView {
    Restore {
        title: String,
        put_back: Vec<String>,
        remove: Vec<String>,
        note: &'static str,
    },
    Details {
        tab: TabId,
        title: String,
        lines: Vec<String>,
        note: &'static str,
    },
    Leftovers {
        rows: Vec<LeftoverRow>,
    },
}

/// The most file names a sheet lists; the rest are counted.
const LISTED: usize = 12;

fn names(files: &[temor_core::vocab::text::RelPath]) -> Vec<String> {
    let mut lines: Vec<String> = files
        .iter()
        .take(LISTED)
        .map(|f| f.as_str().to_owned())
        .collect();
    if files.len() > LISTED {
        lines.push(format!("and {} more", files.len() - LISTED));
    }
    lines
}

impl Hub {
    /// The menu of `tab`; `None` when there is nothing to offer (no git).
    pub fn agent_menu(&self, tab: TabId) -> Option<AgentMenu> {
        self.agents.jobs.available().ok()?;
        let pane = self.ws.focus_of(tab)?;
        let checkpoints = self
            .agents
            .runs
            .get(&pane)
            .map(|run| {
                run.marks()
                    .iter()
                    .map(|mark| (mark.block, format!("Before turn {}", mark.block.0)))
                    .collect()
            })
            .unwrap_or_default();
        let place = match self.agents.trees.by_tab.contains_key(&tab) {
            true => Where::Worktree,
            false => Where::Checkout,
        };
        Some(AgentMenu { checkpoints, place })
    }

    /// The sheet to show, if any.
    pub fn agent_sheet(&self) -> Option<SheetView> {
        Some(match self.agents.sheet.as_ref()? {
            Sheet::Restore { plan, .. } => SheetView::Restore {
                title: "Restore to this checkpoint?".to_owned(),
                put_back: names(&plan.write),
                remove: names(&plan.delete),
                note: IGNORED_NOT_COVERED,
            },
            Sheet::Details(tab) => SheetView::Details {
                tab: *tab,
                title: self.tab_name(*tab),
                lines: self.worktree_lines(*tab),
                note: TEXT_OVERLAP_ONLY,
            },
            Sheet::Leftovers { .. } => SheetView::Leftovers {
                rows: self
                    .agents
                    .trees
                    .leftovers
                    .iter()
                    .map(|w| LeftoverRow {
                        path: w.path.clone(),
                        branch: w.branch.clone().unwrap_or_default(),
                    })
                    .collect(),
            },
        })
    }

    pub(super) fn tab_name(&self, tab: TabId) -> String {
        self.ws
            .tabs
            .tab(tab)
            .map_or_else(String::new, |t| t.title.as_str().to_owned())
    }

    /// Branch, the honest word about what a worktree separates, the findings and the counts.
    fn worktree_lines(&self, tab: TabId) -> Vec<String> {
        let Some(tree) = self.agents.trees.by_tab.get(&tab) else {
            return vec!["This tab works in your checkout".to_owned()];
        };
        let mut lines = vec![
            format!("On branch {}", tree.worktree.branch()),
            "Separate files, shared ports".to_owned(),
        ];
        let overlap = self.agents.trees.overlap(tab);
        lines.extend(
            overlap
                .findings
                .iter()
                .map(|f| f.text(|o| self.other_name(o))),
        );
        if let Some(d) = overlap.divergence {
            lines.push(format!(
                "{} ahead, {} behind, {} files changed",
                d.ahead, d.behind, d.files_changed
            ));
        }
        lines
    }

    fn other_name(&self, other: Other) -> String {
        match other {
            Other::Base => "the base".to_owned(),
            Other::Tab(tab) => self.tab_name(tab),
        }
    }

    /// The line under a tab's title: its worktree's first finding (with the note that the check
    /// reads text only), or its branch. `None` for a tab in the person's checkout.
    pub fn agent_note(&self, tab: TabId) -> Option<String> {
        let tree = self.agents.trees.by_tab.get(&tab)?;
        let overlap = self.agents.trees.overlap(tab);
        Some(match overlap.findings.first() {
            Some(finding) => format!(
                "{} ({})",
                finding.text(|o| self.other_name(o)),
                TEXT_OVERLAP_ONLY.to_lowercase()
            ),
            None => format!("On branch {}", tree.worktree.branch()),
        })
    }
}
