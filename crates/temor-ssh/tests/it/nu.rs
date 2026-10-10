//! The bootstrap's nushell case, with a stand-in `nu` that reports how it was started: the login
//! flag, an `XDG_DATA_DIRS` whose first entry holds `temor.nu`, and the value to restore.

use crate::harness::{Launch, make_bin, scratch};

const FAKE_NU: &str = "#!/bin/sh\n\
echo \"ARGS=$*\"\n\
echo \"X=$XDG_DATA_DIRS\"\n\
echo \"R=${TEMOR_NU_XDG_DATA_DIRS-unset}\"\n\
first=${XDG_DATA_DIRS%%:*}\n\
ls \"$first/nushell/vendor/autoload\"\n";

fn line<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        // The bootstrap's own report (the folder, OSC 7) may share the stand-in's first line.
        .find_map(|l| l.find(key).map(|at| &l[at + key.len()..]))
        .unwrap_or("<missing>")
}

/// Runs the bootstrap with the stand-in as `$SHELL`; `before` runs in the outer shell first.
fn started_with(before: &str) -> (String, String) {
    let s = scratch();
    make_bin(&s.bin, &[], &[("nu", FAKE_NU)]);
    let mut launch = Launch::new(&s);
    launch.shell = s.bin.join("nu").display().to_string();
    launch.command = format!("{before}{}", launch.command);
    launch.input = String::new();
    let ran = launch.run();
    let text = String::from_utf8_lossy(&ran.text).into_owned();
    assert!(ran.status.success(), "{:?}: {text}", ran.status);
    (text, s.tmp.display().to_string())
}

#[test]
fn nushell_starts_as_a_login_shell_with_temors_folder_first_in_the_data_dirs() {
    let cases = [
        (
            "an unset XDG_DATA_DIRS starts from the XDG default and restores nothing",
            "",
            ":/usr/local/share:/usr/share",
            "unset",
        ),
        (
            "a set XDG_DATA_DIRS is kept after temor's entry and handed over to restore",
            "XDG_DATA_DIRS=/opt/x:/usr/share; export XDG_DATA_DIRS; ",
            ":/opt/x:/usr/share",
            "/opt/x:/usr/share",
        ),
    ];
    for (name, before, tail, restore) in cases {
        let (text, tmp) = started_with(before);
        assert_eq!(line(&text, "ARGS="), "-l", "{name}: {text}");
        let x = line(&text, "X=");
        assert!(x.starts_with(&format!("{tmp}/temor.")), "{name}: {x}");
        assert!(x.ends_with(&format!("/nu{tail}")), "{name}: {x}");
        assert_eq!(line(&text, "R="), restore, "{name}");
        assert!(
            text.contains("temor.nu\n"),
            "{name}: no autoload file: {text}"
        );
    }
}
