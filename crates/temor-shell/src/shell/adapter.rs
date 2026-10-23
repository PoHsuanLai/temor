//! One shell's way of being integrated, behind one interface. What differs between shells is
//! behaviour (how to inject the scripts, which files to lay down), so each shell is a type that
//! implements [`ShellAdapter`]; adding a shell is one new type, and the compiler asks for every
//! method. The scripts themselves are embedded in `scripts.rs`.

use std::ffi::OsStr;
use std::path::Path;

use super::plan::{IntegrationPlan, ShellStart, login_flag, plain, prepend_data_dir, shim};
use super::resolve::{Shell, ShellKind};
use super::scripts::{
    BASH_RC, BASH_SCRIPT, FISH_SCRIPT, NU_SCRIPT, ShellScript, ZSH_ENV, ZSH_SCRIPT,
};
use crate::env::ChildEnv;
use crate::report::ReportCaps;

/// How temor integrates with one kind of shell. Every method is pure: it returns data.
pub trait ShellAdapter: Sync {
    /// The detection result this adapter answers to.
    fn kind(&self) -> ShellKind;

    /// The files this shell's integration is made of.
    fn scripts(&self) -> &'static [ShellScript];

    /// How to start `shell` with the integration, its files under `root`. `env` already says the
    /// integration is on.
    fn plan(&self, shell: &Shell, start: ShellStart, env: ChildEnv, root: &Path)
    -> IntegrationPlan;

    /// What this shell's script reports besides the prompt marks (see [`crate::report`]).
    fn caps(&self) -> ReportCaps {
        ReportCaps {
            env: false,
            aliases: false,
        }
    }
}

/// The adapter for a kind of shell. A shell temor has no script for is started plainly.
pub fn adapter_for(kind: ShellKind) -> &'static dyn ShellAdapter {
    match kind {
        ShellKind::Bash => &BashAdapter,
        ShellKind::Zsh => &ZshAdapter,
        ShellKind::Fish => &FishAdapter,
        ShellKind::Nushell => &NuAdapter,
        ShellKind::Other => &PlainAdapter,
    }
}

/// bash: an rc file of temor's, since bash ignores `--rcfile` in a login shell.
#[derive(Debug, Clone, Copy)]
pub struct BashAdapter;

impl ShellAdapter for BashAdapter {
    fn kind(&self) -> ShellKind {
        ShellKind::Bash
    }

    fn scripts(&self) -> &'static [ShellScript] {
        &[BASH_RC, BASH_SCRIPT]
    }

    /// The wrapper reads the profile files itself when `TEMOR_BASH_LOGIN` is set, and bash always
    /// starts as a plain interactive shell.
    fn plan(
        &self,
        shell: &Shell,
        start: ShellStart,
        mut env: ChildEnv,
        root: &Path,
    ) -> IntegrationPlan {
        let rc = shim(root, BASH_RC);
        if start == ShellStart::Login {
            env.set("TEMOR_BASH_LOGIN", "1");
        }
        IntegrationPlan {
            program: shell.path.clone(),
            args: vec!["--rcfile".into(), rc.path.clone().into(), "-i".into()],
            env,
            files: vec![rc, shim(root, BASH_SCRIPT)],
        }
    }
}

/// zsh: a `ZDOTDIR` of temor's that loads the person's own files first.
#[derive(Debug, Clone, Copy)]
pub struct ZshAdapter;

impl ShellAdapter for ZshAdapter {
    fn kind(&self) -> ShellKind {
        ShellKind::Zsh
    }

    fn scripts(&self) -> &'static [ShellScript] {
        &[ZSH_ENV, ZSH_SCRIPT]
    }

    fn caps(&self) -> ReportCaps {
        ReportCaps {
            env: true,
            aliases: true,
        }
    }

    fn plan(
        &self,
        shell: &Shell,
        start: ShellStart,
        mut env: ChildEnv,
        root: &Path,
    ) -> IntegrationPlan {
        if let Some(real) = env.get("ZDOTDIR").map(OsStr::to_os_string) {
            env.set("TEMOR_ZSH_ZDOTDIR", real);
        }
        env.set("ZDOTDIR", root.join(ZSH_ENV.folder));
        let mut args = login_flag(start);
        args.push("-i".into());
        IntegrationPlan {
            program: shell.path.clone(),
            args,
            env,
            files: vec![shim(root, ZSH_ENV), shim(root, ZSH_SCRIPT)],
        }
    }
}

/// fish: a vendor config file found through `XDG_DATA_DIRS`.
#[derive(Debug, Clone, Copy)]
pub struct FishAdapter;

impl ShellAdapter for FishAdapter {
    fn kind(&self) -> ShellKind {
        ShellKind::Fish
    }

    fn scripts(&self) -> &'static [ShellScript] {
        &[FISH_SCRIPT]
    }

    fn plan(
        &self,
        shell: &Shell,
        start: ShellStart,
        mut env: ChildEnv,
        root: &Path,
    ) -> IntegrationPlan {
        prepend_data_dir(
            &mut env,
            &root.join(FISH_SCRIPT.folder),
            "TEMOR_FISH_XDG_DATA_DIRS",
        );
        let mut args = login_flag(start);
        args.push("-i".into());
        IntegrationPlan {
            program: shell.path.clone(),
            args,
            env,
            files: vec![shim(root, FISH_SCRIPT)],
        }
    }
}

/// nushell: a vendor autoload file under `XDG_DATA_DIRS/nushell/vendor/autoload` that switches on
/// nushell's own marks; the entry the plan adds is taken back out by that file, as fish's is.
#[derive(Debug, Clone, Copy)]
pub struct NuAdapter;

impl ShellAdapter for NuAdapter {
    fn kind(&self) -> ShellKind {
        ShellKind::Nushell
    }

    fn scripts(&self) -> &'static [ShellScript] {
        &[NU_SCRIPT]
    }

    fn plan(
        &self,
        shell: &Shell,
        start: ShellStart,
        mut env: ChildEnv,
        root: &Path,
    ) -> IntegrationPlan {
        prepend_data_dir(
            &mut env,
            &root.join(NU_SCRIPT.folder),
            "TEMOR_NU_XDG_DATA_DIRS",
        );
        IntegrationPlan {
            program: shell.path.clone(),
            args: login_flag(start),
            env,
            files: vec![shim(root, NU_SCRIPT)],
        }
    }
}

/// Any other shell: started as it is, with nothing laid down.
#[derive(Debug, Clone, Copy)]
pub struct PlainAdapter;

impl ShellAdapter for PlainAdapter {
    fn kind(&self) -> ShellKind {
        ShellKind::Other
    }

    fn scripts(&self) -> &'static [ShellScript] {
        &[]
    }

    fn plan(
        &self,
        shell: &Shell,
        start: ShellStart,
        env: ChildEnv,
        _root: &Path,
    ) -> IntegrationPlan {
        plain(shell, start, env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_the_adapter_that_names_it() {
        for kind in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Nushell,
            ShellKind::Other,
        ] {
            assert_eq!(adapter_for(kind).kind(), kind);
        }
    }

    #[test]
    fn the_adapters_scripts_are_the_ones_the_ssh_bootstrap_ships() {
        let shipped = crate::shell::scripts::integration_scripts();
        let mut mine: Vec<_> = [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Nushell,
        ]
        .into_iter()
        .flat_map(|kind| adapter_for(kind).scripts().iter().map(|s| s.name))
        .collect();
        let mut all: Vec<_> = shipped.iter().map(|s| s.name).collect();
        mine.sort_unstable();
        all.sort_unstable();
        assert_eq!(mine, all);
    }
}
