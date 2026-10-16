//! `block.lens` through a real plugin.

use temor_plugins::{PluginError, lens_block};

use crate::support::{ALL, Host, launcher};

#[test]
fn a_lens_draws_a_picture_and_a_picture_of_the_wrong_size_is_refused() {
    let host = Host::new();
    host.install("l", &[], ALL);
    let plugin = host.plugin("l");

    let drawn = launcher()
        .lens(&plugin, &lens_block("kubectl get pods", "NAME", "/f", 640))
        .unwrap();
    assert_eq!((drawn.width(), drawn.height()), (2, 1));
    assert_eq!(drawn.rgba(), &[255, 0, 0, 255, 0, 0, 255, 255]);

    let broken = launcher().lens(&plugin, &lens_block("bad", "", "/f", 640));
    assert!(
        matches!(broken, Err(PluginError::Broke { .. })),
        "{broken:?}"
    );
}
