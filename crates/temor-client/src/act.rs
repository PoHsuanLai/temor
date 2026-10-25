//! Changing what is kept: [`Command`]s, applied one at a time on the client's `temor-act` thread,
//! in the order they were given, so the window never waits on the store's writer. What a change
//! does to a value is heard as any commit is, through the values that read it.

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
    }
}

#[cfg(test)]
mod tests {
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
}
