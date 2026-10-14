//! What the Settings window changes in a Space's project (PLAN §7), and what it warns about. Pure:
//! the window sends a [`SpaceChange`], the main window applies it to the live project through the
//! kit's handle, the same way a Space menu's edits go.

use temor_core::space::{Project, Route};
use temor_core::vocab::text::{KeyPath, ProjectRoot, RelPath, RoutePattern};

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
}

impl SpaceChange {
    /// `project` with this change made.
    pub fn apply_to(&self, project: &mut Project) {
        match self {
            Self::Root(root) => project.root = root.clone(),
            Self::SshIdentity(key) => project.profile.ssh_identity = key.clone(),
            Self::Dotenv(file) => project.profile.dotenv = file.clone(),
            Self::Routes(routes) => project.routes = routes.clone(),
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
}
