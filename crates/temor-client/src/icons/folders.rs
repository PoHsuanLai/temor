//! A folder's own icon (PLAN P22), the one the person set on it: on a Mac the custom icon Finder
//! keeps in the resource fork of the folder's `Icon\r` file; on Linux the one a KDE `.directory`
//! file names, else the one GNOME's Files keeps in the folder's metadata (`gio info`). A folder
//! with none set has none: the row shows its letter.

use std::path::Path;
use std::process::{Command, Stdio};

use temor_core::icon::find::{desktop_entry, file_url_path, icns_in_resource_fork};
use temor_core::icon::png_in_icns;

use super::Image;
use super::apps::{ROW_PX, data_dirs, image_file, theme_image};

/// The icon set on the folder `dir`, if one is.
pub(super) fn folder_icon(dir: &Path) -> Option<Image> {
    match cfg!(target_os = "macos") {
        true => finder_icon(dir),
        false => directory_icon(dir).or_else(|| gio_icon(dir)),
    }
}

/// The custom icon Finder set on `dir`.
fn finder_icon(dir: &Path) -> Option<Image> {
    let fork = std::fs::read(dir.join("Icon\r").join("..namedfork/rsrc")).ok()?;
    let png = png_in_icns(icns_in_resource_fork(&fork)?, ROW_PX * 2)?;
    Some(Image::Png(png.to_vec().into()))
}

/// The icon KDE's `.directory` file in `dir` names.
fn directory_icon(dir: &Path) -> Option<Image> {
    let entry = desktop_entry(&std::fs::read_to_string(dir.join(".directory")).ok()?)?;
    theme_image(&data_dirs(), entry.icon.as_deref()?)
}

/// The icon GNOME's Files set on `dir`: a picture file (`metadata::custom-icon`) or a theme's
/// icon (`metadata::custom-icon-name`), as `gio info` reads them.
fn gio_icon(dir: &Path) -> Option<Image> {
    let out = Command::new("gio")
        .args([
            "info",
            "-a",
            "metadata::custom-icon,metadata::custom-icon-name",
        ])
        .arg(dir)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    gio_custom_icon(&text)
}

/// The icon in `gio info`'s words.
fn gio_custom_icon(text: &str) -> Option<Image> {
    text.lines().find_map(|line| {
        let (key, value) = line.trim().split_once(": ")?;
        match key {
            "metadata::custom-icon" => image_file(&file_url_path(value)?),
            "metadata::custom-icon-name" => theme_image(&data_dirs(), value),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_no_icon_set_has_none() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(folder_icon(dir.path()), None);
    }

    #[test]
    fn gnomes_custom_icon_is_the_file_its_metadata_names() {
        let dir = tempfile::tempdir().expect("dir");
        let svg = dir.path().join("my icon.svg");
        std::fs::write(&svg, "<svg/>").expect("svg");
        let url = format!("file://{}", svg.display()).replace(' ', "%20");
        let text = format!("uri: file:///x\nattributes:\n  metadata::custom-icon: {url}\n");
        assert_eq!(gio_custom_icon(&text), Some(Image::Svg("<svg/>".into())));
        assert_eq!(gio_custom_icon("attributes:\n"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finders_custom_icon_is_read_from_the_icon_files_fork() {
        let dir = tempfile::tempdir().expect("dir");
        let png = crate::icons::apps::app_png(
            Path::new("/System/Applications/Calculator.app"),
            ROW_PX * 2,
        )
        .expect("a PNG to put in the icon");
        let mut icns = b"icns".to_vec();
        icns.extend_from_slice(&(16 + png.len() as u32).to_be_bytes());
        icns.extend_from_slice(b"ic11");
        icns.extend_from_slice(&(8 + png.len() as u32).to_be_bytes());
        icns.extend_from_slice(&png);
        let icon = dir.path().join("Icon\r");
        std::fs::write(&icon, "").expect("Icon file");
        std::fs::write(icon.join("..namedfork/rsrc"), fork_holding(&icns)).expect("fork");
        assert_eq!(folder_icon(dir.path()), Some(Image::Png(png.into())));
    }

    /// A resource fork with one `icns` resource, `icns`.
    #[cfg(target_os = "macos")]
    fn fork_holding(icns: &[u8]) -> Vec<u8> {
        let body: Vec<u8> = [(icns.len() as u32).to_be_bytes().as_slice(), icns].concat();
        let (data_at, map_at) = (256u32, 256 + body.len() as u32);
        let mut map = vec![0u8; 24];
        map.extend_from_slice(&28u16.to_be_bytes());
        map.extend_from_slice(&0u16.to_be_bytes());
        map.extend_from_slice(&0u16.to_be_bytes());
        map.extend_from_slice(b"icns");
        map.extend_from_slice(&0u16.to_be_bytes());
        map.extend_from_slice(&10u16.to_be_bytes());
        map.extend_from_slice(&(-16455i16).to_be_bytes());
        map.extend_from_slice(&0xFFFFu16.to_be_bytes());
        map.extend_from_slice(&[0; 8]);
        let mut fork = [data_at, map_at, body.len() as u32, map.len() as u32]
            .iter()
            .flat_map(|n| n.to_be_bytes())
            .collect::<Vec<u8>>();
        fork.resize(data_at as usize, 0);
        fork.extend_from_slice(&body);
        fork.extend_from_slice(&map);
        fork
    }
}
