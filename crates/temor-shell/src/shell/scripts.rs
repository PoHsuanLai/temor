//! The integration scripts, embedded, for injection here and for the SSH bootstrap (§9b).

/// One embedded shell integration file, and where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellScript {
    /// The file's name in the crate's `shell/` directory (`temor.bash`, `zshenv`, …).
    pub name: &'static str,
    /// The folder under the shim root that holds this shell's files (`bash`, `zsh`, `fish`,
    /// `nushell`). It is also what the shell is pointed at: `ZDOTDIR`, or an `XDG_DATA_DIRS` entry.
    /// The local shims and the SSH bootstrap both lay the files out this way.
    pub folder: &'static str,
    /// The file's place inside `folder`.
    pub path: &'static str,
    pub contents: &'static str,
}

impl ShellScript {
    /// The file's place under the shim root, `/`-separated (`bash/temor-bashrc`).
    pub fn relative_path(&self) -> String {
        format!("{}/{}", self.folder, self.path)
    }
}

pub(crate) const BASH_SCRIPT: ShellScript = ShellScript {
    name: "temor.bash",
    folder: "bash",
    path: "temor.bash",
    contents: include_str!("../../shell/temor.bash"),
};
pub(crate) const BASH_RC: ShellScript = ShellScript {
    name: "temor-bashrc",
    folder: "bash",
    path: "temor-bashrc",
    contents: include_str!("../../shell/temor-bashrc"),
};
pub(crate) const ZSH_SCRIPT: ShellScript = ShellScript {
    name: "temor.zsh",
    folder: "zsh",
    path: "temor.zsh",
    contents: include_str!("../../shell/temor.zsh"),
};
pub(crate) const ZSH_ENV: ShellScript = ShellScript {
    name: "zshenv",
    folder: "zsh",
    path: ".zshenv",
    contents: include_str!("../../shell/zshenv"),
};
pub(crate) const FISH_SCRIPT: ShellScript = ShellScript {
    name: "temor.fish",
    folder: "fish",
    path: "fish/vendor_conf.d/temor.fish",
    contents: include_str!("../../shell/temor.fish"),
};
/// nushell reads vendor autoload files from `XDG_DATA_DIRS/nushell/vendor/autoload`.
pub(crate) const NU_SCRIPT: ShellScript = ShellScript {
    name: "temor.nu",
    folder: "nushell",
    path: "nushell/vendor/autoload/temor.nu",
    contents: include_str!("../../shell/temor.nu"),
};

/// Every integration script and shim, for callers that ship them elsewhere (the SSH bootstrap
/// sends them with each connection).
pub fn integration_scripts() -> [ShellScript; 6] {
    [
        BASH_SCRIPT,
        BASH_RC,
        ZSH_SCRIPT,
        ZSH_ENV,
        FISH_SCRIPT,
        NU_SCRIPT,
    ]
}
