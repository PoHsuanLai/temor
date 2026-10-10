//! Symbolic links the remote sends (`ft=symlink`): the link's target arrives as the file's data.
//! A link is made only when the session makes links at all and the target stays inside the
//! transfer's folder: relative, and never up past the folder's top. The folder is the transfer's
//! own, and writes never go through a link (the edge refuses to), so a link can only point at
//! what the transfer itself put there.

use super::helpers::{refuse, reply};
use super::model::{Effect, OpenLink, ReceiveSession, WriteOp};
use crate::kitty::model::{ErrorCode, FileId, KittyId, Message, ReplyStatus};
use crate::name::RelPath;

/// The longest link target kept, in bytes.
const MAX_TARGET: usize = 4096;

/// Whether a data frame carries the last of a link's target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Last {
    No,
    Yes,
}

/// A link's header: its target follows as data.
pub(super) fn open_link(
    session: ReceiveSession,
    id: &KittyId,
    file_id: FileId,
    path: RelPath,
) -> (ReceiveSession, Vec<Effect>) {
    let mut next = session;
    next.entries += 1;
    next.claimed.insert(path.clone());
    next.open_links.insert(
        file_id.clone(),
        OpenLink {
            path,
            target: Vec::new(),
        },
    );
    let started = reply(id, Some(file_id), ReplyStatus::Started);
    (next, vec![started])
}

/// A chunk of a link's target. The last one makes the link, or refuses it.
pub(super) fn on_link_data(
    session: ReceiveSession,
    id: &KittyId,
    file_id: FileId,
    message: Message,
    last: Last,
) -> (ReceiveSession, Vec<Effect>) {
    let mut next = session;
    let Some(mut link) = next.open_links.remove(&file_id) else {
        return refuse(
            next,
            id,
            Some(file_id),
            ErrorCode::Enoent,
            "the link is not open",
        );
    };
    if link.target.len() + message.data.len() > MAX_TARGET {
        return refuse(
            next,
            id,
            Some(file_id),
            ErrorCode::Einval,
            "the link's target is too long",
        );
    }
    link.target.extend_from_slice(&message.data);
    if last == Last::No {
        next.open_links.insert(file_id, link);
        return (next, Vec::new());
    }
    match link_target(&link.path, &link.target) {
        Some(target) => {
            let make = Effect::Write(WriteOp::Symlink {
                path: link.path,
                target,
            });
            (next, vec![make, reply(id, Some(file_id), ReplyStatus::Ok)])
        }
        None => refuse(
            next,
            id,
            Some(file_id),
            ErrorCode::Eperm,
            "the link points outside the folder",
        ),
    }
}

/// The target as text (without kitty's `path:` label) if a link at `link` pointing at it stays inside the folder: not empty,
/// relative, no control characters, and no `..` that climbs past the top.
pub(super) fn link_target(link: &RelPath, raw: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(raw).ok()?;
    // Kitty's sender says what the target is: `path:` is the link's own target text; `fid:` and
    // `fid_abs:` name another file of the transfer by id, which is not followed here.
    let text = text.strip_prefix("path:").unwrap_or(text);
    if text.starts_with("fid:") || text.starts_with("fid_abs:") {
        return None;
    }
    if text.is_empty() || text.starts_with('/') || text.chars().any(char::is_control) {
        return None;
    }
    // How many folders deep the link sits.
    let mut depth = link.components().len().saturating_sub(1);
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => depth = depth.checked_sub(1)?,
            _ => depth += 1,
        }
    }
    Some(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_stays_inside_the_folder_or_is_refused() {
        let rel = |p: &str| RelPath::parse(p).expect("a path");
        #[rustfmt::skip]
        let cases: &[(&str, &str, &[u8], bool)] = &[
            ("a sibling", "a/link", b"file", true),
            ("one up from a subfolder", "a/link", b"../b", true),
            ("up past the top from a subfolder", "a/link", b"../../b", false),
            ("up from the top", "link", b"../b", false),
            ("up and back down", "a/b/link", b"../../a/c", true),
            ("absolute", "link", b"/etc/passwd", false),
            ("empty", "link", b"", false),
            ("a control character", "link", b"a\nb", false),
            ("not text", "link", b"\xff\xfe", false),
            ("dots only", "a/link", b".", true),
            ("kitty's path label", "a/link", b"path:../b", true),
            ("a target by file id", "a/link", b"fid:3", false),
            ("a target by file id, absolute", "a/link", b"fid_abs:3", false),
            ("sneaking up after going down", "link", b"a/../../b", false),
        ];
        for (name, at, target, ok) in cases {
            assert_eq!(link_target(&rel(at), target).is_some(), *ok, "{name}");
        }
    }
}
