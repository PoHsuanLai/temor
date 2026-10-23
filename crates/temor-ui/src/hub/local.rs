//! Panes that live in this computer's own daemon (PLAN M7): the shell runs in a process that
//! outlives the window, and quitting only lets go of it. It is the helper's session machinery
//! with the unix socket in place of `ssh`; the pane, its screen and its reconnects work as for a
//! host. Off unless `TEMOR_LOCAL_DAEMON` is set, until it has had more use.

use std::sync::{Arc, Mutex};

use temor_core::session::{Checkpoint, ShellState};
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::Spawn;
use temor_pane::settings::ShellChoice;
use temor_remote::host::HostEnv;
use temor_ssh::HostAlias;
use temor_wire::ids::SessionId;
use temor_wire::link::{Attempt, LinkState};

use super::model::Hub;
use super::remote::{Busy, LOCAL, LocalStart, RemotePane, Shown};
use crate::edge::remote_install::blob_path;
use crate::edge::remote_link::{RemoteLink, Target};
use crate::view::remote_launch::open_fed_session;

/// The local daemon's name as a host.
pub(super) fn local_alias() -> HostAlias {
    HostAlias::new(LOCAL).expect("a name")
}

impl Hub {
    /// Whether `spawn`'s pane should be a session in the local daemon.
    pub(super) fn local_wanted(&self, spawn: &Spawn) -> bool {
        self.launch.ambient.env.get("TEMOR_LOCAL_DAEMON").is_some()
            && spawn.relaunch.is_none()
            && self.run.intent.is_none()
            && !self.is_quick(spawn.pane)
    }

    /// What starting the daemon needs to know about this computer.
    fn local_env(&self) -> Option<HostEnv> {
        let env = &self.launch.ambient.env;
        let exe = blob_path(
            env.get("TEMOR_REMOTE_HELPER"),
            std::env::current_exe().ok().as_deref(),
        )?;
        exe.is_file().then(|| HostEnv {
            xdg_runtime_dir: env.get("XDG_RUNTIME_DIR").map(Into::into),
            home: env.get("HOME").map(Into::into),
            linger: env.get("TEMOR_REMOTE_LINGER").map(Into::into),
            shell: env.get("SHELL").map(Into::into),
            exe,
        })
    }

    /// How `spawn`'s pane starts in the daemon: the Space's folder and variables, with what the
    /// person had set in the shell it replaces (`kept`) over them.
    fn local_start(
        &mut self,
        spawn: &Spawn,
        kept: Option<Checkpoint>,
        resume: Option<SessionId>,
    ) -> LocalStart {
        let mut start = self.space_start();
        if let Some(kept) = &kept {
            self.carry_env(&mut start.env, spawn.pane, kept);
        }
        let cwd = spawn
            .cwd
            .as_ref()
            .map(|cwd| cwd.as_str().to_owned())
            .or_else(|| kept.as_ref().and_then(|kept| kept.cwd.clone()))
            .or_else(|| start.cwd.map(|cwd| cwd.as_str().to_owned()))
            .or_else(|| {
                self.launch
                    .ambient
                    .env
                    .get("HOME")
                    .map(|home| home.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "/".to_owned());
        let mut env: Vec<(String, String)> = self
            .launch
            .ambient
            .env
            .clone()
            .overlay(&start.env)
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            })
            .filter(|(k, _)| k != "TERM" && k != "COLORTERM")
            .collect();
        env.push(("TERM".to_owned(), "xterm-256color".to_owned()));
        env.push(("COLORTERM".to_owned(), "truecolor".to_owned()));
        LocalStart {
            spawn: spawn.clone(),
            cwd,
            env,
            shell: match &self.launch.settings.shell {
                ShellChoice::Login => None,
                ShellChoice::Program(program) => Some(program.clone()),
            },
            resume,
        }
    }

    /// Makes `spawn`'s pane a session in the local daemon: an empty pane now, its screen when the
    /// daemon has answered. Without a helper beside temor the pane is an ordinary one.
    pub(super) fn begin_local(&mut self, spawn: &Spawn, resume: Option<SessionId>) {
        let kept = self.carried.remove(&spawn.pane);
        // A pane made while a program is being detached takes that program's session.
        let resume = resume.or_else(|| self.remote.detaching.map(|(_, session)| session));
        // A shell that goes on is known by what was kept of it until it next reports.
        let seed = kept
            .as_ref()
            .filter(|_| resume.is_some())
            .map(|kept| ShellState::restored(&kept.replay()));
        let own = self.local_start(spawn, kept, resume);
        let target = Arc::new(Mutex::new(Target::default()));
        let opened = RemoteLink::start(Arc::clone(&target))
            .map_err(|e| e.to_string())
            .and_then(|link| {
                open_fed_session(
                    &self.launch,
                    self.palette,
                    spawn.pane,
                    self.events.clone(),
                    link,
                )
                .map_err(|e| e.to_string())
            });
        let (mut session, feeder) = match opened {
            Ok(opened) => opened,
            Err(err) => {
                self.notices.push(err);
                return self.local_plain(own);
            }
        };
        session.set_focus(self.focus_of(spawn.pane));
        if let Some(seed) = seed {
            session.seed_shell_state(seed);
        }
        let slot = crate::view::slot::Slot {
            palette: self.palette,
            session: Some(session),
            ..Default::default()
        };
        self.slots
            .insert(spawn.pane, std::rc::Rc::new(std::cell::RefCell::new(slot)));
        let host = local_alias();
        self.remote.panes.insert(
            spawn.pane,
            RemotePane {
                alias: host.clone(),
                target,
                feeder,
                session: None,
                shown: Shown::Opening,
            },
        );
        if let Some(session) = own.resume {
            self.remote.saved.insert(spawn.pane, session);
        }
        self.remote.local.insert(spawn.pane, own);
        self.remote
            .lines
            .entry(host.clone())
            .or_default()
            .waiting
            .push(spawn.pane);
        self.local_advance();
    }

    /// Opens the waiting panes' sessions, dialing the daemon first when it is not connected.
    pub(super) fn local_advance(&mut self) {
        let host = local_alias();
        let line = self.remote.lines.entry(host.clone()).or_default();
        if line.conn.is_some() && matches!(line.link, LinkState::Connected { .. }) {
            return self.remote_open_waiting(&host);
        }
        if line.busy != Busy::Idle || !matches!(line.link, LinkState::Connected { .. }) {
            return;
        }
        line.link = LinkState::Reattaching {
            sessions: Vec::new(),
            attempt: Attempt(1),
        };
        self.remote_dial(&host);
    }

    /// The daemon cannot be reached the first time: its panes become ordinary ones.
    pub(super) fn local_fallback(&mut self) {
        let host = local_alias();
        let waiting = self
            .remote
            .lines
            .get_mut(&host)
            .map(|line| std::mem::take(&mut line.waiting))
            .unwrap_or_default();
        for pane in waiting {
            self.remote.panes.remove(&pane);
            self.remote.saved.remove(&pane);
            if let Some(own) = self.remote.local.remove(&pane) {
                self.local_plain(own);
            }
        }
        if let Some(line) = self.remote.lines.get_mut(&host) {
            line.busy = Busy::Idle;
        }
    }

    fn local_plain(&mut self, own: LocalStart) {
        self.open_plain(&own.spawn, None);
    }

    /// The session a pane has in the local daemon, to ask for again at the next launch: the one
    /// it has now, else the one it was saved with and has not got back yet.
    pub(super) fn local_session(&self, pane: temor_core::vocab::ids::PaneId) -> Option<u64> {
        self.remote.local.contains_key(&pane).then_some(())?;
        let live = self.remote.panes.get(&pane).and_then(|p| p.session);
        live.or_else(|| self.remote.saved.get(&pane).copied())
            .map(|session| session.0)
    }

    /// Starts the daemon's helper for the dial.
    pub(super) fn local_dial_env(&self) -> Option<HostEnv> {
        self.local_env()
    }
}

impl Hub {
    /// The focused pane, when it is a daemon pane with a program running: the pane and its
    /// session are noted for [`Hub::detach_finish`], and the caller splits the pane.
    pub(super) fn detach_prepare(&mut self) -> Option<PaneId> {
        let pane = self.keyboard_pane()?;
        self.remote.local.contains_key(&pane).then_some(())?;
        let session = self.remote.panes.get(&pane)?.session?;
        self.live(pane, |session| session.running_command())??;
        self.remote.detaching = Some((pane, session));
        Some(pane)
    }

    /// The split happened: the new pane took the program's session, and the old pane gets a new
    /// shell with the folder and variables of the one it had.
    pub(super) fn detach_finish(&mut self) {
        let Some((from, session)) = self.remote.detaching.take() else {
            return;
        };
        let taken = self
            .remote
            .local
            .iter()
            .any(|(pane, own)| *pane != from && own.resume == Some(session));
        let Some(old) = self.remote.local.get(&from).map(|own| own.spawn.clone()) else {
            return;
        };
        if !taken {
            return;
        }
        let kept = self
            .live(from, |session| session.checkpoint())
            .unwrap_or_default();
        let own = self.local_start(&old, Some(kept), None);
        self.remote.local.insert(from, own);
        self.remote.saved.remove(&from);
        if let Some(pane) = self.remote.panes.get_mut(&from) {
            pane.session = None;
            pane.shown = Shown::Opening;
            let mut target = crate::edge::remote_link::lock(&pane.target);
            target.conn = None;
            target.session = None;
        }
        self.remote
            .lines
            .entry(local_alias())
            .or_default()
            .waiting
            .push(from);
        self.local_advance();
    }
}
