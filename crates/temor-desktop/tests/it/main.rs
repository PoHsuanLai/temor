//! temor-desktop's tests. Everything that touches a bus runs on a private `dbus-daemon` with its
//! own configuration (no service directories, so nothing on the machine is ever activated) and
//! never on the person's session.

mod absent;
mod bus;
mod dock;
mod instance;
mod intents;
mod links;
mod secrets;
mod tailnet;
mod window;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
