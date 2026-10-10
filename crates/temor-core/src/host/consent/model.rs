//! The remembered answers and how a host is found among them.

use crate::remote::Grant;
use serde::{Deserialize, Serialize};
use temor_ssh::HostKey;

/// One thing the bootstrap does on a host that the person is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapAction {
    /// Run the setup with the connection: terminal marks, folder tracking and the terminal
    /// type, all in a temporary folder that is removed when the session ends. Without it the
    /// session is a plain `ssh`.
    Integrate,
    /// Also add a small terminal description file to the host's home folder, when it lacks one.
    /// The only change made outside the temporary folder.
    Terminfo,
}

/// The answers for one host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct BootstrapGrants {
    #[serde(default)]
    pub integrate: Grant,
    #[serde(default)]
    pub terminfo: Grant,
    /// Whether the helper program may be put on the host. Not written while it is still open,
    /// so a book that never asked reads as it did before the question was added.
    #[serde(default, skip_serializing_if = "is_open")]
    pub helper: Grant,
}

fn is_open(grant: &Grant) -> bool {
    *grant == Grant::Ask
}

impl BootstrapGrants {
    /// The answer for `action`.
    pub fn get(&self, action: BootstrapAction) -> Grant {
        match action {
            BootstrapAction::Integrate => self.integrate,
            BootstrapAction::Terminfo => self.terminfo,
        }
    }

    /// These answers with `action` set to `grant`.
    pub fn with(mut self, action: BootstrapAction, grant: Grant) -> Self {
        match action {
            BootstrapAction::Integrate => self.integrate = grant,
            BootstrapAction::Terminfo => self.terminfo = grant,
        }
        self
    }

    /// These answers with every question still open filled from `other`.
    pub(super) fn filled_from(self, other: BootstrapGrants) -> Self {
        let pick = |mine: Grant, theirs: Grant| match mine {
            Grant::Ask => theirs,
            answered => answered,
        };
        Self {
            integrate: pick(self.integrate, other.integrate),
            terminfo: pick(self.terminfo, other.terminfo),
            helper: pick(self.helper, other.helper),
        }
    }
}

/// Who is asking: the verified machine identity when the connection has produced one, and the
/// alias the person typed or picked (all there is before the first connection to a host).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    pub key: Option<HostKey>,
    pub alias: String,
}

/// One remembered host, in the form the session file stores: the key parts as text, the
/// aliases it has been reached by, and its answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HostRecord {
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub grants: BootstrapGrants,
}

impl HostRecord {
    fn key(&self) -> Option<HostKey> {
        HostKey::from_parts(self.fingerprint.as_deref(), self.node.as_deref())
    }

    fn set_key(&mut self, key: &HostKey) {
        self.fingerprint = key.fingerprint().map(|f| f.as_str().to_owned());
        self.node = key.node().map(|n| n.as_str().to_owned());
    }

    fn known_by(&self, alias: &str) -> bool {
        self.aliases.iter().any(|a| a == alias)
    }
}

/// Every remembered answer. Serialises as a plain list of [`HostRecord`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConsentBook {
    records: Vec<HostRecord>,
}

impl ConsentBook {
    /// The book of stored records.
    pub fn from_records(records: Vec<HostRecord>) -> Self {
        Self { records }
    }

    /// The records, for the session file.
    pub fn records(&self) -> &[HostRecord] {
        &self.records
    }

    /// The answers for the machine `who` names; every question open when it is unknown. When
    /// several records name it (an alias recorded before the machine's identity was known),
    /// the earliest answer to each question wins.
    ///
    /// A machine matches by identity first. An alias matches only a record that has no
    /// identity yet, or when the caller has none: if the machine behind an alias turned out to
    /// be a different one, its predecessor's answers do not carry over.
    pub fn grants(&self, who: &Who) -> BootstrapGrants {
        self.matching(who)
            .into_iter()
            .fold(BootstrapGrants::default(), |acc, i| {
                acc.filled_from(self.records[i].grants)
            })
    }

    /// Indexes of the records that are the machine `who` names, in book order.
    pub(super) fn matching(&self, who: &Who) -> Vec<usize> {
        let hit = |r: &HostRecord| match (&who.key, r.key()) {
            (Some(mine), Some(theirs)) => mine.same_machine(&theirs),
            _ => r.known_by(&who.alias),
        };
        (0..self.records.len())
            .filter(|&i| hit(&self.records[i]))
            .collect()
    }

    /// Folds the records at `indexes` into one that knows `who`, merging keys, aliases and
    /// answers (the earliest answered record wins each question), and returns its index. A new
    /// record when `indexes` is empty.
    pub(super) fn fold(&mut self, indexes: &[usize], who: &Who) -> usize {
        let mut merged = HostRecord::default();
        let mut key: Option<HostKey> = None;
        for &i in indexes {
            let record = &self.records[i];
            key = match (key, record.key()) {
                (Some(k), Some(r)) => Some(k.merged(&r)),
                (k, r) => k.or(r),
            };
            for alias in &record.aliases {
                if !merged.known_by(alias) {
                    merged.aliases.push(alias.clone());
                }
            }
            merged.grants = merged.grants.filled_from(record.grants);
        }
        if let Some(mine) = &who.key {
            // The key the connection just verified wins over an older one (a rotated host key).
            key = Some(key.map_or_else(|| mine.clone(), |k| mine.merged(&k)));
        }
        if let Some(key) = &key {
            merged.set_key(key);
        }
        if !merged.known_by(&who.alias) {
            merged.aliases.push(who.alias.clone());
        }
        let at = indexes.first().copied().unwrap_or(self.records.len());
        let mut kept = Vec::with_capacity(self.records.len() + 1);
        for (i, record) in self.records.drain(..).enumerate() {
            match (i == at, indexes.contains(&i)) {
                (true, _) => kept.push(merged.clone()),
                (false, true) => {}
                (false, false) => kept.push(record),
            }
        }
        if at == kept.len() {
            kept.push(merged);
        }
        self.records = kept;
        indexes.first().map_or(self.records.len() - 1, |&i| i)
    }

    pub(super) fn set_helper(&mut self, index: usize, grant: Grant) {
        if let Some(record) = self.records.get_mut(index) {
            record.grants.helper = grant;
        }
    }

    pub(super) fn set_grant(&mut self, index: usize, action: BootstrapAction, grant: Grant) {
        if let Some(record) = self.records.get_mut(index) {
            record.grants = record.grants.with(action, grant);
        }
    }
}
