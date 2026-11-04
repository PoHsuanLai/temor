//! The address at the top of the sidebar (PLAN §7, v14 B1): the folders down to what the row in
//! front shows, as a browser's address bar says where a tab is. The home folder is `~`. Folding a
//! long path's middle is the path bar's (quire's `PathControl`).

/// One folder (or the file at the end) on the way down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    /// What it is called: its own name, `~` for the home folder, `/` for the root.
    pub name: String,
    /// Its absolute path.
    pub path: String,
}

/// Every folder from the top (`/`, or `~` for a path in the home folder `home`) down to the
/// absolute `path`. A relative or empty path has none.
pub fn crumbs_of(path: &str, home: &str) -> Vec<Crumb> {
    if !path.starts_with('/') {
        return Vec::new();
    }
    let home = home.trim_end_matches('/');
    let in_home = !home.is_empty()
        && path
            .strip_prefix(home)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'));
    let (top, rest) = match in_home {
        true => (("~", home), &path[home.len()..]),
        false => (("/", ""), path),
    };
    let mut crumbs = vec![Crumb {
        name: top.0.to_owned(),
        path: match top.1 {
            "" => "/".to_owned(),
            home => home.to_owned(),
        },
    }];
    let mut at = top.1.to_owned();
    for name in rest.split('/').filter(|name| !name.is_empty()) {
        at = format!("{at}/{name}");
        crumbs.push(Crumb {
            name: name.to_owned(),
            path: at.clone(),
        });
    }
    crumbs
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
