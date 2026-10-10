//! Choosing the desktop's store, reading kdeglobals through it, and noticing a change.

use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};

use chordkit::{Action, Chord, Context, Desktop, KeymapSource, Platform, StandardAction};

use super::{Polled, SystemKeys};

const KDE: Platform = Platform::Linux {
    desktop: Desktop::Kde,
};
const GNOME: Platform = Platform::Linux {
    desktop: Desktop::Gnome,
};

#[test]
fn each_desktop_reads_its_own_store() {
    let config = std::path::Path::new("/cfg");
    let run = std::path::Path::new("/run/temor");
    let rows = [
        (
            "Plasma",
            KDE,
            SystemKeys::Kde {
                kdeglobals: "/cfg/kdeglobals".into(),
            },
        ),
        (
            "GNOME",
            GNOME,
            SystemKeys::Gnome {
                database: "/cfg/dconf/user".into(),
                dump: "/run/temor/keys/dconf.dump".into(),
            },
        ),
        (
            "our desktop",
            Platform::Linux {
                desktop: Desktop::Ours,
            },
            SystemKeys::Keycap {
                config_home: config.to_owned(),
            },
        ),
        (
            "another Linux desktop",
            Platform::Linux {
                desktop: Desktop::Other,
            },
            SystemKeys::Conventions,
        ),
        ("a Mac", Platform::MacOs, SystemKeys::Conventions),
        ("Windows", Platform::Windows, SystemKeys::Conventions),
    ];
    for (name, platform, want) in rows {
        assert_eq!(SystemKeys::choose(platform, config, run), want, "{name}");
    }
}

#[test]
fn the_persons_kde_shortcuts_replace_the_conventions() {
    let dir = tempfile::tempdir().expect("scratch dir");
    let redo = Action::Standard(StandardAction::Redo);
    let canonical = |text: &str| text.parse::<Chord>().expect("a chord").canonical();
    // (name, kdeglobals text or none for no file, Redo's chord)
    let rows = [
        ("changed", Some("[Shortcuts]\nRedo=Ctrl+Y\n"), "Ctrl+Y"),
        ("no file", None, "Ctrl+Shift+Z"),
    ];
    for (name, text, want) in rows {
        let path = dir.path().join(format!("kdeglobals-{name}"));
        if let Some(text) = text {
            std::fs::write(&path, text).expect("write kdeglobals");
        }
        let keys = SystemKeys::Kde { kdeglobals: path };
        let keymap = keys.source().load(KDE).expect("load");
        let got: Vec<String> = keymap
            .chords_in(&redo, Context::Normal)
            .into_iter()
            .map(Chord::canonical)
            .collect();
        assert_eq!(got, [canonical(want)], "{name}");
    }
}

#[test]
fn a_changed_file_calls_back_and_a_dropped_watch_stops() {
    let dir = tempfile::tempdir().expect("scratch dir");
    let path = dir.path().join("kdeglobals");
    std::fs::write(&path, "[Shortcuts]\n").expect("write");
    let source = Polled {
        inner: chordkit::ConventionSource,
        watched: path.clone(),
        every: Duration::from_millis(20),
    };
    let (told, heard) = channel::<()>();
    let watch = source
        .watch(Box::new(move || {
            let _ = told.send(());
        }))
        .expect("watch");
    // A later modified time is a change, whatever the clock's resolution.
    let file = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open");
    file.set_modified(SystemTime::now() + Duration::from_secs(60))
        .expect("set the time");
    assert!(
        heard.recv_timeout(Duration::from_secs(10)).is_ok(),
        "no call after the change"
    );
    drop(watch);
    // The thread ends and drops the callback with it, which closes the channel.
    assert!(
        heard.recv_timeout(Duration::from_secs(10)).is_err(),
        "the watch went on after it was dropped"
    );
}
