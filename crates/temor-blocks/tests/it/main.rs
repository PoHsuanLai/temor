//! temor-blocks' one integration-test binary: sanitising a proposal, through property tests.

mod sanitise_props;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
