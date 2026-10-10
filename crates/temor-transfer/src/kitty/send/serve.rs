//! Answering a `receive` request: the terminal picks a file from the shelf and offers it.

use std::path::PathBuf;

use super::model::{SendEffect, SendFile, SendSession};
use crate::kitty::model::{
    Action, ErrorCode, FileId, KittyId, Message, Reply, ReplyStatus, UnixNanos,
};
use crate::name::FileName;
use crate::units::Bytes;

/// A file the shelf can hand out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub name: FileName,
    pub path: PathBuf,
    pub size: Bytes,
    pub modified: Option<UnixNanos>,
}

/// A request that found its file: read `path` and feed the chunks to `session`.
#[derive(Debug, Clone)]
pub struct Served {
    pub path: PathBuf,
    pub session: SendSession,
    pub first: Vec<SendEffect>,
}

/// Picks the file a `receive` request names (`n=`), or the newest when it names none, and starts
/// offering it under the request's id. `shelf` is newest first. When nothing fits, the reply to
/// send back is the error.
pub fn serve(request: &Message, shelf: &[Offered]) -> Result<Served, Reply> {
    let refuse = |code, message: &str| Reply {
        id: request.id.clone(),
        file: None::<FileId>,
        status: ReplyStatus::Failed {
            code,
            message: message.to_owned(),
        },
    };
    if request.action != Action::Receive {
        return Err(refuse(
            ErrorCode::Einval,
            "that is not a request for a file",
        ));
    }
    let found = match request.name.as_deref() {
        None => shelf.first(),
        Some(wanted) => shelf.iter().find(|offered| {
            offered.name.as_str() == wanted
                || offered
                    .path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy() == wanted)
        }),
    };
    let Some(offered) = found else {
        let message = match request.name {
            None => "there is nothing on the shelf",
            Some(_) => "no file with that name is on the shelf",
        };
        return Err(refuse(ErrorCode::Enoent, message));
    };
    let file = SendFile {
        name: offered.name.clone(),
        size: offered.size,
        modified: offered.modified,
    };
    let id: KittyId = request.id.clone();
    let (session, first) = SendSession::new(id, vec![file]);
    Ok(Served {
        path: offered.path.clone(),
        session,
        first,
    })
}
