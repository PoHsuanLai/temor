// Adapted from libghostty-rs `crates/libghostty-vt-sys/build.rs` (master 8953a74).
// Copyright (c) 2026 Uzair Aftab, Leah Amelia Chen. MIT licence; see NOTICE.
//
// Changes for temor: the Ghostty source must come from GHOSTTY_SOURCE_DIR (no network clone)
// at the pinned commit, Zig's packages from GHOSTTY_ZIG_SYSTEM_DIR (no fetching) unless
// explicitly allowed, the optimize mode follows OPT_LEVEL only,
// static linking only, ZIG_GLOBAL_CACHE_DIR is honoured and never defaults to ~/.cache, the
// pkg-config / dynamic / iOS paths are dropped, and bindgen never runs here (src/bindings.rs is
// checked in; scripts/gen-bindings.sh regenerates it).
#![allow(clippy::panic)]

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Keep in step with `GHOSTTY_COMMIT` in src/lib.rs, scripts/fetch-ghostty.sh and README.md.
const GHOSTTY_COMMIT: &str = "a5df6bc683d47342b773eb113df10e36acd42da9";
const ZIG_MAJOR_MINOR: &str = "0.16";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for var in [
        "GHOSTTY_SOURCE_DIR",
        "GHOSTTY_ZIG_SYSTEM_DIR",
        "ZIG_GLOBAL_CACHE_DIR",
        "LIBGHOSTTY_VT_SYS_CPU",
        "TEMOR_GHOSTTY_OPTIMIZE",
        "TEMOR_GHOSTTY_ALLOW_UNPINNED",
        "TEMOR_GHOSTTY_ALLOW_FETCH",
        "OPT_LEVEL",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }

    // Without `native` the crate only type-checks against the checked-in bindings.
    if env::var_os("CARGO_FEATURE_NATIVE").is_none() || env::var_os("DOCS_RS").is_some() {
        return;
    }

    let target = env::var("TARGET").expect("cargo sets TARGET for build scripts");
    let host = env::var("HOST").expect("cargo sets HOST for build scripts");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts"));
    let ghostty_dir = ghostty_source_dir();
    check_zig();

    let install_prefix = out_dir.join("ghostty-install");
    let optimize = zig_optimize_mode();
    let cpu = env::var("LIBGHOSTTY_VT_SYS_CPU").unwrap_or_else(|_| "baseline".to_owned());
    assert!(
        !cpu.is_empty(),
        "LIBGHOSTTY_VT_SYS_CPU must not be empty when set"
    );

    // Zig writes ~/.cache/zig unless told otherwise; keep it out of $HOME.
    let global_cache = env::var_os("ZIG_GLOBAL_CACHE_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.join("zig-global-cache"));

    let mut build = Command::new("zig");
    build
        .arg("build")
        .arg("-Demit-lib-vt=true")
        .arg(format!("-Doptimize={optimize}"))
        // Artefacts may run on older CPUs than the build host.
        .arg(format!("-Dcpu={cpu}"))
        .arg("-Demit-xcframework=false")
        .arg("-Dapp-runtime=none")
        .arg("--prefix")
        .arg(&install_prefix)
        .arg("--cache-dir")
        .arg(out_dir.join("zig-cache"))
        .arg("--global-cache-dir")
        .arg(&global_cache)
        .env("ZIG_GLOBAL_CACHE_DIR", &global_cache)
        .current_dir(&ghostty_dir);

    // An immutable, pre-fetched package store (scripts/fetch-ghostty.sh): Zig never fetches.
    if let Some(dir) = env::var_os("GHOSTTY_ZIG_SYSTEM_DIR").filter(|v| !v.is_empty()) {
        let dir = PathBuf::from(dir);
        assert!(
            dir.is_dir(),
            "GHOSTTY_ZIG_SYSTEM_DIR does not exist: {}",
            dir.display()
        );
        build.arg("--system").arg(&dir);
    } else if env::var("TEMOR_GHOSTTY_ALLOW_FETCH").as_deref() == Ok("1") {
        println!(
            "cargo:warning=GHOSTTY_ZIG_SYSTEM_DIR is not set; Zig may fetch packages from the \
             network (allowed by TEMOR_GHOSTTY_ALLOW_FETCH=1)"
        );
    } else {
        panic!(
            "\n\ntemor-vt-sys: GHOSTTY_ZIG_SYSTEM_DIR is not set, so Zig would fetch Ghostty's \
             packages from the network.\nPoint it at the store crates/temor-vt-sys/scripts/\
             fetch-ghostty.sh fills (<dest>/zig-store/p), or set TEMOR_GHOSTTY_ALLOW_FETCH=1 to \
             let Zig fetch.\n"
        );
    }

    if target != host {
        build.arg(format!("-Dtarget={}", zig_target(&target)));
    }

    run(build, "zig build");

    let lib_dir = install_prefix.join("lib");
    let lib_name = if target.contains("windows") {
        "ghostty-vt-static.lib"
    } else {
        "libghostty-vt.a"
    };
    assert!(
        lib_dir.join(lib_name).is_file(),
        "expected the libghostty-vt static library at {}",
        lib_dir.join(lib_name).display()
    );
    let include_dir = install_prefix.join("include");
    assert!(
        include_dir.join("ghostty").join("vt.h").is_file(),
        "expected header at {}",
        include_dir.join("ghostty").join("vt.h").display()
    );

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if target.contains("windows") {
        println!("cargo:rustc-link-lib=static:+verbatim={lib_name}");
    } else {
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }
    println!("cargo:include={}", include_dir.display());
}

/// The pinned Ghostty tree. There is no network fallback: fetching belongs to
/// scripts/fetch-ghostty.sh, which also fills the offline Zig package store.
fn ghostty_source_dir() -> PathBuf {
    let Some(dir) = env::var_os("GHOSTTY_SOURCE_DIR").filter(|v| !v.is_empty()) else {
        panic!(
            "\n\ntemor-vt-sys: GHOSTTY_SOURCE_DIR is not set.\n\
             It must point at a Ghostty checkout at commit {GHOSTTY_COMMIT}.\n\
             Fetch it (and the offline Zig package store) once with:\n\n    \
             crates/temor-vt-sys/scripts/fetch-ghostty.sh <dest>\n\n\
             then export GHOSTTY_SOURCE_DIR=<dest>/src, GHOSTTY_ZIG_SYSTEM_DIR=<dest>/zig-store/p\n\
             and ZIG_GLOBAL_CACHE_DIR=<a writable dir>. To type-check without Zig, build with\n\
             --no-default-features.\n"
        );
    };
    let dir = PathBuf::from(dir);
    assert!(
        dir.join("build.zig").is_file(),
        "GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
        dir.display()
    );
    check_pin(&dir);
    dir
}

/// The bindings in src/bindings.rs are generated from the pinned headers, so building against
/// any other tree can silently mismatch the C ABI. A tree at another commit, or one that is not
/// a git checkout at all, fails the build unless `TEMOR_GHOSTTY_ALLOW_UNPINNED=1`.
fn check_pin(dir: &Path) {
    if let Some(git_dir) = git(dir, &["rev-parse", "--absolute-git-dir"]) {
        // A checkout of another commit rewrites HEAD; rebuild then.
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
    }
    let problem = match git(dir, &["rev-parse", "HEAD"]) {
        Some(head) if head == GHOSTTY_COMMIT => return,
        Some(head) => format!("is at {head}"),
        None => "is not a git checkout, so its commit cannot be checked".to_owned(),
    };
    if env::var("TEMOR_GHOSTTY_ALLOW_UNPINNED").as_deref() == Ok("1") {
        println!(
            "cargo:warning=GHOSTTY_SOURCE_DIR {problem}; temor pins {GHOSTTY_COMMIT} \
             (allowed by TEMOR_GHOSTTY_ALLOW_UNPINNED=1, the bindings may not match)"
        );
        return;
    }
    panic!(
        "\n\ntemor-vt-sys: GHOSTTY_SOURCE_DIR ({}) {problem}, but the checked-in bindings are \
         for {GHOSTTY_COMMIT}.\nFetch the pinned tree with crates/temor-vt-sys/scripts/\
         fetch-ghostty.sh, or set TEMOR_GHOSTTY_ALLOW_UNPINNED=1 to build anyway (for a pin \
         bump, before regenerating the bindings).\n",
        dir.display()
    );
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Ghostty's build.zig requires the same Zig major.minor it was written for.
fn check_zig() {
    let out = Command::new("zig").arg("version").output();
    let version = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        _ => {
            panic!(
                "temor-vt-sys: `zig` was not found on PATH; Zig {ZIG_MAJOR_MINOR}.x is required. \
                 crates/temor-vt-sys/scripts/fetch-zig.sh <dir> fetches the pinned one from \
                 ziglang.org (checked against its SHA-256) and prints the PATH line to use"
            )
        }
    };
    assert!(
        version.starts_with(&format!("{ZIG_MAJOR_MINOR}.")),
        "temor-vt-sys: Ghostty {GHOSTTY_COMMIT} needs Zig {ZIG_MAJOR_MINOR}.x, found {version}. \
         crates/temor-vt-sys/scripts/fetch-zig.sh <dir> fetches the pinned one from ziglang.org \
         and prints the PATH line to use"
    );
}

/// Zig's optimize mode, from `OPT_LEVEL` and never from `DEBUG`: a Debug libghostty runs
/// `vt_write` thousands of times slower and reflow spends minutes in integrity checks, which
/// makes even dev builds and tests unusable. ReleaseSafe (checked, fast) for `opt-level = 0`,
/// ReleaseFast otherwise. `TEMOR_GHOSTTY_OPTIMIZE=Debug|ReleaseSafe|ReleaseFast` overrides.
fn zig_optimize_mode() -> &'static str {
    if let Ok(mode) = env::var("TEMOR_GHOSTTY_OPTIMIZE") {
        return match mode.as_str() {
            "Debug" => "Debug",
            "ReleaseSafe" => "ReleaseSafe",
            "ReleaseFast" => "ReleaseFast",
            other => panic!(
                "TEMOR_GHOSTTY_OPTIMIZE must be one of Debug, ReleaseSafe, ReleaseFast \
                 (got '{other}')"
            ),
        };
    }
    match env::var("OPT_LEVEL").as_deref() {
        Ok("0") | Err(_) => "ReleaseSafe",
        Ok(_) => "ReleaseFast",
    }
}

fn run(mut command: Command, context: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {context}: {error}"));
    assert!(status.success(), "{context} failed with status {status}");
}

fn zig_target(target: &str) -> &'static str {
    match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "aarch64-apple-darwin" => "aarch64-macos-none",
        "x86_64-apple-darwin" => "x86_64-macos-none",
        other => panic!("temor-vt-sys: unsupported Rust target for the Zig build: {other}"),
    }
}
