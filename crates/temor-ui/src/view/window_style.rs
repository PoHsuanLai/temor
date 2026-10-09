//! The window's own sheet: the column and the terminal region's sizing. quire has no column
//! component at v0.3.1, and a `RawKeySurface` takes no style of its own. Colours and type come
//! from quire's tokens and the pane's palette, so this sheet has no design values.

pub const WINDOW_CSS: &str = "\
.temor-window { display: flex; flex-direction: column; width: 100%; height: 100%; }\
.temor-keys { flex: 1 1 auto; min-height: 0; position: relative; overflow: hidden; }\
.temor-grid { position: relative; width: 100%; height: 100%; }\
.temor-preedit { position: absolute; left: 0; bottom: 0; white-space: pre; text-decoration: underline; }";
