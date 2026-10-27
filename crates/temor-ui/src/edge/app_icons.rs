//! The Mac app a program belongs to, and its icon (PLAN P22): the program is found on the `PATH`,
//! its real path followed out of the links, and the `.app` bundle around it gives its icon file.
//! `code`, `docker`, `zed` and anything else installed inside an app are found this way, with no
//! list. Each program is looked up once.
//! TODO(platform:linux): the freedesktop icon theme's icon for a `.desktop` file's app.
//! TODO(platform:macos): a folder's custom icon (Finder keeps it in the resource fork of the
//! folder's `Icon\r` file).

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use temor_client::model::icon::{bundle_of, png_in_icns};

/// The size the row draws an icon at, in points; the PNG is at least twice that.
const ROW_PX: u32 = 16;

/// A program's app, and the app's icon as a PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIcon {
    pub app: PathBuf,
    pub png: Arc<[u8]>,
}

/// The apps of the programs looked up so far, by the program's name and the `PATH` it was looked
/// for along.
#[derive(Debug, Default)]
pub struct AppIcons {
    seen: HashMap<(String, OsString), Option<AppIcon>>,
}

impl AppIcons {
    /// The app the program `name` belongs to, found along `path` (the shell's `PATH`), with its
    /// icon; none for a program in no app, or an app with no icon file.
    pub fn of(&mut self, name: &str, path: &OsStr) -> Option<AppIcon> {
        if !cfg!(target_os = "macos") {
            return None;
        }
        let key = (name.to_owned(), path.to_owned());
        if let Some(known) = self.seen.get(&key) {
            return known.clone();
        }
        let found = look_up(name, path);
        self.seen.insert(key, found.clone());
        found
    }
}

fn look_up(name: &str, path: &OsStr) -> Option<AppIcon> {
    let executable = on_path(name, path)?;
    let real = std::fs::canonicalize(executable).ok()?;
    let app = bundle_of(&real)?.to_owned();
    let png = app_png(&app, ROW_PX * 2)?;
    Some(AppIcon {
        app,
        png: png.into(),
    })
}

/// The first `name` along `path` that is a file; `name` itself when it is a path.
fn on_path(name: &str, path: &OsStr) -> Option<PathBuf> {
    if name.contains('/') {
        return Some(PathBuf::from(name));
    }
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The icon of the app bundle at `app`, as a PNG at least `px` pixels square where the app has
/// one that big.
pub fn app_png(app: &Path, px: u32) -> Option<Vec<u8>> {
    let icns = std::fs::read(icon_file(app)?).ok()?;
    png_in_icns(&icns, px).map(<[u8]>::to_vec)
}

/// The bundle's icon file: the one its `Info.plist` names (`CFBundleIconFile`, with or without
/// its `.icns`), else `AppIcon.icns`.
fn icon_file(app: &Path) -> Option<PathBuf> {
    let resources = app.join("Contents/Resources");
    let name =
        bundle_icon_name(&app.join("Contents/Info.plist")).unwrap_or_else(|| "AppIcon".to_owned());
    let file = match name.ends_with(".icns") {
        true => resources.join(name),
        false => resources.join(format!("{name}.icns")),
    };
    file.is_file().then_some(file)
}

/// `CFBundleIconFile` in the property list at `plist`, read by the system's `plutil`, which
/// reads the binary form as well as the XML one.
fn bundle_icon_name(plist: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(plist)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let name = String::from_utf8(out.stdout).ok()?;
    let name = name.trim();
    (out.status.success() && !name.is_empty()).then(|| name.to_owned())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn an_apps_icon_is_a_png_from_its_bundle() {
        let png = app_png(Path::new("/System/Applications/Calculator.app"), 32)
            .expect("Calculator's icon");
        assert_eq!(&png[1..4], b"PNG");
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        assert!(width >= 32, "{width}");
        assert!(app_png(Path::new("/no/such.app"), 32).is_none());
    }

    #[test]
    fn a_program_in_no_app_has_none_and_is_looked_up_once() {
        let path = OsStr::new("/bin:/usr/bin");
        let mut icons = AppIcons::default();
        assert_eq!(icons.of("ls", path), None);
        assert_eq!(icons.seen.len(), 1);
        assert_eq!(icons.of("no-such-program-here", path), None);
    }
}
