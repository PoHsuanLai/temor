//! What a new shell starts with in the Space on screen (PLAN §7): its folder, its environment
//! (dotenv file, then profile, then the agent's socket, then the account store's secrets) and its
//! key agent. The core puts the plan together (`space::launch`); this reads the dotenv file,
//! starts the agent and asks the account store.

use ds::components::app::spaces::SpaceId;
use temor_core::space::{Project, RuntimeDir, dotenv_path, launch};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_shell::ChildEnv;

use super::model::Hub;
use crate::edge::space_start::read_dotenv;

/// Where and with what a new shell starts.
#[derive(Debug, Default)]
pub(super) struct SpaceStart {
    /// The project's folder, when it has one.
    pub(super) cwd: Option<Cwd>,
    /// The variables to set over the surroundings' own.
    pub(super) env: ChildEnv,
}

impl Hub {
    /// The start of a new shell in the Space on screen. Anything that could not be had (a line
    /// of the dotenv file, the agent, a secret) is told to the person and left out.
    pub(super) fn space_start(&mut self) -> SpaceStart {
        let space = self.book.active();
        let project = self.project_here(space);
        let dotenv = match dotenv_path(&project) {
            Ok(path) => path.and_then(|path| read_dotenv(&path)),
            Err(err) => {
                self.notices.push(err.to_string());
                None
            }
        };
        let plan = launch(space, &project, dotenv.as_deref(), &self.runtime_dir());
        for (line, fault) in &plan.dotenv_faults {
            let said = format!(
                "Line {line} of the Space's variables file: {}",
                fault.describe()
            );
            self.notices.push(said);
        }
        if let Some(fault) = &plan.agent_fault {
            self.notices.push(fault.to_string());
        }
        let mut env = ChildEnv::new();
        for (key, value) in &plan.env {
            env.set(key.as_str(), value.as_str());
        }
        if let Some(agent) = &plan.agent {
            let home = self.launch.ambient.home.clone();
            if let Err(err) = self.keep.agents.ensure(space, agent, &home) {
                self.notices.push(err.to_string());
                env.remove(agent.env.0.as_str());
            }
        }
        for credential in &plan.credentials {
            match self.keep.credentials.0.secret(&credential.account) {
                Ok(Some(secret)) => env.set(credential.env.as_str(), secret),
                Ok(None) => self.notices.push(format!(
                    "No saved key for {} was found",
                    credential.account.as_str()
                )),
                Err(err) => self.notices.push(err.to_string()),
            }
        }
        SpaceStart { cwd: plan.cwd, env }
    }

    /// The Space's project with a root written `~/work` made absolute under the person's home,
    /// as the shell's folder and the variables file's path are both read from it.
    fn project_here(&self, space: SpaceId) -> Project {
        let home = self.launch.ambient.home.to_string_lossy();
        self.book.project_of(space).with_home(&home)
    }

    /// The person's runtime folder, as the agent's socket folder is made under it.
    fn runtime_dir(&self) -> RuntimeDir {
        let ambient = &self.launch.ambient;
        let text = match ambient.env.get("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
            Some(dir) => dir.to_string_lossy().into_owned(),
            None => ambient.runtime_dir.display().to_string(),
        };
        RuntimeDir(text)
    }

    /// A new tab with no folder named opens in the Space's project folder.
    pub(super) fn in_project(&self, input: WorkspaceIn) -> WorkspaceIn {
        match input {
            WorkspaceIn::NewTab { cwd: None } => WorkspaceIn::NewTab {
                cwd: self
                    .project_here(self.book.active())
                    .root
                    .map(|root| Cwd(root.0)),
            },
            other => other,
        }
    }
}
