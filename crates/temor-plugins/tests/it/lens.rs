//! `block.lens` through a real plugin.

use temor_plugins::{PluginError, lens_block};

use crate::support::{ALL, Host, example_program, launcher};

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

#[test]
fn the_demo_lens_installs_from_its_manifest_and_draws_a_block_six_rows_tall() {
    let host = Host::new();
    let shipped = include_str!("../../examples/lens-demo.toml");
    let program = example_program("lens_demo");
    let manifest: String = shipped
        .lines()
        .map(|line| match line.starts_with("path = ") {
            true => format!("path = {:?}", program.display().to_string()),
            false => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    host.write("lens-demo.toml", &manifest);
    let lenses = host.catalog().lenses("ls -l");
    let plugin = lenses.first().expect("the demo lens for ls");
    assert!(host.catalog().lenses("git status").is_empty());

    let drawn = launcher()
        .lens(plugin, &lens_block("ls", "a\nlonger\n", "/f", 800))
        .unwrap();
    assert_eq!((drawn.width(), drawn.height()), (800, 100));
}
