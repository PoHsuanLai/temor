//! temor-ui's tests: the pane engine on a real PTY and bash (scratch HOME), the session driven by
//! direct calls (keys, pointer, paste, find), and the window in ds-harness (event dispatch only,
//! never OS input).

mod pane_engine;
mod session;
mod support;
mod window;
