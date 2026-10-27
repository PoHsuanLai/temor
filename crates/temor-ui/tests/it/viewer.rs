//! A viewer pane in the window (PLAN §10): ⌘-click on a file the shell listed opens a viewer
//! beside the terminal, ⌘W closes it, and Esc gives the keyboard back to the terminal. Event
//! dispatch only, as the rest of the window tests; the viewer reads real files in a scratch
//! folder.

use std::time::{Duration, Instant};

use dioxus::html::Modifiers;
use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input, PointerAction, PointerInput};
use temor_ui::view::probe::Probe;

use super::drive::{chord, focus_pane, type_line, wait_until};
use super::support::{scratch_path, tools_present};
use super::window::harness;

fn viewers(h: &Harness) -> usize {
    h.html().matches("aria-label=\"File viewer\"").count()
}

fn terminals(h: &Harness) -> usize {
    h.html().matches("aria-label=\"Terminal\"").count()
}

/// A ⌘-click: on our desktop that is Ctrl+Shift held, as the window's keymap reads it.
fn command_click(h: &mut Harness, at: Point) {
    let held = Modifiers::CONTROL | Modifiers::SHIFT;
    let click = PointerInput::new(at, PointerAction::Click(PointerButton::Primary)).with_mods(held);
    h.send(Input::Pointer(click));
    h.advance(Duration::from_millis(60));
}

/// A window whose shell has listed `a.txt` and `b.txt`, two files in its folder.
pub fn listed(name: &str, probe: &Probe) -> Harness {
    list_in(harness(name, probe), name, probe)
}

/// `h`, its shell having listed `a.txt` and `b.txt`, two files in its folder.
fn list_in(mut h: Harness, name: &str, probe: &Probe) -> Harness {
    let home = scratch_path(name).join("home");
    for file in ["a.txt", "b.txt"] {
        std::fs::write(home.join(file), format!("{file} says hello\n")).expect("scratch file");
    }
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "echo a.txt b.txt");
    wait_until(&mut h, probe, "the listing", |s| {
        s.lines().any(|l| l.trim_end() == "a.txt b.txt")
    });
    h
}

/// ⌘-clicks down the third character of each row until a viewer opens. The pane starts 60px
/// left of the focusing click at x=300 (the tab sidebar is to its left), and its gutter selects
/// a block, so the click goes a few cells in. The listing's row is not
/// known to the test (the font decides the rows' height), so every row of the pane is tried; a
/// click on anything that is not a file does nothing.
fn open_by_clicking(h: &mut Harness) {
    let mut y = 14.0;
    while viewers(h) == 0 && y < 320.0 {
        command_click(
            h,
            Point {
                x: Px(272.0),
                y: Px(y),
            },
        );
        y += 4.0;
    }
    let start = Instant::now();
    while viewers(h) == 0 {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no viewer opened"
        );
    }
    h.advance(Duration::from_millis(300));
}

#[test]
fn command_click_on_a_listed_file_opens_a_viewer_beside_the_terminal() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-open", &probe);
    assert_eq!((terminals(&h), viewers(&h)), (1, 0));
    open_by_clicking(&mut h);
    assert_eq!((terminals(&h), viewers(&h)), (1, 1), "{}", h.html());
}

#[test]
fn command_w_closes_the_viewer_and_leaves_the_terminal() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-close", &probe);
    open_by_clicking(&mut h);
    chord(&mut h, 'w');
    assert_eq!((terminals(&h), viewers(&h)), (1, 0), "{}", h.html());
}

#[test]
fn escape_gives_the_keyboard_back_to_the_terminal() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-escape", &probe);
    open_by_clicking(&mut h);
    // The viewer has the keyboard: what is typed does not reach the shell.
    type_line(&mut h, "echo lost");
    h.advance(Duration::from_millis(300));
    assert!(!probe.screen().contains("echo lost"), "{}", probe.screen());
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    // Now it is the shell's, and the viewer is still there.
    type_line(&mut h, "echo found");
    wait_until(&mut h, &probe, "the shell to hear it", |s| {
        s.lines().any(|l| l.trim_end() == "found")
    });
    assert_eq!(viewers(&h), 1);
}

#[test]
fn the_page_bar_names_the_file_and_hides_the_console_and_closes_the_page() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-stage", &probe);
    open_by_clicking(&mut h);
    let html = h.html();
    assert!(html.contains("temor-page-name"), "{html}");
    assert!(
        html.contains(">a.txt<") || html.contains(">b.txt<"),
        "the bar names the page: {html}"
    );
    super::drive::press(&mut h, ".temor-page-tool [aria-label=\"Console\"]");
    assert_eq!((terminals(&h), viewers(&h)), (0, 1), "the page alone");
    super::drive::press(&mut h, ".temor-page-tool [aria-label=\"Console\"]");
    assert_eq!((terminals(&h), viewers(&h)), (1, 1), "the console is back");
    super::drive::press(&mut h, ".temor-page-bar [aria-label=\"Close Page\"]");
    assert_eq!((terminals(&h), viewers(&h)), (1, 0), "{}", h.html());
    // Back opens the closed page again.
    chord(&mut h, '[');
    assert_eq!((terminals(&h), viewers(&h)), (1, 1), "{}", h.html());
}

/// ⌘-click on a folder the shell printed opens the folder's page: the tab open in it, then its
/// folders and files. A folder opens its own page, Back returns, and a file opens in the viewer.
#[test]
fn command_click_on_a_folder_opens_its_page_and_walks_into_it() {
    if !super::support::window_tools_present() {
        return;
    }
    use super::drive::{press, wait_html};
    let name = "viewer-folder";
    let probe = Probe::new();
    let mut h = harness(name, &probe);
    let proj = scratch_path(name).join("home").join("proj");
    std::fs::create_dir_all(proj.join("src")).expect("scratch folder");
    std::fs::write(proj.join("notes.txt"), "notes\n").expect("scratch file");
    std::fs::write(proj.join("src").join("deep.txt"), "deep\n").expect("scratch file");
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "cd proj && echo \"$PWD\"");
    wait_until(&mut h, &probe, "the folder", |s| {
        s.lines().any(|l| l.trim_end().ends_with("/proj"))
    });
    let mut y = 14.0;
    while !h.html().contains("temor-folder") && y < 320.0 {
        command_click(
            &mut h,
            Point {
                x: Px(272.0),
                y: Px(y),
            },
        );
        y += 4.0;
    }
    wait_html(&mut h, "the folder's page", |html| {
        html.contains("Tabs Here") && html.contains(">src<") && html.contains(">notes.txt<")
    });
    assert!(
        h.html().contains("class=\"temor-page-name\">proj<"),
        "the bar names the folder: {}",
        h.html()
    );
    let files = ".temor-folder > .temor-history-day:last-child .temor-history-list";
    press(&mut h, &format!("{files} > :first-child"));
    wait_html(&mut h, "the subfolder's page", |html| {
        html.contains(">deep.txt<") && !html.contains("Tabs Here")
    });
    chord(&mut h, '[');
    wait_html(&mut h, "the folder again", |html| {
        html.contains(">notes.txt<")
    });
    press(&mut h, &format!("{files} > :last-child"));
    wait_html(&mut h, "the file in the viewer", |html| {
        html.contains("aria-label=\"File viewer\"") && html.contains(">notes.txt<")
    });
}

/// The address under the sidebar's head names the shell's folder; a folder in it opens that
/// folder's page, the address follows the page, and Back in the head returns.
#[test]
fn a_folder_in_the_address_opens_its_page_and_back_returns() {
    if !super::support::window_tools_present() {
        return;
    }
    use super::drive::{press, wait_html};
    let name = "viewer-address";
    let probe = Probe::new();
    let mut h = harness(name, &probe);
    let proj = scratch_path(name).join("home").join("proj");
    std::fs::create_dir_all(proj.join("src")).expect("scratch folder");
    std::fs::write(proj.join("notes.txt"), "notes\n").expect("scratch file");
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "cd proj/src && echo \"$PWD\"");
    let crumbs = ".temor-address .ds-path-segment";
    wait_html(&mut h, "the shell's folder", |html| {
        html.contains("class=\"ds-path-name ds-truncate\">src<")
    });
    let names = |h: &Harness| -> Vec<String> {
        (1..=3)
            .filter_map(|at| h.text_of(&format!("{crumbs}:nth-of-type({at})")))
            .map(|name| name.trim().to_owned())
            .collect()
    };
    assert_eq!(names(&h), ["~", "proj", "src"]);
    press(&mut h, &format!("{crumbs}:nth-of-type(2)"));
    wait_html(&mut h, "the folder's page", |html| {
        html.contains(">notes.txt<")
    });
    assert_eq!(names(&h), ["~", "proj"], "the address follows the page");
    press(&mut h, &format!("{crumbs}:nth-of-type(1)"));
    wait_html(&mut h, "the home folder's page", |html| {
        html.contains("class=\"temor-page-name\">home<")
    });
    press(&mut h, ".temor-walk > :first-child");
    wait_html(&mut h, "the folder again", |html| {
        html.contains(">notes.txt<")
    });
}

/// What a test's Pop Out was asked to open.
#[derive(Default)]
struct PoppedOut(std::sync::Mutex<Vec<std::path::PathBuf>>);

impl temor_ui::view::pop_out::OpenPopOut for PoppedOut {
    fn open(&self, file: &std::path::Path) {
        self.0.lock().expect("the list").push(file.to_owned());
    }
}

/// The page bar's Pop Out takes the file off the row's stage for a window of its own; Back brings
/// it back to the stage.
#[test]
fn pop_out_moves_the_page_to_a_window_of_its_own() {
    if !super::support::window_tools_present() {
        return;
    }
    use super::drive::press;
    let name = "viewer-pop-out";
    let probe = Probe::new();
    let popped = std::sync::Arc::new(PoppedOut::default());
    let h = super::window::harness_building(
        name,
        &probe,
        None,
        |_| {},
        ds_harness::WindowHosting::Absent,
        {
            let popped = popped.clone();
            move |config| config.with_context(temor_ui::view::pop_out::PopOutWindows(popped))
        },
    );
    let mut h = list_in(h, name, &probe);
    open_by_clicking(&mut h);
    press(&mut h, ".temor-page-bar [aria-label=\"Pop Out\"]");
    h.advance(Duration::from_millis(200));
    let asked = popped.0.lock().expect("the list").clone();
    assert_eq!(asked.len(), 1, "one window asked for");
    assert!(
        asked[0].ends_with("a.txt") || asked[0].ends_with("b.txt"),
        "{asked:?}"
    );
    assert_eq!(viewers(&h), 0, "the page left the stage");
    chord(&mut h, '[');
    h.advance(Duration::from_millis(300));
    assert_eq!(viewers(&h), 1, "Back brings it back");
}
