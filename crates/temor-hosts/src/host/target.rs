//! Naming a host's folder or file the way a person writes it: `prod:/srv/app/notes.txt`, and the
//! title a host's tab carries.

use temor_ssh::{HostAlias, RemoteFilePath};

/// A file on a host, as typed or shown: `host:/absolute/path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPath {
    pub host: HostAlias,
    pub path: RemoteFilePath,
}

/// Reads `host:/path`. `None` when there is no colon, the host cannot be an ssh destination, or
/// the path is not an absolute path scp can take as written. A relative path or `~/` has no
/// meaning without a folder to start from: the pane's folder (OSC 7) gives it one, see
/// [`resolve_in`].
pub fn parse_host_path(text: &str) -> Option<HostPath> {
    let (host, path) = text.split_once(':')?;
    Some(HostPath {
        host: HostAlias::new(host.trim()).ok()?,
        path: RemoteFilePath::new(path).ok()?,
    })
}

/// The absolute path a person means by `typed` in a pane whose host folder is `cwd`: an absolute
/// path as it is, anything else relative to `cwd` (`./` and a bare name). `None` without a folder
/// to resolve against, or for `~/` and `..`, which the host would have to expand.
pub fn resolve_in(cwd: Option<&str>, typed: &str) -> Option<RemoteFilePath> {
    let typed = typed.trim();
    if typed.starts_with('/') {
        return RemoteFilePath::new(typed).ok();
    }
    if !resolvable(typed) {
        return None;
    }
    let bare = typed.strip_prefix("./").unwrap_or(typed);
    let base = cwd?.trim_end_matches('/');
    RemoteFilePath::new(&format!("{base}/{bare}")).ok()
}

/// Whether `typed` can be resolved on a host at all, given a folder to resolve against: an
/// absolute path, or a relative one without `~` or `..`, which the host would have to expand.
/// What [`resolve_in`] accepts, before it looks at the folder.
pub fn resolvable(typed: &str) -> bool {
    let typed = typed.trim();
    if typed.starts_with('/') {
        return true;
    }
    let bare = typed.strip_prefix("./").unwrap_or(typed);
    !bare.is_empty() && !bare.starts_with('~') && !bare.split('/').any(|part| part == "..")
}

/// The title of a host's tab: `host:folder`, or the host alone before its folder is known.
pub fn host_title(alias: &str, cwd: Option<&str>) -> String {
    match cwd.filter(|dir| !dir.is_empty()) {
        Some(dir) => format!("{alias}:{dir}"),
        None => alias.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{host_title, parse_host_path, resolvable, resolve_in};

    #[test]
    fn a_host_path_needs_a_usable_host_and_an_absolute_path() {
        let rows = [
            ("prod:/etc/hosts", Some(("prod", "/etc/hosts"))),
            ("me@prod:/a b/c.txt", Some(("me@prod", "/a b/c.txt"))),
            ("prod:relative.txt", None),
            ("prod:/glob/*", None),
            ("-oProxyCommand=x:/a", None),
            ("no colon", None),
            (":/a", None),
        ];
        for (text, want) in rows {
            let got = parse_host_path(text)
                .map(|p| (p.host.as_str().to_owned(), p.path.as_str().to_owned()));
            assert_eq!(
                got,
                want.map(|(h, p)| (h.to_owned(), p.to_owned())),
                "{text}"
            );
        }
    }

    #[test]
    fn a_typed_path_resolves_against_the_panes_folder_and_never_leaves_it() {
        let rows = [
            (Some("/srv/app"), "/etc/hosts", Some("/etc/hosts")),
            (Some("/srv/app"), "notes.txt", Some("/srv/app/notes.txt")),
            (Some("/srv/app/"), "./notes.txt", Some("/srv/app/notes.txt")),
            (Some("/srv/app"), "../secret", None),
            (Some("/srv/app"), "~/x", None),
            (None, "notes.txt", None),
            (None, "/etc/hosts", Some("/etc/hosts")),
            (Some("/srv/app"), "  ", None),
        ];
        for (cwd, typed, want) in rows {
            let got = resolve_in(cwd, typed);
            assert_eq!(got.as_ref().map(|p| p.as_str()), want, "{cwd:?} {typed:?}");
            // With a folder, a path resolves exactly when it is `resolvable`.
            if cwd.is_some() {
                assert_eq!(resolvable(typed), want.is_some(), "{typed:?}");
            }
        }
    }

    #[test]
    fn the_title_is_the_host_then_its_folder_once_known() {
        assert_eq!(host_title("prod", Some("/srv/app")), "prod:/srv/app");
        assert_eq!(host_title("prod", Some("")), "prod");
        assert_eq!(host_title("prod", None), "prod");
    }
}
