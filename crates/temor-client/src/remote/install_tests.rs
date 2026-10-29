use super::*;

fn name(digit: char) -> HelperHash {
    HelperHash(digit.to_string().repeat(64))
}

#[test]
fn the_probe_reads_the_system_the_folder_and_the_builds_there() {
    let wanted = name('a');
    let cases = [
        (
            "Linux\nx86_64\nwritable\n".to_owned(),
            (HostOs::Linux, HostArch::X86_64, CacheDir::Usable, None),
        ),
        (
            "Linux\narm64\nwritable\n".to_owned(),
            (HostOs::Linux, HostArch::Aarch64, CacheDir::Usable, None),
        ),
        (
            "Linux\nriscv64\nwritable\n".to_owned(),
            (HostOs::Linux, HostArch::Other, CacheDir::Usable, None),
        ),
        (
            "Darwin\narm64\nwritable\n".to_owned(),
            (HostOs::Other, HostArch::Aarch64, CacheDir::Usable, None),
        ),
        (
            "Linux\nx86_64\nreadonly\n".to_owned(),
            (HostOs::Linux, HostArch::X86_64, CacheDir::Unusable, None),
        ),
        (
            format!(
                "Linux\nx86_64\nwritable\n{}\nnot-a-hash\n{}\n",
                name('b').0,
                wanted.0
            ),
            (
                HostOs::Linux,
                HostArch::X86_64,
                CacheDir::Usable,
                Some(wanted.clone()),
            ),
        ),
        (
            format!("Linux\nx86_64\nwritable\n{}\n", name('b').0),
            (
                HostOs::Linux,
                HostArch::X86_64,
                CacheDir::Usable,
                Some(name('b')),
            ),
        ),
        (
            "".to_owned(),
            (HostOs::Other, HostArch::Other, CacheDir::Unusable, None),
        ),
    ];
    for (text, (os, arch, cache, installed)) in cases {
        let probe = parse_probe(&text, std::slice::from_ref(&wanted)).probe;
        assert_eq!(
            (probe.os, probe.arch, probe.cache, probe.installed),
            (os, arch, cache, installed),
            "{text:?}"
        );
    }
}

#[test]
fn a_session_gets_the_hosts_login_variables_and_says_what_terminal_it_is() {
    let text = "Linux\nx86_64\nwritable\nenv:HOME=/home/me\nenv:PATH=/usr/bin:/bin\n\
env:TERM=dumb\nsome noise\n";
    let found = parse_probe(text, &[name('a')]);
    assert_eq!(found.login.home(), Some("/home/me"));
    let env = found.login.session_env();
    assert!(env.contains(&("PATH".to_owned(), "/usr/bin:/bin".to_owned())));
    let terms: Vec<_> = env.iter().filter(|(name, _)| name == "TERM").collect();
    assert_eq!(
        terms.len(),
        1,
        "the host's own TERM never survives: {env:?}"
    );
    assert_eq!(terms[0].1, "xterm-256color");
    assert_eq!(HostLogin::default().home(), None);
}

#[test]
fn the_blob_is_looked_for_where_the_person_said_then_beside_the_program() {
    let exe = Path::new("/opt/temor/bin/temor");
    let cases = [
        (Some("/x/helper"), Some(exe), Some("/x/helper")),
        (Some(""), Some(exe), Some("/opt/temor/bin/temor-remote")),
        (None, Some(exe), Some("/opt/temor/bin/temor-remote")),
        (None, None, None),
    ];
    for (explicit, exe, want) in cases {
        let explicit = explicit.map(std::ffi::OsStr::new);
        assert_eq!(
            blob_path(explicit, exe),
            want.map(PathBuf::from),
            "{explicit:?} {exe:?}"
        );
    }
}

#[test]
fn the_install_names_the_build_by_its_hash_and_prints_what_it_stored() {
    let hash = ContentHash([0x0f; 32]);
    let script = install_script(&hash);
    assert!(script.contains(&hash.to_hex()));
    assert!(script.contains("sha256sum"));
    assert!(!script.contains('\n'), "one line for the remote shell");
    assert_eq!(
        helper_path(&hash),
        format!("~/.cache/temor-remote/{}/temor-remote", hash.to_hex())
    );
}
