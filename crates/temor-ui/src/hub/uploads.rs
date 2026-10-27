//! Files dropped on a pane that runs on another computer (PLAN §9, §9b): each regular file is
//! copied into the pane's folder there, through the computer's helper when it has one and with
//! `scp` otherwise, and never over a file of the same name. The shelf lists them while they go
//! (`temor_core::shelf::upload_step`). A folder is not sent: it says so.

use std::path::{Path, PathBuf};

use temor_core::shelf::{UploadFault, UploadId, UploadIn, upload_step};
use temor_core::vocab::ids::{Bytes, PaneId};
use temor_ssh::{HostAlias, RemoteFilePath};
use temor_wire::message::RemotePath;

use super::model::Hub;
use super::remote_edit::EditTier;
use temor_client::remote::upload::UploadJobs;

/// One file to send, with where it goes.
struct Item {
    local: PathBuf,
    name: String,
    size: Bytes,
    remote: RemoteFilePath,
}

/// The file `path` as an item for `folder`, or the words that say why not.
fn item_for(path: &Path, folder: &str) -> Result<Item, String> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let shown = if name.is_empty() { "That" } else { name };
    if name.is_empty() {
        return Err("That file's name can't be sent".to_owned());
    }
    let size = match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => meta.len(),
        Ok(meta) if meta.is_dir() => {
            return Err(format!("{shown} is a folder; send files one at a time"));
        }
        _ => return Err(format!("{shown} can't be read here")),
    };
    let remote = RemoteFilePath::new(&format!("{}/{name}", folder.trim_end_matches('/')))
        .map_err(|_| format!("{shown} has a name that can't be sent"))?;
    Ok(Item {
        local: path.to_owned(),
        name: name.to_owned(),
        size: Bytes(size),
        remote,
    })
}

impl Hub {
    /// Whether `pane` is a session on another computer.
    pub(super) fn is_host_pane(&self, pane: PaneId) -> bool {
        self.ssh.panes.contains_key(&pane)
    }

    /// Sends the dropped files to the folder of the host pane `pane`. What the person should be
    /// told comes back.
    pub fn upload_drop(&mut self, pane: PaneId, paths: &[PathBuf]) -> Vec<String> {
        let Some(host) = self.alias_of(pane) else {
            return Vec::new();
        };
        let Some(folder) = self.book.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()) else {
            return vec!["temor doesn't know this pane's folder yet".to_owned()];
        };
        let tier = match self.edit_tier(&host) {
            Ok(tier) => tier,
            Err(_) => {
                return vec![format!(
                    "The connection to {} isn't running; reconnect, then drop the files again",
                    host.as_str()
                )];
            }
        };
        let mut told = Vec::new();
        for path in paths {
            match item_for(path, &folder) {
                Ok(item) => self.start_upload(&host, &folder, tier, item),
                Err(words) => told.push(words),
            }
        }
        told
    }

    fn start_upload(&mut self, host: &HostAlias, folder: &str, tier: EditTier, item: Item) {
        let id = self.shelf.uploads.next_id();
        let list = std::mem::take(&mut self.shelf.uploads);
        self.shelf.uploads = upload_step(
            list,
            UploadIn::Started {
                name: item.name,
                host: host.as_str().to_owned(),
                folder: folder.to_owned(),
                size: item.size,
            },
        );
        let jobs = UploadJobs::new(self.ssh.work.sender());
        match (tier, self.remote.helper_conn(host)) {
            (EditTier::Helper, Some(conn)) => {
                let remote = RemotePath(item.remote.as_str().to_owned());
                jobs.via_helper(id, conn, item.local, remote);
            }
            (EditTier::Helper, None) => self.on_upload(id, Err(UploadFault::NoConnection)),
            (EditTier::Scp, _) => {
                let env = self.ssh.env.clone();
                jobs.via_scp(id, env, host.clone(), item.local, item.remote);
            }
        }
    }

    /// A copy ended.
    pub(super) fn on_upload(&mut self, id: UploadId, result: Result<(), UploadFault>) {
        let list = std::mem::take(&mut self.shelf.uploads);
        self.shelf.uploads = upload_step(list, UploadIn::Ended { id, result });
    }

    /// So many bytes of an upload have reached the host.
    pub(super) fn on_upload_progress(&mut self, id: UploadId, sent: Bytes) {
        let list = std::mem::take(&mut self.shelf.uploads);
        self.shelf.uploads = upload_step(list, UploadIn::Progress { id, sent });
    }

    /// The person took an upload off the list.
    pub fn clear_upload(&mut self, id: UploadId) {
        let list = std::mem::take(&mut self.shelf.uploads);
        self.shelf.uploads = upload_step(list, UploadIn::Cleared(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_path_becomes_a_file_in_the_folder_or_a_reason() {
        let dir = std::env::temp_dir().join(format!("temor-upload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let file = dir.join("a b.txt");
        std::fs::write(&file, b"x").expect("file");
        let ok = item_for(&file, "/srv/app/").expect("a file");
        assert_eq!(ok.remote.as_str(), "/srv/app/a b.txt");
        assert_eq!(ok.name, "a b.txt");
        let cases = [
            ("a folder", dir.clone(), "is a folder"),
            ("missing", dir.join("gone"), "can't be read here"),
            ("no name", PathBuf::from("/"), "can't be sent"),
        ];
        for (name, path, words) in cases {
            let said = item_for(&path, "/srv").err().unwrap_or_default();
            assert!(said.contains(words), "{name}: {said}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
