//! What the session file keeps about hosts, beside the tabs: the remembered answers
//! ([`ConsentBook`]) and which panes are on which host, so a restored tab reconnects. They sit
//! under one `hosts` key of the file, which readers that do not know it skip.

use serde::{Deserialize, Serialize};

use super::consent::{ConsentBook, HostRecord};
use crate::error::CoreError;
use crate::vocab::ids::PaneId;
use crate::vocab::lenient::lenient_list;

/// One pane that is a session on a host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneHost {
    pub pane: PaneId,
    /// The name it was reached by (an ssh_config alias, or a tailnet name).
    pub alias: String,
}

/// The host parts of a session file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostsFile {
    pub book: ConsentBook,
    pub panes: Vec<PaneHost>,
}

#[derive(Serialize)]
struct Write<'a> {
    consent: &'a [HostRecord],
    panes: &'a [PaneHost],
}

#[derive(Deserialize)]
struct Read {
    #[serde(default, deserialize_with = "lenient_list")]
    consent: Vec<HostRecord>,
    #[serde(default, deserialize_with = "lenient_list")]
    panes: Vec<PaneHost>,
}

/// `session` (the text of a session file) with `hosts` added under the `hosts` key, replacing
/// what was there. An empty `hosts` removes the key. Fails when `session` is not a JSON object.
pub fn splice_hosts(session: &str, hosts: &HostsFile) -> Result<String, CoreError> {
    let bad = |why: &str| CoreError::SessionEncode(why.to_owned());
    let mut value: serde_json::Value =
        serde_json::from_str(session).map_err(|e| CoreError::SessionEncode(e.to_string()))?;
    let map = value
        .as_object_mut()
        .ok_or_else(|| bad("the session is not an object"))?;
    match hosts == &HostsFile::default() {
        true => {
            map.remove("hosts");
        }
        false => {
            let write = Write {
                consent: hosts.book.records(),
                panes: &hosts.panes,
            };
            let hosts =
                serde_json::to_value(write).map_err(|e| CoreError::SessionEncode(e.to_string()))?;
            map.insert("hosts".to_owned(), hosts);
        }
    }
    serde_json::to_string_pretty(&value).map_err(|e| CoreError::SessionEncode(e.to_string()))
}

/// The hosts in `session`: whatever parses, nothing for a file without them or a damaged one.
pub fn read_hosts(session: &str) -> HostsFile {
    let read = serde_json::from_str::<serde_json::Value>(session)
        .ok()
        .and_then(|value| value.get("hosts").cloned())
        .and_then(|hosts| serde_json::from_value::<Read>(hosts).ok());
    match read {
        Some(read) => HostsFile {
            book: ConsentBook::from_records(read.consent),
            panes: read.panes,
        },
        None => HostsFile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{HostsFile, PaneHost, read_hosts, splice_hosts};
    use crate::host::consent::{ConsentBook, HostRecord};
    use crate::vocab::ids::PaneId;

    fn sample() -> HostsFile {
        HostsFile {
            book: ConsentBook::from_records(vec![HostRecord {
                aliases: vec!["prod".to_owned()],
                ..HostRecord::default()
            }]),
            panes: vec![PaneHost {
                pane: PaneId(3),
                alias: "prod".to_owned(),
            }],
        }
    }

    #[test]
    fn hosts_survive_a_round_trip_beside_the_tabs_and_an_empty_set_removes_the_key() {
        let session = r#"{"version": 1, "next_tab": 4}"#;
        let with = splice_hosts(session, &sample()).expect("splice");
        assert_eq!(read_hosts(&with), sample());
        let kept: serde_json::Value = serde_json::from_str(&with).expect("json");
        assert_eq!(kept["next_tab"], 4, "the rest of the file is kept");
        let without = splice_hosts(&with, &HostsFile::default()).expect("splice");
        assert!(!without.contains("hosts"), "{without}");
    }

    #[test]
    fn damaged_host_parts_cost_only_themselves() {
        let text = r#"{"version": 1, "hosts": {"consent": [7, {"aliases": ["a"]}],
            "panes": [{"pane": 1, "alias": "a"}, "x"]}}"#;
        let got = read_hosts(text);
        assert_eq!(got.book.records().len(), 1);
        assert_eq!(got.panes.len(), 1);
        assert_eq!(read_hosts("not json"), HostsFile::default());
        assert!(splice_hosts("[1]", &sample()).is_err());
    }
}
