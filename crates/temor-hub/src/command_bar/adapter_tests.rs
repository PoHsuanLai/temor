//! The bar on the palette: the rows go in, the highlight is the palette's, a pick comes out.

use temor_client::model::bar::{BarAction, BarContext, Candidate, PathResolver, RowKey, Target};
use temor_client::model::vocab::ai::AiSwitch;
use temor_client::model::vocab::ids::TabId;

use super::BarModel;

struct OnPath;
impl PathResolver for OnPath {
    fn resolves(&self, program: &str) -> bool {
        program == "htop"
    }
}

fn tabs() -> Vec<Candidate> {
    (1..=3)
        .map(|n| Candidate {
            title: format!("logs {n}"),
            target: Target::Tab(TabId(n)),
        })
        .collect()
}

fn keys(model: &BarModel) -> Vec<&str> {
    model.rows().iter().map(|row| row.key.0.as_str()).collect()
}

#[test]
fn typing_filters_in_the_fixed_order_and_the_palette_owns_the_highlight() {
    let candidates = tabs();
    let cx = BarContext {
        candidates: &candidates,
        resolver: &OnPath,
        ai: AiSwitch::Off,
    };
    let model = BarModel::default().toggled(cx);
    assert!(model.is_open());
    assert_eq!(
        keys(&model)[0],
        "new-tab",
        "an empty bar leads with a plain tab"
    );

    let model = model.typed("logs".to_owned(), cx);
    assert_eq!(keys(&model), ["tab:1", "tab:2", "tab:3"]);
    assert_eq!(model.selected(), 0, "a new query starts at the top");

    // Down moves the palette's highlight and stops at the last row.
    let model = model.selecting(2).selecting(9);
    assert_eq!(model.selected(), 2);
    // Rows the window lists after the bar's own (a viewer's commands) extend where it can go.
    assert_eq!(model.clone().selecting_among(9, 2).selected(), 4);

    let model = model.typed("htop -d 2".to_owned(), cx);
    assert_eq!(
        keys(&model),
        ["run"],
        "the Run row, since htop is on the path"
    );
    assert_eq!(model.selected(), 0);
}

#[test]
fn a_picked_row_comes_back_once_and_closes_the_bar() {
    let candidates = tabs();
    let cx = BarContext {
        candidates: &candidates,
        resolver: &OnPath,
        ai: AiSwitch::Off,
    };
    let model = BarModel::default().toggled(cx).typed("logs".to_owned(), cx);
    let (still_open, none) = model.clone().picked(&RowKey("tab:9".to_owned()), cx);
    assert_eq!((still_open.is_open(), none), (true, None));

    let (done, row) = model.picked(&RowKey("tab:2".to_owned()), cx);
    assert_eq!(
        row.map(|row| row.action),
        Some(BarAction::Go(Target::Tab(TabId(2))))
    );
    assert!(!done.is_open());
    assert_eq!(done.rows().len(), 0);
}

#[test]
fn closing_and_opening_again_starts_empty() {
    let candidates = tabs();
    let cx = BarContext {
        candidates: &candidates,
        resolver: &OnPath,
        ai: AiSwitch::Off,
    };
    let model = BarModel::default()
        .toggled(cx)
        .typed("logs".to_owned(), cx)
        .selecting(1)
        .closed(cx);
    assert!(!model.is_open());
    let again = model.toggled(cx);
    assert_eq!((again.query(), again.selected()), ("", 0));
    assert!(!again.toggled(cx).is_open(), "the same chord closes it");
}
