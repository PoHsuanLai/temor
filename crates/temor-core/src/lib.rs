//! The base of temor's pure rules (PLAN.md §2): the vocabulary and the small rules the others
//! share (addresses, menus, icons, facts, the viewer, history's days, a pane's kind). No I/O, no
//! threads, no UI and no foreign code. A session and its blocks are `temor-blocks`, the layout
//! `temor-layout`, other computers `temor-hosts` and agents `temor-agent`, each above this one.

pub mod address;
pub mod error;
pub mod facts;
pub mod folder;
pub mod history;
pub mod icon;
pub mod menu;
pub mod pane;
pub mod viewer;
pub mod vocab;
