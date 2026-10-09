use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;
use crate::env::ChildEnv;

fn env(pairs: &[(&str, &str)]) -> ChildEnv {
    pairs.iter().copied().collect()
}

#[test]
fn shell_kind_reads_the_file_name() {
    const CASES: &[(&str, ShellKind)] = &[
        ("/bin/bash", ShellKind::Bash),
        ("-bash", ShellKind::Bash),
        ("/usr/bin/zsh", ShellKind::Zsh),
        ("/opt/homebrew/bin/fish", ShellKind::Fish),
        ("/bin/sh", ShellKind::Other),
        ("/usr/bin/nu", ShellKind::Nushell),
        ("/opt/homebrew/bin/nushell", ShellKind::Nushell),
    ];
    for (path, kind) in CASES {
        assert_eq!(ShellKind::of(Path::new(path)), *kind, "{path}");
    }
}

#[test]
fn resolve_prefers_passwd_then_shell_var_then_sh() {
    let all = |_: &Path| true;
    let shell_var = env(&[("SHELL", "/usr/bin/fish")]);
    const CASES: &[(Option<&str>, &str)] = &[
        (Some("/usr/bin/zsh"), "/usr/bin/zsh"),
        (None, "/usr/bin/fish"),
        (Some("/usr/sbin/nologin"), "/usr/bin/fish"),
        (Some("relative/zsh"), "/usr/bin/fish"),
    ];
    for (passwd, want) in CASES {
        let got = resolve_shell(passwd.map(PathBuf::from), &shell_var, all);
        assert_eq!(got.path, PathBuf::from(want), "{passwd:?}");
    }
    let got = resolve_shell(None, &env(&[]), all);
    assert_eq!(got, Shell::at("/bin/sh"));
    let missing = resolve_shell(Some("/usr/bin/zsh".into()), &shell_var, |p: &Path| {
        p != Path::new("/usr/bin/zsh")
    });
    assert_eq!(missing.kind, ShellKind::Fish);
}

#[test]
fn passwd_lookup_finds_the_users_shell() {
    let text = "root:x:0:0:root:/root:/bin/bash\nme:x:1000:1000::/home/me:/usr/bin/zsh\nbad line\n";
    assert_eq!(
        passwd_login_shell(text, UserId(1000)),
        Some("/usr/bin/zsh".into())
    );
    assert_eq!(
        passwd_login_shell(text, UserId(0)),
        Some("/bin/bash".into())
    );
    assert_eq!(passwd_login_shell(text, UserId(42)), None);
}

fn args(plan: &IntegrationPlan) -> Vec<OsString> {
    plan.args.clone()
}

#[test]
fn bash_plan_uses_an_rcfile_under_the_runtime_dir() {
    let rt = Path::new("/run/temor");
    let plan = plan_integration(&Shell::at("/bin/bash"), ShellStart::Login, env(&[]), rt);
    let rc = rt.join("shell-integration/bash/temor-bashrc");
    assert_eq!(
        args(&plan),
        vec![OsString::from("--rcfile"), rc.clone().into(), "-i".into()]
    );
    assert_eq!(plan.env.get("TEMOR_BASH_LOGIN"), Some("1".as_ref()));
    assert!(plan.files.iter().all(|file| file.path.starts_with(rt)));
    assert_eq!(plan.files[0].path, rc);
}

#[test]
fn zsh_plan_keeps_the_real_zdotdir() {
    let rt = Path::new("/run/temor");
    let plan = plan_integration(
        &Shell::at("/bin/zsh"),
        ShellStart::Interactive,
        env(&[("ZDOTDIR", "/home/me/.config/zsh")]),
        rt,
    );
    let dir = rt.join("shell-integration/zsh");
    assert_eq!(plan.env.get("ZDOTDIR"), Some(dir.as_os_str()));
    assert_eq!(
        plan.env.get("TEMOR_ZSH_ZDOTDIR"),
        Some("/home/me/.config/zsh".as_ref())
    );
    let unset = plan_integration(
        &Shell::at("/bin/zsh"),
        ShellStart::Interactive,
        env(&[]),
        rt,
    );
    assert_eq!(unset.env.get("TEMOR_ZSH_ZDOTDIR"), None);
}

#[test]
fn fish_plan_prepends_a_data_dir() {
    let rt = Path::new("/run/temor");
    let plan = plan_integration(
        &Shell::at("/bin/fish"),
        ShellStart::Interactive,
        env(&[]),
        rt,
    );
    assert_eq!(
        plan.env.get("XDG_DATA_DIRS"),
        Some("/run/temor/shell-integration/fish:/usr/local/share:/usr/share".as_ref())
    );
    assert_eq!(plan.env.get("TEMOR_FISH_XDG_DATA_DIRS"), None);
    assert_eq!(
        plan.files[0].path,
        rt.join("shell-integration/fish/fish/vendor_conf.d/temor.fish")
    );
}

#[test]
fn opt_out_and_other_shells_start_plainly() {
    let rt = Path::new("/run/temor");
    let off = env(&[("TEMOR_SHELL_INTEGRATION", "0")]);
    let plan = plan_integration(&Shell::at("/bin/bash"), ShellStart::Login, off.clone(), rt);
    assert_eq!(args(&plan), vec![OsString::from("-l")]);
    assert_eq!(plan.env, off);
    assert!(plan.files.is_empty());
    let sh = plan_integration(&Shell::at("/bin/sh"), ShellStart::Interactive, env(&[]), rt);
    assert!(sh.args.is_empty() && sh.files.is_empty());
}
