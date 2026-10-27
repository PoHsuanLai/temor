//! Changing what is kept: [`Command`]s, applied one at a time on the client's `temor-act` thread,
//! in the order they were given, so the window never waits on the store's writer. What a change
//! does to a value is heard as any commit is, through the values that read it.

use temor_schema::secret::kept_in_history;
use temor_schema::session::{At, RanCommand, Session, SessionId};
use temor_schema::url::TemorUrl;
use temor_store::{Shelf, SqliteStore, StoreError};

/// A change to what is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Put `place` on `shelf` at `at` (the end when `None`, or when `at` is past it). A place
    /// already there moves.
    Keep {
        shelf: Shelf,
        place: TemorUrl,
        at: Option<usize>,
    },
    /// Take `place` off `shelf`.
    Let { shelf: Shelf, place: TemorUrl },
    /// A session began, or what is known of it changed.
    Begin(Session),
    /// A session ended at `at`.
    End { session: SessionId, at: At },
    /// A command a session ran, or how it ended. One history may not keep (typed after a space,
    /// or holding a secret) is dropped here, so no caller can write one.
    Ran(RanCommand),
    /// Forget the history from before `at` (PLAN §2b's retention).
    ForgetBefore(At),
}

/// Apply `command` to `store`.
pub(crate) fn apply(store: &SqliteStore, command: Command) -> Result<(), StoreError> {
    match command {
        Command::Keep { shelf, place, at } => {
            let mut places = store.shelf(shelf)?;
            places.retain(|kept| *kept != place);
            let at = at.unwrap_or(places.len()).min(places.len());
            places.insert(at, place);
            store.set_shelf(shelf, &places)
        }
        Command::Let { shelf, place } => {
            let mut places = store.shelf(shelf)?;
            let before = places.len();
            places.retain(|kept| *kept != place);
            match places.len() == before {
                true => Ok(()),
                false => store.set_shelf(shelf, &places),
            }
        }
        Command::Begin(session) => store.save_session(&session),
        Command::End { session, at } => store.end_session(session, at),
        Command::Ran(ran) => match kept_in_history(&ran.command) {
            true => store.record_command(&ran),
            false => Ok(()),
        },
        Command::ForgetBefore(at) => store.forget_before(at).map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use temor_schema::ids::BlockId;
    use temor_schema::url::{Host, Location};

    use super::*;

    fn place(path: &str) -> TemorUrl {
        TemorUrl::Location(Location {
            host: Host::Local,
            path: path.to_owned(),
            within: None,
        })
    }

    fn keep(at: Option<usize>, path: &str) -> Command {
        Command::Keep {
            shelf: Shelf::Favourites,
            place: place(path),
            at,
        }
    }

    #[test]
    fn a_place_is_kept_where_asked_once_and_let_go() {
        let store = SqliteStore::in_memory().expect("store");
        for command in [
            keep(None, "/a/"),
            keep(None, "/b/"),
            keep(Some(0), "/c/"),
            keep(Some(9), "/a/"),
        ] {
            apply(&store, command).expect("apply");
        }
        let kept = || store.shelf(Shelf::Favourites).expect("read");
        assert_eq!(kept(), [place("/c/"), place("/b/"), place("/a/")]);
        apply(
            &store,
            Command::Let {
                shelf: Shelf::Favourites,
                place: place("/b/"),
            },
        )
        .expect("let");
        assert_eq!(kept(), [place("/c/"), place("/a/")]);
    }

    fn ran(block: u64, command: &str) -> Command {
        Command::Ran(RanCommand {
            session: SessionId::from_bits(7),
            block: BlockId(block),
            command: command.to_owned(),
            folder: "/src".to_owned(),
            exit: Some(0),
            started: At(1_000),
            ended: Some(At(2_000)),
        })
    }

    #[test]
    fn a_session_and_its_commands_are_kept_but_never_a_secret_or_a_spaced_command() {
        let store = SqliteStore::in_memory().expect("store");
        let session = Session {
            id: SessionId::from_bits(7),
            host: Host::Local,
            folder: "/src".to_owned(),
            program: None,
            started: At(500),
            ended: None,
        };
        for command in [
            Command::Begin(session.clone()),
            ran(1, "cargo build"),
            ran(2, " echo hidden"),
            ran(3, "export GITHUB_TOKEN=abc"),
            ran(4, "ls"),
            Command::End {
                session: session.id,
                at: At(9_000),
            },
        ] {
            apply(&store, command).expect("apply");
        }
        let kept: Vec<String> = store
            .commands_of(session.id)
            .expect("commands")
            .into_iter()
            .map(|ran| ran.command)
            .collect();
        assert_eq!(kept, ["cargo build", "ls"]);
        let ended = store.session(session.id).expect("read").expect("kept");
        assert_eq!(ended.ended, Some(At(9_000)));
        apply(&store, Command::ForgetBefore(At(1_500))).expect("forget");
        assert!(store.commands_of(session.id).expect("commands").is_empty());
    }
}
