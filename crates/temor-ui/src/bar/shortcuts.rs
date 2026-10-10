//! Which of the bar's rows are also a pane action with a chord, so the row can show it.

use temor_core::bar::{BarAction, Target};
use temor_pane::keys::Action;

/// The pane action a row does, when there is one.
pub fn action_of(row: &BarAction) -> Option<Action> {
    match row {
        BarAction::NewTab => Some(Action::NewTab),
        BarAction::Go(Target::Closed(_)) => Some(Action::ReopenTab),
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
    use temor_core::vocab::ids::Seq;
    use temor_core::vocab::text::CommandText;

    #[test]
    fn only_the_rows_that_are_a_pane_action_have_one() {
        let rows = [
            (BarAction::NewTab, Some(Action::NewTab)),
            (
                BarAction::Go(Target::Closed(Seq(3))),
                Some(Action::ReopenTab),
            ),
            (BarAction::Run(CommandText("ls".to_owned())), None),
        ];
        for (row, want) in rows {
            assert_eq!(action_of(&row), want, "{row:?}");
        }
    }
}
