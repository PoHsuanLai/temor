/// Who keeps the history of an agent's file changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rewind {
    /// docket saves a restore point before each turn.
    Docket,
    /// The agent keeps its own history; docket saves nothing and says so.
    Own,
}

/// Programs that keep their own history (lowercase, exact process names).
const KEEPS_OWN: &[&str] = &["claude"];

/// Who keeps the history for the agent program `program`. An unknown program is `Docket`, which
/// over-saves rather than under-saves.
pub fn rewind_of(program: &str) -> Rewind {
    let program = program.to_ascii_lowercase();
    match KEEPS_OWN.contains(&program.as_str()) {
        true => Rewind::Own,
        false => Rewind::Docket,
    }
}
