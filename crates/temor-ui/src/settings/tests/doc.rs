//! The document, its schema, and its reading: defaults match the pane's, every setting has a key,
//! and a value outside the pane's limits is refused by the file's reader.

use chordkit::Overrides;
use ds_settings::schema::{KeyKind, Schema};
use ds_settings::{ConfigRoot, Store};
use temor_pane::settings::{
    CursorBlinking, CursorStyle, FontChoice, NotifyAfter, ShellChoice, StartIn, TerminalSettings,
};

use crate::settings::{APP, StartFolder, TerminalDoc, from_terminal, schema};

fn store_with(text: &str) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("scratch dir");
    std::fs::create_dir_all(dir.path().join("temor")).expect("folder");
    std::fs::write(dir.path().join("temor/settings.toml"), text).expect("file");
    let store = Store::new(ConfigRoot::Scratch(dir.path().to_path_buf()), APP);
    (dir, store)
}

#[test]
fn the_defaults_are_the_panes_defaults() {
    let (settings, problems) = TerminalDoc::default().terminal(Overrides::default());
    assert_eq!(settings, TerminalSettings::default());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        from_terminal(&TerminalSettings::default()),
        TerminalDoc::default()
    );
}

#[test]
fn the_schema_survives_its_own_file_and_names_every_setting_once() {
    let schema = schema();
    assert_eq!(Schema::from_toml(&schema.to_toml()), Ok(schema.clone()));
    // Every field of the pane's settings but the keys (their own file) has one key.
    let TerminalSettings {
        font: _,
        font_size: _,
        scrollback: _,
        cursor: _,
        cursor_blink: _,
        shell: _,
        start_in: _,
        keys: _,
        notify_after: _,
        offer_add: _,
        offer_remove: _,
    } = TerminalSettings::default();
    let paths: Vec<&str> = schema.key.iter().map(|key| key.path.0.as_str()).collect();
    assert_eq!(
        paths,
        [
            "font.family",
            "font.size",
            "cursor.style",
            "cursor.blink",
            "scrollback.lines",
            "shell.program",
            "shell.start_in",
            "notify.after",
            "offer.add",
            "offer.remove",
        ]
    );
    let size = schema.key.iter().find(|key| key.path.0 == "font.size");
    assert!(matches!(
        size.map(|key| &key.kind),
        Some(KeyKind::Bounded {
            min: 6,
            max: 72,
            ..
        })
    ));
}

#[test]
fn each_key_sets_its_setting() {
    let (_dir, store) = store_with(
        r#"
[font]
family = "Fira Code"
size = 15
[cursor]
style = "bar"
blink = "off"
[scrollback]
lines = 500
[shell]
program = "/bin/zsh"
start_in = "current"
[notify]
after = 30
[offer]
add = ["kubectl debug -it", "kubectl   exec"]
remove = ["su"]
"#,
    );
    let loaded = store.load::<TerminalDoc>();
    assert!(
        loaded.invalid.is_empty() && loaded.unknown.is_empty(),
        "{loaded:?}"
    );
    let (settings, problems) = loaded.value.terminal(Overrides::default());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(settings.font, FontChoice::Named("Fira Code".to_owned()));
    assert_eq!(settings.font_size.points(), 15.0);
    assert_eq!(settings.cursor, CursorStyle::Bar);
    assert_eq!(settings.cursor_blink, CursorBlinking::Off);
    assert_eq!(settings.scrollback.lines(), 500);
    assert_eq!(settings.shell, ShellChoice::Program("/bin/zsh".to_owned()));
    assert_eq!(settings.start_in, StartIn::Current);
    assert_eq!(settings.notify_after, NotifyAfter::Seconds(30));
    assert_eq!(settings.offer_add, ["kubectl debug -it", "kubectl exec"]);
    assert_eq!(settings.offer_remove, ["su"]);
}

#[test]
fn a_value_outside_the_panes_limits_costs_only_its_key() {
    // (file, the key refused)
    for (text, key) in [
        ("[font]\nsize = 5\n", "font.size"),
        ("[font]\nsize = 73\n", "font.size"),
        ("[scrollback]\nlines = 1000001\n", "scrollback.lines"),
        ("[notify]\nafter = 86401\n", "notify.after"),
        ("[cursor]\nstyle = \"blink\"\n", "cursor.style"),
    ] {
        let (_dir, store) = store_with(&format!("{text}[shell]\nprogram = \"fish\"\n"));
        let loaded = store.load::<TerminalDoc>();
        let refused: Vec<&str> = loaded.invalid.iter().map(|k| k.path.as_str()).collect();
        assert_eq!(refused, [key], "{text}");
        let (settings, _) = loaded.value.terminal(Overrides::default());
        let want = TerminalSettings {
            shell: ShellChoice::Program("fish".to_owned()),
            ..TerminalSettings::default()
        };
        assert_eq!(settings, want, "{text}");
    }
}

#[test]
fn a_size_written_with_a_fraction_or_as_a_whole_number_both_read() {
    for (text, want) in [("size = 13.5", 13.5), ("size = 14", 14.0)] {
        let (_dir, store) = store_with(&format!("[font]\n{text}\n"));
        let loaded = store.load::<TerminalDoc>();
        assert!(loaded.invalid.is_empty(), "{text}: {loaded:?}");
        assert_eq!(loaded.value.font.size.0.points(), want, "{text}");
    }
}

#[test]
fn names_the_pane_cannot_use_are_problems_and_fall_back() {
    let mut doc = TerminalDoc::default();
    doc.font.family = "  ".to_owned();
    doc.shell.program = String::new();
    doc.offer.add = vec!["ok one".to_owned(), "  ".to_owned()];
    let (settings, problems) = doc.terminal(Overrides::default());
    assert_eq!(settings.font, FontChoice::SystemMonospace);
    assert_eq!(settings.shell, ShellChoice::Login);
    assert_eq!(settings.offer_add, ["ok one"]);
    assert_eq!(problems.len(), 3, "{problems:?}");
}

#[test]
fn start_in_reads_each_word_and_a_bad_one_costs_only_itself() {
    // (the file, the setting read, the key refused)
    let rows = [
        ("usual", StartIn::Usual, None),
        ("current", StartIn::Current, None),
        ("home", StartIn::Home, None),
        ("elsewhere", StartIn::Usual, Some("shell.start_in")),
    ];
    for (word, want, refused) in rows {
        let (_dir, store) = store_with(&format!("[shell]\nstart_in = \"{word}\"\n"));
        let loaded = store.load::<TerminalDoc>();
        let bad: Vec<&str> = loaded.invalid.iter().map(|k| k.path.as_str()).collect();
        assert_eq!(bad, refused.into_iter().collect::<Vec<_>>(), "{word}");
        let (settings, _) = loaded.value.terminal(Overrides::default());
        assert_eq!(settings.start_in, want, "{word}");
        assert_eq!(
            from_terminal(&settings).shell.start_in,
            StartFolder::from(want),
            "{word}"
        );
    }
}
