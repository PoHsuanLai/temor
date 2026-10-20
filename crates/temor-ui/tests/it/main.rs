//! temor-ui's tests: the pane engine on a real PTY and bash (scratch HOME), the session driven by
//! direct calls (keys, pointer, paste, find), and the window in ds-harness (event dispatch only,
//! never OS input).

mod agents_git;
mod agents_socket;
mod agents_window;
mod ambient;
mod bar;
mod blocks;
mod cards;
mod coherence;
mod desktop;
mod facts;
mod host_files;
mod lens_room;
mod offer;
mod peek;
mod persist;
mod remote;
mod resizing;
mod resizing_chrome;
mod run_mode;
mod session;
mod settings;
mod shelf;
mod sidebar;
mod size_audit;
mod spaces;
mod ssh;
mod support;
mod viewer;
mod window;
mod window_title;
mod workflow;
mod worktree_git;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
