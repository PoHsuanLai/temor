//! Carrying out the row the person picked in the bar (PLAN §7, D6). Every row is the person's
//! own pick. A command typed for them is sent with Enter only on the Run row, which says it
//! will run; anything else waits at the prompt for their Enter.

use temor_core::bar::{BarAction, BarRow, Target, Visits};
use temor_core::vocab::text::{CommandText, Cwd};
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};

use super::hosts::HostRows;
use super::model::Hub;
use crate::session::Session;

/// Whether a command typed into a new tab is submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Submit {
    /// Enter is sent: the row said it runs.
    Enter,
    /// The command waits at the prompt for the person's Enter.
    Wait,
}

impl Hub {
    /// Does what `row` says.
    pub(super) fn run_bar_row(&mut self, row: BarRow) {
        match row.action {
            BarAction::NewTab => self.act(WorkspaceIn::NewTab { cwd: None }),
            BarAction::Run(command) => self.command_in_new_tab(&command, Submit::Enter),
            BarAction::Go(target) => self.go_to(target),
            BarAction::Plugin { plugin, item } => self.plugin_pick(plugin, item),
            ask @ (BarAction::Suggest(_) | BarAction::AskAgent(_) | BarAction::AskAbout(_)) => {
                if let Err(why) = self.bar.seams.model.ask(&ask) {
                    self.notices.push(why);
                }
            }
        }
    }

    fn go_to(&mut self, target: Target) {
        match target {
            Target::Tab(tab) => self.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(tab)))),
            Target::Closed(_) => self.act(WorkspaceIn::Reopen),
            Target::Action(action) => self.run_window_action(action),
            Target::Folder(folder) => {
                let first = self
                    .ws
                    .tabs
                    .place
                    .tabs
                    .iter()
                    .find(|tab| tab.folder == Some(folder))
                    .map(|tab| tab.id);
                if let Some(tab) = first {
                    self.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(tab))));
                }
            }
            Target::Space(space) => {
                if let Err(why) = self.bar.seams.spaces.go_to(space) {
                    self.notices.push(why);
                }
            }
            Target::Host(host) => {
                let row = self.host_rows().into_iter().find(|row| {
                    row.destination()
                        .is_some_and(|d| d.as_str() == host.as_str())
                });
                match row {
                    Some(row) => self.open_host(&row),
                    None => self.open_host_named(host.as_str()),
                }
            }
            Target::RecentCommand(command) => self.command_in_new_tab(&command, Submit::Wait),
            Target::Directory { cwd, .. } => {
                self.note_visit(&cwd);
                self.act(WorkspaceIn::NewTab { cwd: Some(cwd) });
            }
            // A tab given a folder is never taken to the host: this computer's shell, at home.
            Target::LocalTab => {
                let home = self.launch.ambient.home.to_string_lossy().into_owned();
                self.act(WorkspaceIn::NewTab {
                    cwd: Some(Cwd(home)),
                });
            }
            // Saved layouts are not stored anywhere yet, so none is offered.
            Target::Layout(_) => {}
        }
    }

    /// One more visit to `cwd`, at now.
    fn note_visit(&mut self, cwd: &Cwd) {
        let now = (self.bar.seams.now)();
        let before = self
            .bar
            .visits
            .get(cwd.as_str())
            .copied()
            .unwrap_or(Visits {
                count: 0,
                last: now,
            });
        self.bar
            .visits
            .insert(cwd.as_str().to_owned(), before.visited(now));
    }

    /// Opens a tab and types `command` into its shell.
    fn command_in_new_tab(&mut self, command: &CommandText, submit: Submit) {
        self.act(WorkspaceIn::NewTab { cwd: None });
        let Some(slot) = self.keyboard_pane().and_then(|pane| self.slots.get(&pane)) else {
            return;
        };
        if let Some(session) = slot.borrow().session.as_ref() {
            type_into(session, command, submit);
        }
    }
}

/// Types a command the person chose. A control character in it is never sent.
fn type_into(session: &Session, command: &CommandText, submit: Submit) {
    if !command.typable() {
        return;
    }
    let tail = match submit {
        Submit::Enter => "\n",
        Submit::Wait => "",
    };
    session
        .pane()
        .send(format!("{}{tail}", command.as_str()).as_bytes());
}
