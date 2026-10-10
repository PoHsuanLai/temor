//! The window's side of the desktop link: what it publishes to the desktop services after each
//! draw (its panes, commands, context and the dock count), and how it carries out what they ask
//! (open or close a pane, type a command, read text, and the second launch's new tab).

use std::collections::BTreeSet;
use std::path::PathBuf;

use dioxus::prelude::spawn;
use ds_blitz::WindowHandle;
use temor_core::block::Outcome;
use temor_core::notify::Focus as NoteFocus;
use temor_core::split::Direction;
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_desktop::{
    BadgeCount, CommandInfo, CommandRef, Exit, Focus, PaneInfo, PaneKind, PaneOpen, PaneRef,
    Placement, PortRefusal, Request, SpaceRef, WindowContext,
};

use super::desk::Desk;
use super::model::{Hub, HubRef};
use crate::edge::desktop_port::{Act, Answer, Ask, DesktopLink, Snapshot};
use crate::view::title::window_title;

impl Hub {
    /// What the desktop services see now, and how many agents wait on the person, for the dock.
    pub fn desktop_snapshot(&self) -> (Snapshot, BadgeCount) {
        let keyboard = self.keyboard_pane();
        let order = self.ws.tabs.panes();
        let panes: Vec<PaneInfo> = order.iter().map(|p| self.pane_info(*p, keyboard)).collect();
        let commands: std::collections::BTreeMap<_, _> = order
            .iter()
            .map(|p| (PaneRef(p.0), self.commands_of(*p)))
            .collect();
        let focused = keyboard.and_then(|p| panes.iter().find(|i| i.id == PaneRef(p.0)).cloned());
        let last_command = focused
            .as_ref()
            .and_then(|f| commands.get(&f.id))
            .and_then(|list| list.last())
            .cloned();
        let (title, cwd) = self.window_parts();
        let context = WindowContext {
            title: window_title(&title, cwd.as_deref()),
            focused,
            last_command,
            selection: None,
            pane_count: u32::try_from(panes.len()).unwrap_or(u32::MAX),
            // TODO(spaces): the Space the window is in, once Spaces reach the window.
            space: SpaceRef {
                local: 0,
                link: None,
            },
        };
        // The dock asks for the person where an agent waits on them; unread tabs are the tab
        // row's business.
        let waiting = u32::try_from(self.waiting_count()).unwrap_or(u32::MAX);
        (
            Snapshot {
                context,
                panes,
                commands,
            },
            BadgeCount(waiting),
        )
    }

    fn pane_info(&self, pane: PaneId, keyboard: Option<PaneId>) -> PaneInfo {
        let cwd = self
            .live(pane, |s| s.cwd().map(PathBuf::from))
            .flatten()
            .or_else(|| self.ws.cwds.get(&pane).map(|c| PathBuf::from(c.as_str())));
        PaneInfo {
            id: PaneRef(pane.0),
            title: self.pane_name(pane),
            cwd,
            kind: PaneKind::Terminal,
            focus: match keyboard == Some(pane) {
                true => Focus::Focused,
                false => Focus::Background,
            },
        }
    }

    fn commands_of(&self, pane: PaneId) -> Vec<CommandInfo> {
        self.live(pane, |session| {
            session
                .block_outcomes()
                .into_iter()
                .map(|(id, outcome)| CommandInfo {
                    id: CommandRef(id.0),
                    line: session.block_command(id).unwrap_or_default().to_owned(),
                    exit: exit_of(outcome),
                })
                .collect()
        })
        .unwrap_or_default()
    }

    /// Carries out one message from the desktop services.
    pub(super) fn serve(&mut self, ask: Ask) {
        match ask {
            Ask::Launch(request, _token) => self.act(new_tab_for(request)),
            Ask::Act(act, reply) => {
                let _ = reply.send(self.perform(act));
            }
        }
    }

    pub(super) fn perform(&mut self, act: Act) -> Result<Answer, PortRefusal> {
        match act {
            Act::Open(open) => self.open_for_agent(open).map(Answer::Pane),
            Act::Close(pane) => {
                let pane = self.known_pane(pane)?;
                self.act(WorkspaceIn::ClosePane(Some(pane)));
                Ok(Answer::Done)
            }
            Act::Run(pane, line) => self.type_line(pane, &line).map(|()| Answer::Done),
            Act::ReadPane(pane, range) => self.read_rows(pane, range).map(Answer::Text),
            Act::ReadCommand(pane, command) => self.read_output(pane, command).map(Answer::Text),
        }
    }

    pub(super) fn known_pane(&self, pane: PaneRef) -> Result<PaneId, PortRefusal> {
        let id = PaneId(pane.0);
        self.ws
            .tab_of(id)
            .map(|_| id)
            .ok_or(PortRefusal::NoSuchPane)
    }

    fn open_for_agent(&mut self, open: PaneOpen) -> Result<PaneRef, PortRefusal> {
        if open.kind != PaneKind::Terminal {
            return Err(PortRefusal::Failed(
                "temor can only open a terminal this way for now".to_owned(),
            ));
        }
        let input = match open.placement {
            Placement::Tab => WorkspaceIn::NewTab {
                cwd: open.cwd.as_deref().map(cwd_of),
            },
            Placement::SplitRight => WorkspaceIn::Split {
                towards: Direction::Right,
            },
            Placement::SplitDown => WorkspaceIn::Split {
                towards: Direction::Down,
            },
            _ => {
                return Err(PortRefusal::Failed(
                    "temor cannot put a pane there".to_owned(),
                ));
            }
        };
        let before: BTreeSet<PaneId> = self.ws.tabs.panes().into_iter().collect();
        let held = self.ws.front().and_then(|tab| self.ws.focus_of(tab));
        self.act(input);
        match self
            .ws
            .tabs
            .panes()
            .into_iter()
            .find(|p| !before.contains(p))
        {
            Some(pane) => {
                self.hand_keyboard_back(held, pane);
                Ok(PaneRef(pane.0))
            }
            None => {
                Err(PortRefusal::Failed(self.notices.pop().unwrap_or_else(
                    || "there is no room for another pane".to_owned(),
                )))
            }
        }
    }
}

impl Hub {
    /// A pane the person did not open (an agent's) does not take the keyboard: the pane that had
    /// it keeps it, and the new pane's shell counts as in the background.
    fn hand_keyboard_back(&mut self, held: Option<PaneId>, opened: PaneId) {
        let Some(held) = refocus_after_agent_open(held, opened) else {
            return;
        };
        self.act(WorkspaceIn::Focus(held));
        if let Some(slot) = self.slots.get(&opened) {
            if let Some(session) = slot.borrow_mut().session.as_mut() {
                session.set_focus(NoteFocus::Unfocused);
            }
        }
    }
}

/// The pane to give the keyboard back to after an agent opened `opened`: the one that had it,
/// unless there was none or the open already left it there.
pub(super) fn refocus_after_agent_open(held: Option<PaneId>, opened: PaneId) -> Option<PaneId> {
    held.filter(|held| *held != opened)
}

/// The folder a request names, as the workspace keeps it.
fn cwd_of(path: &std::path::Path) -> Cwd {
    Cwd(path.to_string_lossy().into_owned())
}

/// What a second launch asks for. temor has one window, so a new window is a new tab.
pub(super) fn new_tab_for(request: Request) -> WorkspaceIn {
    match request {
        Request::NewWindow => WorkspaceIn::NewTab { cwd: None },
        Request::NewTab { cwd } => WorkspaceIn::NewTab {
            cwd: cwd.as_deref().map(cwd_of),
        },
    }
}

/// How a command stands, as the desktop services say it.
pub(super) fn exit_of(outcome: Outcome) -> Exit {
    match outcome {
        Outcome::Running => Exit::Running,
        Outcome::Succeeded => Exit::Code(0),
        Outcome::Failed(code) | Outcome::Stopped(code) => Exit::Code(code.0),
        Outcome::Unreported => Exit::Unknown,
    }
}

/// Hears the desktop services for as long as the window lives.
///
/// A later launch also raises `window`: with the activation token it came with on Wayland, else
/// by asking the system to focus it.
pub fn spawn_desktop_loop(desk: Desk, link: DesktopLink, window: Option<WindowHandle>) {
    let Some(mut asks) = link.take_asks() else {
        return;
    };
    spawn(async move {
        while let Some(ask) = asks.recv().await {
            if let (Ask::Launch(_, token), Some(window)) = (&ask, &window) {
                window.focus_with_token(token.clone());
            }
            let notices = {
                let mut hub = desk.hub.0.borrow_mut();
                hub.serve(ask);
                std::mem::take(&mut hub.notices)
            };
            for text in notices {
                desk.say(text);
            }
            desk.bump();
        }
    });
}

impl HubRef {
    /// Shows the desktop services the window as it is now.
    pub fn publish_to(&self, link: &DesktopLink) {
        let (snapshot, count) = self.0.borrow().desktop_snapshot();
        link.publish(snapshot, count);
    }
}
