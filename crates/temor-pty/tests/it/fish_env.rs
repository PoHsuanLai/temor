//! The fish shim takes only its own entry back out of `XDG_DATA_DIRS`.

use temor_pty::{ChildEnv, Shell, ShellStart, prepare_shell};

use crate::support::{Scratch, Term, program_or_skip, setsid, size};

/// Starts fish with `conf_d` (name, contents) in the scratch config and returns what
/// `XDG_DATA_DIRS` holds there, or `<unset>`.
fn data_dirs_in_fish(conf_d: Option<(&str, &str)>) -> Option<String> {
    let fish = program_or_skip("fish", "fish_env")?;
    let scratch = Scratch::new();
    if let Some((name, contents)) = conf_d {
        let dir = scratch.home.join(".config/fish/conf.d");
        std::fs::create_dir_all(&dir).expect("conf.d");
        std::fs::write(dir.join(name), contents).expect("snippet");
    }
    let spec = prepare_shell(
        &Shell::at(fish),
        ShellStart::Interactive,
        scratch.contract_env(&ChildEnv::new()),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(
        term.read_until_has(0, "\x1b]133;B"),
        "no prompt: {:?}",
        term.text()
    );
    let start = term.out.len();
    term.send(
        "echo \"xdg=[$XDG_DATA_DIRS]\" (set -q XDG_DATA_DIRS; and echo set; or echo unset) end-(math 1+1)\r",
    );
    assert!(term.read_until_has(start, "end-2"), "{:?}", term.text());
    let text = String::from_utf8_lossy(&term.out[start..]).into_owned();
    // The last `xdg=[` is the output; the typed command line shows `xdg=[$XDG…` first.
    let at = text.rfind("xdg=[").expect("the echoed line");
    let end = text[at..].find(" end-2").expect("the end marker") + at;
    let line = text[at..end].to_owned();
    Some(line)
}

#[test]
fn fish_restores_an_unset_xdg_data_dirs() {
    if crate::support::pty_unsupported("fish_restores_an_unset_xdg_data_dirs") {
        return;
    }
    let Some(line) = data_dirs_in_fish(None) else {
        return;
    };
    // Other vendor snippets (Fedora's flatpak.fish) may add their own entries; temor's own
    // entry must be gone, and its stand-in for the default must not linger as a set value.
    assert!(!line.contains("shell-integration"), "{line:?}");
    assert_ne!(line, "xdg=[/usr/local/share:/usr/share] set");
}

/// Why: restoring the saved value wiped what earlier conf.d snippets had added.
#[test]
fn fish_keeps_what_an_earlier_snippet_added() {
    if crate::support::pty_unsupported("fish_keeps_what_an_earlier_snippet_added") {
        return;
    }
    let snippet = (
        "00-add.fish",
        "set -gx XDG_DATA_DIRS $XDG_DATA_DIRS:/opt/extra\n",
    );
    let Some(line) = data_dirs_in_fish(Some(snippet)) else {
        return;
    };
    assert!(!line.contains("shell-integration"), "{line:?}");
    assert!(
        line.ends_with("/usr/local/share:/usr/share:/opt/extra] set"),
        "{line:?}"
    );
}
