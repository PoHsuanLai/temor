//! Building temor's keymap: a source's bindings (the platform's conventions, later the
//! desktop's own settings), then the person's changes, then temor's actions registered with
//! their defaults (PLAN §6). Pure over its inputs: the platform and the source come from the
//! caller, and nothing here reads the environment or a file.

use chordkit::{
    AppId, Chord, Context, DefaultChord, Keymap, KeymapSource, Overrides, Platform, Registration,
};

use super::action::{APP, Action};
use super::declare::{Declared, declared};
use crate::error::SettingsError;

/// A problem with the keymap, with the settings line it came from (0 when no line is to blame).
pub type KeyIssue = (usize, SettingsError);

/// The keymap for `platform` from `source`, with `overrides` laid over it and temor's actions
/// registered. Every problem is returned and the rest applies: a source that cannot be read
/// leaves the platform's conventions, a refused chord leaves the action's default, and a
/// default another action holds leaves that one action unbound.
pub fn build_keymap(
    source: &dyn KeymapSource,
    platform: Platform,
    overrides: &Overrides,
) -> (Keymap, Vec<KeyIssue>) {
    let mut issues: Vec<KeyIssue> = Vec::new();
    let base = match source.load(platform) {
        Ok(keymap) => keymap,
        Err(err) => {
            issues.push((0, SettingsError::Key(err.to_string())));
            Keymap::conventional(platform)
        }
    };
    let (mut keymap, refused) = base.with_overrides(overrides);
    issues.extend(
        refused
            .into_iter()
            .map(|problem| (problem.line, SettingsError::Key(problem.what))),
    );
    issues.extend(register_declared(&mut keymap));
    (keymap, issues)
}

/// Registers temor's actions one at a time, so one refused default costs only its own action.
fn register_declared(keymap: &mut Keymap) -> Vec<KeyIssue> {
    let app = match AppId::new(APP) {
        Ok(app) => app,
        Err(err) => return vec![(0, SettingsError::Key(err.to_string()))],
    };
    let mut issues = Vec::new();
    for declared in declared() {
        match registration(&app, &declared) {
            Ok(Some(registration)) => {
                if let Err(conflict) = keymap.register_with(&registration) {
                    issues.push((0, SettingsError::Key(conflict.to_string())));
                }
            }
            Ok(None) => {}
            Err(err) => issues.push((0, err)),
        }
    }
    issues
}

/// One action's registration: its default (and fallback), and the standard actions it forgoes.
/// `None` for an action with no app id of temor's.
fn registration(app: &AppId, declared: &Declared) -> Result<Option<Registration>, SettingsError> {
    let Some(chordkit::Action::App(id)) = declared.action.to_chordkit() else {
        return Ok(None);
    };
    let parse = |text: &str| {
        text.parse::<DefaultChord>()
            .map_err(|err| SettingsError::Key(format!("{}: {err}", declared.action.label())))
    };
    let chord = parse(&declared.chord)?;
    let registration = Registration::new(app).forgo_all(declared.forgo.iter().copied());
    Ok(Some(match declared.fallback {
        Some(fallback) => registration.action_with_fallback(id, chord, parse(fallback)?),
        None => registration.action(id, chord),
    }))
}

/// The first shortcut that runs `action` in ordinary window content, written for the platform
/// (`⌘D` on a Mac or our desktop, `Ctrl+D` elsewhere), for menus and labels. `None` when the
/// action has no keymap entry or is unbound.
pub fn shortcut(keymap: &Keymap, action: Action) -> Option<String> {
    let named = action.to_chordkit()?;
    let chords: Vec<Chord> = keymap.chords_of(&named);
    chords.first().map(|chord| chord.display(keymap.platform()))
}

/// The shortcut for `action` as typed in a terminal window: what a person presses there
/// (`Ctrl+Shift+D` on Windows or KDE, where plain Ctrl is the program's), for settings help.
pub fn terminal_shortcut(keymap: &Keymap, action: Action) -> Option<String> {
    let named = action.to_chordkit()?;
    keymap
        .chords_in(&named, Context::Terminal)
        .first()
        .map(|chord| chord.display(keymap.platform()))
}
