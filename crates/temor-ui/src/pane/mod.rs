//! The window's side of the pane engine: the channel its events arrive on. The engine itself is
//! `temor-pane`; the window only says where the events go.

mod sink;

pub use sink::UiSink;
