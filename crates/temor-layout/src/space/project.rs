//! What a Space keeps (the `P` of `Spaces<P, R>`): its project folder, the profile new shells
//! start with, and the routes that send new sessions to it. Written flat beside the Space's look,
//! so no key here may be `id`, `name`, `dots`, `grain`, `theme` or `card_accent`.

use super::credential::Credential;
use serde::{Deserialize, Deserializer, Serialize};
use temor_core::vocab::home::expand_home;
use temor_core::vocab::text::{EnvKey, EnvValue, KeyPath, ProjectRoot, RelPath, RoutePattern};
use temor_schema::lenient::{lenient_list, lenient_or_default};

/// A Space's project. Every key defaults, so a Space the kit made before temor wrote anything
/// reads as an empty project, and a key that does not parse reads as its default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Project {
    /// The folder new tabs open in; none means the person's home folder.
    #[serde(default, deserialize_with = "lenient_or_default")]
    pub root: Option<ProjectRoot>,
    /// What new shells in this Space start with.
    #[serde(default, deserialize_with = "lenient_or_default")]
    pub profile: Profile,
    /// Which new sessions open in this Space.
    #[serde(default, deserialize_with = "lenient_list")]
    pub routes: Vec<Route>,
}

impl Project {
    /// This project with a root written `~/work` made absolute under `home`, so the shell's
    /// folder and the dotenv file's path are both the folder the person meant.
    pub fn with_home(mut self, home: &str) -> Self {
        self.root = self
            .root
            .map(|root| ProjectRoot(expand_home(root.as_str(), home)));
        self
    }
}

/// The environment a Space's new shells get (D10). Applies to new shells only; credentials come
/// from the account store, never from here.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Profile {
    /// Variables set in every new shell, in order.
    #[serde(default, deserialize_with = "lenient_env")]
    pub env: Vec<(EnvKey, EnvValue)>,
    /// The key the Space's own SSH agent holds.
    #[serde(default, deserialize_with = "lenient_or_default")]
    pub ssh_identity: Option<KeyPath>,
    /// A dotenv file under the project root, read into new shells.
    #[serde(default, deserialize_with = "lenient_or_default")]
    pub dotenv: Option<RelPath>,
    /// Secrets new shells get from the account store, each as a reference to an account and
    /// the variable it fills. Never a value.
    #[serde(
        default,
        deserialize_with = "lenient_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub credentials: Vec<Credential>,
}

/// A rule that opens matching new sessions in this Space, such as `ssh prod-*`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Route {
    /// What it matches.
    pub pattern: RoutePattern,
}

/// The entries whose key is a usable variable name; the others are dropped.
fn lenient_env<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<(EnvKey, EnvValue)>, D::Error> {
    Ok(lenient_list::<D, (String, EnvValue)>(d)?
        .into_iter()
        .filter_map(|(key, value)| EnvKey::new(key).ok().map(|key| (key, value)))
        .collect())
}
