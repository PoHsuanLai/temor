//! What the Settings window changes in a Space's project (PLAN §7), and what it warns about. Pure:
//! the window sends a [`SpaceChange`], the main window applies it to the live project through the
//! kit's handle, the same way a Space menu's edits go.

use ds::components::app::spaces::SpaceId;
use temor_client::model::space::{AccountRef, Credential, Project, Route};
use temor_client::model::vocab::text::{
    EnvKey, EnvValue, KeyPath, ProjectRoot, RelPath, RoutePattern,
};

/// A profile's variables, in order.
pub type Variables = Vec<(EnvKey, EnvValue)>;

/// One change to one Space's project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceChange {
    /// The folder new tabs open in; none is the home folder.
    Root(Option<ProjectRoot>),
    /// The key the Space's own agent holds.
    SshIdentity(Option<KeyPath>),
    /// The dotenv file read into new shells.
    Dotenv(Option<RelPath>),
    /// The routes, in full.
    Routes(Vec<Route>),
    /// The variables new shells get, in full.
    Env(Variables),
    /// The credentials (which account fills which variable), in full. Never a secret.
    Credentials(Vec<Credential>),
}

impl SpaceChange {
    /// `project` with this change made.
    pub fn apply_to(&self, project: &mut Project) {
        match self {
            Self::Root(root) => project.root = root.clone(),
            Self::SshIdentity(key) => project.profile.ssh_identity = key.clone(),
            Self::Dotenv(file) => project.profile.dotenv = file.clone(),
            Self::Routes(routes) => project.routes = routes.clone(),
            Self::Env(env) => project.profile.env = env.clone(),
            Self::Credentials(list) => project.profile.credentials = list.clone(),
        }
    }
}

/// The text of a field as an optional value: blank is none.
pub fn optional(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// What a person should know about a route `pattern`, or none when it is fine.
pub fn route_warning(pattern: &str) -> Option<&'static str> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Some("This is empty, so it matches nothing.");
    }
    if pattern.chars().all(|c| c == '*') {
        return Some("This matches every command, so it takes every new session.");
    }
    None
}

/// What a person should know about a project's folder, or none.
pub fn root_warning(root: &str) -> Option<&'static str> {
    let root = root.trim();
    (!root.is_empty() && !root.starts_with(['/', '~']))
        .then_some("Use a full path, starting with / or ~.")
}

/// What a person should know about a dotenv file's path, or none: it is read from under the
/// folder, so a full path or one that climbs out is not used.
pub fn dotenv_warning(file: &str) -> Option<&'static str> {
    let file = file.trim();
    let outside = file.starts_with('/') || file.split('/').any(|part| part == "..");
    outside.then_some("Name a file inside the folder, such as .env.")
}

/// `routes` with `pattern` added at the end. A blank pattern is added too, so the warning can
/// show it and the person can fix or remove it.
pub fn routes_with(mut routes: Vec<Route>, pattern: &str) -> Vec<Route> {
    routes.push(Route {
        pattern: RoutePattern(pattern.trim().to_owned()),
    });
    routes
}

/// `routes` without the one at `at`.
pub fn routes_without(mut routes: Vec<Route>, at: usize) -> Vec<Route> {
    if at < routes.len() {
        routes.remove(at);
    }
    routes
}

/// `routes` with the one at `at` matching `pattern` instead.
pub fn routes_edited(mut routes: Vec<Route>, at: usize, pattern: &str) -> Vec<Route> {
    if let Some(route) = routes.get_mut(at) {
        route.pattern = RoutePattern(pattern.trim().to_owned());
    }
    routes
}

/// A variable's name as the profile takes it, or why not, in words.
pub fn variable_name(name: &str) -> Result<EnvKey, String> {
    EnvKey::new(name.trim())
        .map_err(|_| "A variable's name cannot be empty or hold an equals sign.".to_owned())
}

/// `env` with `name` set to `value`: a variable of that name has its value replaced, else it is
/// added at the end.
pub fn env_with(mut env: Variables, name: &str, value: &str) -> Result<Variables, String> {
    let name = variable_name(name)?;
    let value = EnvValue(value.to_owned());
    match env.iter_mut().find(|(have, _)| *have == name) {
        Some(entry) => entry.1 = value,
        None => env.push((name, value)),
    }
    Ok(env)
}

/// `env` with the variable at `at` named `name` (another of that name is refused).
pub fn env_renamed(mut env: Variables, at: usize, name: &str) -> Result<Variables, String> {
    let name = variable_name(name)?;
    if env
        .iter()
        .enumerate()
        .any(|(i, (have, _))| i != at && *have == name)
    {
        return Err(format!(
            "There is already a variable called {}.",
            name.as_str()
        ));
    }
    if let Some(entry) = env.get_mut(at) {
        entry.0 = name;
    }
    Ok(env)
}

/// `env` with the value of the variable at `at` replaced.
pub fn env_valued(mut env: Variables, at: usize, value: &str) -> Variables {
    if let Some(entry) = env.get_mut(at) {
        entry.1 = EnvValue(value.to_owned());
    }
    env
}

/// `env` without the variable at `at`.
pub fn env_without(mut env: Variables, at: usize) -> Variables {
    if at < env.len() {
        env.remove(at);
    }
    env
}

/// The account that holds the secret a Space fills `env` from: named for the Space and the
/// variable, so no two credentials share one. Letters, digits and `.`, `_`, `-` only, at most 64
/// bytes.
pub fn account_for(space: SpaceId, env: &EnvKey) -> Result<AccountRef, String> {
    let slug: String = env
        .as_str()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut name = format!("temor.space{}.{slug}", space.0);
    name.truncate(64);
    AccountRef::new(name).map_err(|why| why.to_string())
}

/// `list` with a credential filling `env` from `account`, replacing one for the same variable.
pub fn credentials_with(
    mut list: Vec<Credential>,
    env: EnvKey,
    account: AccountRef,
) -> Vec<Credential> {
    list.retain(|have| have.env != env);
    list.push(Credential { env, account });
    list
}

/// `list` without the credential for `env`.
pub fn credentials_without(mut list: Vec<Credential>, env: &EnvKey) -> Vec<Credential> {
    list.retain(|have| have.env != *env);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routes(patterns: &[&str]) -> Vec<Route> {
        patterns
            .iter()
            .map(|p| Route {
                pattern: RoutePattern((*p).to_owned()),
            })
            .collect()
    }

    #[test]
    fn an_empty_or_all_star_route_is_warned_about_and_a_real_one_is_not() {
        let rows = [
            ("", true),
            ("   ", true),
            ("*", true),
            ("**", true),
            (" * ", true),
            ("ssh prod-*", false),
            ("~/work", false),
            ("*prod", false),
        ];
        for (pattern, warned) in rows {
            assert_eq!(route_warning(pattern).is_some(), warned, "{pattern:?}");
        }
    }

    #[test]
    fn a_change_touches_only_its_own_part_of_the_project() {
        let mut project = Project {
            routes: routes(&["ssh a"]),
            ..Project::default()
        };
        SpaceChange::Root(Some(ProjectRoot("/srv/work".to_owned()))).apply_to(&mut project);
        SpaceChange::Dotenv(Some(RelPath(".env".to_owned()))).apply_to(&mut project);
        SpaceChange::Routes(routes(&["ssh a", "ssh b"])).apply_to(&mut project);
        assert_eq!(
            project.root.as_ref().map(ProjectRoot::as_str),
            Some("/srv/work")
        );
        assert_eq!(
            project.profile.dotenv.as_ref().map(RelPath::as_str),
            Some(".env")
        );
        assert_eq!(project.routes, routes(&["ssh a", "ssh b"]));
        SpaceChange::Root(None).apply_to(&mut project);
        assert_eq!(project.root, None);
        assert!(project.profile.ssh_identity.is_none());
    }

    #[test]
    fn route_lists_grow_shrink_and_edit_in_place() {
        let list = routes(&["a", "b"]);
        assert_eq!(routes_with(list.clone(), " c "), routes(&["a", "b", "c"]));
        assert_eq!(routes_without(list.clone(), 0), routes(&["b"]));
        assert_eq!(routes_without(list.clone(), 9), list);
        assert_eq!(routes_edited(list.clone(), 1, "z"), routes(&["a", "z"]));
        assert_eq!(routes_edited(list.clone(), 9, "z"), list);
    }

    #[test]
    fn folders_and_files_are_checked_for_where_they_point() {
        assert_eq!(root_warning(""), None);
        assert_eq!(root_warning("~/work"), None);
        assert!(root_warning("work").is_some());
        assert_eq!(dotenv_warning(".env"), None);
        assert_eq!(dotenv_warning("config/.env"), None);
        assert!(dotenv_warning("/etc/env").is_some());
        assert!(dotenv_warning("../.env").is_some());
        assert_eq!(optional("  "), None);
        assert_eq!(optional(" a "), Some("a".to_owned()));
    }

    #[test]
    fn variables_are_added_renamed_valued_and_removed_by_position() {
        let env = env_with(Vec::new(), " TOKEN_URL ", "https://a").expect("a name");
        let env = env_with(env, "OTHER", "1").expect("a name");
        // The same name again replaces the value rather than adding a second.
        let env = env_with(env, "OTHER", "2").expect("a name");
        let shown: Vec<(&str, &str)> = env
            .iter()
            .map(|(k, v)| (k.as_str(), v.0.as_str()))
            .collect();
        assert_eq!(shown, [("TOKEN_URL", "https://a"), ("OTHER", "2")]);
        assert!(env_with(env.clone(), "", "x").is_err());
        assert!(env_with(env.clone(), "A=B", "x").is_err());
        assert!(env_renamed(env.clone(), 0, "OTHER").is_err());
        let renamed = env_renamed(env.clone(), 0, "URL").expect("free name");
        assert_eq!(renamed[0].0.as_str(), "URL");
        assert_eq!(env_valued(env.clone(), 1, "3")[1].1.0, "3");
        assert_eq!(env_without(env.clone(), 0).len(), 1);
        assert_eq!(env_without(env.clone(), 9), env);
    }

    #[test]
    fn a_credential_is_named_for_its_space_and_variable_and_replaced_not_doubled() {
        let key = EnvKey::new("API_KEY").expect("a name");
        let account = account_for(SpaceId(3), &key).expect("a valid account");
        assert_eq!(account.as_str(), "temor.space3.api-key");
        let long = EnvKey::new("X".repeat(100)).expect("a name");
        assert!(
            account_for(SpaceId(3), &long)
                .expect("cut to fit")
                .as_str()
                .len()
                <= 64
        );
        let list = credentials_with(Vec::new(), key.clone(), account.clone());
        let list = credentials_with(list, key.clone(), account);
        assert_eq!(list.len(), 1);
        assert!(credentials_without(list, &key).is_empty());
    }
}
