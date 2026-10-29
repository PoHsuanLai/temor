//! The helper builds this temor carries, one per kind of processor (PLAN §9b, Tier B). The build
//! for this computer's own processor is `temor-remote` beside the program (or where
//! `TEMOR_REMOTE_HELPER` says); the build for another kind is `temor-remote-<kind>` in the same
//! folder, with `<kind>` as `uname -m` says (`x86_64`, `aarch64`). A host is given the build of
//! its own processor; a host whose kind has no build stays on a plain connection.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use temor_core::remote::HostArch;
use temor_core::vocab::text::HelperHash;

use crate::remote::install::{HelperBlob, blob_path, read_blob};

/// `uname -m` as the kinds of processor the helper is built for.
pub fn arch_of(uname: &str) -> HostArch {
    match uname {
        "x86_64" | "amd64" => HostArch::X86_64,
        "aarch64" | "arm64" => HostArch::Aarch64,
        _ => HostArch::Other,
    }
}

/// The word in a build's file name for `arch`; none for a kind there is no build for.
fn kind_word(arch: HostArch) -> Option<&'static str> {
    match arch {
        HostArch::X86_64 => Some("x86_64"),
        HostArch::Aarch64 => Some("aarch64"),
        HostArch::Other => None,
    }
}

/// Where the build for a host of kind `host` is looked for, given this computer's kind `own`:
/// the plain build for the same kind, else `temor-remote-<kind>` beside it. `None` for a kind
/// there is no build for.
pub fn build_path(
    explicit: Option<&OsStr>,
    exe: Option<&Path>,
    own: HostArch,
    host: HostArch,
) -> Option<PathBuf> {
    let plain = blob_path(explicit, exe)?;
    let word = kind_word(host)?;
    match host == own {
        true => Some(plain),
        false => Some(plain.with_file_name(format!("temor-remote-{word}"))),
    }
}

/// What the person is told when a computer's kind of processor has no helper build.
pub fn no_build_words(host: &str) -> String {
    format!(
        "temor has no helper for the kind of computer {host} is; \
         using a plain connection that ends if the network drops"
    )
}

/// The builds this temor carries, each read from its file the first time it is asked for.
#[derive(Debug)]
pub struct HelperBuilds {
    own: HostArch,
    explicit: Option<OsString>,
    exe: Option<PathBuf>,
    x86_64: OnceLock<Option<HelperBlob>>,
    aarch64: OnceLock<Option<HelperBlob>>,
}

impl HelperBuilds {
    /// The builds of a temor for a computer of kind `own`, run from `exe`, with the person's
    /// `TEMOR_REMOTE_HELPER` as `explicit`.
    pub fn new(own: HostArch, explicit: Option<&OsStr>, exe: Option<&Path>) -> Self {
        Self {
            own,
            explicit: explicit.map(OsStr::to_owned),
            exe: exe.map(Path::to_owned),
            x86_64: OnceLock::new(),
            aarch64: OnceLock::new(),
        }
    }

    /// The build for a host of kind `host`; none when there is no file for that kind.
    pub fn for_host(&self, host: HostArch) -> Option<&HelperBlob> {
        let slot = match host {
            HostArch::X86_64 => &self.x86_64,
            HostArch::Aarch64 => &self.aarch64,
            HostArch::Other => return None,
        };
        slot.get_or_init(|| {
            let at = build_path(
                self.explicit.as_deref(),
                self.exe.as_deref(),
                self.own,
                host,
            )?;
            read_blob(&at)
        })
        .as_ref()
    }

    /// Whether any build is carried.
    pub fn any(&self) -> bool {
        [HostArch::X86_64, HostArch::Aarch64]
            .into_iter()
            .any(|arch| self.for_host(arch).is_some())
    }

    /// The names of every build carried, so a probe can tell which of a host's builds is ours.
    pub fn names(&self) -> Vec<HelperHash> {
        [HostArch::X86_64, HostArch::Aarch64]
            .into_iter()
            .filter_map(|arch| self.for_host(arch).map(HelperBlob::name))
            .collect()
    }
}

#[cfg(test)]
#[path = "builds_tests.rs"]
mod tests;
