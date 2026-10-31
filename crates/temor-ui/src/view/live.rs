//! The client as a window holds it (PLAN §2b): one for the process, and its values as signals.
//!
//! A value the client keeps read ([`temor_client::Live`]) becomes a signal here, set from a task
//! that follows it. Rendering reads the signal and nothing else, so no frame waits on the store,
//! and a component draws again when its value changes. Ported from mailo's `ui/live.rs`.

use std::path::Path;
use std::sync::Arc;

use dioxus::dioxus_core::{Task, provide_root_context};
use dioxus::prelude::*;
use temor_client::{Client, ClientConfig, Live, Query, Snapshot};

/// The process's client: over `temor.db` in `state_dir`, or with no state folder (or a database
/// that cannot be opened, which is said), over an empty one in memory that keeps nothing.
pub(crate) fn open_client(state_dir: Option<&Path>) -> Client {
    match state_dir.map(|dir| Client::open_in(dir, ClientConfig::default())) {
        Some(Ok(client)) => client,
        Some(Err(why)) => {
            eprintln!("temor: history is not kept this time: {why}");
            Client::in_memory()
        }
        None => Client::in_memory(),
    }
}

/// The client a window keeps through: the program's, or for a window built without one (a
/// test's), one over `temor.db` in its own state folder, so what it keeps is there when a window
/// opens on that folder again. It is shared from the root, as [`client`] gives it.
pub(crate) fn window_client(state_dir: Option<&Path>) -> Client {
    if let Some(client) = try_consume_context::<Client>() {
        return client;
    }
    let opened =
        state_dir.and_then(|dir| Client::open_in(dir, ClientConfig { external: false }).ok());
    provide_root_context(opened.unwrap_or_else(Client::in_memory))
}

/// The app's client: the one the program gave every window, or for a window built without one
/// (a unit test's), one in memory that reads on the thread that draws, made once and shared from
/// the root.
pub(crate) fn use_client() -> Client {
    use_hook(client)
}

/// [`use_client`], from a handler or a task.
pub(crate) fn client() -> Client {
    match try_consume_context::<Client>() {
        Some(client) => client,
        None => provide_root_context(Client::in_memory()),
    }
}

/// The value `key` asks for, as a signal. `key` is a memo: it follows the signals it reads, and
/// an equal query is not asked again. While a new query is read the value shows the last one's,
/// so a change of folder does not flash empty.
pub(crate) fn use_live<Q: Query>(key: impl FnMut() -> Q + 'static) -> Memo<Snapshot<Q::Output>> {
    let client = use_client();
    let query = use_memo(key);
    let held = use_memo(move || Held(client.live(query())));
    // Moved by the task that follows the value, so the frame reads it again.
    let mut told = use_signal(|| 0u64);
    let mut last = use_hook(|| CopyValue::new(None::<Arc<Q::Output>>));
    let out = use_memo(move || {
        let _ = told();
        let now = held.read().0.snapshot().showing(last.peek().clone());
        if let Some(value) = now.value() {
            last.set(Some(value.clone()));
        }
        now
    });
    let mut following = use_hook(|| CopyValue::new(None::<Task>));
    use_effect(move || {
        let mut live = held.read().0.clone();
        if let Some(task) = following.take() {
            task.cancel();
        }
        following.set(Some(spawn(async move {
            while live.changed().await {
                told += 1;
            }
        })));
    });
    out
}

/// A held value, as a memo keeps it: a new one only when the query changed, and never equal to
/// the last, so what follows it moves on to it.
struct Held<T>(Live<T>);

impl<T> PartialEq for Held<T> {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use temor_client::Shelf;
    use temor_client::query::OnShelf;

    use super::*;

    thread_local! {
        static SEEN: RefCell<Option<usize>> = const { RefCell::new(None) };
    }

    #[allow(non_snake_case)]
    fn Pins() -> Element {
        let pins = use_live(|| OnShelf(Shelf::Pins));
        let count = pins.read().value().map(|pins| pins.len());
        SEEN.with(|seen| *seen.borrow_mut() = count);
        rsx! {}
    }

    #[test]
    fn a_window_with_no_client_reads_an_empty_one_on_its_first_frame() {
        let mut dom = VirtualDom::new(Pins);
        dom.rebuild_in_place();
        assert_eq!(SEEN.with(|seen| seen.borrow_mut().take()), Some(0));
    }

    #[test]
    fn a_state_folder_keeps_the_database_and_none_keeps_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        let kept = open_client(Some(&dir.path().join("state")));
        assert!(kept.file().is_some());
        assert!(
            dir.path()
                .join("state")
                .join(temor_client::DB_FILE)
                .exists()
        );
        assert!(open_client(None).file().is_none());
    }
}
