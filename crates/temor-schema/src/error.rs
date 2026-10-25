//! What a value of the schema refuses to be.

/// Why text could not become a schema value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaError {
    /// An environment variable's name was empty, or held `=` or NUL.
    #[error("not an environment variable name: {0:?}")]
    BadEnvKey(String),
    /// Text that is not a `temor://` URL: the reason, and the text.
    #[error("not a temor URL ({why}): {text:?}")]
    BadUrl { why: &'static str, text: String },
    /// Text that is not a session's id.
    #[error("not a session id: {0:?}")]
    BadSessionId(String),
}
