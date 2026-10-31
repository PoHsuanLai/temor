# temor-vt-sys

Raw bindings to libghostty-vt, linked statically, for `temor-vt` only.

| Pin | Value |
|---|---|
| Ghostty | the commit in `ghostty.pin` (github.com/PoHsuanLai/ghostty branch `temor`: main 7ec9e26 of 2026-10-08 plus two additions: a placement-data key, `INTERNAL_ID`, and `ghostty_terminal_erase_history_from`); `build.rs`, the scripts and `GHOSTTY_COMMIT` in `src/lib.rs` all read that file |
| Zig | the version in `zig.pin` on `PATH`, by major.minor (Ghostty's `build.zig` requires it); `scripts/fetch-zig.sh` fetches that version from ziglang.org and checks its pinned sha256 |
| bindgen | 0.72.1, run by hand through `scripts/gen-bindings.sh`; never by `build.rs` |

## Building

`build.rs` never touches the network. It needs a Ghostty tree at the pinned commit and, for a
fully offline build, Zig's package store for it. Fetch both once (this is the only online step):

```sh
crates/temor-vt-sys/scripts/fetch-ghostty.sh /mnt/data/cargo/<lane>-temor/ghostty
```

A machine without Zig 0.16 (most distributions package an older one) gets it the same way; the
script prints the `PATH` line to use:

```sh
crates/temor-vt-sys/scripts/fetch-zig.sh /mnt/data/cargo/<lane>-temor/zig
```

then build with:

```sh
export GHOSTTY_SOURCE_DIR=/mnt/data/cargo/<lane>-temor/ghostty/src        # required
export GHOSTTY_ZIG_SYSTEM_DIR=/mnt/data/cargo/<lane>-temor/ghostty/zig-store/p   # no Zig fetches
export ZIG_GLOBAL_CACHE_DIR=/mnt/data/cargo/<lane>-temor/zig-cache       # Zig's build cache
cargo build
```

| Variable | Meaning |
|---|---|
| `GHOSTTY_SOURCE_DIR` | **Required.** The Ghostty checkout. The build fails with instructions when it is missing, and fails when its `HEAD` is not the pin or it is not a git checkout (the bindings are checked in). Rebuilds when its `HEAD` changes. |
| `TEMOR_GHOSTTY_ALLOW_UNPINNED=1` | Build against another commit (or a tree without `.git`) with a warning; for pin bumps, before regenerating the bindings. |
| `GHOSTTY_ZIG_SYSTEM_DIR` | **Required unless fetching is allowed.** The extracted Zig package store; passed as `zig build --system`, so Zig never fetches. |
| `TEMOR_GHOSTTY_ALLOW_FETCH=1` | Build without the store and let Zig fetch packages online. |
| `ZIG_GLOBAL_CACHE_DIR` | Zig's global cache, passed as `--global-cache-dir`. When unset, the build uses a directory under `OUT_DIR`, never `~/.cache/zig`. |
| `TEMOR_GHOSTTY_OPTIMIZE` | `Debug`, `ReleaseSafe` or `ReleaseFast`. By default the mode follows `OPT_LEVEL`, never `DEBUG`: ReleaseSafe for `opt-level = 0` (dev builds and tests), ReleaseFast otherwise. A Debug libghostty makes `vt_write` thousands of times slower and reflow takes minutes, so use it only to debug libghostty itself. |
| `LIBGHOSTTY_VT_SYS_CPU` | Zig CPU model; `baseline` by default so binaries run on older CPUs. |

The store is the whole of Ghostty's `build.zig.zon.txt` (39 packages, about 540 MB extracted at
this pin), not only lib-vt's own dependencies: Zig resolves lazy dependencies while it builds the
graph. Zig 0.16 extracts packages into the project's `zig-pkg/`; the fetch script moves them out
of the source tree into `zig-store/p`.

### Without Zig

The `native` feature (on by default) builds and links the library. Without it the crate only
type-checks against the checked-in bindings, so `cargo check --workspace --no-default-features`
needs neither Zig nor `GHOSTTY_SOURCE_DIR`. CI machines without the Ghostty tree run that check;
anything that links libghostty needs the variables above.

## Bumping the pin

1. Change the commit in `ghostty.pin` (and `zig.pin`, with the sums in `scripts/fetch-zig.sh`, if
   Zig moves). Nothing else names it.
2. Run `scripts/fetch-ghostty.sh <dest>` and `GHOSTTY_SOURCE_DIR=<dest>/src scripts/gen-bindings.sh`.
3. Fix `temor-vt` against the new bindings and pass its conformance tests (`tests/it`).

## Related gaps

The OSC 1337 pre-scan in `temor-vt` catches `File=` only; iTerm2's multipart form
(`MultipartFile=`, `FilePart=`, `FileEnd`) is not caught yet (PLAN §9).

## Licences

`build.rs` and `tools/gen-bindings` are adapted from libghostty-rs (MIT); Ghostty is MIT. See
`NOTICE`.
