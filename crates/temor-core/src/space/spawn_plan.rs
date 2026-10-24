//! What a shell starts with when it carries state from another: the Space's variables are the
//! base layer, and what the person changed in the old shell lies over it. Pure; the edge turns
//! the plan into a spawn, and supplies the new Space's secrets from the account store (a
//! carried secret is never copied: `Replay::withheld` names it).
//!
//! A carried variable is the Space's own while it still holds the value the old Space gave it,
//! so moving to another Space swaps those for the new Space's values (or drops them when the new
//! Space does not set them), and keeps what the person set themselves.

use std::collections::BTreeMap;

use crate::session::{Replay, never_carried};

/// Where a variable of a new shell comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The Space on screen sets it.
    Space,
    /// The person set it in the shell they came from.
    User,
}

/// A new shell's start: folder, variables with their origin, aliases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnPlan {
    pub cwd: Option<String>,
    /// Sorted by name.
    pub env: Vec<(String, String, Origin)>,
    pub aliases: Vec<(String, String)>,
    /// Names of secrets of the old shell that were not copied.
    pub withheld: Vec<String>,
}

/// The plan for a shell in a Space whose own start is `new_base` (and `new_cwd`), carrying the
/// state of a shell that started from `old_base`. With nothing carried it is the Space's start.
pub fn spawn_plan(
    old_base: &[(String, String)],
    new_base: &[(String, String)],
    new_cwd: Option<&str>,
    carried: Option<&Replay>,
) -> SpawnPlan {
    let old: BTreeMap<&str, &str> = old_base
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut env: BTreeMap<String, (String, Origin)> = new_base
        .iter()
        .map(|(k, v)| (k.clone(), (v.clone(), Origin::Space)))
        .collect();
    let Some(carried) = carried else {
        return SpawnPlan {
            cwd: new_cwd.map(str::to_owned),
            env: env.into_iter().map(|(k, (v, o))| (k, v, o)).collect(),
            ..SpawnPlan::default()
        };
    };
    for (key, value) in &carried.env {
        // What names the old session or loads code is the new shell's own to set.
        if never_carried(key) {
            continue;
        }
        // Still what the old Space gave it: the new Space's value (set above) stands.
        if old.get(key.as_str()) == Some(&value.as_str()) {
            continue;
        }
        env.insert(key.clone(), (value.clone(), Origin::User));
    }
    SpawnPlan {
        cwd: carried.cwd.clone().or_else(|| new_cwd.map(str::to_owned)),
        env: env.into_iter().map(|(k, (v, o))| (k, v, o)).collect(),
        aliases: carried.aliases.clone(),
        withheld: carried.withheld.clone(),
    }
}

#[cfg(test)]
#[path = "spawn_plan_tests.rs"]
mod tests;
