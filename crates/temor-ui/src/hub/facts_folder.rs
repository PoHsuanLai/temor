//! The folder chip of a pane on a host (PLAN P16, §9b Tier B): the folder the helper read, with
//! the helper's home folder written as `~`, as a pane on this computer shows its own.

use std::path::Path;

use temor_wire::message::FolderFacts;

use super::rows::tidy;

/// The folder as its chip says it. None when the helper did not say its home (an older one):
/// the chip then stays hidden, as it did before the helper told.
pub(super) fn host_folder(sent: &FolderFacts) -> Option<String> {
    let home = sent.home.as_ref()?;
    Some(tidy(&sent.dir.0, Path::new(&home.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::message::RemotePath;

    fn sent(dir: &str, home: Option<&str>) -> FolderFacts {
        FolderFacts {
            dir: RemotePath(dir.to_owned()),
            venv: None,
            home: home.map(|home| RemotePath(home.to_owned())),
            root: None,
            folders: vec![],
        }
    }

    #[test]
    fn the_hosts_home_is_written_as_a_tilde_and_other_folders_stay_whole() {
        let table = [
            ("home itself", "/home/me", Some("/home/me"), Some("~")),
            (
                "under home",
                "/home/me/app/src",
                Some("/home/me"),
                Some("~/app/src"),
            ),
            (
                "outside home",
                "/srv/app",
                Some("/home/me"),
                Some("/srv/app"),
            ),
            (
                "a sibling of home",
                "/home/mel",
                Some("/home/me"),
                Some("/home/mel"),
            ),
            ("an older helper", "/home/me/app", None, None),
        ];
        for (why, dir, home, want) in table {
            assert_eq!(host_folder(&sent(dir, home)).as_deref(), want, "{why}");
        }
    }
}
