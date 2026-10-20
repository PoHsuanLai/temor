//! The shell offer in the window (ds-harness, event dispatch only). A stand-in `ssh` on a
//! private `PATH` keeps running without printing, which is all the timing rule looks at. After a
//! Yes it saves the first line it is sent (the question to the shell), answers it the way a zsh
//! would, and saves the next bytes (the setup), so the test sees what the pane is given and in
//! which order. Nothing else leaves the scratch folders.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ds_harness::Harness;
use temor_ui::view::probe::Probe;

use super::drive::{focus_pane, press, type_line, wait_html, wait_until};
use super::support::{scratch_path, tools_present};
use super::window::harness_with;

const QUESTION: &str = "Show commands as blocks in this shell?";

/// A window whose `ssh` reads a line of raw input into `saved`, prints a shell's name and a
/// prompt, reads 40 more bytes into `saved` with `-load` added to its name, then sleeps. The
/// first command, `ssh box`, has been typed.
fn window_running_ssh(name: &str, saved: &Path) -> (Harness, Probe) {
    // Beside the window's scratch folder, which `harness_with` empties when it starts.
    let bin = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-bin"));
    let _ = std::fs::remove_dir_all(&bin);
    std::fs::create_dir_all(&bin).expect("bin folder");
    let script = format!(
        "#!/bin/sh\nstty -icanon -echo\nIFS= read -r line\nprintf '%s\\n' \"$line\" > '{}'\nprintf 'temor-shell:zsh\\n$ '\nhead -c 40 > '{}-load'\nexec sleep 20\n",
        saved.display(),
        saved.display()
    );
    let ssh = bin.join("ssh");
    std::fs::write(&ssh, script).expect("stand-in ssh");
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let probe = Probe::new();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let mut h = harness_with(name, &probe, None, move |ambient| {
        ambient.env.set("PATH", path);
    });
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "ssh box");
    (h, probe)
}

/// Steps the window until `path` holds at least `len` bytes.
fn wait_bytes(h: &mut Harness, path: &Path, len: usize) -> Vec<u8> {
    let start = Instant::now();
    loop {
        h.advance(Duration::from_millis(25));
        let bytes = std::fs::read(path).unwrap_or_default();
        if bytes.len() >= len {
            return bytes;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "nothing was typed"
        );
    }
}

fn saved_path(name: &str) -> PathBuf {
    scratch_path(name).join("saved-input")
}

#[test]
fn yes_asks_the_shell_its_name_then_types_the_setup_written_for_it() {
    if !tools_present() {
        return;
    }
    let name = "offer-yes";
    let (mut h, _probe) = window_running_ssh(name, &saved_path(name));
    wait_html(&mut h, "the offer", |html| html.contains(QUESTION));
    let before = std::fs::read(saved_path(name)).unwrap_or_default();
    assert!(before.is_empty(), "nothing is typed before Yes");

    press(&mut h, ".ds-inline-banner .ds-button:first-of-type");
    let asked = wait_bytes(&mut h, &saved_path(name), 40);
    assert!(
        asked.starts_with(b" sh -c 'echo temor-shell:"),
        "{}",
        String::from_utf8_lossy(&asked)
    );
    let mut load = saved_path(name).into_os_string();
    load.push("-load");
    let typed = wait_bytes(&mut h, &PathBuf::from(load), 40);
    assert!(
        typed.starts_with(b" [ -n \"$ZSH_VERSION\" ]"),
        "{}",
        String::from_utf8_lossy(&typed)
    );
    assert!(!h.html().contains(QUESTION), "the offer goes away");
}

#[test]
fn never_for_the_program_is_kept_in_the_settings_file() {
    if !tools_present() {
        return;
    }
    let name = "offer-never";
    let (mut h, _probe) = window_running_ssh(name, &saved_path(name));
    wait_html(&mut h, "the offer", |html| html.contains(QUESTION));
    assert!(h.html().contains("Never for ssh"));

    press(&mut h, ".ds-inline-banner .ds-button:last-of-type");
    assert!(!h.html().contains(QUESTION));
    // The test's config folder is not called temor, so the file is in a temor folder inside it.
    let settings = scratch_path(name).join("home/config/temor/settings.toml");
    let text = std::fs::read_to_string(&settings).expect("the choice is written");
    let table: toml::Table = text.parse().expect("a settings file");
    let remove = table["offer"]["remove"].as_array().expect("the list");
    assert_eq!(remove.len(), 1);
    assert_eq!(remove[0].as_str(), Some("ssh"));
}
