//! Changing the person's key file (`keys.conf`, chordkit's line form), as pure functions over its
//! text: bind an action, reset one, reset all. Comments and lines that belong to no action are
//! kept as they are. The keymap in force says who holds a chord, so a conflict is refused and
//! names the action that has it (PLAN §6).

use chordkit::{Action as Named, Context, DefaultChord, Keymap, OverrideEntry, Overrides};

use super::action::Action;
use crate::error::SettingsError;

/// `text` with `action` on `chord` alone (written `Primary+Alt+N`), its older lines gone.
///
/// Refused, with the file untouched, when `chord` is not a chord, is kept by the system, or is
/// already the shortcut of another action in `keymap` (the keymap in force): the error names that
/// action. An action the keymap does not know cannot be bound.
pub fn bind_key(
    text: &str,
    keymap: &Keymap,
    action: Action,
    chord: &str,
) -> Result<String, SettingsError> {
    let named = action
        .to_chordkit()
        .ok_or_else(|| SettingsError::KeyNotBindable(action.label().to_owned()))?;
    let wanted: DefaultChord = chord
        .trim()
        .parse()
        .map_err(|err| SettingsError::Key(format!("{chord:?} is not a shortcut: {err}")))?;
    let platform = keymap.platform();
    for context in [Context::Terminal, Context::Normal] {
        let Some(concrete) = wanted.resolve(platform, context) else {
            continue;
        };
        let holder = keymap
            .bindings(context)
            .iter()
            .find(|pair| pair.1 == concrete && pair.0 != named);
        if let Some(pair) = holder {
            return Err(SettingsError::KeyHeld {
                chord: concrete.display(platform),
                held_by: holder_name(&pair.0),
            });
        }
    }
    let entry = OverrideEntry {
        line: 0,
        action: named.clone(),
        chords: vec![wanted],
    };
    let (_, problems) = keymap
        .clone()
        .with_overrides(&Overrides::from_entries([entry]));
    if let Some(problem) = problems.into_iter().next() {
        return Err(SettingsError::Key(problem.what));
    }
    let mut kept = without(text, &named.id());
    let line = format!("{} = {}\n", named.id(), wanted.canonical());
    kept.push_str(&line);
    Ok(kept)
}

/// `text` without the lines that change `action`: it answers to its own key again.
pub fn reset_key(text: &str, action: Action) -> String {
    match action.to_chordkit() {
        Some(named) => without(text, &named.id()),
        None => text.to_owned(),
    }
}

/// `text` without any line that changes a key: every action answers to its own key again.
/// Comments, blank lines and lines no action owns stay.
pub fn reset_all_keys(text: &str) -> String {
    kept_lines(text, |key| !is_key_action(key))
}

/// Whether `key` (the left side of a key line) names a standard action or one of temor's own.
/// Another app's action id is not one: it is reported as unknown.
pub fn is_key_action(key: &str) -> bool {
    match key.parse::<Named>() {
        Ok(Named::Standard(_)) => true,
        Ok(other) => Action::from_chordkit(&other).is_some(),
        Err(_) => false,
    }
}

/// The lines of `text` except those that set the action with id `id`.
fn without(text: &str, id: &str) -> String {
    kept_lines(text, |key| key != id)
}

/// The lines of `text` whose left side of `=` (when they have one) satisfies `keep`, each ended
/// by a newline.
fn kept_lines(text: &str, keep: impl Fn(&str) -> bool) -> String {
    text.lines()
        .filter(|line| match line.split_once('=') {
            Some((key, _)) if !key.trim_start().starts_with('#') => keep(key.trim()),
            _ => true,
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

/// What a person reads for the action that holds a chord.
fn holder_name(held: &Named) -> String {
    match Action::from_chordkit(held) {
        Some(ours) => ours.label().to_owned(),
        None => match held {
            Named::Standard(standard) => standard.label(),
            other => other.id(),
        },
    }
}
