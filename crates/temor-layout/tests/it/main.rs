//! temor-layout's one integration-test binary: the split tree and the bar's ranking, through property tests.

mod bar_rank;
mod split_props;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}
