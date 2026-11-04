use anyview_pane::PaneCommand;
use ds::prelude::PaletteClose;

use super::viewer_find::{Escape, Reply, entry_row, escape, reply};

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
fn escape_steps_back_from_the_files_find_and_then_closes_but_a_click_outside_closes_at_once() {
    #[rustfmt::skip]
    let cases = [
        (true,  PaletteClose::Escape,  Escape::StepBack),
        (false, PaletteClose::Escape,  Escape::Close),
        (true,  PaletteClose::Pick,    Escape::StepBack),
        (false, PaletteClose::Pick,    Escape::Close),
        (true,  PaletteClose::Outside, Escape::Close),
        (false, PaletteClose::Outside, Escape::Close),
    ];
    for (finding, how, then) in cases {
        assert_eq!(escape(finding, how), then, "{finding:?}, {how:?}");
    }
}
