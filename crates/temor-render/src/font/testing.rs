//! The bundled test fonts (tests/fonts, OFL), for unit tests.

use super::{FontConfig, FontFamily, FontSet, FontSource};
use crate::metrics::FontSize;

pub(crate) fn bundled() -> FontSource {
    FontSource::Bundled(vec![
        include_bytes!("../../tests/fonts/NotoSansMono-Regular-subset.ttf").to_vec(),
        include_bytes!("../../tests/fonts/NotoSansMono-Bold-subset.ttf").to_vec(),
        include_bytes!("../../tests/fonts/NotoSansMonoCJK-subset.otf").to_vec(),
        include_bytes!("../../tests/fonts/NotoColorEmoji-subset.ttf").to_vec(),
    ])
}

/// Noto Sans Mono at `px` logical pixels, with the CJK and emoji fallbacks.
pub(crate) fn fonts(px: f32) -> FontSet {
    let config = FontConfig {
        family: FontFamily::Named("Noto Sans Mono".into()),
        size: FontSize::new(px),
    };
    FontSet::load(&config, bundled()).expect("bundled fonts load")
}
