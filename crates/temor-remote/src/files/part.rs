//! Streamed writes: a big file arrives in parts, staged beside its target and put in place once,
//! when the last part has arrived.

use temor_wire::message::{FileStamp, PartEnd, RemotePath, WriteCheck};
use temor_wire::units::ByteOffset;

use super::service::FileService;
use super::write::{WriteDecision, stage_name, write_decision};
use crate::error::Error;

impl FileService {
    /// Takes one part of a streamed write. Parts that are not the last stage the bytes (`None`).
    /// The last stages its bytes, checks `check` against the file now, and replaces the file: the
    /// new stamp. A check that fails drops the staged bytes and is a conflict. [`PartEnd::Abort`]
    /// drops them and ignores `bytes`.
    ///
    /// As in [`FileService::write`], the check and the replace are two steps.
    pub fn write_part(
        &self,
        path: &RemotePath,
        check: &WriteCheck,
        offset: ByteOffset,
        end: PartEnd,
        bytes: &[u8],
    ) -> Result<Option<FileStamp>, Error> {
        let target = self.target(path)?;
        let leaf = target.rel.last().ok_or_else(|| Error::BadPath {
            path: target.shown.clone(),
        })?;
        let name = stage_name(leaf);
        let discard = || {
            target
                .beneath
                .discard_part(&target.rel, &name, &target.shown)
        };
        match end {
            PartEnd::Abort => discard().map(|()| None),
            PartEnd::More => {
                target
                    .beneath
                    .stage_part(&target.rel, &name, offset.0, bytes, &target.shown)?;
                Ok(None)
            }
            PartEnd::Last => {
                target
                    .beneath
                    .stage_part(&target.rel, &name, offset.0, bytes, &target.shown)?;
                let finished = self.finish_part(&target, &name, check);
                if finished.is_err() {
                    // The staged bytes are ours and useless now; failing to drop them changes
                    // nothing to report.
                    let _ = discard();
                }
                finished.map(Some)
            }
        }
    }

    fn finish_part(
        &self,
        target: &super::service::Target,
        name: &str,
        check: &WriteCheck,
    ) -> Result<FileStamp, Error> {
        let current = self.current(target)?;
        if write_decision(check, current) == WriteDecision::Conflict {
            return Err(Error::Conflict { current });
        }
        target
            .beneath
            .commit_part(&target.rel, name, &target.shown)?;
        Ok(self.meta(target)?.stamp)
    }
}
