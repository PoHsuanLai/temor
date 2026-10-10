use anyview_pane::PaneCommand;

use super::viewer_find::{Escape, Finding, Reply, entry_row, escape, reply};

#[test]
fn the_entry_row_shows_for_a_query_that_names_it_and_a_pick_is_answered() {
    let cases: [(&str, bool); 5] = [
        ("", true),
        ("find", true),
        ("  IN this", true),
        ("file", true),
        ("zoom", false),
    ];
    for (query, shown) in cases {
        assert_eq!(entry_row((), query).is_some(), shown, "query {query:?}");
    }
    assert_eq!(reply(&PaneCommand::ShowAllHits), Reply::ListAll);
    assert_eq!(reply(&PaneCommand::OpenFile), Reply::Go);
}

#[test]
fn escape_steps_back_from_the_files_find_and_then_closes() {
    assert_eq!(escape(Finding::On), Escape::StepBack);
    assert_eq!(escape(Finding::Off), Escape::Close);
}
