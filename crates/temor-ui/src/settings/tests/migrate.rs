//! The one-time conversion of `terminal.conf`, on files in a scratch folder.

use std::path::{Path, PathBuf};

use chordkit::Overrides;
use temor_pane::settings::{CursorStyle, ShellChoice};

use crate::edge::Ambient;
use crate::settings::{TerminalDoc, convert_terminal_conf, load_settings};

const OLD: &str = "# my terminal\n\
                   font.size = 15\n\
                   cursor.style = bar\n\
                   shell.program = /bin/zsh\n\
                   offer.add = kubectl debug -it\n\
                   temor.split-right = Primary+Alt+N\n\
                   font.size = huge\n\
                   keys.copy = ctrl+c\n\
                   bogus = 1\n";

/// An ambient whose config folder is `<scratch>/config/temor`, as on a real machine.
fn ambient(scratch: &Path) -> Ambient {
    let home = scratch.join("home");
    Ambient {
        home: home.clone(),
        env: temor_shell::ChildEnv::new(),
        config_dir: scratch.join("config/temor"),
        runtime_dir: home.join("runtime"),
    }
}

fn with_old(text: &str) -> (tempfile::TempDir, Ambient) {
    let dir = tempfile::tempdir().expect("scratch dir");
    let ambient = ambient(dir.path());
    std::fs::create_dir_all(&ambient.config_dir).expect("config folder");
    std::fs::write(ambient.old_settings_path(), text).expect("terminal.conf");
    (dir, ambient)
}

fn old_names(ambient: &Ambient) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(&ambient.config_dir)
        .expect("config folder")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("terminal.conf"))
        })
        .collect();
    found.sort();
    found
}

#[test]
fn the_old_text_splits_into_settings_and_keys_and_keeps_what_it_cannot_read() {
    let converted = convert_terminal_conf(OLD);
    assert_eq!(converted.doc.font.size.0.points(), 15.0);
    assert_eq!(converted.doc.offer.add, ["kubectl debug -it"]);
    // The key line is in chordkit's form; the refused ones are comments, never lost.
    let (keys, problems) = Overrides::parse(&converted.keys_text);
    assert!(problems.is_empty(), "{problems:?}");
    let ids: Vec<String> = keys.entries().iter().map(|e| e.action.id()).collect();
    assert_eq!(ids, ["temor.split-right"]);
    assert!(
        converted
            .keys_text
            .contains("# not read from terminal.conf")
    );
    assert!(converted.keys_text.contains("keys.copy = ctrl+c"));
    assert!(converted.settings_notes.contains("font.size = huge"));
    assert!(converted.settings_notes.contains("bogus = 1"));
    let lines: Vec<usize> = converted.issues.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, [7, 8, 9]);
}

#[test]
fn a_launch_converts_once_and_keeps_the_old_file() {
    let (_dir, ambient) = with_old(OLD);
    let first = load_settings(&ambient).expect("loads");
    assert_eq!(first.settings.font_size.points(), 15.0);
    assert_eq!(first.settings.cursor, CursorStyle::Bar);
    assert_eq!(
        first.settings.shell,
        ShellChoice::Program("/bin/zsh".to_owned())
    );
    assert_eq!(first.settings.keys.entries().len(), 1);
    // Told: the move, and each line that was not read.
    assert!(
        first.notes[0].contains("terminal.conf.old"),
        "{:?}",
        first.notes
    );
    assert_eq!(
        first
            .notes
            .iter()
            .filter(|n| n.contains("terminal.conf line"))
            .count(),
        3,
        "{:?}",
        first.notes
    );
    // The old file is renamed, the new ones are there.
    let names: Vec<String> = old_names(&ambient)
        .iter()
        .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
        .collect();
    assert_eq!(names, ["terminal.conf.old"]);
    assert_eq!(
        std::fs::read_to_string(ambient.config_dir.join("terminal.conf.old")).expect("old"),
        OLD
    );
    assert!(ambient.keys_path().is_file());
    assert!(ambient.settings_dir().join("settings.toml").is_file());
    let doc = ambient.settings_store().load::<TerminalDoc>().value;
    assert_eq!(doc.font.size.0.points(), 15.0);

    // The second launch reads the new files and says nothing about the old.
    let second = load_settings(&ambient).expect("loads");
    assert_eq!(second.settings, first.settings);
    assert!(second.notes.is_empty(), "{:?}", second.notes);
}

#[test]
fn nothing_is_converted_over_a_settings_file_or_without_an_old_one() {
    // An existing settings.toml wins; terminal.conf stays where it is.
    let (_dir, ambient) = with_old("font.size = 15\n");
    std::fs::write(
        ambient.settings_dir().join("settings.toml"),
        "[font]\nsize = 20\n",
    )
    .expect("settings.toml");
    let loaded = load_settings(&ambient).expect("loads");
    assert_eq!(loaded.settings.font_size.points(), 20.0);
    assert!(loaded.notes.is_empty(), "{:?}", loaded.notes);
    assert!(ambient.old_settings_path().is_file());
    assert!(!ambient.keys_path().exists());

    // Neither file: the defaults, and nothing is written.
    let dir = tempfile::tempdir().expect("scratch dir");
    let bare = ambient(dir.path());
    let loaded = load_settings(&bare).expect("loads");
    assert_eq!(
        loaded.settings,
        temor_pane::settings::TerminalSettings::default()
    );
    assert!(!bare.config_dir.exists());
}

#[test]
fn an_old_file_that_is_already_set_aside_is_not_overwritten() {
    let (_dir, ambient) = with_old("font.size = 15\n");
    std::fs::write(ambient.config_dir.join("terminal.conf.old"), "earlier").expect("old");
    load_settings(&ambient).expect("loads");
    let names: Vec<String> = old_names(&ambient)
        .iter()
        .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
        .collect();
    assert_eq!(names, ["terminal.conf.old", "terminal.conf.old.1"]);
    assert_eq!(
        std::fs::read_to_string(ambient.config_dir.join("terminal.conf.old")).expect("old"),
        "earlier"
    );
}
