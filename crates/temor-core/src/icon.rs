//! A tab's icon (PLAN P22): the first of the Mac app the program belongs to, the icon pack's
//! brand for the program (or the runtime its command line names), and the letter tile coloured
//! from its name. A shell at its prompt shows its folder's own icon when the folder has one set,
//! else the letter tile. Pure: the window finds the app and the folder's icon and draws what this
//! picks.

pub mod find;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A brand in the icon pack: its glyph, drawn in white on its colour.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Brand {
    pub glyph: Glyph,
    /// The brand colour, `0xRRGGBB`.
    pub colour: u32,
}

/// Where a brand's glyph comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Glyph {
    /// One temor carries, by its Simple Icons slug.
    Carried(&'static str),
    /// An SVG file the person named in their icon pack.
    File(PathBuf),
}

/// What a tab shows before its title.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TabIcon {
    /// The icon of the Mac app at this path (a `.app` bundle).
    App(PathBuf),
    Brand(Brand),
    /// The icon set on this folder.
    Folder(PathBuf),
    /// A letter on a tile coloured from `key`.
    Letter {
        initial: char,
        key: String,
    },
}

/// What the icon is chosen from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconAsk<'a> {
    /// The command running in the tab, as the shell said it; none at the prompt.
    pub command: Option<&'a str>,
    /// The `.app` bundle the running program's executable is in, when it is in one.
    pub app: Option<&'a Path>,
    /// The tab's folder, when it has an icon of its own set.
    pub folder_icon: Option<&'a Path>,
    /// What the tab is called: the letter tile's letter and colour.
    pub title: &'a str,
}

/// The icon pack: program names to brands. The carried brands, with the person's own entries
/// over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    programs: BTreeMap<String, Brand>,
}

/// The brands temor carries: a Simple Icons slug, its colour, and the programs that take it.
const CARRIED: &[(&str, u32, &[&str])] = &[
    ("claude", 0xD97757, &["claude"]),
    ("openai", 0x412991, &["codex"]),
    ("googlegemini", 0x8E75B2, &["gemini"]),
    ("githubcopilot", 0x000000, &["copilot", "gh-copilot"]),
    ("ollama", 0x000000, &["ollama"]),
    (
        "python",
        0x3776AB,
        &[
            "python", "ipython", "pip", "pytest", "jupyter", "conda", "poetry",
        ],
    ),
    ("uv", 0xDE5FE9, &["uv", "uvx"]),
    ("nodedotjs", 0x5FA04E, &["node", "npx", "tsx", "ts-node"]),
    ("npm", 0xCB3837, &["npm"]),
    ("pnpm", 0xF69220, &["pnpm", "pnpx"]),
    ("yarn", 0x2C8EBB, &["yarn"]),
    ("deno", 0x70FFAF, &["deno"]),
    ("bun", 0x000000, &["bun", "bunx"]),
    (
        "ruby",
        0xCC342D,
        &["ruby", "irb", "gem", "bundle", "bundler", "rails", "rake"],
    ),
    ("go", 0x00ADD8, &["go", "gofmt"]),
    (
        "rust",
        0x000000,
        &["cargo", "rustc", "rustup", "rust-analyzer"],
    ),
    ("openjdk", 0x000000, &["java", "javac", "jshell"]),
    ("apachemaven", 0xC71A36, &["mvn"]),
    ("gradle", 0x02303A, &["gradle", "gradlew"]),
    ("php", 0x777BB4, &["php", "composer"]),
    ("perl", 0x39457E, &["perl"]),
    ("lua", 0x2C2D72, &["lua", "luajit"]),
    ("swift", 0xF05138, &["swift", "swiftc"]),
    ("kotlin", 0x7F52FF, &["kotlin", "kotlinc"]),
    ("elixir", 0x4B275F, &["elixir", "iex", "mix"]),
    ("git", 0xF05032, &["git", "tig", "lazygit"]),
    ("docker", 0x2496ED, &["docker", "docker-compose"]),
    ("podman", 0x892CA0, &["podman"]),
    (
        "kubernetes",
        0x326CE5,
        &["kubectl", "k9s", "kubectx", "minikube", "kind"],
    ),
    ("helm", 0x0F1689, &["helm"]),
    ("terraform", 0x844FBA, &["terraform", "tofu"]),
    ("amazonwebservices", 0x232F3E, &["aws"]),
    ("googlecloud", 0x4285F4, &["gcloud", "gsutil"]),
    ("postgresql", 0x4169E1, &["psql", "pg_dump", "postgres"]),
    ("mysql", 0x4479A1, &["mysql"]),
    ("redis", 0xFF4438, &["redis-cli", "redis-server"]),
    ("sqlite", 0x003B57, &["sqlite3", "sqlite"]),
    ("mongodb", 0x47A248, &["mongosh", "mongo"]),
    ("vim", 0x019733, &["vim", "vi", "view"]),
    ("neovim", 0x57A143, &["nvim"]),
    ("gnuemacs", 0x7F5AB6, &["emacs"]),
    ("zedindustries", 0x084CCF, &["zed"]),
    ("cmake", 0x064F8C, &["cmake"]),
    ("homebrew", 0xFBB040, &["brew"]),
    ("gnubash", 0x4EAA25, &["bash"]),
    ("zsh", 0xF15A24, &["zsh"]),
    ("fishshell", 0x34C534, &["fish"]),
    ("nushell", 0x4E9A06, &["nu"]),
    ("tmux", 0x1BB91F, &["tmux"]),
];

/// The Simple Icons slugs temor carries a glyph for.
pub fn carried_slugs() -> impl Iterator<Item = &'static str> {
    CARRIED.iter().map(|(slug, _, _)| *slug)
}

/// Words that run another program: the program after them is the one shown, and the word itself
/// only when that one has no icon.
const LAUNCHERS: &[&str] = &[
    "sudo",
    "doas",
    "env",
    "nohup",
    "time",
    "exec",
    "command",
    "nice",
    "caffeinate",
    "npx",
    "bunx",
    "uvx",
    "pnpx",
    "uv",
    "poetry",
    "bundle",
    "pipx",
];

/// After these launchers, a word that is a subcommand of theirs, not a program, is passed over.
const SUBCOMMANDS: &[&str] = &["run", "exec", "tool"];

impl Default for Pack {
    fn default() -> Self {
        let mut programs = BTreeMap::new();
        for (slug, colour, names) in CARRIED {
            for name in *names {
                programs.insert(
                    (*name).to_owned(),
                    Brand {
                        glyph: Glyph::Carried(slug),
                        colour: *colour,
                    },
                );
            }
        }
        Pack { programs }
    }
}

impl Pack {
    /// The carried pack with `entries` over it: a program to a brand of the person's.
    pub fn with(mut self, entries: impl IntoIterator<Item = (String, Brand)>) -> Self {
        self.programs.extend(entries);
        self
    }

    /// The brand a carried slug stands for, for an entry that names one (`tool = "python"`).
    pub fn carried(slug: &str) -> Option<Brand> {
        CARRIED
            .iter()
            .find(|(carried, _, _)| *carried == slug)
            .map(|(slug, colour, _)| Brand {
                glyph: Glyph::Carried(slug),
                colour: *colour,
            })
    }

    /// The brand for the program `name`: as it is, then without a version at its end
    /// (`python3.12` is Python).
    pub fn brand_of(&self, name: &str) -> Option<&Brand> {
        self.programs
            .get(name)
            .or_else(|| self.programs.get(unversioned(name)))
    }
}

/// `name` without the version at its end: `python3.12`, `gcc-13` and `node18` lose it.
fn unversioned(name: &str) -> &str {
    let bare = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-');
    match bare.is_empty() {
        true => name,
        false => bare,
    }
}

/// The programs a command line runs, the one shown first: past `VAR=value` words and the
/// launchers (`sudo`, `npx`, `uv run` and the like), each a path's last part. `uv run train.py`
/// is `train.py`, then `uv`.
pub fn programs_of(command: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut after_launcher = false;
    for word in command.split_whitespace() {
        // A pipeline's later commands are not what the tab runs.
        if matches!(word, "|" | "&&" | "||" | ";") {
            break;
        }
        let word = word.trim_matches(|c| c == '"' || c == '\'');
        if word.contains('=') && !word.starts_with('-') && found.is_empty() {
            continue;
        }
        if word.starts_with('-') {
            continue;
        }
        if after_launcher && SUBCOMMANDS.contains(&word) {
            continue;
        }
        let name = word.rsplit('/').next().unwrap_or(word);
        if name.is_empty() {
            continue;
        }
        found.push(name.to_owned());
        after_launcher = LAUNCHERS.contains(&name);
        if !after_launcher {
            break;
        }
    }
    found.reverse();
    found
}

/// The `.app` bundle a program's executable is inside: the outermost, as a helper app nests in
/// its app's bundle.
pub fn bundle_of(executable: &Path) -> Option<&Path> {
    executable.ancestors().filter(|dir| is_bundle(dir)).last()
}

fn is_bundle(dir: &Path) -> bool {
    dir.extension().is_some_and(|ext| ext == "app")
}

/// The PNG in an `.icns` file that best draws `px` pixels square: the smallest at least that
/// wide, else the widest. Modern icon files hold PNGs; older kinds of image are passed over.
pub fn png_in_icns(icns: &[u8], px: u32) -> Option<&[u8]> {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
    let body = icns.strip_prefix(b"icns")?.get(4..)?;
    let mut pngs: Vec<(u32, &[u8])> = Vec::new();
    let mut rest = body;
    while rest.len() >= 8 {
        let len = u32::from_be_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
        let Some(data) = rest.get(8..len.max(8)) else {
            break;
        };
        if data.starts_with(PNG) && data.len() >= 24 {
            let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            pngs.push((width, data));
        }
        rest = &rest[len.max(8)..];
    }
    let fits = pngs
        .iter()
        .filter(|(width, _)| *width >= px)
        .min_by_key(|(width, _)| *width);
    fits.or_else(|| pngs.iter().max_by_key(|(width, _)| *width))
        .map(|(_, data)| *data)
}

/// The tab's icon.
pub fn icon_for(ask: &IconAsk<'_>, pack: &Pack) -> TabIcon {
    let letter = || TabIcon::Letter {
        initial: initial_of(ask.title),
        key: ask.title.to_owned(),
    };
    let Some(command) = ask.command else {
        return match ask.folder_icon {
            Some(folder) => TabIcon::Folder(folder.to_owned()),
            None => letter(),
        };
    };
    if let Some(app) = ask.app {
        return TabIcon::App(app.to_owned());
    }
    programs_of(command)
        .iter()
        .find_map(|program| pack.brand_of(program))
        .map_or_else(letter, |brand| TabIcon::Brand(brand.clone()))
}

/// The letter tile's letter: the title's first letter or digit, in capitals; `?` for none.
pub fn initial_of(title: &str) -> char {
    title
        .chars()
        .find(|c| c.is_alphanumeric())
        .and_then(|c| c.to_uppercase().next())
        .unwrap_or('?')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask<'a>(command: Option<&'a str>, title: &'a str) -> IconAsk<'a> {
        IconAsk {
            command,
            app: None,
            folder_icon: None,
            title,
        }
    }

    fn slug(icon: &TabIcon) -> Option<&'static str> {
        match icon {
            TabIcon::Brand(Brand {
                glyph: Glyph::Carried(slug),
                ..
            }) => Some(slug),
            _ => None,
        }
    }

    #[test]
    fn a_command_is_known_by_its_program_past_launchers_paths_and_settings() {
        assert_eq!(programs_of("claude --resume"), ["claude"]);
        assert_eq!(programs_of("/usr/bin/python3 train.py"), ["python3"]);
        assert_eq!(programs_of("FOO=1 BAR=2 node app.js"), ["node"]);
        assert_eq!(programs_of("sudo -E docker ps"), ["docker", "sudo"]);
        assert_eq!(programs_of("uv run train.py"), ["train.py", "uv"]);
        assert_eq!(programs_of("npx vite"), ["vite", "npx"]);
        assert_eq!(programs_of("git log | less"), ["git"]);
        assert!(programs_of("").is_empty());
    }

    #[test]
    fn the_pack_knows_agents_runtimes_and_tools_and_a_version_does_not_hide_one() {
        let pack = Pack::default();
        let brand = |command| slug(&icon_for(&ask(Some(command), "x"), &pack));
        assert_eq!(brand("claude"), Some("claude"));
        assert_eq!(brand("codex exec"), Some("openai"));
        assert_eq!(brand("python3.12 train.py"), Some("python"));
        assert_eq!(brand("cargo nextest run"), Some("rust"));
        assert_eq!(brand("sudo kubectl get pods"), Some("kubernetes"));
        // What a launcher runs has no icon: the launcher's own.
        assert_eq!(brand("npx some-tool"), Some("nodedotjs"));
        assert_eq!(brand("uv run train.py"), Some("uv"));
        assert_eq!(brand("my-own-script"), None);
    }

    #[test]
    fn an_app_comes_first_and_an_unknown_program_is_its_letter() {
        let pack = Pack::default();
        let app = Path::new("/Applications/Docker.app");
        let docker = IconAsk {
            app: Some(app),
            ..ask(Some("docker ps"), "docker")
        };
        assert_eq!(icon_for(&docker, &pack), TabIcon::App(app.to_owned()));
        assert_eq!(
            icon_for(&ask(Some("./deploy.sh"), "deploy"), &pack),
            TabIcon::Letter {
                initial: 'D',
                key: "deploy".to_owned()
            }
        );
    }

    #[test]
    fn at_the_prompt_the_folder_shows_its_own_icon_or_its_letter() {
        let pack = Pack::default();
        assert_eq!(
            icon_for(&ask(None, "~/ml"), &pack),
            TabIcon::Letter {
                initial: 'M',
                key: "~/ml".to_owned()
            }
        );
        let folder = Path::new("/Users/po/Design");
        let custom = IconAsk {
            folder_icon: Some(folder),
            ..ask(None, "Design")
        };
        assert_eq!(icon_for(&custom, &pack), TabIcon::Folder(folder.to_owned()));
    }

    #[test]
    fn the_persons_entries_go_over_the_carried_ones() {
        let mine = Brand {
            glyph: Glyph::File(PathBuf::from("/icons/train.svg")),
            colour: 0x112233,
        };
        let pack = Pack::default().with([
            ("train".to_owned(), mine.clone()),
            ("python".to_owned(), Pack::carried("rust").expect("carried")),
        ]);
        assert_eq!(pack.brand_of("train"), Some(&mine));
        assert_eq!(
            pack.brand_of("python3").map(|b| &b.glyph),
            Some(&Glyph::Carried("rust"))
        );
    }

    #[test]
    fn the_bundle_is_the_outermost_app_around_the_executable() {
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/Docker.app/Contents/Resources/bin/docker"
            )),
            Some(Path::new("/Applications/Docker.app"))
        );
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/Code.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper"
            )),
            Some(Path::new("/Applications/Code.app"))
        );
        assert_eq!(bundle_of(Path::new("/usr/bin/git")), None);
    }

    /// A PNG's first 24 bytes, saying it is `width` wide.
    fn png(width: u32) -> Vec<u8> {
        let mut data = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        data.extend(width.to_be_bytes());
        data.extend(width.to_be_bytes());
        data
    }

    fn element(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.extend(u32::try_from(data.len() + 8).expect("small").to_be_bytes());
        out.extend(data);
        out
    }

    #[test]
    fn the_icon_file_gives_the_smallest_png_wide_enough() {
        let mut body = Vec::new();
        body.extend(element(b"is32", b"old pixels"));
        body.extend(element(b"ic07", &png(128)));
        body.extend(element(b"ic11", &png(32)));
        body.extend(element(b"ic12", &png(64)));
        let mut icns = b"icns".to_vec();
        icns.extend(u32::try_from(body.len() + 8).expect("small").to_be_bytes());
        icns.extend(body);
        let width = |found: Option<&[u8]>| found.map(|data| data[19]);
        assert_eq!(width(png_in_icns(&icns, 40)), Some(64));
        assert_eq!(width(png_in_icns(&icns, 32)), Some(32));
        assert_eq!(width(png_in_icns(&icns, 512)), Some(128), "else the widest");
        assert_eq!(png_in_icns(b"not an icon", 32), None);
    }

    #[test]
    fn a_letter_is_the_first_letter_or_digit() {
        assert_eq!(initial_of("~/ml"), 'M');
        assert_eq!(initial_of("2fa"), '2');
        assert_eq!(initial_of("…"), '?');
    }
}
