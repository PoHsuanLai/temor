//! Routes (PLAN §7): which Space a new session opens in. `ssh prod-*` goes to Ops, a folder
//! pattern to the Space that owns the folder. The matching is the core's; this expands `~` in
//! folder patterns to the home folder, as the folders it is given are written in full.

use std::path::Path;

use ds::components::app::spaces::SpaceId;
use temor_core::space::{SpaceRoute, route, route_folder, routes_of};
use temor_core::vocab::text::RoutePattern;

use super::model::{Hub, HubRef};

impl Hub {
    /// The Space a new session running `command` opens in, when a route says so.
    pub fn space_for_command(&self, command: &str) -> Option<SpaceId> {
        self.with_routes(|routes| route(command, routes))
    }

    /// The Space a new session started in `folder` opens in, when a route says so.
    pub fn space_for_folder(&self, folder: &str) -> Option<SpaceId> {
        self.with_routes(|routes| route_folder(folder, routes))
    }

    fn with_routes<T>(&self, pick: impl FnOnce(&[SpaceRoute<'_>]) -> T) -> T {
        let home = &self.launch.ambient.home;
        let spaces = self.book.spaces.list();
        let expanded: Vec<(SpaceId, RoutePattern)> = routes_of(spaces)
            .into_iter()
            .map(|route| (route.space, expand_home(route.pattern, home)))
            .collect();
        let routes: Vec<SpaceRoute<'_>> = expanded
            .iter()
            .map(|(space, pattern)| SpaceRoute {
                space: *space,
                pattern,
            })
            .collect();
        pick(&routes)
    }
}

/// `pattern` with a leading `~` written as `home`; other patterns as they are.
pub(super) fn expand_home(pattern: &RoutePattern, home: &Path) -> RoutePattern {
    let text = pattern.as_str();
    match text.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            RoutePattern(format!("{}{rest}", home.display()))
        }
        _ => pattern.clone(),
    }
}

impl HubRef {
    /// The Space a new session running `command` opens in, when a route says so.
    pub fn space_for_command(&self, command: &str) -> Option<SpaceId> {
        self.0.borrow().space_for_command(command)
    }

    /// The Space a new session started in `folder` opens in, when a route says so.
    pub fn space_for_folder(&self, folder: &str) -> Option<SpaceId> {
        self.0.borrow().space_for_folder(folder)
    }
}
