//! A window that records what the desktop services asked of it.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use std::sync::Mutex;
use temor_desktop::{
    CommandInfo, CommandRef, Exit, Focus, LineRange, Network, PaneInfo, PaneKind, PaneOpen,
    PaneRef, PortRefusal, SandboxResult, SandboxRun, SpaceRef, WindowContext, WindowPort,
};

/// What the fake window was asked to do.
#[derive(Debug, Default)]
pub struct Log {
    pub opened: Vec<PaneOpen>,
    pub closed: Vec<PaneRef>,
    pub typed: Vec<(PaneRef, String)>,
    pub sandboxed: Vec<SandboxRun>,
    pub undone: Vec<String>,
}

#[derive(Debug, Default)]
pub struct FakeWindow {
    pub log: Mutex<Log>,
}

impl FakeWindow {
    fn pane() -> PaneInfo {
        PaneInfo {
            id: PaneRef(1),
            title: "zsh".to_owned(),
            cwd: Some("/home/me/src".into()),
            kind: PaneKind::Terminal,
            focus: Focus::Focused,
        }
    }
}

impl WindowPort for FakeWindow {
    fn context(&self) -> WindowContext {
        WindowContext {
            title: "temor".to_owned(),
            focused: Some(Self::pane()),
            last_command: None,
            selection: Some("cargo build".to_owned()),
            pane_count: 1,
            space: SpaceRef {
                local: 3,
                link: None,
            },
        }
    }

    fn panes(&self) -> Vec<PaneInfo> {
        vec![Self::pane()]
    }

    fn read_pane(&self, pane: PaneRef, range: LineRange) -> Result<String, PortRefusal> {
        match pane {
            PaneRef(1) => Ok(format!("lines {}+{}", range.from, range.count)),
            _ => Err(PortRefusal::NoSuchPane),
        }
    }

    fn commands(&self, _pane: PaneRef) -> Result<Vec<CommandInfo>, PortRefusal> {
        Ok(vec![CommandInfo {
            id: CommandRef(7),
            line: "ls".to_owned(),
            exit: Exit::Code(0),
        }])
    }

    fn read_command(&self, _pane: PaneRef, _command: CommandRef) -> Result<String, PortRefusal> {
        Ok("a b c".to_owned())
    }

    fn open_pane(&self, open: PaneOpen) -> Result<PaneRef, PortRefusal> {
        self.log.lock().unwrap().opened.push(open);
        Ok(PaneRef(9))
    }

    fn close_pane(&self, pane: PaneRef) -> Result<(), PortRefusal> {
        self.log.lock().unwrap().closed.push(pane);
        Ok(())
    }

    fn run_in_pane(&self, pane: PaneRef, line: &str) -> Result<(), PortRefusal> {
        self.log.lock().unwrap().typed.push((pane, line.to_owned()));
        Ok(())
    }

    async fn run_sandboxed(&self, run: SandboxRun) -> Result<SandboxResult, PortRefusal> {
        let changed = matches!(run.network, Network::Closed);
        self.log.lock().unwrap().sandboxed.push(run);
        Ok(SandboxResult {
            exit: Exit::Code(0),
            output: "done".to_owned(),
            undo: changed.then(|| "overlay-5".to_owned()),
        })
    }

    fn undo(&self, token: &str) -> Result<(), PortRefusal> {
        self.log.lock().unwrap().undone.push(token.to_owned());
        Ok(())
    }
}
