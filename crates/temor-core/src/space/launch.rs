//! What a new shell in a Space starts with: the folder, the environment and the agent, put
//! together from the profile and the dotenv file's text. Pure; the edge reads the file, starts
//! the agent, asks the account store for the credentials and spawns the shell.

use super::agent::{AgentPlan, RuntimeDir, agent_plan};
use super::credential::Credential;
use super::dotenv::{DotenvFault, parse_dotenv};
use super::project::Project;
use crate::error::CoreError;
use crate::vocab::text::{Cwd, EnvKey, EnvValue};
use ds_style::space::list::SpaceId;

/// A new shell's start, before the edge adds the secrets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpaceLaunch {
    /// The folder it opens in; none means the person's home folder.
    pub cwd: Option<Cwd>,
    /// The variables to set, in order, each name once. From the dotenv file first, then the
    /// profile's own (which win), then the agent's socket (which wins over both).
    pub env: Vec<(EnvKey, EnvValue)>,
    /// The agent to start for this Space, when its profile holds a key.
    pub agent: Option<AgentPlan>,
    /// Variables the edge fills from the account store; they win over `env`.
    pub credentials: Vec<Credential>,
    /// Lines of the dotenv file that were skipped, for a notice.
    pub dotenv_faults: Vec<(usize, DotenvFault)>,
    /// Why the agent was not planned, when it could not be.
    pub agent_fault: Option<CoreError>,
}

/// The dotenv file to read for `project`, as `<root>/<relative path>`, or none when the profile
/// names no file or the project has no root. A path that is absolute or climbs with `..` is
/// refused: a profile cannot read outside its project.
pub fn dotenv_path(project: &Project) -> Result<Option<String>, CoreError> {
    let (Some(root), Some(rel)) = (&project.root, &project.profile.dotenv) else {
        return Ok(None);
    };
    let rel = rel.as_str();
    let leaves = rel.starts_with('/') || rel.split('/').any(|part| part == "..");
    if rel.is_empty() || leaves {
        return Err(CoreError::BadDotenvPath(rel.to_owned()));
    }
    Ok(Some(format!(
        "{}/{rel}",
        root.as_str().trim_end_matches('/')
    )))
}

/// A new shell's start in `space`. `dotenv` is the text of the file at [`dotenv_path`], when it
/// could be read.
pub fn launch(
    space: SpaceId,
    project: &Project,
    dotenv: Option<&str>,
    runtime: &RuntimeDir,
) -> SpaceLaunch {
    let file = dotenv.map(parse_dotenv).unwrap_or_default();
    let mut env: Vec<(EnvKey, EnvValue)> = Vec::new();
    for (key, value) in file
        .vars
        .into_iter()
        .chain(project.profile.env.iter().cloned())
    {
        set(&mut env, key, value);
    }
    let (agent, agent_fault) = match agent_plan(space, &project.profile, runtime) {
        Ok(plan) => (plan, None),
        Err(fault) => (None, Some(fault)),
    };
    if let Some(plan) = &agent {
        set(&mut env, plan.env.0.clone(), plan.env.1.clone());
    }
    SpaceLaunch {
        cwd: project
            .root
            .as_ref()
            .map(|root| Cwd(root.as_str().to_owned())),
        env,
        agent,
        credentials: project.profile.credentials.clone(),
        dotenv_faults: file.faults,
        agent_fault,
    }
}

/// Sets `key`: a later value replaces an earlier one and the name keeps its first place.
fn set(env: &mut Vec<(EnvKey, EnvValue)>, key: EnvKey, value: EnvValue) {
    match env.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 = value,
        None => env.push((key, value)),
    }
}
