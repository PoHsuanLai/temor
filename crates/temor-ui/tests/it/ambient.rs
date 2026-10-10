//! The surroundings' paths: the runtime folder is never the shared temporary directory, and
//! without `XDG_RUNTIME_DIR` it is temor's own folder under the cache. Pure: nothing is created.

use std::ffi::OsString;
use std::path::PathBuf;

use temor_ui::edge::Ambient;

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect()
}

#[test]
fn the_runtime_folder_follows_xdg_runtime_dir_when_set() {
    let ambient = Ambient::from_vars(vars(&[
        ("HOME", "/home/x"),
        ("XDG_RUNTIME_DIR", "/run/user/7"),
    ]))
    .expect("a home directory");
    assert_eq!(ambient.runtime_dir, PathBuf::from("/run/user/7/temor"));
}

#[test]
fn without_xdg_runtime_dir_the_folder_is_under_the_cache_and_never_shared_tmp() {
    let ambient = Ambient::from_vars(vars(&[("HOME", "/home/x")])).expect("a home directory");
    assert_eq!(
        ambient.runtime_dir,
        PathBuf::from("/home/x/.cache/temor/run")
    );
    assert!(!ambient.runtime_dir.starts_with(std::env::temp_dir()));
}

#[test]
fn the_downloads_folder_is_the_variable_then_user_dirs_then_home_downloads() {
    let user_dirs = "XDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/Got\"\n";
    let cases: [(&[(&str, &str)], Option<&str>, &str); 5] = [
        (&[("HOME", "/h")], None, "/h/Downloads"),
        (&[("HOME", "/h")], Some(user_dirs), "/h/Got"),
        (
            &[("HOME", "/h"), ("XDG_DOWNLOAD_DIR", "/data/in")],
            Some(user_dirs),
            "/data/in",
        ),
        // A relative value is not a folder to trust.
        (
            &[("HOME", "/h")],
            Some("XDG_DOWNLOAD_DIR=\"in\"\n"),
            "/h/Downloads",
        ),
        (
            &[("HOME", "/h")],
            Some("XDG_DOWNLOAD_DIRS=\"/x\"\n"),
            "/h/Downloads",
        ),
    ];
    for (env, file, want) in cases {
        let ambient = Ambient::from_vars(vars(env)).expect("a home directory");
        assert_eq!(
            ambient.downloads_dir(file),
            PathBuf::from(want),
            "{env:?} {file:?}"
        );
    }
}
