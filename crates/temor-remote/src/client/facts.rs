//! The request for a session's folder facts.

use temor_wire::ids::RemoteSessionId;
use temor_wire::message::{FolderFacts, Reply, Request};

use super::connect::Client;
use crate::error::Error;

impl Client {
    /// What the helper reads about the folder of `session` for the chips: git's text and the
    /// version files. A helper that does not offer it is [`Error::NotOffered`].
    pub fn folder_facts(&self, session: RemoteSessionId) -> Result<FolderFacts, Error> {
        match self.ask(Request::Facts { session })? {
            Reply::Facts(facts) => Ok(facts),
            _ => Err(Error::UnexpectedReply),
        }
    }
}
