//! The window's base (PLAN §2b): what the window's state and its views both stand on. How it is
//! launched and what it is given (fonts, launcher, clipboard, the desktop's link), the settings
//! file and its document, the edges it reads the system through, a pane's slot of window state
//! with the block list's layout and tiles, the window's look, and the words the state and the
//! views share. Neither `temor-hub` nor `temor-views` is named here.

pub mod card_colours;
pub mod desktop;
pub mod edge;
pub mod error;
pub mod launch;
pub mod pop_out;
pub mod probe;
pub mod remote_launch;
pub mod run;
pub mod segments;
pub mod settings;
pub mod shelf_words;
pub mod sidebar_room;
pub mod slot;
pub mod space_handle;
pub mod theme;
pub mod tiles;
pub mod title;
