//! Sending a dropped file to another computer (PLAN §9, §9b): through the computer's helper when
//! it has one (the file streams in parts, and is never put over a file of the same name), with
//! `scp` over the shared connection otherwise (a look at the name first, for the same promise).
//! Each runs on a thread of its own and reports as an [`SshEvent::Upload`].

use std::path::PathBuf;
use std::sync::Arc;

use temor_core::shelf::{UploadFault, UploadId};
use temor_core::vocab::ids::Bytes;
use temor_ssh::{HostAlias, RemoteFilePath, RemoteFiles, RemoteState, SshRemoteFiles};
use temor_wire::message::{RemotePath, WriteCheck};
use tokio::sync::mpsc::UnboundedSender;

use crate::remote::conn::Conn;
use crate::remote::edit::{conflict_of, fault_of};
use crate::ssh::SshEnv;
use crate::ssh::work::{SshEvent, edit_fault};
use temor_core::host::edit::EditFault;

/// Starts uploads. Cloning shares the channel.
#[derive(Debug, Clone)]
pub struct UploadJobs {
    events: UnboundedSender<SshEvent>,
}

impl UploadJobs {
    pub fn new(events: UnboundedSender<SshEvent>) -> Self {
        Self { events }
    }

    fn run(&self, id: UploadId, job: impl FnOnce() -> Result<(), UploadFault> + Send + 'static) {
        let events = self.events.clone();
        let started = std::thread::Builder::new()
            .name("temor-upload".to_owned())
            .spawn(move || {
                let result = job();
                // The window being gone leaves nobody to tell.
                let _ = events.send(SshEvent::Upload { id, result });
            });
        if started.is_err() {
            let result = Err(UploadFault::NoConnection);
            let _ = self.events.send(SshEvent::Upload { id, result });
        }
    }

    /// Copies `local` to `remote` through the helper, unless a file is there already.
    pub fn via_helper(&self, id: UploadId, conn: Arc<Conn>, local: PathBuf, remote: RemotePath) {
        let events = self.events.clone();
        self.run(id, move || {
            // Bytes add up as parts arrive; the window being gone leaves nobody to tell.
            let mut total = 0u64;
            let mut sent = |part: u64| {
                total += part;
                let _ = events.send(SshEvent::UploadProgress {
                    id,
                    sent: Bytes(total),
                });
            };
            let check = WriteCheck::MustBeNew;
            match conn.client().upload_with(&local, &remote, check, &mut sent) {
                Ok(_) => Ok(()),
                Err(error) if conflict_of(&error).is_some() => Err(UploadFault::Exists),
                Err(error) => Err(match fault_of(&error) {
                    EditFault::NoConnection => UploadFault::NoConnection,
                    _ => UploadFault::Refused,
                }),
            }
        });
    }

    /// Copies `local` to `remote` with `scp`, unless a file is there already.
    pub fn via_scp(
        &self,
        id: UploadId,
        env: SshEnv,
        host: HostAlias,
        local: PathBuf,
        remote: RemoteFilePath,
    ) {
        self.run(id, move || {
            let layout = env.layout.as_ref().map_err(|_| UploadFault::NoConnection)?;
            let files = SshRemoteFiles::new(env.runner.clone(), env.tools.clone(), layout.clone());
            let fault = |error| match edit_fault(&error) {
                EditFault::NoConnection => UploadFault::NoConnection,
                _ => UploadFault::Refused,
            };
            match files.stat(&host, &remote).map_err(fault)? {
                RemoteState::Missing => {}
                _ => return Err(UploadFault::Exists),
            }
            files.store(&host, &local, &remote).map_err(fault)
        });
    }
}
