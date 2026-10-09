//! Route matching: command globs, folder prefixes and which route wins.

use super::{Project, Route, SpaceRoute, route, route_folder, routes_of};
use crate::vocab::text::RoutePattern;
use ds_style::space::list::{SpaceId, Spaces};

/// Space 0 is the fallback with no routes; the rest are given as patterns per Space.
fn table(per_space: &[&[&str]]) -> Vec<(SpaceId, RoutePattern)> {
    per_space
        .iter()
        .enumerate()
        .flat_map(|(at, patterns)| {
            patterns
                .iter()
                .map(move |p| (SpaceId(at as u64), RoutePattern((*p).to_owned())))
        })
        .collect()
}

fn borrowed(table: &[(SpaceId, RoutePattern)]) -> Vec<SpaceRoute<'_>> {
    table
        .iter()
        .map(|(space, pattern)| SpaceRoute {
            space: *space,
            pattern,
        })
        .collect()
}

#[test]
fn commands_route_by_glob_and_the_most_specific_route_wins() {
    let patterns = table(&[
        &[],
        &["ssh prod-*", "kubectl *"],
        &["ssh *", "psql"],
        &["ssh prod-db?", "docker compose *"],
    ]);
    let routes = borrowed(&patterns);
    #[rustfmt::skip]
    let cases: &[(&str, &str, Option<u64>)] = &[
        ("a host under the prefix", "ssh prod-web1", Some(1)),
        ("the prefix with options after it", "ssh prod-web1 -p 2222", Some(1)),
        ("the more literal glob wins", "ssh prod-db1", Some(3)),
        ("a general glob catches the rest", "ssh staging", Some(2)),
        ("blanks collapse", "  ssh    prod-web1  ", Some(1)),
        ("no wildcard means the whole command", "psql", Some(2)),
        ("not the whole command", "psql mydb", None),
        ("a star may match nothing at all", "ssh prod-", Some(1)),
        ("a trailing blank is trimmed, so a star after a space needs more", "kubectl ", None),
        ("case matters", "SSH prod-web1", None),
        ("the command must start with the words", "echo ssh prod-web1", None),
        ("no route", "ls", None),
        ("an empty command", "", None),
    ];
    for (name, command, want) in cases {
        assert_eq!(route(command, &routes), want.map(SpaceId), "{name}");
    }
}

#[test]
fn a_tie_goes_to_the_earlier_space_and_an_empty_pattern_matches_nothing() {
    let patterns = table(&[&[], &["ssh a*"], &["ssh b*", "ssh a*", ""]]);
    let routes = borrowed(&patterns);
    assert_eq!(route("ssh abc", &routes), Some(SpaceId(1)));
    assert_eq!(route("", &routes), None);
    assert_eq!(route("anything", &routes), None);
}

#[test]
fn folders_route_by_prefix_and_the_longest_wins() {
    let patterns = table(&[&[], &["~/work"], &["~/work/api/", "/srv"], &["ssh *"]]);
    let routes = borrowed(&patterns);
    #[rustfmt::skip]
    let cases: &[(&str, &str, Option<u64>)] = &[
        ("the folder itself", "~/work", Some(1)),
        ("below it", "~/work/web/src", Some(1)),
        ("the longer prefix wins", "~/work/api/v2", Some(2)),
        ("a trailing slash is ignored", "~/work/api/", Some(2)),
        ("a folder that only shares letters", "~/workshop", None),
        ("an absolute prefix", "/srv/www", Some(2)),
        ("elsewhere", "/home/me", None),
        ("a command pattern is not a folder", "ssh x", None),
    ];
    for (name, folder, want) in cases {
        assert_eq!(route_folder(folder, &routes), want.map(SpaceId), "{name}");
    }
    assert_eq!(
        route("~/work", &routes),
        None,
        "a folder pattern is not a command"
    );
}

#[test]
fn routes_are_read_from_the_spaces_in_order() {
    let projects = [
        Project::default(),
        Project {
            routes: vec![Route {
                pattern: RoutePattern("ssh prod-*".into()),
            }],
            ..Project::default()
        },
    ];
    let spaces: Spaces<Project, ()> = Spaces::first_run(projects, Project::default);
    let routes = routes_of(spaces.list());
    assert_eq!(routes.len(), 1);
    assert_eq!(route("ssh prod-1", &routes), Some(SpaceId(1)));
}
