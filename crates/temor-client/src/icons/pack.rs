//! The icon pack (PLAN P22): the brand glyphs temor carries, and the person's own entries from
//! `icons.toml` in temor's config folder, which go over them:
//!
//! ```toml
//! train = "python"                                       # a carried brand, by its slug
//! deploy = { svg = "icons/deploy.svg", colour = "#112233" } # an SVG of theirs, on its colour
//! ```
//!
//! A relative path is under the config folder; `~/` is the home folder. The carried glyphs are
//! Simple Icons 13.21 (simpleicons.org, CC0 1.0): each brand's mark is its owner's.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use temor_core::icon::{Brand, Glyph, Pack};

/// The file the person's entries are in, inside temor's config folder.
const FILE: &str = "icons.toml";

/// The carried glyphs, by slug.
const CARRIED_SVG: &[(&str, &str)] = &[
    (
        "amazonwebservices",
        include_str!("brands/amazonwebservices.svg"),
    ),
    ("apachemaven", include_str!("brands/apachemaven.svg")),
    ("bun", include_str!("brands/bun.svg")),
    ("claude", include_str!("brands/claude.svg")),
    ("cmake", include_str!("brands/cmake.svg")),
    ("deno", include_str!("brands/deno.svg")),
    ("docker", include_str!("brands/docker.svg")),
    ("elixir", include_str!("brands/elixir.svg")),
    ("fishshell", include_str!("brands/fishshell.svg")),
    ("git", include_str!("brands/git.svg")),
    ("githubcopilot", include_str!("brands/githubcopilot.svg")),
    ("gnubash", include_str!("brands/gnubash.svg")),
    ("gnuemacs", include_str!("brands/gnuemacs.svg")),
    ("go", include_str!("brands/go.svg")),
    ("googlecloud", include_str!("brands/googlecloud.svg")),
    ("googlegemini", include_str!("brands/googlegemini.svg")),
    ("gradle", include_str!("brands/gradle.svg")),
    ("helm", include_str!("brands/helm.svg")),
    ("homebrew", include_str!("brands/homebrew.svg")),
    ("kotlin", include_str!("brands/kotlin.svg")),
    ("kubernetes", include_str!("brands/kubernetes.svg")),
    ("lua", include_str!("brands/lua.svg")),
    ("mongodb", include_str!("brands/mongodb.svg")),
    ("mysql", include_str!("brands/mysql.svg")),
    ("neovim", include_str!("brands/neovim.svg")),
    ("nodedotjs", include_str!("brands/nodedotjs.svg")),
    ("npm", include_str!("brands/npm.svg")),
    ("nushell", include_str!("brands/nushell.svg")),
    ("ollama", include_str!("brands/ollama.svg")),
    ("openai", include_str!("brands/openai.svg")),
    ("openjdk", include_str!("brands/openjdk.svg")),
    ("perl", include_str!("brands/perl.svg")),
    ("php", include_str!("brands/php.svg")),
    ("pnpm", include_str!("brands/pnpm.svg")),
    ("podman", include_str!("brands/podman.svg")),
    ("postgresql", include_str!("brands/postgresql.svg")),
    ("python", include_str!("brands/python.svg")),
    ("redis", include_str!("brands/redis.svg")),
    ("ruby", include_str!("brands/ruby.svg")),
    ("rust", include_str!("brands/rust.svg")),
    ("sqlite", include_str!("brands/sqlite.svg")),
    ("swift", include_str!("brands/swift.svg")),
    ("terraform", include_str!("brands/terraform.svg")),
    ("tmux", include_str!("brands/tmux.svg")),
    ("uv", include_str!("brands/uv.svg")),
    ("vim", include_str!("brands/vim.svg")),
    ("yarn", include_str!("brands/yarn.svg")),
    ("zedindustries", include_str!("brands/zedindustries.svg")),
    ("zsh", include_str!("brands/zsh.svg")),
];

/// The pack and every glyph it can draw, read once.
#[derive(Debug, Clone, Default)]
pub struct IconPack {
    pub pack: Pack,
    svgs: HashMap<Glyph, Arc<str>>,
}

impl IconPack {
    /// The carried pack with the entries in `icons.toml` over it, and what was wrong with the
    /// file, to tell the person. No file is no entries.
    pub fn load(config_dir: &Path, home: &Path) -> (IconPack, Vec<String>) {
        match std::fs::read_to_string(config_dir.join(FILE)) {
            Ok(text) => IconPack::read(&text, config_dir, home),
            Err(_) => IconPack::read("", config_dir, home),
        }
    }

    /// The pack from `text`, the file's words; paths in it are under `base`.
    pub fn read(text: &str, base: &Path, home: &Path) -> (IconPack, Vec<String>) {
        let mut svgs: HashMap<Glyph, Arc<str>> = CARRIED_SVG
            .iter()
            .map(|(slug, svg)| (Glyph::Carried(slug), Arc::from(*svg)))
            .collect();
        let mut trouble = Vec::new();
        let table = match text.parse::<toml::Table>() {
            Ok(table) => table,
            Err(err) => {
                trouble.push(format!("{FILE}: {}", err.message()));
                toml::Table::new()
            }
        };
        let mut entries = Vec::new();
        for (program, value) in table {
            match entry(&value, base, home) {
                Ok((brand, svg)) => {
                    if let (Glyph::File(path), Some(svg)) = (&brand.glyph, svg) {
                        svgs.insert(Glyph::File(path.clone()), svg);
                    }
                    entries.push((program, brand));
                }
                Err(why) => trouble.push(format!("{FILE}: {program}: {why}")),
            }
        }
        let pack = IconPack {
            pack: Pack::default().with(entries),
            svgs,
        };
        (pack, trouble)
    }

    /// The SVG that draws `glyph`.
    pub fn svg(&self, glyph: &Glyph) -> Option<Arc<str>> {
        self.svgs.get(glyph).cloned()
    }
}

/// One entry: a carried slug, or an SVG file and its colour (read now).
fn entry(
    value: &toml::Value,
    base: &Path,
    home: &Path,
) -> Result<(Brand, Option<Arc<str>>), String> {
    if let Some(slug) = value.as_str() {
        return Pack::carried(slug)
            .map(|brand| (brand, None))
            .ok_or_else(|| format!("temor carries no icon called {slug:?}"));
    }
    let table = value
        .as_table()
        .ok_or("give a carried icon's name, or { svg = ..., colour = ... }")?;
    let path = table
        .get("svg")
        .and_then(toml::Value::as_str)
        .ok_or("say which SVG file (svg = ...)")?;
    let path = under(path, base, home);
    let svg = std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let colour = match table.get("colour").or_else(|| table.get("color")) {
        None => 0x000000,
        Some(colour) => colour
            .as_str()
            .and_then(hex)
            .ok_or("a colour is written #RRGGBB")?,
    };
    let brand = Brand {
        glyph: Glyph::File(path),
        colour,
    };
    Ok((brand, Some(Arc::from(svg))))
}

/// `path` as written in the file: `~/` is the home folder, a relative path is under `base`.
fn under(path: &str, base: &Path, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => base.join(path),
    }
}

/// `#RRGGBB` as `0xRRGGBB`.
fn hex(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('#')?;
    (digits.len() == 6).then(|| u32::from_str_radix(digits, 16).ok())?
}

#[cfg(test)]
mod tests {
    use temor_core::icon::carried_slugs;

    use super::*;

    #[test]
    fn every_carried_brand_has_its_glyph() {
        for slug in carried_slugs() {
            assert!(
                CARRIED_SVG.iter().any(|(carried, _)| *carried == slug),
                "{slug}"
            );
        }
        assert_eq!(CARRIED_SVG.len(), carried_slugs().count());
        assert!(CARRIED_SVG.iter().all(|(_, svg)| svg.starts_with("<svg")));
    }

    #[test]
    fn the_persons_entries_name_a_carried_brand_or_an_svg_of_theirs() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("train.svg"), "<svg/>").expect("svg");
        let text = r##"
            train = { svg = "train.svg", colour = "#112233" }
            fit = "python"
            bad = "no-such-brand"
            gone = { svg = "missing.svg" }
        "##;
        let (pack, trouble) = IconPack::read(text, dir.path(), Path::new("/home/po"));
        let train = pack.pack.brand_of("train").expect("train");
        assert_eq!(train.colour, 0x112233);
        assert_eq!(pack.svg(&train.glyph).as_deref(), Some("<svg/>"));
        let fit = pack.pack.brand_of("fit").expect("fit");
        assert_eq!(fit.glyph, Glyph::Carried("python"));
        assert!(pack.svg(&fit.glyph).is_some());
        assert_eq!(trouble.len(), 2, "{trouble:?}");
        let (_, trouble) = IconPack::read("not = [toml", dir.path(), Path::new("/"));
        assert_eq!(trouble.len(), 1);
    }

    #[test]
    fn a_path_is_under_the_config_folder_or_home() {
        let base = Path::new("/c/temor");
        let home = Path::new("/home/po");
        assert_eq!(under("a.svg", base, home), base.join("a.svg"));
        assert_eq!(under("~/i/a.svg", base, home), home.join("i/a.svg"));
        assert_eq!(under("/abs/a.svg", base, home), PathBuf::from("/abs/a.svg"));
        assert_eq!(hex("#D97757"), Some(0xD97757));
        assert_eq!(hex("D97757"), None);
    }
}
