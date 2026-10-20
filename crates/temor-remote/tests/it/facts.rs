//! A session's folder read for the chips: git's text and the version files, through the link.

use temor_remote::{Error, hello};
use temor_wire::hello::Capability;
use temor_wire::ids::SessionId;
use temor_wire::message::{RemoteError, RemotePath};

use crate::fixtures::git;
use crate::support::{HELPER, Rig, tools_present};

#[test]
fn a_sessions_folder_is_read_with_git_and_the_version_files_up_to_the_repository() {
    if !tools_present() || temor_testkit::program_or_skip("git", "facts").is_none() {
        return;
    }
    let rig = Rig::new("facts-folder");
    let repo = rig.home.join("repo");
    let sub = repo.join("src");
    std::fs::create_dir_all(&sub).expect("folders");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").expect("file");
    std::fs::write(repo.join(".nvmrc"), "v20\n").expect("version file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    std::fs::write(repo.join("a.txt"), "one\ntwo\n").expect("change");
    std::fs::write(repo.join("new.txt"), "x\n").expect("untracked");
    std::fs::write(rig.home.join(".nvmrc"), "v14\n").expect("above the repository");

    let mut open = rig.bash();
    open.cwd = RemotePath(sub.display().to_string());
    let conn = rig.connect();
    let id = conn.client.open_session(open).expect("open");
    let facts = conn.client.folder_facts(id).expect("facts");

    let git = facts.git.expect("git spoke");
    assert_eq!(git.head.as_deref().map(str::trim), Some("main"));
    assert!(
        git.shortstat
            .as_deref()
            .is_some_and(|s| s.contains("1 file changed")),
        "{git:?}"
    );
    assert_eq!(git.untracked.as_deref(), Some("?? new.txt\n"));
    assert_eq!(facts.root, Some(RemotePath(repo.display().to_string())));
    let dirs: Vec<&str> = facts.folders.iter().map(|f| f.dir.0.as_str()).collect();
    assert_eq!(
        dirs,
        [
            sub.to_str().unwrap_or_default(),
            repo.to_str().unwrap_or_default()
        ],
        "the walk stops at the repository"
    );
    assert_eq!(facts.folders[1].files[0].name, ".nvmrc");
    assert_eq!(facts.folders[1].files[0].text, "v20\n");
    // The home is the helper's own, so the window can write the folder as `~/repo/src`.
    assert_eq!(facts.home, Some(RemotePath(rig.home.display().to_string())));
}

#[test]
fn the_facts_name_the_folder_the_session_is_in_and_no_virtualenv_where_there_is_none() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("facts-plain");
    let conn = rig.connect();
    let id = conn.client.open_session(rig.bash()).expect("open");
    let facts = conn.client.folder_facts(id).expect("facts");
    // The scratch folder may sit inside some repository above it; a plain one does not.
    assert_eq!(facts.dir, RemotePath(rig.home.display().to_string()));
    assert!(facts.venv.is_none());
}

#[test]
fn facts_are_asked_only_of_a_helper_that_offers_them_and_only_for_a_session_it_has() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("facts-asked");
    let old = rig.connect_with(hello(HELPER, &[Capability::Sessions]));
    let id = old.client.open_session(rig.bash()).expect("open");
    assert!(matches!(
        old.client.folder_facts(id),
        Err(Error::NotOffered(Capability::Facts))
    ));
    let conn = rig.connect();
    assert!(matches!(
        conn.client.folder_facts(SessionId(999)),
        Err(Error::Remote(RemoteError::NoSuchSession { .. }))
    ));
}
