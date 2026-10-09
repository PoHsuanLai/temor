//! Host-key fingerprints from `ssh-keygen` output. The fingerprint is the identity; the shown
//! name is what the person typed or picked (PLAN §9b).

use crate::error::SshError;

/// The length of a SHA-256 digest in unpadded base64.
const SHA256_BASE64_LEN: usize = 43;
const PREFIX: &str = "SHA256:";

/// An `SHA256:<base64>` host-key fingerprint, as `ssh-keygen -l` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// Validates the text: `SHA256:` then 43 base64 characters.
    pub fn new(text: &str) -> Result<Self, SshError> {
        let body = text.strip_prefix(PREFIX).unwrap_or_default();
        let valid = body.len() == SHA256_BASE64_LEN
            && body
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/');
        match valid {
            true => Ok(Self(text.to_owned())),
            false => Err(SshError::BadFingerprint(text.to_owned())),
        }
    }

    /// The text, `SHA256:…`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name a host is shown as: never empty, with no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShownName(String);

impl ShownName {
    /// Validates the name; `None` when it is empty or has control characters.
    pub fn new(text: &str) -> Option<Self> {
        let usable = !text.trim().is_empty() && !text.chars().any(char::is_control);
        usable.then(|| Self(text.to_owned()))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The first `SHA256:` fingerprint in `ssh-keygen -l` output, such as
/// `256 SHA256:… host (ED25519)`. Comment lines (`# Host … found`, from `ssh-keygen -F`) are
/// skipped. A fingerprint that is present but malformed is an error, not a skip.
pub fn fingerprint_from_keygen(output: &str) -> Result<Fingerprint, SshError> {
    for line in output.lines().filter(|l| !l.trim_start().starts_with('#')) {
        if let Some(token) = line.split_whitespace().find(|t| t.starts_with(PREFIX)) {
            return Fingerprint::new(token);
        }
    }
    Err(SshError::NoFingerprint)
}
