//! The address from plain paths.

use super::*;

fn names(path: &str) -> Vec<String> {
    crumbs_of(path, "/home/ada")
        .into_iter()
        .map(|crumb| crumb.name)
        .collect()
}

#[test]
fn a_path_in_the_home_folder_starts_at_the_tilde() {
    assert_eq!(
        names("/home/ada/ml/runs/loss.png"),
        ["~", "ml", "runs", "loss.png"]
    );
    let crumbs = crumbs_of("/home/ada/ml/runs/", "/home/ada/");
    assert_eq!(crumbs[0].path, "/home/ada");
    assert_eq!(crumbs[2].path, "/home/ada/ml/runs");
    assert_eq!(names("/home/ada/"), ["~"]);
    assert_eq!(names("/home/ada"), ["~"]);
}

#[test]
fn a_path_elsewhere_starts_at_the_root() {
    assert_eq!(names("/etc/nginx"), ["/", "etc", "nginx"]);
    assert_eq!(crumbs_of("/etc/nginx", "")[2].path, "/etc/nginx");
    assert_eq!(names("/"), ["/"]);
    // A folder that only begins with the home folder's name is not in it.
    assert_eq!(names("/home/adam/x"), ["/", "home", "adam", "x"]);
}

#[test]
fn a_relative_path_has_no_address() {
    assert!(names("ml/runs").is_empty());
    assert!(names("").is_empty());
}
