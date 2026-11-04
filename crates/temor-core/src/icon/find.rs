//! Where a picture for a tab is found on disk (PLAN P22), read from the files' own formats: a Mac
//! folder's custom icon in the resource fork of its `Icon\r` file, and a Linux app's icon named by
//! its `.desktop` file and looked for in the freedesktop icon theme. Pure: the client reads the
//! files and hands their bytes and text here.

use std::path::{Path, PathBuf};

/// The `icns` resource in a classic Mac resource fork: where Finder keeps a folder's custom icon
/// (the fork of the folder's `Icon\r` file). `None` for a fork without one, or one cut short.
pub fn icns_in_resource_fork(fork: &[u8]) -> Option<&[u8]> {
    let u16_at = |at: usize| Some(u16::from_be_bytes(fork.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_be_bytes(fork.get(at..at + 4)?.try_into().ok()?));
    let data = u32_at(0)? as usize;
    let map = u32_at(4)? as usize;
    let types = map + u16_at(map + 24)? as usize;
    let count = u16_at(types)? as usize + 1;
    (0..count).find_map(|index| {
        let entry = types + 2 + index * 8;
        if fork.get(entry..entry + 4)? != b"icns" {
            return None;
        }
        let refs = types + u16_at(entry + 6)? as usize;
        // The first `icns` the fork holds (Finder writes one, id -16455).
        let offset = u32_at(refs + 4)? as usize & 0x00FF_FFFF;
        let at = data + offset;
        let len = u32_at(at)? as usize;
        fork.get(at + 4..at + 4 + len)
    })
}

/// What a freedesktop `.desktop` file says of its app: the program its `Exec` line runs, the one
/// its `TryExec` names, and its `Icon`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopEntry {
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub icon: Option<String>,
}

impl DesktopEntry {
    /// Whether the app is the program `name` (by its file name), or the program at `real`, its
    /// real path.
    pub fn runs(&self, name: &str, real: Option<&Path>) -> bool {
        [&self.exec, &self.try_exec]
            .into_iter()
            .flatten()
            .any(|program| {
                let program = Path::new(program);
                program.file_name().is_some_and(|file| file == name)
                    || real.is_some_and(|real| program.is_absolute() && program == real)
            })
    }
}

/// The `[Desktop Entry]` group of a `.desktop` file (or of a KDE folder's `.directory`). The
/// translated keys (`Icon[de]`) are passed over; `None` for a file without the group.
pub fn desktop_entry(text: &str) -> Option<DesktopEntry> {
    let mut entry = None;
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
            if inside {
                entry.get_or_insert_with(DesktopEntry::default);
            }
            continue;
        }
        let (Some(entry), true) = (entry.as_mut(), inside) else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Exec" => entry.exec = exec_program(value),
            "TryExec" if !value.is_empty() => entry.try_exec = Some(value.to_owned()),
            "Icon" if !value.is_empty() => entry.icon = Some(value.to_owned()),
            _ => {}
        }
    }
    entry
}

/// The program an `Exec` line runs: its first word, past an `env` and the variables it sets
/// (`env BAMF_DESKTOP_FILE_HINT=… /snap/bin/code %F` runs `/snap/bin/code`).
pub fn exec_program(exec: &str) -> Option<String> {
    let mut words = exec_words(exec).into_iter();
    let mut first = words.next()?;
    if Path::new(&first)
        .file_name()
        .is_some_and(|file| file == "env")
    {
        first = loop {
            let word = words.next()?;
            match word.as_str() {
                // `env`'s options that take the next word (`-u NAME`, `-C DIR`).
                "-u" | "-C" | "--unset" | "--chdir" => {
                    words.next()?;
                }
                _ if word.starts_with('-') || word.contains('=') => {}
                _ => break word,
            }
        };
    }
    (!first.is_empty() && !first.starts_with('%')).then_some(first)
}

/// An `Exec` line's words, as the specification quotes them: a word in double quotes keeps its
/// spaces, and a backslash in it keeps the next character.
fn exec_words(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', _) => quoted = !quoted,
            ('\\', true) => word.extend(chars.next()),
            (c, false) if c.is_whitespace() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            (c, _) => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// The theme sizes an app icon is looked for at, best first for a row's 16 points drawn on a
/// screen of two pixels a point: 32, then the vector one, then the nearest larger and smaller.
const SIZES: &[&str] = &[
    "32x32", "scalable", "48x48", "64x64", "128x128", "256x256", "512x512", "24x24", "22x22",
    "16x16",
];

/// The files the freedesktop icon `name` may be in under the data folders `bases` (each the
/// folder with `icons/` in it, best first), best first: the `hicolor` theme every app installs
/// into, at the sizes above, then the old `pixmaps` folder. An icon written as a path is that
/// file alone.
pub fn theme_icon_paths(bases: &[PathBuf], name: &str) -> Vec<PathBuf> {
    if Path::new(name).is_absolute() {
        return vec![PathBuf::from(name)];
    }
    let mut paths = Vec::new();
    for base in bases {
        for size in SIZES {
            let dir = base.join("icons/hicolor").join(size).join("apps");
            let ext = match *size {
                "scalable" => "svg",
                _ => "png",
            };
            paths.push(dir.join(format!("{name}.{ext}")));
        }
    }
    for base in bases {
        for ext in ["png", "svg"] {
            paths.push(base.join("pixmaps").join(format!("{name}.{ext}")));
        }
    }
    paths
}

/// The path in a `file://` URL, its `%XX` escapes undone (what GNOME's `metadata::custom-icon`
/// holds); `None` for another kind of URL.
pub fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        let escaped = (byte == b'%')
            .then(|| std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok())
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(byte);
                at += 1;
            }
        }
    }
    let path = PathBuf::from(String::from_utf8(out).ok()?);
    path.is_absolute().then_some(path)
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
