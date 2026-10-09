//! A Tailscale node id: the machine's identity inside a tailnet, which survives a rename.

/// A node id as the tailnet reports it, such as `nodeid:1a2b3c` or `n123CNTRL`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(String);

impl NodeId {
    /// Validates the id: 1 to 128 ASCII letters, digits and `-_.:`.
    pub fn new(text: &str) -> Option<Self> {
        let usable = !text.is_empty()
            && text.len() <= 128
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
        usable.then(|| Self(text.to_owned()))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
