use super::*;

#[test]
fn uname_words_name_the_kinds_the_helper_is_built_for() {
    for (said, want) in [
        ("x86_64", HostArch::X86_64),
        ("amd64", HostArch::X86_64),
        ("aarch64", HostArch::Aarch64),
        ("arm64", HostArch::Aarch64),
        ("riscv64", HostArch::Other),
        ("", HostArch::Other),
    ] {
        assert_eq!(arch_of(said), want, "{said:?}");
    }
}

#[test]
fn the_build_is_the_plain_one_for_this_computers_kind_and_a_named_one_for_another() {
    use HostArch::{Aarch64, Other, X86_64};
    let exe = Some(Path::new("/opt/temor/bin/temor"));
    let said = Some(OsStr::new("/x/helper"));
    // (name, explicit, this computer, the host) -> the file looked for
    let cases = [
        (
            "same kind",
            None,
            X86_64,
            X86_64,
            Some("/opt/temor/bin/temor-remote"),
        ),
        (
            "same kind, arm",
            None,
            Aarch64,
            Aarch64,
            Some("/opt/temor/bin/temor-remote"),
        ),
        (
            "the other kind",
            None,
            X86_64,
            Aarch64,
            Some("/opt/temor/bin/temor-remote-aarch64"),
        ),
        (
            "the other kind, arm",
            None,
            Aarch64,
            X86_64,
            Some("/opt/temor/bin/temor-remote-x86_64"),
        ),
        ("no build for the host's kind", None, X86_64, Other, None),
        (
            "named: the same kind is the named file",
            said,
            X86_64,
            X86_64,
            Some("/x/helper"),
        ),
        (
            "named: another kind sits beside it",
            said,
            X86_64,
            Aarch64,
            Some("/x/temor-remote-aarch64"),
        ),
        (
            "a computer of an unlisted kind has only the named ones",
            None,
            Other,
            Aarch64,
            Some("/opt/temor/bin/temor-remote-aarch64"),
        ),
    ];
    for (name, explicit, own, host, want) in cases {
        assert_eq!(
            build_path(explicit, exe, own, host),
            want.map(PathBuf::from),
            "{name}"
        );
    }
    assert_eq!(
        build_path(None, None, X86_64, Aarch64),
        None,
        "no program, no folder"
    );
}

#[test]
fn builds_are_read_per_kind_and_a_missing_kind_has_none() {
    let dir = tempfile::tempdir().expect("a scratch folder");
    std::fs::write(dir.path().join("temor-remote"), b"own build").expect("written");
    std::fs::write(dir.path().join("temor-remote-aarch64"), b"arm build").expect("written");
    let exe = dir.path().join("temor");
    let builds = HelperBuilds::new(HostArch::X86_64, None, Some(&exe));
    let own = builds.for_host(HostArch::X86_64).expect("the plain build");
    let arm = builds.for_host(HostArch::Aarch64).expect("the named build");
    assert_eq!(own.bytes.as_slice(), b"own build");
    assert_eq!(arm.bytes.as_slice(), b"arm build");
    assert!(builds.for_host(HostArch::Other).is_none());
    assert_eq!(builds.names(), vec![own.name(), arm.name()]);
    assert!(builds.any());
    // A temor with only the plain build has none for the other kind.
    std::fs::remove_file(dir.path().join("temor-remote-aarch64")).expect("removed");
    let alone = HelperBuilds::new(HostArch::X86_64, None, Some(&exe));
    assert!(alone.for_host(HostArch::Aarch64).is_none());
    assert_eq!(alone.names(), vec![own.name()]);
    assert!(!HelperBuilds::new(HostArch::X86_64, None, None).any());
}

#[test]
fn the_words_for_a_host_without_a_build_name_it_and_use_no_jargon() {
    let said = no_build_words("pi");
    assert!(said.contains("pi"));
    assert!(!said.to_lowercase().contains("aarch64"));
}
