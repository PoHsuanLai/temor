//! temor-core's one integration-test binary: the public surface, through property tests and
//! round trips.

mod bar_rank;
mod sanitise_props;
mod serde_shapes;
mod split_props;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
