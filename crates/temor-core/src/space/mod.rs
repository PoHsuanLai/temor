//! A Space is a project (D2, D10): the kit's `Spaces<Project, Place>`, with temor's payload and
//! place.

mod agent;
mod carry;
#[cfg(test)]
mod carry_tests;
mod credential;
mod dotenv;
#[cfg(test)]
mod dotenv_tests;
mod launch;
#[cfg(test)]
mod launch_tests;
mod place;
mod project;
mod route;
#[cfg(test)]
mod route_tests;

pub use agent::{AgentPlan, RuntimeDir, agent_plan};
pub use carry::carry_tab;
pub use credential::{AccountRef, Credential};
pub use dotenv::{Dotenv, DotenvFault, parse_dotenv};
pub use launch::{SpaceLaunch, dotenv_path, launch};
pub use place::{Folder, Pin, Place, Tab};
pub use project::{Profile, Project, Route};
pub use route::{SpaceRoute, route, route_folder, routes_of};

/// temor's Spaces: each keeps a [`Project`] and recalls a [`Place`].
pub type TemorSpaces = ds_style::space::list::Spaces<Project, Place>;
