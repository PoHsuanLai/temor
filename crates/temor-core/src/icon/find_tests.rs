use super::*;

/// A resource fork holding `kind` resources, the first with `data`, as Finder writes one.
fn fork_with(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let data_at = 256u32;
    let body: Vec<u8> = [(data.len() as u32).to_be_bytes().as_slice(), data].concat();
    let map_at = data_at + body.len() as u32;
    // The map: a copy of the header (16), the next map (4), the file ref (2), attributes (2),
    // then the type list's and the name list's offsets, then the type list and the references.
    let mut map = vec![0u8; 24];
    map.extend_from_slice(&28u16.to_be_bytes());
    map.extend_from_slice(&0u16.to_be_bytes());
    // The type list: one type, one resource, its references 10 bytes after the list starts.
    map.extend_from_slice(&0u16.to_be_bytes());
    map.extend_from_slice(kind);
    map.extend_from_slice(&0u16.to_be_bytes());
    map.extend_from_slice(&10u16.to_be_bytes());
    // The reference: id -16455, no name, attributes 0 and the data at offset 0.
    map.extend_from_slice(&(-16455i16).to_be_bytes());
    map.extend_from_slice(&0xFFFFu16.to_be_bytes());
    map.extend_from_slice(&0u32.to_be_bytes());
    map.extend_from_slice(&0u32.to_be_bytes());
    let mut fork = Vec::new();
    fork.extend_from_slice(&data_at.to_be_bytes());
    fork.extend_from_slice(&map_at.to_be_bytes());
    fork.extend_from_slice(&(body.len() as u32).to_be_bytes());
    fork.extend_from_slice(&(map.len() as u32).to_be_bytes());
    fork.resize(data_at as usize, 0);
    fork.extend_from_slice(&body);
    fork.extend_from_slice(&map);
    fork
}

#[test]
fn a_folders_custom_icon_is_the_icns_resource_in_its_icon_files_fork() {
    let icns = b"icns\x00\x00\x00\x08";
    let fork = fork_with(b"icns", icns);
    assert_eq!(icns_in_resource_fork(&fork), Some(icns.as_slice()));
    assert_eq!(icns_in_resource_fork(&fork_with(b"ICN#", icns)), None);
    assert_eq!(icns_in_resource_fork(&fork[..40]), None);
    assert_eq!(icns_in_resource_fork(b""), None);
}

#[test]
fn a_desktop_file_names_its_program_and_icon_in_its_own_group() {
    let text = "\
[Desktop Entry]
Name=Visual Studio Code
Exec=env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/code.desktop /snap/bin/code --force-user-env %F
Icon[de]=code-de
Icon=vscode
TryExec=code

[Desktop Action new-empty-window]
Exec=/snap/bin/code --new-window
Icon=other
";
    let entry = desktop_entry(text).expect("an entry");
    assert_eq!(entry.exec.as_deref(), Some("/snap/bin/code"));
    assert_eq!(entry.try_exec.as_deref(), Some("code"));
    assert_eq!(entry.icon.as_deref(), Some("vscode"));
    assert!(entry.runs("code", None));
    assert!(!entry.runs("codium", None));
    assert!(entry.runs("vs", Some(Path::new("/snap/bin/code"))));
    assert_eq!(desktop_entry("[Other]\nIcon=x\n"), None);
}

#[test]
fn an_exec_line_runs_its_first_word_past_env() {
    let cases = [
        ("zed %U", Some("zed")),
        ("\"/opt/My App/app\" --x", Some("/opt/My App/app")),
        ("\"/opt/a \\\"b\\\"\" %u", Some("/opt/a \"b\"")),
        ("env A=1 B=2 ghostty", Some("ghostty")),
        ("/usr/bin/env -u X kitty", Some("kitty")),
        ("env A=1", None),
        ("", None),
    ];
    for (exec, want) in cases {
        assert_eq!(exec_program(exec).as_deref(), want, "{exec}");
    }
}

#[test]
fn a_theme_icon_is_looked_for_at_the_rows_size_first_then_in_pixmaps() {
    let bases = [
        PathBuf::from("/home/po/.local/share"),
        PathBuf::from("/usr/share"),
    ];
    let paths = theme_icon_paths(&bases, "zed");
    assert_eq!(
        paths[0],
        Path::new("/home/po/.local/share/icons/hicolor/32x32/apps/zed.png")
    );
    assert_eq!(
        paths[1],
        Path::new("/home/po/.local/share/icons/hicolor/scalable/apps/zed.svg")
    );
    assert!(paths.contains(&PathBuf::from(
        "/usr/share/icons/hicolor/48x48/apps/zed.png"
    )));
    assert_eq!(
        paths.last().map(PathBuf::as_path),
        Some(Path::new("/usr/share/pixmaps/zed.svg"))
    );
    assert_eq!(
        theme_icon_paths(&bases, "/opt/zed/zed.png"),
        [PathBuf::from("/opt/zed/zed.png")]
    );
}

#[test]
fn a_file_url_is_its_path_with_escapes_undone() {
    assert_eq!(
        file_url_path("file:///home/po/My%20Icons/a.png"),
        Some(PathBuf::from("/home/po/My Icons/a.png"))
    );
    assert_eq!(file_url_path("file:///a%zz"), Some(PathBuf::from("/a%zz")));
    assert_eq!(file_url_path("https://x/a.png"), None);
}
