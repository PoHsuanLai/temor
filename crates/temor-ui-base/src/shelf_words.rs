//! What the Downloads list says: of each file, where it came from, how big it is and why it did
//! not arrive; of each upload, how far it got; and of each thing a person can do with a file, one
//! label for the button, the menu item and the hover on its own, and the hover hint.

use temor_client::model::shelf::{
    FailWhy, Row, RowAction as Act, Upload, UploadFault, UploadState,
};
use temor_client::model::transfer::Origin;
use temor_client::model::vocab::ids::Bytes;

/// Where Use in pane sends a file: the pane the person was typing in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneTarget {
    /// No pane has the keyboard.
    Nowhere,
    /// A pane on this computer: the path is typed there.
    Here,
    /// A pane on this host: the file is sent there.
    Host(String),
}

/// What a row's X says and does for assistive technology, on a download and on an upload alike.
pub const REMOVE_FROM_LIST: &str = "Remove from List";

/// What the page says when it lists nothing.
pub const EMPTY: &str = "Files you save or receive appear here";

/// Why a file did not arrive.
pub fn failure(why: FailWhy) -> &'static str {
    match why {
        FailWhy::Refused => "You said no",
        FailWhy::Broken => "The transfer stopped before it finished",
        FailWhy::TooBig => "The file is too big",
        FailWhy::Unwritable => "temor could not save it in your Downloads folder",
    }
}

/// A file's size and where it came from, such as `1.2 MB, from prod`.
pub fn detail(row: &Row) -> String {
    let from = match &row.origin {
        Origin::Remote { host, .. } => format!("from {}", host.as_str()),
        Origin::LocalStream { .. } => "from a program in this tab".to_owned(),
        Origin::Local { .. } => "saved here".to_owned(),
        Origin::Pipe => "from a pipe".to_owned(),
    };
    match row.size {
        Some(size) => format!("{}, {from}", human_size(size)),
        None => from,
    }
}

/// `999 bytes`, `1.0 KB`, `12.3 MB`: powers of 1000, one decimal above bytes.
pub fn human_size(size: Bytes) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if size.0 < 1000 {
        return match size.0 {
            1 => "1 byte".to_owned(),
            n => format!("{n} bytes"),
        };
    }
    let mut tenths = size.0 / 100; // tenths of a KB
    let mut unit = 0;
    while tenths >= 10_000 && unit + 1 < UNITS.len() {
        tenths /= 1000;
        unit += 1;
    }
    format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
}

/// What a row says about an upload, in plain words.
pub fn upload_detail(upload: &Upload) -> String {
    let (host, folder) = (&upload.host, &upload.folder);
    match upload.state {
        UploadState::Sending if upload.sent.0 > 0 => format!(
            "Sending to {host}, {folder}: {} of {}",
            human_size(upload.sent),
            human_size(upload.size)
        ),
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

/// The words for one action of a file: its label (a button's, a menu item's) and its hover hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActWords {
    pub label: String,
    pub tip: String,
}

impl ActWords {
    fn new(label: impl Into<String>, tip: impl Into<String>) -> Option<Self> {
        Some(Self {
            label: label.into(),
            tip: tip.into(),
        })
    }
}

/// The words for `act`, with `target` the terminal that has the keyboard. `None` where the action
/// has nothing to say: Use in pane, when no terminal has the keyboard.
pub fn act_words(act: Act, target: &PaneTarget) -> Option<ActWords> {
    match act {
        Act::Open => ActWords::new("Open", "Open the file"),
        Act::Reveal => ActWords::new("Show in Folder", "Show the file in your files"),
        Act::CopyPath => ActWords::new("Copy Path", "Copy the file's path"),
        Act::ShowInTab(_) => ActWords::new("Show Tab", "Go back to the tab it came from"),
        Act::Clear => ActWords::new(REMOVE_FROM_LIST, "Take it off the list; the file stays"),
        Act::UseInPane => match target {
            PaneTarget::Nowhere => None,
            PaneTarget::Here => ActWords::new(
                "Paste Path",
                "Type the file's path into the terminal you were using",
            ),
            PaneTarget::Host(host) => ActWords::new(
                format!("Send to {host}"),
                format!("Copy the file to {host}, into the folder of the terminal you were using"),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use temor_client::model::shelf::UploadId;

    use super::*;

    #[test]
    fn sizes_read_plainly() {
        let cases = [
            (1, "1 byte"),
            (999, "999 bytes"),
            (1000, "1.0 KB"),
            (12_345_678, "12.3 MB"),
            (2_000_000_000, "2.0 GB"),
        ];
        for (n, want) in cases {
            assert_eq!(human_size(Bytes(n)), want, "{n}");
        }
    }

    #[test]
    fn an_upload_says_how_far_it_got_and_a_failure_what_went_wrong() {
        let upload = |sent, state| Upload {
            id: UploadId(1),
            name: "a".to_owned(),
            host: "prod".to_owned(),
            folder: "/srv".to_owned(),
            size: Bytes(10_000),
            sent: Bytes(sent),
            state,
        };
        assert_eq!(
            upload_detail(&upload(4_000, UploadState::Sending)),
            "Sending to prod, /srv: 4.0 KB of 10.0 KB"
        );
        assert_eq!(
            upload_detail(&upload(0, UploadState::Failed(UploadFault::NoConnection))),
            "The connection to prod isn't running"
        );
    }

    #[test]
    fn every_action_has_one_label_for_its_button_and_its_menu_and_use_in_pane_needs_a_terminal() {
        let label = |act, target: &PaneTarget| act_words(act, target).map(|w| w.label);
        let here = PaneTarget::Here;
        let table = [
            ("open", label(Act::Open, &here), Some("Open")),
            ("reveal", label(Act::Reveal, &here), Some("Show in Folder")),
            ("clear", label(Act::Clear, &here), Some("Remove from List")),
            (
                "paste here",
                label(Act::UseInPane, &here),
                Some("Paste Path"),
            ),
            (
                "no terminal",
                label(Act::UseInPane, &PaneTarget::Nowhere),
                None,
            ),
        ];
        for (name, got, want) in table {
            assert_eq!(got.as_deref(), want, "{name}");
        }
    }
}
