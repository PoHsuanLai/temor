//! Files dropped on a pane that runs on another computer, on their way there. The downloads
//! shelf lists them under the downloads: each is "sending", then "sent" or says why not. Pure;
//! the window starts the copy and tells this when it ends.

/// One upload, numbered in the order the drops arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UploadId(pub u64);

/// Why a file did not get there. Each reads as a plain sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UploadFault {
    /// A file of that name is already in the folder; nothing was replaced.
    Exists,
    /// The connection to the computer is not running.
    NoConnection,
    /// The computer would not take the file (no room, no permission, no such folder).
    Refused,
    /// The file could not be read on this computer.
    Unreadable,
}

/// How far an upload got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UploadState {
    Sending,
    Sent,
    Failed(UploadFault),
}

/// One file going to another computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub id: UploadId,
    /// The file's name.
    pub name: String,
    /// The computer's name as the person knows it.
    pub host: String,
    /// The folder on that computer.
    pub folder: String,
    pub state: UploadState,
}

/// What happens to the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadIn {
    /// A file starts on its way.
    Started {
        name: String,
        host: String,
        folder: String,
    },
    /// The copy ended.
    Ended {
        id: UploadId,
        result: Result<(), UploadFault>,
    },
    /// The person took one off the list. Only a finished one goes.
    Cleared(UploadId),
}

/// The uploads, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Uploads {
    items: Vec<Upload>,
    next: u64,
}

impl Uploads {
    /// The uploads, newest first.
    pub fn items(&self) -> &[Upload] {
        &self.items
    }

    /// The id the next `Started` gets.
    pub fn next_id(&self) -> UploadId {
        UploadId(self.next.saturating_add(1))
    }
}

/// The list after `input`.
pub fn upload_step(mut list: Uploads, input: UploadIn) -> Uploads {
    match input {
        UploadIn::Started { name, host, folder } => {
            let id = list.next_id();
            list.next = id.0;
            list.items.insert(
                0,
                Upload {
                    id,
                    name,
                    host,
                    folder,
                    state: UploadState::Sending,
                },
            );
        }
        UploadIn::Ended { id, result } => {
            if let Some(item) = list.items.iter_mut().find(|item| item.id == id) {
                item.state = match result {
                    Ok(()) => UploadState::Sent,
                    Err(fault) => UploadState::Failed(fault),
                };
            }
        }
        UploadIn::Cleared(id) => list
            .items
            .retain(|item| item.id != id || item.state == UploadState::Sending),
    }
    list
}

/// What a row says about the upload, in plain words.
pub fn upload_detail(upload: &Upload) -> String {
    let (host, folder) = (&upload.host, &upload.folder);
    match upload.state {
        UploadState::Sending => format!("Sending to {host}, {folder}"),
        UploadState::Sent => format!("Sent to {host}, {folder}"),
        UploadState::Failed(UploadFault::Exists) => {
            format!("{host} already has a file with this name in {folder}")
        }
        UploadState::Failed(UploadFault::NoConnection) => {
            format!("The connection to {host} isn't running")
        }
        UploadState::Failed(UploadFault::Refused) => {
            format!("{host} wouldn't take the file into {folder}")
        }
        UploadState::Failed(UploadFault::Unreadable) => {
            "temor could not read the file here".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(name: &str) -> UploadIn {
        UploadIn::Started {
            name: name.to_owned(),
            host: "prod".to_owned(),
            folder: "/srv".to_owned(),
        }
    }

    #[test]
    fn uploads_go_from_sending_to_sent_or_failed_and_only_finished_ones_clear() {
        let mut list = Uploads::default();
        for name in ["a", "b", "c"] {
            list = upload_step(list, started(name));
        }
        let ids: Vec<_> = list
            .items()
            .iter()
            .map(|u| (u.name.as_str(), u.id.0))
            .collect();
        assert_eq!(ids, [("c", 3), ("b", 2), ("a", 1)], "newest first");
        list = upload_step(
            list,
            UploadIn::Ended {
                id: UploadId(1),
                result: Ok(()),
            },
        );
        list = upload_step(
            list,
            UploadIn::Ended {
                id: UploadId(2),
                result: Err(UploadFault::Exists),
            },
        );
        let states: Vec<_> = list.items().iter().map(|u| u.state).collect();
        assert_eq!(
            states,
            [
                UploadState::Sending,
                UploadState::Failed(UploadFault::Exists),
                UploadState::Sent
            ]
        );
        for id in [1, 2, 3] {
            list = upload_step(list, UploadIn::Cleared(UploadId(id)));
        }
        let left: Vec<_> = list.items().iter().map(|u| u.id.0).collect();
        assert_eq!(left, [3], "one still sending stays");
    }

    #[test]
    fn a_failure_says_what_to_do_about_it() {
        let mut list = upload_step(Uploads::default(), started("a"));
        list = upload_step(
            list,
            UploadIn::Ended {
                id: UploadId(1),
                result: Err(UploadFault::NoConnection),
            },
        );
        assert_eq!(
            upload_detail(&list.items()[0]),
            "The connection to prod isn't running"
        );
    }
}
