use super::{Rewind, rewind_of};

#[test]
fn only_agents_that_keep_their_own_history_are_left_to_it() {
    let cases = [
        ("claude", Rewind::Own),
        ("Claude", Rewind::Own),
        ("codex", Rewind::Docket),
        ("aider", Rewind::Docket),
        ("something-new", Rewind::Docket),
    ];
    for (program, want) in cases {
        assert_eq!(rewind_of(program), want, "{program}");
    }
}
