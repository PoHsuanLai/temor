//! The Settings window (ds-harness, event dispatch only, never OS input): ⌘, and the command bar
//! ask for it, a changed font size writes `settings.toml`, a recorded chord writes `keys.conf`
//! and a held one is refused with the holder's name, Reset All empties the changes, and a Space's
//! folder reaches `spaces.json` through the main window. The window opens on a page the way the
//! main window asks: the harness has no event loop to open a window on, so the opener is a
//! recorder and the Settings root is drawn directly.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds::prelude::SystemPrefs;
use ds_harness::{Harness, HarnessConfig, Input, Viewport, WindowHosting};
use ds_lint::{LintConfig, Rule, markup};
use temor_client::model::space::{AccountRef, Credential, Profile, Project, Route};
use temor_core::vocab::text::{EnvKey, EnvValue, ProjectRoot, RoutePattern};
use temor_pane::keys::SettingsPane;
use temor_pane::settings::TerminalSettings;
use temor_ui::edge::{Ambient, CredentialFault, CredentialSource, Credentials};
use temor_ui::settings::TerminalDoc;
use temor_ui::settings_window::{
    OpenSettings, SETTINGS_CSS, SettingsAsked, SettingsWindows, SpaceBoard, SpaceChange, SpaceEdit,
    SpaceRow, settings_root,
};
use temor_ui::view::probe::Probe;
use temor_ui::{AppearanceSource, Launch, LauncherChoice};

use super::drive::{chord, focus_pane, press, rects, wait_until};
use super::support::{scratch, tools_present};
use super::window::harness_building;

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
        first_words: Default::default(),
        first_run: Default::default(),
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

/// Replaces what the text field `selector` names holds with `text` and presses Return.
fn type_into(h: &mut Harness, selector: &str, text: &str) {
    press(h, selector);
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
    press(h, selector);
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
    focus_pane(&mut h);
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
#[cfg_attr(
    target_os = "macos",
    ignore = "the harness's Backspace deletes nothing in a text field on macOS (quire builds text editing per OS)"
)]
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
    assert_eq!(settings_text(&ambient).font.size.points(), 20.0);
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
    press(&mut h, "button[aria-label=\"Remove kubectl debug -it\"]");
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

    // Split right now holds the chord it was just given.
    record(&mut h, split_down, &all, 'n');
    assert!(
        h.html().contains("is already used by Split right"),
        "the refusal names the action that holds the chord:\n{}",
        h.html()
    );
    assert!(!keys_text(&ambient).contains("temor.split-down"));

    press(&mut h, "button[aria-label=\"Reset every shortcut\"]");
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

/// A change the Settings window sends (a folder, a variable) is kept: the main window applies it
/// through the kit's handle, as a Space menu's edit goes, and the Spaces are kept in the store.
#[test]
fn a_space_folder_sent_to_the_main_window_is_kept() {
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
    board.send(SpaceEdit {
        space,
        change: SpaceChange::Env(vec![(key("RUST_LOG"), EnvValue("debug".to_owned()))]),
    });
    let start = std::time::Instant::now();
    loop {
        h.advance(Duration::from_millis(100));
        let kept = temor_store::SqliteStore::open(state.join("temor.db"))
            .ok()
            .and_then(|store| store.kept("spaces").ok().flatten())
            .and_then(|text| {
                serde_json::from_str::<temor_client::model::space::TemorSpaces>(&text).ok()
            });
        if let Some(spaces) = kept
            && spaces.list().iter().any(|s| {
                s.payload.root.as_ref().map(ProjectRoot::as_str) == Some("/srv/work")
                    && s.payload.profile.env == [(key("RUST_LOG"), EnvValue("debug".to_owned()))]
            })
        {
            break;
        }
        assert!(
            start.elapsed() < super::support::SLOW_WAIT,
            "the folder and the variable were never kept"
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

fn key(name: &str) -> EnvKey {
    EnvKey::new(name).expect("a variable name")
}

/// The Spaces page over one Space `Home` holding `profile`, and the board it publishes to.
fn spaces_page(
    name: &str,
    profile: Profile,
    extra: impl FnOnce(HarnessConfig) -> HarnessConfig,
) -> (Harness, SpaceBoard) {
    let board = SpaceBoard::default();
    board.publish(vec![home(profile)]);
    let (h, _) = settings_window(name, SettingsPane::Spaces, {
        let board = board.clone();
        move |config| extra(config.with_context(board))
    });
    (h, board)
}

fn home(profile: Profile) -> SpaceRow {
    SpaceRow {
        id: ds::components::app::spaces::SpaceId(0),
        name: "Home".to_owned(),
        project: Project {
            profile,
            ..Project::default()
        },
    }
}

/// The change of the last edit the page sent.
fn last_change(board: &SpaceBoard) -> SpaceChange {
    board
        .take_edits()
        .pop()
        .expect("the page sent an edit")
        .change
}

/// A variable is added, its value edited and the variable removed from the Spaces page.
#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "the harness's Backspace deletes nothing in a text field on macOS (quire builds text editing per OS)"
)]
fn a_variable_is_added_edited_and_removed_on_the_spaces_page() {
    let profile = Profile {
        env: vec![(key("A"), EnvValue("1".to_owned()))],
        ..Profile::default()
    };
    let (mut h, board) = spaces_page("settings-variables", profile, |c| c);

    type_into(&mut h, "input[aria-label=\"New variable name\"]", "B");
    // Return in the value field adds the variable.
    type_into(&mut h, "input[aria-label=\"New variable value\"]", "9");
    assert_eq!(
        last_change(&board),
        SpaceChange::Env(vec![
            (key("A"), EnvValue("1".to_owned())),
            (key("B"), EnvValue("9".to_owned())),
        ])
    );

    type_into(&mut h, "input[aria-label=\"Value of A\"]", "2");
    assert_eq!(
        last_change(&board),
        SpaceChange::Env(vec![(key("A"), EnvValue("2".to_owned()))])
    );
    // The main window writes the edit and sends the Space back, as it does after every edit.
    board.publish(vec![home(Profile {
        env: vec![(key("A"), EnvValue("2".to_owned()))],
        ..Profile::default()
    })]);
    h.advance(Duration::from_millis(200));

    press(&mut h, "button[aria-label=\"Remove A\"]");
    assert_eq!(last_change(&board), SpaceChange::Env(Vec::new()));
}

/// Each route's Remove button is named for its route, so one of several is the one that goes.
#[test]
fn each_route_has_its_own_named_remove_button() {
    let route = |pattern: &str| Route {
        pattern: RoutePattern(pattern.to_owned()),
    };
    let project = Project {
        routes: vec![route("ssh a"), route("ssh b")],
        ..Project::default()
    };
    let board = SpaceBoard::default();
    board.publish(vec![SpaceRow {
        id: ds::components::app::spaces::SpaceId(0),
        name: "Home".to_owned(),
        project,
    }]);
    let (mut h, _) = settings_window("settings-routes", SettingsPane::Spaces, {
        let board = board.clone();
        move |config| config.with_context(board)
    });
    press(&mut h, "button[aria-label=\"Remove route ssh a\"]");
    assert_eq!(
        last_change(&board),
        SpaceChange::Routes(vec![route("ssh b")])
    );
}

/// A secret store in memory.
#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

impl Memory {
    fn get(&self, account: &str) -> Option<String> {
        self.0.lock().ok()?.get(account).cloned()
    }
}

impl CredentialSource for Memory {
    fn secret(&self, account: &AccountRef) -> Result<Option<String>, CredentialFault> {
        Ok(self.get(account.as_str()))
    }
    fn set_secret(&self, account: &AccountRef, secret: &str) -> Result<(), CredentialFault> {
        if let Ok(mut held) = self.0.lock() {
            held.insert(account.as_str().to_owned(), secret.to_owned());
        }
        Ok(())
    }
    fn remove_secret(&self, account: &AccountRef) -> Result<(), CredentialFault> {
        if let Ok(mut held) = self.0.lock() {
            held.remove(account.as_str());
        }
        Ok(())
    }
}

/// What `got` finds, once it finds something: the page is drawn again meanwhile, so what was
/// handed to a thread of its own comes back.
fn settle<T>(h: &mut Harness, what: &str, mut got: impl FnMut(&mut Harness) -> Option<T>) -> T {
    let start = std::time::Instant::now();
    loop {
        if let Some(found) = got(h) {
            return found;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "timed out waiting for {what}"
        );
        h.advance(Duration::from_millis(20));
    }
}

/// The edits the page has sent since the last look, once there are any.
fn edits_of(board: &SpaceBoard) -> Option<Vec<SpaceEdit>> {
    Some(board.take_edits()).filter(|edits| !edits.is_empty())
}

/// A credential is set, replaced and removed on the Spaces page; its secret goes to the store
/// alone and is in neither the page nor the change sent to the main window.
#[test]
fn a_credential_is_set_replaced_and_removed_and_its_secret_is_never_in_the_page() {
    let store = Arc::new(Memory::default());
    let (mut h, board) = spaces_page("settings-credentials", Profile::default(), {
        let source = Credentials(store.clone());
        move |config| config.with_context(source)
    });
    let account = "temor.space0.api-key";

    type_into(&mut h, "input[aria-label=\"New secret name\"]", "API_KEY");
    // Return in the secret field adds the credential.
    type_into(
        &mut h,
        "input[aria-label=\"New secret value\"]",
        "first-s3cret",
    );
    // The store is asked on a thread of its own.
    let edits = settle(&mut h, "the credential added", |_| edits_of(&board));
    assert_eq!(store.get(account).as_deref(), Some("first-s3cret"));
    let change = edits.last().expect("an edit").change.clone();
    let credential = Credential {
        env: key("API_KEY"),
        account: AccountRef::new(account).expect("an account"),
    };
    assert_eq!(change, SpaceChange::Credentials(vec![credential.clone()]));
    assert!(!format!("{change:?}").contains("first-s3cret"));
    assert!(
        !h.html().contains("first-s3cret"),
        "the page shows no secret"
    );

    // The main window publishes the Space with the credential; the page says it is set.
    board.publish(vec![home(Profile {
        credentials: vec![credential],
        ..Profile::default()
    })]);
    settle(&mut h, "the credential shown as set", |h: &mut Harness| {
        h.html().contains("Replace API_KEY").then_some(())
    });

    type_into(
        &mut h,
        "input[aria-label=\"New value for API_KEY\"]",
        "second-s3cret",
    );
    settle(&mut h, "the secret replaced", |_| {
        (store.get(account).as_deref() == Some("second-s3cret")).then_some(())
    });
    assert!(!h.html().contains("first-s3cret") && !h.html().contains("second-s3cret"));

    press(&mut h, "button[aria-label=\"Remove credential API_KEY\"]");
    let edits = settle(&mut h, "the credential removed", |_| edits_of(&board));
    assert_eq!(store.get(account), None);
    assert_eq!(
        edits.last().expect("an edit").change,
        SpaceChange::Credentials(Vec::new())
    );
}

/// The Font row is a pop-up of every font the computer has with the system's first. (A pop-up's
/// menu needs a renderer to open, so the harness checks what the row lists: the pop-up sizes
/// itself to every title.)
#[test]
fn the_font_row_lists_the_computers_fonts() {
    let (mut h, _ambient) = settings_window("settings-font-row", SettingsPane::Appearance, |c| c);
    assert!(
        !rects(&h, "[aria-label=\"Font\"]").is_empty(),
        "the font pop-up"
    );
    let html = h.html();
    assert!(
        html.contains("System default"),
        "the system's font is listed"
    );
    for name in temor_render::font_catalog().families() {
        let sizer = format!("<span class=\"ds-popup-sizer\" aria-hidden=\"true\">{name}</span>");
        assert!(html.contains(&sizer), "{name} is listed");
    }
    if let Some(path) = std::env::var_os("TEMOR_SHOT") {
        let picture = h
            .render_over(ds_harness::Backdrop::Scheme)
            .expect("a frame");
        let file = std::fs::File::create(path).expect("the shot file");
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(file),
            picture.width(),
            picture.height(),
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .expect("header")
            .write_image_data(picture.as_raw())
            .expect("pixels");
    }
}
