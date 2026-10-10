use anyview_pane::PaneCommand;

use super::viewer_find::{Reply, entry_row, reply};

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
