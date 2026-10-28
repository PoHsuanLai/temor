//! Input: a key event becomes a stroke, a stroke is routed to an action or to the program, and
//! pointer clicks are counted for word and line selection. The Dioxus adapters live in the
//! window (`view`); what is here is the mapping and the rules.

mod click;
mod keys;
mod route;
mod shortcut;

pub use click::{Click, ClickState, next_click};
pub use keys::{is_altgr_text, pressed, stroke, vt_mods};
pub use route::{FindBar, KittyKeys, Routed, Stroke, route, route_with};
pub use shortcut::terminal_shortcut_of;

#[cfg(test)]
mod tests;
