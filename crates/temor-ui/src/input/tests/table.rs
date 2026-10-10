//! The key tables' rows and how one is routed: through the stroke and the router, with the
//! keymap a platform's conventions give.

use chordkit::{Desktop, Keymap, Overrides, Platform};
use dioxus::html::{Code, Key, Location, Modifiers as Held};
use ds::edit::raw_key::{KeyPhase, RawKey};
use temor_pane::keys::Action;
use temor_vt::Ime;

use super::super::keys::stroke;
use super::super::route::{FindBar, KittyKeys, Routed, route_with};
use crate::edge::keymap_for;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Went {
    Action(Action),
    Program,
    Dropped,
}

/// One key event and where it goes.
pub(super) struct Row {
    pub(super) name: &'static str,
    pub(super) key: Key,
    pub(super) code: Code,
    pub(super) held: Held,
    pub(super) unshifted: Option<&'static str>,
    pub(super) text: &'static str,
    pub(super) want: Went,
}

pub(super) fn ch(text: &str) -> Key {
    Key::Character(text.into())
}

pub(super) fn row(name: &'static str, key: Key, code: Code, held: Held, want: Went) -> Row {
    Row {
        name,
        key,
        code,
        held,
        unshifted: None,
        text: "",
        want,
    }
}

pub(super) fn went(keymap: &Keymap, row: &Row, find: FindBar) -> Went {
    went_with(keymap, row, find, KittyKeys::Off)
}

pub(super) fn went_with(keymap: &Keymap, row: &Row, find: FindBar, kitty: KittyKeys) -> Went {
    let raw = RawKey {
        phase: KeyPhase::Press,
        key: row.key.clone(),
        code: row.code,
        modifiers: row.held,
        location: Location::Standard,
        text: (!row.text.is_empty()).then(|| row.text.to_owned()),
        unshifted: row.unshifted.map(ch),
    };
    let stroke = stroke(&raw, Ime::Idle).unwrap_or_else(|| panic!("{}: no stroke", row.name));
    match route_with(stroke, keymap, find, kitty) {
        Routed::Action(action) => Went::Action(action),
        Routed::Shell(_) => Went::Program,
        Routed::Ignored => Went::Dropped,
    }
}

pub(super) fn keymap_of(platform: Platform) -> Keymap {
    keymap_for(platform, &Overrides::default()).0
}

pub(super) fn check(platform: Platform, find: FindBar, rows: &[Row]) {
    let keymap = keymap_of(platform);
    for row in rows {
        assert_eq!(
            went(&keymap, row, find),
            row.want,
            "{platform:?}: {}",
            row.name
        );
    }
}

pub(super) const OURS: Platform = Platform::Linux {
    desktop: Desktop::Ours,
};
pub(super) const KDE: Platform = Platform::Linux {
    desktop: Desktop::Kde,
};
