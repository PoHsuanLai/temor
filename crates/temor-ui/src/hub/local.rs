//! Panes that live in this computer's own daemon (PLAN M7): the shell runs in a process that
//! outlives the window, and quitting only lets go of it. It is the helper's session machinery
//! with the unix socket in place of `ssh`; the pane, its screen and its reconnects work as for a
//! host. Off unless the setting `shell.keep_running = on` or `TEMOR_LOCAL_DAEMON` is set.

use std::sync::{Arc, Mutex};

use temor_core::session::{Checkpoint, ShellState};
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::Spawn;
use temor_pane::settings::ShellChoice;
use temor_remote::host::HostEnv;
use temor_shell::report::{KEY_VAR, ReportKey};
use temor_ssh::HostAlias;
use temor_wire::ids::SessionId;
use temor_wire::link::{Attempt, LinkState};

use super::model::Hub;
use super::remote::{Busy, LOCAL, LocalStart, RemotePane, Shown};
use crate::edge::remote_install::blob_path;
use crate::edge::remote_link::{RemoteLink, Target};
use crate::edge::replay::fresh_key;
use crate::view::remote_launch::open_fed_session;

/// The local daemon's name as a host.
pub(super) fn local_alias() -> HostAlias {
    HostAlias::new(LOCAL).expect("a name")
}

impl Hub {
    /// Whether `spawn`'s pane should be a session in the local daemon.
    pub(super) fn local_wanted(&self, spawn: &Spawn) -> bool {
        (self.launch.settings.keep_running
            || self.launch.ambient.env.get("TEMOR_LOCAL_DAEMON").is_some())
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
    /// person had set in the shell it replaces (`kept`) over them, and `report_key` for its
    /// reports. The environment is scrubbed as a local pane's is (temor-pty's `base_env`).
    fn local_start(
        &mut self,
        spawn: &Spawn,
        kept: Option<Checkpoint>,
        (resume, report_key): (Option<SessionId>, ReportKey),
    ) -> LocalStart {
        let mut start = self.space_start();
        let replay = kept
            .as_ref()
            .and_then(|kept| self.carry_env(&mut start.env, spawn.pane, kept));
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
        let surroundings = self.launch.ambient.env.clone().overlay(&start.env);
        let mut env: Vec<(String, String)> = temor_pty::base_env(surroundings.iter())
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            })
            .collect();
        env.push(("TERM".to_owned(), "xterm-256color".to_owned()));
        env.push(("COLORTERM".to_owned(), "truecolor".to_owned()));
        env.push((KEY_VAR.to_owned(), report_key.as_str().to_owned()));
        if let Some(replay) = replay {
            env.push((
                "TEMOR_REPLAY".to_owned(),
                replay.to_string_lossy().into_owned(),
            ));
        }
        LocalStart {
            spawn: spawn.clone(),
            cwd,
            env,
            shell: match &self.launch.settings.shell {
                ShellChoice::Login => None,
                ShellChoice::Program(program) => Some(program.clone()),
            },
            resume,
            report_key,
        }
    }

    /// Makes `spawn`'s pane a session in the local daemon: an empty pane now, its screen when the
    /// daemon has answered. Without a helper beside temor the pane is an ordinary one.
    pub(super) fn begin_local(&mut self, spawn: &Spawn, resume: Option<SessionId>) {
        let kept = self.carried.remove(&spawn.pane);
        // A pane made while a program is being detached takes that program's session.
        let detached = self.remote.detaching.filter(|_| resume.is_none());
        let resume = resume.or(detached.map(|(_, session)| session));
        // A shell that goes on keeps the key it was started with: the detached pane's, or the
        // one kept when temor quit.
        let key = match (detached, resume) {
            (Some((from, _)), _) => self
                .live(from, |session| session.report_key().cloned())
                .flatten(),
            (None, Some(_)) => kept
                .as_ref()
                .and_then(|kept| kept.report_key.as_deref())
                .and_then(ReportKey::parse),
            (None, None) => None,
        }
        .unwrap_or_else(fresh_key);
        // A shell that goes on is known by what was kept of it until it next reports.
        let seed = kept
            .clone()
            .filter(|_| resume.is_some())
            .map(|kept| (ShellState::restored(&kept.replay()), kept));
        let own = self.local_start(spawn, kept, (resume, key));
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
        session.set_report_key(Some(own.report_key.clone()));
        if let Some((state, kept)) = seed {
            session.continue_from(state, kept);
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

    /// Notes in `kept` what the next launch needs to find `pane`'s daemon session again: the
    /// session, and the key its shell reports with.
    pub(super) fn keep_local(&self, pane: PaneId, kept: &mut Checkpoint) {
        kept.daemon_session = self.local_session(pane);
        kept.report_key = kept
            .daemon_session
            .and(self.remote.local.get(&pane))
            .map(|own| own.report_key.as_str().to_owned());
    }

    /// Starts the daemon's helper for the dial.
    pub(super) fn local_dial_env(&self) -> Option<HostEnv> {
        self.local_env()
    }
}

impl Hub {
    /// The focused pane and its daemon session, when it is a daemon pane with a program running.
    fn detachable(&self) -> Option<(PaneId, SessionId)> {
        let pane = self.keyboard_pane()?;
        self.remote.local.contains_key(&pane).then_some(())?;
        let session = self.remote.panes.get(&pane)?.session?;
        self.live(pane, |session| session.running_command())??;
        Some((pane, session))
    }

    /// Whether "Detach running program" can do anything now (the command bar lists it only
    /// then).
    pub(super) fn can_detach_focused(&self) -> bool {
        self.detachable().is_some()
    }

    /// The focused pane, when it is a daemon pane with a program running: the pane and its
    /// session are noted for [`Hub::detach_finish`], and the caller splits the pane.
    pub(super) fn detach_prepare(&mut self) -> Option<PaneId> {
        let (pane, session) = self.detachable()?;
        self.remote.detaching = Some((pane, session));
        // A command has run: the new shell left in the pane is not a new window's first prompt.
        self.first_hint = None;
        Some(pane)
    }

    /// Ends a daemon session without closing the pane that shows it: the pane keeps its last
    /// frame when the daemon says the session is gone.
    pub(super) fn local_hang_up(&mut self, session: SessionId) {
        let conn = self
            .remote
            .lines
            .get(&local_alias())
            .and_then(|line| line.conn.clone());
        if let Some(conn) = conn {
            self.remote.work.close(conn, session);
        }
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
        let own = self.local_start(&old, Some(kept), (None, fresh_key()));
        if let Some(slot) = self.slots.get(&from)
            && let Some(session) = slot.borrow_mut().session.as_mut()
        {
            session.set_report_key(Some(own.report_key.clone()));
        }
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
