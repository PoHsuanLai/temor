//! `helper_step`: putting the helper on a host (§9b Tier B): probe, ask, upload, check the hash.
//! A host that cannot run it stays on Tier A with no error.

use super::model::Grant;
use crate::vocab::text::HelperHash;
use ds_core::word::Word;

/// Where the helper stands on one host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HelperState {
    /// Not there, or not looked for.
    Absent,
    /// The host can run it; the person is being asked.
    Offered,
    /// Being uploaded.
    Uploading,
    /// Running this build.
    Ready(HelperHash),
    /// The person said no.
    Refused,
    /// The host cannot run it.
    Unsupported,
}

/// The host's operating system, as `uname -s` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum HostOs {
    /// Linux: the helper's only system.
    Linux,
    /// Anything else.
    Other,
}

/// The host's processor, as `uname -m` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum HostArch {
    /// x86_64.
    X86_64,
    /// aarch64.
    Aarch64,
    /// Anything else.
    Other,
}

/// Whether the helper's folder can hold a program that runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum CacheDir {
    /// `~/.cache` is writable and not mounted `noexec`.
    Usable,
    /// Read-only, missing, or `noexec`.
    Unusable,
}

/// What probing the host found.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Probe {
    /// Its system.
    pub os: HostOs,
    /// Its processor.
    pub arch: HostArch,
    /// Its cache folder.
    pub cache: CacheDir,
    /// The build already there, if any.
    pub installed: Option<HelperHash>,
}

impl Probe {
    /// Whether the helper can run there.
    pub fn fits(&self) -> bool {
        self.os == HostOs::Linux && self.arch != HostArch::Other && self.cache == CacheDir::Usable
    }
}

/// What moves the helper's state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HelperIn {
    /// The probe finished.
    Probed(Probe),
    /// The person answered the offer.
    Answered(Grant),
    /// The upload finished, and the file on the host hashes to this.
    Uploaded(HelperHash),
    /// The upload failed.
    UploadFailed,
}

/// What the helper wants done.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HelperOut {
    /// Ask the person whether to install it.
    AskPerson,
    /// Remember the answer for this host.
    Remember(Grant),
    /// Upload this build.
    Upload(HelperHash),
    /// Remove a build from the host.
    RemoveFromHost(HelperHash),
    /// Use the helper: Tier B.
    UseHelper(HelperHash),
    /// Stay on Tier A.
    StayPlain,
}

/// The helper after `input`, wanting build `wanted`, with the host's remembered `consent`.
pub fn helper_step(
    state: HelperState,
    input: HelperIn,
    wanted: &HelperHash,
    consent: Grant,
) -> (HelperState, Vec<HelperOut>) {
    use HelperState as S;
    match (state, input) {
        (S::Absent | S::Ready(_), HelperIn::Probed(probe)) => probed(probe, wanted, consent),
        (S::Offered, HelperIn::Answered(Grant::Allowed)) => (
            S::Uploading,
            vec![
                HelperOut::Remember(Grant::Allowed),
                HelperOut::Upload(wanted.clone()),
            ],
        ),
        (S::Offered, HelperIn::Answered(Grant::Refused)) => (
            S::Refused,
            vec![HelperOut::Remember(Grant::Refused), HelperOut::StayPlain],
        ),
        (S::Uploading, HelperIn::Uploaded(hash)) if hash == *wanted => {
            (S::Ready(hash.clone()), vec![HelperOut::UseHelper(hash)])
        }
        (S::Uploading, HelperIn::Uploaded(hash)) => (
            S::Unsupported,
            vec![HelperOut::RemoveFromHost(hash), HelperOut::StayPlain],
        ),
        (S::Uploading, HelperIn::UploadFailed) => (S::Unsupported, vec![HelperOut::StayPlain]),
        (state, _) => (state, vec![]),
    }
}

fn probed(probe: Probe, wanted: &HelperHash, consent: Grant) -> (HelperState, Vec<HelperOut>) {
    if !probe.fits() {
        return (HelperState::Unsupported, vec![HelperOut::StayPlain]);
    }
    match consent {
        Grant::Refused => (HelperState::Refused, vec![HelperOut::StayPlain]),
        Grant::Ask => (HelperState::Offered, vec![HelperOut::AskPerson]),
        Grant::Allowed if probe.installed.as_ref() == Some(wanted) => (
            HelperState::Ready(wanted.clone()),
            vec![HelperOut::UseHelper(wanted.clone())],
        ),
        Grant::Allowed => {
            let mut out: Vec<HelperOut> = probe
                .installed
                .into_iter()
                .map(HelperOut::RemoveFromHost)
                .collect();
            out.push(HelperOut::Upload(wanted.clone()));
            (HelperState::Uploading, out)
        }
    }
}
