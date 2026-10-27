use ds_style::space::list::{SpaceId, Spaces};
use temor_core::error::CoreError;
use temor_core::space::Project;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::WorkspaceIn;

use super::Book;

/// A book of `n` Spaces, the first on screen, with one tab there.
fn book_of(n: usize) -> Book {
    let mut book = Book::new();
    book.begin(Spaces::first_run(
        (0..n).map(|_| Project::default()),
        Project::default,
    ));
    book.step(WorkspaceIn::NewTab { cwd: None });
    book
}

fn panes(book: &Book) -> Vec<PaneId> {
    book.ws.tabs.panes()
}

#[test]
fn a_pane_opened_in_another_space_takes_an_id_past_every_space() {
    let mut book = book_of(2);
    book.step(WorkspaceIn::NewTab { cwd: None });
    let first = panes(&book);
    book.swap_to(SpaceId(1));
    book.step(WorkspaceIn::NewTab { cwd: None });
    let there = panes(&book);
    assert_eq!(there.len(), 1);
    assert!(
        first.iter().all(|pane| pane.0 < there[0].0),
        "{first:?} {there:?}"
    );
}

#[test]
fn a_space_put_away_comes_back_as_it_was() {
    let mut book = book_of(2);
    let here = panes(&book);
    book.swap_to(SpaceId(1));
    assert!(panes(&book).is_empty(), "a Space never shown has no tab");
    assert_eq!(book.active(), SpaceId(1));
    book.swap_to(SpaceId(0));
    assert_eq!(panes(&book), here);
}

#[test]
fn a_request_about_a_parked_pane_names_the_space_that_has_it() {
    let mut book = book_of(2);
    let pane = panes(&book)[0];
    assert_eq!(book.parked_owner_of(&WorkspaceIn::Focus(pane)), None);
    book.swap_to(SpaceId(1));
    assert_eq!(
        book.parked_owner_of(&WorkspaceIn::Focus(pane)),
        Some(SpaceId(0))
    );
    assert_eq!(book.owner_of(pane), Some(SpaceId(0)));
    assert_eq!(book.parked_owner_of(&WorkspaceIn::Reopen), None);
}

#[test]
fn a_tab_carried_to_another_space_leaves_this_one() {
    let mut book = book_of(2);
    book.step(WorkspaceIn::NewTab { cwd: None });
    let tab = book.ws.front().expect("a tab in front");
    let before = panes(&book);
    assert_eq!(
        book.carry(tab, SpaceId(0)),
        Ok(false),
        "it is there already"
    );
    assert_eq!(book.carry(tab, SpaceId(9)), Err(CoreError::UnknownSpace(9)));
    assert_eq!(book.carry(tab, SpaceId(1)), Ok(true));
    let after = panes(&book);
    assert_eq!(after.len(), 1);
    let moved: Vec<PaneId> = before.into_iter().filter(|p| !after.contains(p)).collect();
    assert_eq!(moved.len(), 1);
    assert_eq!(book.owner_of(moved[0]), Some(SpaceId(1)));
}

#[test]
fn a_pane_a_tab_has_is_not_adopted_again() {
    let mut book = book_of(2);
    let pane = panes(&book)[0];
    assert_eq!(
        book.adopt(SpaceId(1), pane, None).map(|_| ()),
        Err(CoreError::DuplicatePane(pane.0))
    );
    let loose = PaneId(40);
    book.adopt(SpaceId(1), loose, None).expect("adopted");
    assert_eq!(book.owner_of(loose), Some(SpaceId(1)));
    assert_eq!(book.active(), SpaceId(0), "the Space shown stays");
}

#[test]
fn a_deleted_space_with_tabs_is_answered_for_its_panes_to_end() {
    let mut book = book_of(3);
    book.swap_to(SpaceId(1));
    book.step(WorkspaceIn::NewTab { cwd: None });
    book.swap_to(SpaceId(2));
    let mut all = book.spaces().clone();
    all.remove(SpaceId(1))
        .expect("a Space not on screen can go");
    assert_eq!(book.follow(&all), vec![SpaceId(1)]);
    assert!(book.unpark(SpaceId(1)).is_some());
    assert_eq!(book.parked().count(), 1, "the first Space stays");
}

#[test]
fn the_kept_spaces_recall_where_each_space_is() {
    let mut book = book_of(2);
    book.swap_to(SpaceId(1));
    let kept = book.for_keeping();
    assert_eq!(kept.recall().of(SpaceId(0)).tabs.len(), 1);
    assert!(kept.recall().of(SpaceId(1)).tabs.is_empty());
}
