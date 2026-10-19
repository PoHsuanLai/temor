//! The desktop's settings read through quire with temor's own folders, and `keys.conf` watched.

use std::ffi::OsString;
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};

use chordkit::{Action, Chord, Context, Desktop, Platform, StandardAction};

use super::{system_keys, watch_file};
use crate::edge::Ambient;

const KDE: Platform = Platform::Linux {
    desktop: Desktop::Kde,
};

/// An `Ambient` whose home is `home`, with nothing else from the process.
fn ambient_in(home: &std::path::Path) -> Ambient {
    let vars = [(OsString::from("HOME"), home.as_os_str().to_owned())];
    Ambient::from_vars(vars).expect("an ambient")
}

fn redo_chords(platform: Platform, ambient: &Ambient) -> Vec<String> {
    let redo = Action::Standard(StandardAction::Redo);
    let keymap = system_keys(platform, ambient)
        .source()
        .load(platform)
        .expect("load");
    keymap
        .chords_in(&redo, Context::Normal)
        .into_iter()
        .map(Chord::canonical)
        .collect()
}

#[test]
fn the_persons_kde_shortcuts_come_from_temors_config_home() {
    let home = tempfile::tempdir().expect("scratch home");
    let ambient = ambient_in(home.path());
    let canonical = |text: &str| text.parse::<Chord>().expect("a chord").canonical();
    assert_eq!(
        redo_chords(KDE, &ambient),
        [canonical("Ctrl+Shift+Z")],
        "no file"
    );
    let config = home.path().join(".config");
    std::fs::create_dir_all(&config).expect("config home");
    std::fs::write(config.join("kdeglobals"), "[Shortcuts]\nRedo=Ctrl+Y\n").expect("write");
    assert_eq!(redo_chords(KDE, &ambient), [canonical("Ctrl+Y")], "changed");
}

#[test]
fn our_desktop_and_the_others_take_their_conventions() {
    let home = tempfile::tempdir().expect("scratch home");
    let ambient = ambient_in(home.path());
    let ours = Platform::Linux {
        desktop: Desktop::Ours,
    };
    for platform in [ours, Platform::MacOs, Platform::Windows] {
        let redo = Action::Standard(StandardAction::Redo);
        let conventional =
            chordkit::Keymap::conventional(platform).chords_in(&redo, Context::Normal);
        assert_eq!(system_keys(platform, &ambient).platform(), platform);
        assert_eq!(
            redo_chords(platform, &ambient),
            conventional
                .into_iter()
                .map(Chord::canonical)
                .collect::<Vec<_>>(),
            "{platform:?}"
        );
    }
}

#[test]
fn a_changed_file_calls_back_and_a_dropped_watch_stops() {
    let dir = tempfile::tempdir().expect("scratch dir");
    let path = dir.path().join("keys.conf");
    std::fs::write(&path, "").expect("write");
    let (told, heard) = channel::<()>();
    let watch = watch_file(
        &path,
        Duration::from_millis(20),
        Box::new(move || {
            let _ = told.send(());
        }),
    )
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
