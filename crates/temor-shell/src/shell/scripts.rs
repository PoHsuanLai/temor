//! The integration scripts, embedded, for injection here and for the SSH bootstrap (§9b).

/// One embedded shell integration file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellScript {
    /// The file's name in the crate's `shell/` directory (`temor.bash`, `zshenv`, …).
    pub name: &'static str,
    pub contents: &'static str,
}

pub(crate) const BASH_SCRIPT: ShellScript = ShellScript {
    name: "temor.bash",
    contents: include_str!("../../shell/temor.bash"),
};
pub(crate) const BASH_RC: ShellScript = ShellScript {
    name: "temor-bashrc",
    contents: include_str!("../../shell/temor-bashrc"),
};
pub(crate) const ZSH_SCRIPT: ShellScript = ShellScript {
    name: "temor.zsh",
    contents: include_str!("../../shell/temor.zsh"),
};
pub(crate) const ZSH_ENV: ShellScript = ShellScript {
    name: "zshenv",
    contents: include_str!("../../shell/zshenv"),
};
pub(crate) const FISH_SCRIPT: ShellScript = ShellScript {
    name: "temor.fish",
    contents: include_str!("../../shell/temor.fish"),
};
pub(crate) const NU_SCRIPT: ShellScript = ShellScript {
    name: "temor.nu",
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
