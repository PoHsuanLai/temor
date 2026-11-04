//! Which of the bar's rows are also a pane action with a chord, so the row can show it.

use temor_client::model::bar::{BarAction, Target};
use temor_panes::pane::keys::Action;

/// The pane action a row does, when there is one.
pub fn action_of(row: &BarAction) -> Option<Action> {
    match row {
        BarAction::NewTab => Some(Action::NewTab),
        BarAction::Go(Target::Closed(_)) => Some(Action::ReopenTab),
        BarAction::Go(Target::Action(action)) => Some(*action),
        BarAction::Go(_)
        | BarAction::Run(_)
        | BarAction::Plugin { .. }
        | BarAction::Suggest(_)
        | BarAction::AskAgent(_)
        | BarAction::AskAbout(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_client::model::vocab::ids::Seq;
    use temor_client::model::vocab::text::CommandText;

    #[test]
    fn only_the_rows_that_are_a_pane_action_have_one() {
        let rows = [
            (BarAction::NewTab, Some(Action::NewTab)),
            (
                BarAction::Go(Target::Closed(Seq(3))),
                Some(Action::ReopenTab),
            ),
            (
                BarAction::Go(Target::Action(Action::SplitRight)),
                Some(Action::SplitRight),
            ),
            (BarAction::Run(CommandText("ls".to_owned())), None),
        ];
        for (row, want) in rows {
            assert_eq!(action_of(&row), want, "{row:?}");
        }
    }
}
