//! temor-client's one integration-test binary: the model's stored and wire forms.

mod serde_shapes;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
