//! The Settings window (ds-harness, event dispatch only, never OS input): ⌘, and the command bar
//! ask for it, a changed font size writes `settings.toml`, a recorded chord writes `keys.conf`
//! and a held one is refused with the holder's name, Reset All empties the changes, and a Space's
//! folder reaches `spaces.json` through the main window. The window opens on a page the way the
//! main window asks: the harness has no event loop to open a window on, so the opener is a
//! recorder and the Settings root is drawn directly.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px, SystemPrefs};
use ds_harness::{Driver, Harness, HarnessConfig, Input, Viewport, WindowHosting};
use ds_lint::{LintConfig, Rule, markup};
use temor_core::space::Project;
use temor_core::vocab::text::ProjectRoot;
use temor_pane::keys::SettingsPane;
use temor_pane::settings::TerminalSettings;
use temor_ui::edge::{Ambient, SpacesStartup, load_spaces};
use temor_ui::settings::TerminalDoc;
use temor_ui::settings_window::{
    OpenSettings, SETTINGS_CSS, SettingsAsked, SettingsWindows, SpaceBoard, SpaceChange, SpaceEdit,
    SpaceRow, settings_root,
};
use temor_ui::view::probe::Probe;
use temor_ui::{AppearanceSource, Launch, LauncherChoice};

use super::support::{scratch, tools_present};
use super::window::{chord, harness_building, wait_until};

/// The opener of a test: remembers each ask.
#[derive(Default)]
struct Recorder(Mutex<Vec<Option<SettingsPane>>>);

impl Recorder {
    fn asks(&self) -> Vec<Option<SettingsPane>> {
        self.0.lock().map(|asks| asks.clone()).unwrap_or_default()
    }
}

impl OpenSettings for Recorder {
    fn open(&self, at: Option<SettingsPane>) {
        if let Ok(mut asks) = self.0.lock() {
            asks.push(at);
        }
    }
}

/// The Settings window over a scratch config folder, opened on `page`; `extra` may add root
/// contexts. Returns the window and the surroundings it reads and writes.
fn settings_window(
    name: &str,
    page: SettingsPane,
    extra: impl FnOnce(HarnessConfig) -> HarnessConfig,
) -> (Harness, Ambient) {
    let home = scratch(name).join("home");
    let ambient = Ambient {
        home: home.clone(),
        env: [("HOME", home.as_os_str())]
            .into_iter()
            .map(|(k, v)| (std::ffi::OsString::from(k), v.to_os_string()))
            .collect(),
        config_dir: home.join("config"),
        runtime_dir: home.join("runtime"),
    };
    let launch = Launch {
        ambient: ambient.clone(),
        settings: TerminalSettings::default(),
        appearance: AppearanceSource::scratch(home.join("config"), SystemPrefs::default()),
        keymap: temor_ui::edge::keymap_for(
            chordkit::Platform::Linux {
                desktop: chordkit::Desktop::Ours,
            },
            &chordkit::Overrides::default(),
        )
        .0,
        launcher: LauncherChoice::ThisProgram,
        probe: None,
        state_dir: None,
    };
    let asked = SettingsAsked::default();
    asked.ask(page);
    let config = HarnessConfig::new(Viewport {
        width: 780,
        height: 620,
        scale_percent: 100,
    })
    .with_context(launch)
    .with_context(asked);
    let mut h = Harness::new(settings_root, extra(config));
    h.map();
    h.advance(Duration::from_millis(200));
    (h, ambient)
}

/// Clicks the element `selector` names, as a person's click would.
fn click(h: &mut Harness, selector: &str) {
    let at: Point = h
        .centre(selector)
        .unwrap_or_else(|| panic!("nothing matches {selector}:\n{}", h.html()));
    h.send(Input::click(at));
    h.advance(Duration::from_millis(200));
}

/// Replaces what the text field `selector` names holds with `text` and presses Return.
fn type_into(h: &mut Harness, selector: &str, text: &str) {
    click(h, selector);
    for _ in 0..8 {
        h.send(Input::key(ShortcutKey::Backspace));
    }
    for c in text.chars() {
        h.send(Input::key(ShortcutKey::Char(c)));
    }
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(300));
}

/// Records `held` + `key` in the shortcut recorder `selector` names.
fn record(h: &mut Harness, selector: &str, held: &[ShortcutKey], key: char) {
    click(h, selector);
    h.send(Input::chord(held, ShortcutKey::Char(key)));
    h.advance(Duration::from_millis(300));
}

fn settings_text(ambient: &Ambient) -> TerminalDoc {
    ambient.settings_store().load::<TerminalDoc>().value
}

fn keys_text(ambient: &Ambient) -> String {
    std::fs::read_to_string(ambient.keys_path()).unwrap_or_default()
}

/// ⌘, asks for the window, and so does a page's row in the command bar, turned to its page.
#[test]
fn command_comma_and_the_bars_page_rows_ask_for_the_window() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let asked = Arc::new(Recorder::default());
    let mut h = harness_building(
        "settings-open",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        {
            let asked = asked.clone();
            move |config| config.with_context(SettingsWindows(asked))
        },
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(200));
    chord(&mut h, ',');
    assert_eq!(asked.asks(), [None], "⌘, asks for the window");

    chord(&mut h, 't');
    for c in "keyboard".chars() {
        h.send(Input::key(ShortcutKey::Char(c)));
    }
    h.advance(Duration::from_millis(300));
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(300));
    assert_eq!(
        asked.asks(),
        [None, Some(SettingsPane::Keyboard)],
        "the bar's Settings: Keyboard row asks for that page"
    );
}

/// A font size typed into the page is written to `settings.toml`; one out of range is refused in
/// words and the file is left alone.
#[test]
fn a_changed_font_size_writes_settings_toml_and_a_bad_one_is_refused() {
    let (mut h, ambient) = settings_window("settings-font", SettingsPane::Appearance, |c| c);
    let size = "input[aria-label=\"Font size\"]";
    type_into(&mut h, size, "3");
    assert!(
        h.html().contains("takes a number from 6 to 72"),
        "a refusal says what the setting takes:\n{}",
        h.html()
    );
    assert!(
        ambient
            .settings_store()
            .path::<TerminalDoc>()
            .is_some_and(|path| !path.exists()),
        "nothing was written"
    );
    type_into(&mut h, size, "20");
    assert_eq!(settings_text(&ambient).font.size.0.points(), 20.0);
}

/// Shell offer entries are added and removed from the lists the page shows.
#[test]
fn the_shell_offer_list_adds_and_removes_a_command() {
    let (mut h, ambient) = settings_window("settings-offer", SettingsPane::General, |c| c);
    type_into(
        &mut h,
        "input[aria-label=\"Also offer for\"]",
        "kubectl debug -it",
    );
    assert_eq!(settings_text(&ambient).offer.add, ["kubectl debug -it"]);
    click(&mut h, "button[aria-label=\"Remove\"]");
    assert!(settings_text(&ambient).offer.add.is_empty());
}

/// A recorded chord is written to `keys.conf`, one a second action holds is refused naming that
/// action, and Reset All takes the changes away.
#[test]
fn a_recorded_chord_writes_keys_conf_a_held_one_is_refused_and_reset_all_clears() {
    let (mut h, ambient) = settings_window("settings-keys", SettingsPane::Keyboard, |c| c);
    let split_right = "button[aria-label=\"Change the shortcut for Split right\"]";
    let split_down = "button[aria-label=\"Change the shortcut for Split down\"]";
    let all = [ShortcutKey::Ctrl, ShortcutKey::Shift, ShortcutKey::Alt];

    record(&mut h, split_right, &all, 'n');
    assert!(
        keys_text(&ambient).contains("temor.split-right"),
        "{}",
        keys_text(&ambient)
    );

    // Ctrl+Shift+N is New tab's.
    record(
        &mut h,
        split_down,
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        'n',
    );
    assert!(
        h.html().contains("is already used by New tab"),
        "the refusal names the action that holds the chord:\n{}",
        h.html()
    );
    assert!(!keys_text(&ambient).contains("temor.split-down"));

    click(&mut h, "button[aria-label=\"Reset every shortcut\"]");
    assert!(
        !keys_text(&ambient).contains("temor.split-right"),
        "{}",
        keys_text(&ambient)
    );
}

/// The Spaces page sends a folder typed into it to the main window, by the board.
#[test]
fn a_spaces_folder_typed_in_the_page_is_sent_to_the_main_window() {
    let board = SpaceBoard::default();
    board.publish(vec![SpaceRow {
        id: ds::components::app::spaces::SpaceId(0),
        name: "Home".to_owned(),
        project: Project::default(),
    }]);
    let (mut h, _) = settings_window("settings-spaces-page", SettingsPane::Spaces, {
        let board = board.clone();
        move |config| config.with_context(board)
    });
    assert!(
        h.html().contains("Home"),
        "the Space's name heads its section"
    );
    type_into(&mut h, "input[aria-label=\"Folder\"]", "/srv/work");
    assert_eq!(
        board.take_edits(),
        [SpaceEdit {
            space: ds::components::app::spaces::SpaceId(0),
            change: SpaceChange::Root(Some(ProjectRoot("/srv/work".to_owned()))),
        }]
    );
}

/// A change the Settings window sends reaches `spaces.json`: the main window applies it through
/// the kit's handle, as a Space menu's edit goes, and the file is written.
#[test]
fn a_space_folder_sent_to_the_main_window_reaches_spaces_json() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let state = scratch("settings-spaces-file").join("state");
    let board = SpaceBoard::default();
    let mut h = harness_building(
        "settings-spaces-file",
        &probe,
        Some(state.clone()),
        |_| {},
        WindowHosting::Absent,
        {
            let board = board.clone();
            move |config| config.with_context(board)
        },
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let space = board
        .rows()
        .first()
        .map(|row| row.id)
        .expect("the window published its Space");
    board.send(SpaceEdit {
        space,
        change: SpaceChange::Root(Some(ProjectRoot("/srv/work".to_owned()))),
    });
    let start = std::time::Instant::now();
    loop {
        h.advance(Duration::from_millis(100));
        if let Ok(SpacesStartup::Loaded(spaces)) = load_spaces(&state)
            && spaces
                .list()
                .iter()
                .any(|s| s.payload.root.as_ref().map(ProjectRoot::as_str) == Some("/srv/work"))
        {
            break;
        }
        assert!(
            start.elapsed() < super::support::SLOW_WAIT,
            "the folder never reached spaces.json"
        );
    }
}

/// Rule 2 of the coherence rules for every page: no class nothing styles.
#[test]
fn every_page_uses_only_styled_classes() {
    let css = format!("{}\n{SETTINGS_CSS}", ds::stylesheet());
    for (at, page) in SettingsPane::ALL.into_iter().enumerate() {
        let (h, _) = settings_window(&format!("settings-coherence-{at}"), page, |c| c);
        let html = h.html();
        let unstyled: Vec<_> = markup(&html, &css, &LintConfig::new(&ds::kits()))
            .into_iter()
            .filter(|offence| offence.rule == Rule::UnstyledClass)
            .collect();
        assert!(unstyled.is_empty(), "{page:?}: {unstyled:#?}");
    }
}
