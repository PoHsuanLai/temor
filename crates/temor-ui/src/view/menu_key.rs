//! A menu row's key equivalent, when the action has one.

use ds::components::menus::item::item::MenuItem;
use ds_core::vocab::Shortcut;

/// `item` with `key` as its key equivalent, or as it is when there is none.
pub fn with_key_opt<T>(item: MenuItem<T>, key: Option<Shortcut>) -> MenuItem<T> {
    match key {
        Some(key) => item.with_key(key),
        None => item,
    }
}
