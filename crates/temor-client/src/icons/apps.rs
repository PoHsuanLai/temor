//! The app a program belongs to, and its icon (PLAN P22). On a Mac the program is found on the
//! `PATH`, its real path followed out of the links, and the `.app` bundle around it gives its icon
//! file: `code`, `docker`, `zed` and anything else installed inside an app are found this way,
//! with no list. On Linux the app is the `.desktop` file whose `Exec` or `TryExec` runs the
//! program, and its icon the one the freedesktop icon theme has under the name it gives.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use temor_core::icon::find::{desktop_entry, theme_icon_paths};
use temor_core::icon::{bundle_of, png_in_icns};

use super::Image;

/// The size the row draws an icon at, in points; the picture is at least twice that.
pub(super) const ROW_PX: u32 = 16;

/// A program's app, and the app's icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIcon {
    /// The app: its `.app` bundle on a Mac, its `.desktop` file on Linux.
    pub app: PathBuf,
    pub image: Image,
}

/// The app the program `name` belongs to, found along `path` (the shell's `PATH`), with its
/// icon; none for a program in no app, or an app with no icon.
pub(super) fn app_of(name: &str, path: &OsStr) -> Option<AppIcon> {
    let executable = on_path(name, path)?;
    let real = std::fs::canonicalize(executable).ok()?;
    match cfg!(target_os = "macos") {
        true => bundle_app(&real),
        false => desktop_app(&data_dirs(), name, &real),
    }
}

fn bundle_app(real: &Path) -> Option<AppIcon> {
    let app = bundle_of(real)?.to_owned();
    let png = app_png(&app, ROW_PX * 2)?;
    Some(AppIcon {
        app,
        image: Image::Png(png.into()),
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

/// The `.desktop` file under the data folders `bases` whose app is the program `name` at `real`,
/// and the icon it names.
fn desktop_app(bases: &[PathBuf], name: &str, real: &Path) -> Option<AppIcon> {
    bases.iter().find_map(|base| {
        desktop_files(&base.join("applications")).find_map(|file| {
            let entry = desktop_entry(&std::fs::read_to_string(&file).ok()?)?;
            if !entry.runs(name, Some(real)) {
                return None;
            }
            let image = theme_image(bases, entry.icon.as_deref()?)?;
            Some(AppIcon { app: file, image })
        })
    })
}

/// The `.desktop` files in `dir` and the folders in it (an app may install into a vendor's
/// folder of its own).
fn desktop_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_owned()];
    while let Some(dir) = dirs.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in read.flatten().map(|entry| entry.path()) {
            match path.is_dir() {
                true => dirs.push(path),
                false if path.extension().is_some_and(|ext| ext == "desktop") => files.push(path),
                false => {}
            }
        }
    }
    files.sort();
    files.into_iter()
}

/// The freedesktop data folders, best first: the person's (`XDG_DATA_HOME`, else
/// `~/.local/share`), then the system's (`XDG_DATA_DIRS`, else `/usr/local/share:/usr/share`).
pub(super) fn data_dirs() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    let home = var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| var("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let system = var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(std::env::split_paths(&system))
        .filter(|dir| dir.is_absolute())
        .collect()
}

/// The freedesktop icon `icon` (a theme name or a path) as the first file of it that reads.
pub(super) fn theme_image(bases: &[PathBuf], icon: &str) -> Option<Image> {
    theme_icon_paths(bases, icon)
        .into_iter()
        .find_map(|path| image_file(&path))
}

/// The picture in the PNG or SVG file at `path`.
pub(super) fn image_file(path: &Path) -> Option<Image> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => {
            let bytes = std::fs::read(path).ok()?;
            bytes
                .starts_with(b"\x89PNG")
                .then(|| Image::Png(bytes.into()))
        }
        "svg" => Some(Image::Svg(std::fs::read_to_string(path).ok()?.into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
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
    fn a_program_in_no_app_has_none() {
        let path = OsStr::new("/bin:/usr/bin");
        assert_eq!(app_of("no-such-program-here", path), None);
    }

    #[test]
    fn a_linux_apps_icon_is_the_one_its_desktop_file_names_in_the_theme() {
        let dir = tempfile::tempdir().expect("dir");
        let share = dir.path().join("share");
        let apps = share.join("applications/vendor");
        let icons = share.join("icons/hicolor/scalable/apps");
        std::fs::create_dir_all(&apps).expect("apps");
        std::fs::create_dir_all(&icons).expect("icons");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        std::fs::write(bin.join("tool"), "").expect("program");
        std::fs::write(
            apps.join("tool.desktop"),
            "[Desktop Entry]\nExec=tool %U\nIcon=tool-icon\n",
        )
        .expect("desktop file");
        std::fs::write(icons.join("tool-icon.svg"), "<svg/>").expect("icon");
        let real = std::fs::canonicalize(bin.join("tool")).expect("real");
        let bases = [share.clone()];
        let found = desktop_app(&bases, "tool", &real).expect("the app");
        assert_eq!(found.app, apps.join("tool.desktop"));
        assert_eq!(found.image, Image::Svg("<svg/>".into()));
        assert_eq!(desktop_app(&bases, "other", Path::new("/bin/other")), None);
        assert_eq!(theme_image(&bases, "none-such"), None);
    }
}
