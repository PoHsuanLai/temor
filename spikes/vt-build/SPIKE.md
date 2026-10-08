# Spike P0(a): libghostty-vt from Rust

Date: 2026-10-09. Machine: Fedora 44, x86_64, Rust 1.98.1, shared build box.
Builds ran through the agent cargo shim: 8 jobs, `CPUWeight=idle`, sccache on the Rust crates.

## Verdict

**Go on libghostty-vt. No-go on the crates.io pin as it stands.**

- **The engine.** The build is reproducible and fully offline. Everything temor reads back works:
  render state, kitty images, the formatter, the key and mouse encoders, scrollback and reflow.
- **The crates.io pin is too old.** `libghostty-vt` 0.2.2 pins Ghostty `a887df4` (2026-07-11).
  That commit lacks seven things temor needs:
  - the notification callback (OSC 9 and 777)
  - the progress callback (OSC 9;4)
  - OSC 7501
  - the unknown-sequence callback (transfers)
  - the OSC 133 event callback with the exit code
  - `RENDER_HOLD`
  - snapshot
- **Ghostty main has all of them.** Main is `7ec9e26` (2026-10-08). I proved each one through raw
  FFI (part B below).
- **The plan.**
  - Pin Ghostty main, or a later commit.
  - Use a libghostty-rs fork or PR that carries regenerated bindings plus safe wrappers for the
    new callbacks.
  - Pin Zig 0.16.
  - Before temor depends on it, upstream or work around OSC 1337 `File=`. Ghostty swallows it, so
    the unknown-sequence callback never sees it.

## Pins

| What | Commit | Zig |
|---|---|---|
| `libghostty-vt-sys` 0.2.2 (crates.io) `GHOSTTY_COMMIT` | `a887df42c56f6de86c0fe6da9c4eeca37931e083` (2026-07-11, 1.3.2-dev) | **0.15.2 exactly**: `requireZig` needs the same major and minor, so `/usr/bin/zig` 0.16.0 fails |
| libghostty-rs master `8953a74` (unreleased) | `22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018` (2026-08-06) | 0.16 |
| Ghostty main (part B) | `7ec9e26a29399b30ef78f06985fc75045b3cecb6` (2026-10-08) | 0.16.0 (`/usr/bin/zig` works) |

## How build.rs gets Ghostty

- **Source.** `GHOSTTY_SOURCE_DIR` wins and must contain `build.zig`. Without it, build.rs runs
  `git clone --filter=blob:none` of github.com/ghostty-org/ghostty into `OUT_DIR` and checks out
  the pinned commit, which needs the network.
- **The Zig build.** It then runs:

  ```
  zig build -Demit-lib-vt=true -Doptimize=<Debug|ReleaseFast> -Dcpu=baseline -Demit-xcframework=false -Dapp-runtime=none --prefix OUT/ghostty-install --cache-dir OUT/zig-cache
  ```

  It links `static=ghostty-vt`.
- **With `GHOSTTY_ZIG_SYSTEM_DIR`.** It adds `--system <dir> --global-cache-dir OUT/zig-global-cache`,
  so Zig never fetches.
- **Without `GHOSTTY_ZIG_SYSTEM_DIR`.** Zig uses `ZIG_GLOBAL_CACHE_DIR`, or **`~/.cache/zig`** if
  that is unset, and fetches from deps.files.ghostty.org. Always set `ZIG_GLOBAL_CACHE_DIR`.
- **Zig is found on `PATH`** only. There is no `ZIG` env var.

## Commands (part A, crates.io 0.2.2)

```sh
cd /home/pohsuanlai/temor/spikes/vt-build
# one-time, online
P0=/mnt/data/cargo/p0-temor
curl -sSfLo $P0/dl/zig.tar.xz https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz   # sha256 02aa270f…3239, checked
tar -C $P0 -xf $P0/dl/zig.tar.xz && mv $P0/zig-x86_64-linux-0.15.2 $P0/zig-0.15.2
git clone --filter=blob:none --no-checkout https://github.com/ghostty-org/ghostty.git $P0/ghostty-src
git -C $P0/ghostty-src checkout a887df42c56f6de86c0fe6da9c4eeca37931e083
. ./env.sh && cargo build            # fills $P0/zig-global-cache/p (25 packages, 339 MB) and ~/.cargo registry
# offline, clean, network namespace with no network (loopback up only so sccache can start)
rm -rf $P0/target-a
unshare -rn bash -c 'ip link set lo up; . ./env.sh; export GHOSTTY_ZIG_SYSTEM_DIR=$P0/zig-global-cache/p; cargo build --offline'
$P0/target-a/debug/vt-build-spike      # 47 checks: 39 pass, 0 fail, 8 gap (2 xfail lines count as pass)
cargo check --features send-check --bin send_check   # expected to FAIL: proves !Send
```

`env.sh` sets the following, so nothing is written under `$HOME`:

- `PATH`, with Zig 0.15.2 first
- `ZIG_GLOBAL_CACHE_DIR`
- `GHOSTTY_SOURCE_DIR`
- `CARGO_TARGET_DIR=/mnt/data/cargo/p0-temor/target-a`

## Fully offline build: what is needed

1. **A Ghostty source tree** at the pinned commit, passed as `GHOSTTY_SOURCE_DIR`.
2. **Zig's package store.**
   - Get it by running `ZIG_GLOBAL_CACHE_DIR=X nix/build-support/fetch-zig-cache.sh` in the
     Ghostty tree, or by doing one online build with `ZIG_GLOBAL_CACHE_DIR=X`.
   - Then pass `GHOSTTY_ZIG_SYSTEM_DIR=X/p`.
   - **The whole store is needed, not just lib-vt's own deps.** Zig resolves lazy deps
     (vaxis, z2d, freetype tarballs…) while it builds the graph. A store holding only `uucode`
     failed with "lazy dependency package not found".
   - The store is 25 packages, 339 MB.
3. **The Zig binary** at the version the pin requires, on `PATH`.
4. **Cargo crates.** Either the warm `~/.cargo` registry with `--offline` (tested), or
   `cargo vendor` for packaging.

**Verified:**

- **With `GHOSTTY_ZIG_SYSTEM_DIR`, no network.** The clean build passed, and the binary links only
  libc and libgcc_s.
- **With only a warm `ZIG_GLOBAL_CACHE_DIR`, no network.** This also builds, but it keeps
  writable state outside the target dir.
- **Recommendation for temor.** Use the `GHOSTTY_ZIG_SYSTEM_DIR` form; it is what §14 already
  assumes.
- **`--system` and linking.** Ghostty's docs say `--system` makes deps link dynamically. For
  lib-vt that did not leak into the final binary.

## Timings and sizes (Ghostty a887df4, Zig 0.15.2)

| | Time |
|---|---|
| Clean dev build, offline (`cargo` "Finished") | **39.5 s**, of which the libghostty build script took 36.9 s |
| Clean release build, offline | **72 s**, of which the build script took 69.6 s (ReleaseFast) |
| Incremental (touch `main.rs`) | 1.2–2.2 s |
| No-op | about 1 s (build script `Fresh`, no Zig rerun) |
| First online build (fetch Zig packages and compile) | 76 s inside cargo |

The 4 m 14 s wall time also counts waiting for a build slot.

| Artefact | Size |
|---|---|
| `libghostty-vt.a`, Debug | 42.8 MB |
| `libghostty-vt.a`, ReleaseFast | 15.6 MB |
| test binary, dev | 28.7 MB |
| test binary, release | 2.92 MB (2.37 MB stripped) |
| Ghostty `.git` (blob:none) | 70 MB |

Ghostty main with Zig 0.16 (part B) also built; the bindings regeneration run took 1 m 44 s,
including the Zig build.

## Capability table

**Legend for the "Safe API or -sys" column:**
- **safe**: the crates.io 0.2.2 safe crate exposes it.
- **-sys@main**: it exists only at Ghostty main; I called it through raw FFI in `main-pin/`.
- **GAP**: it exists nowhere.

| Capability | Works? | Safe API or -sys | Notes / recommendation |
|---|---|---|---|
| Terminal 80×24, `vt_write` | yes | safe | |
| DA1 → WRITE_PTY | yes | safe | Replies `ESC[?62;22c` even with no DA handler. |
| XTVERSION, BEL, ENQ | yes | safe | |
| OSC 0/2 title | yes | safe (`on_title_changed` + `title()`) | |
| OSC 7 cwd | yes | safe (`on_pwd_changed` + `pwd()`) | Gives the raw `file://host/path` URL; temor parses the host. |
| OSC 52 clipboard write | yes, text only | safe | **Binary data becomes `""`** in 0.2.x (`ClipboardContent.data: &str`). Fixed in 0.3.0 (`&[u8]`), so take 0.3. |
| OSC 8 hyperlinks | yes | safe (`GridRef::hyperlink_uri`, `Cell::has_hyperlink`) | No callback needed. |
| OSC 133 marks on cells/rows | yes | safe (`semantic_content`, `semantic_prompt`) | |
| OSC 133 events + exit code | at main only | -sys@main `SEMANTIC_PROMPT` (42) | Passed: kinds 1–4, exit code 1, command text. **Contribute a safe wrapper.** |
| OSC 9 / 777 notification | at main only | -sys@main `DESKTOP_NOTIFICATION` (29) | Libghostty-rs master already wraps it safely (`on_desktop_notification`). |
| OSC 9;4 progress | at main only | -sys@main `PROGRESS_REPORT` (30) | Master wraps it safely. |
| OSC 7501 program status | at main only | -sys@main `PROGRAM_STATUS` (46) | Passed: state, kind, progress, app, message. **Contribute a safe wrapper.** |
| UNKNOWN_SEQUENCE (OSC 5113, APC) | at main only | -sys@main `UNKNOWN_SEQUENCE` (35) + `UNKNOWN_MAX_BYTES` (36) | OSC 5113 and an unknown APC arrive. **Contribute a safe wrapper.** |
| UNKNOWN_SEQUENCE for OSC 1337 `File=` | **no, even at main** | GAP | Ghostty parses 1337 and drops `File=` as "unimplemented" (`osc/parsers/iterm2.zig`), so it is never reported. **Upstream: report unimplemented 1337 keys as unknown.** The fallback is a byte pre-scan in temor-vt before `vt_write`. |
| Mode 2026 synchronized output | yes (poll) | safe `mode(Mode::SYNC_OUTPUT)` | |
| RENDER_HOLD callback | at main only | -sys@main `RENDER_HOLD` (41) | Passed: `[true,false]`. **Contribute a safe wrapper.** |
| Wide chars, emoji, combining, ZWJ | yes | safe | **Mode 2027 is OFF by default**: 👍🏽 splits into two cells. temor-vt must `set_mode(GRAPHEME_CLUSTER, true)` on creation. |
| SGR (bold, italic, underline styles and colour, 256 colours, RGB, strike, overline) | yes | safe | |
| Render state, two-phase `begin_update` / `end` | yes | safe (`RenderState::begin_update` → `Update::end`) | First frame `Full`. After one edit: `Partial`, rows [1,4] (the old cursor row plus the edited row). Cells give graphemes, style and resolved fg/bg. Cursor viewport and style, palette. |
| Kitty PNG a=T | yes | safe `graphics::set_png_decoder` (wraps `ghostty_sys_set(DECODE_PNG)`) with our own `DecodePng` on `png` 0.18 | The crate's own `RustPngDecoder` **cannot be constructed** (private field, no constructor; its doc example calls a `new()` that doesn't exist). Its `decode_png` also decodes into an empty `Vec`. Use our own decoder. |
| Kitty raw RGBA f=32 | yes | safe | Byte-exact. |
| `KITTY_IMAGE_STORAGE_LIMIT` | yes | safe `set_kitty_image_storage_limit` | |
| Placements iterator, layer filter, `render_info`, pixel data, generation | yes | safe | Storage and per-image `generation` are monotonic and process-wide. `Image::data()` returns borrowed RGBA. |
| Scrollback + viewport (top, delta, bottom, scrollbar) | yes | safe | |
| Resize with reflow | yes | safe | 150 chars: 80 cols → 160 → 50 cols; reflowed exactly. |
| Formatter plain / HTML / VT of a range | yes | safe (`Formatter` + `Selection::new(gridref,gridref)`) | HTML uses `var(--vt-palette-N)` CSS variables. |
| Snapshot encode/restore | at main only | -sys@main `ghostty_snapshot_encode_alloc` + `decoder_new_buf` + `decode` | Passed: plain, VT (styles and links), cursor and title all equal; 1.9 KB. Libghostty-rs master has a safe `snapshot.rs`. The VT formatter replay workaround available at 0.2.2 is **lossy**: it drops hyperlinks, cursor position and title. |
| Key encoder, legacy and kitty (`CSI > 1 u`) | yes | safe | Legacy: `a`, `^C`, `\r`, `ESC[A`, `ESC`. Kitty: Ctrl+C → `ESC[99;5u`, Esc → `ESC[27u`. |
| Mouse encoder SGR | yes | safe | `ESC[<0;6;4M` and `…m`. |
| Terminal across threads under a Mutex | yes, with `unsafe` | **needs `unsafe impl Send`** | See Threading. |

## Threading

- **The types are not `Send`.** `Terminal`, `RenderState` and the encoders are `!Send + !Sync`.
  `src/bin/send_check.rs` fails to compile, as expected: `NonNull<TerminalImpl>`,
  `*const AllocatorVtable` and the **boxed callbacks (`dyn PtyWriteFn`, …) are not `Send`**.
  So `Mutex<Terminal>` is not `Sync`, and the plan's §4b pattern does not compile with the safe
  crate alone.
- **The C side has no thread-local state.** I grepped `src/terminal`, `unicode` and `simd` for
  `threadlocal`: no hits.
- **The workaround passed.** `ffi_gap::SendTerminal` adds an `unsafe impl Send`.
  - A writer thread made 2000 `vt_write` calls, each with a DSR query; the replies went over an
    mpsc channel from inside the callback.
  - A render thread ran 300 × (`begin_update` under the lock, `end` outside it).
  - All 2000 replies arrived, and the final screen was correct.
- **The workaround is only sound if every callback is `Send`.** temor-vt must register callbacks
  only through its own `Send`-bounded setters.
- **Recommendation: contribute upstream.** Add a `Send`-able terminal mode to libghostty-rs: a
  `SendTerminal`, or `Send` bounds on handlers behind a type parameter. Until that lands, keep one
  named module `temor-vt/src/ffi_send.rs` with `#[allow(unsafe_code)]`.
- **The PNG decoder is per-thread.** It lives in a crate `thread_local!`, while
  `ghostty_sys_set(DECODE_PNG)` is process-global. Install it on the PTY I/O thread that calls
  `vt_write`, or PNGs silently fail on other threads.

## API problems found

1. **The crates.io pin lags upstream by three months.** Bumping it is not drop-in: regenerating
   bindings against main and building the master safe crate gives about 140 errors. Ghostty enums
   became `GHOSTTY_ENUM_TYPED` (int), so every `#[repr(u32)]` mirror breaks, and
   `ghostty_render_state_colors_get` was removed. The fix is mechanical but real work.
2. **The required Zig version moves with the pin** (0.15.2 → 0.16) and must match major.minor.
   Pin Zig alongside Ghostty.
3. **`RustPngDecoder` is unusable**, as described in the table.
4. **`ClipboardContent.data: &str` loses binary data** (0.2.x).
5. **Mode 2027 is off by default.**
6. **There is no `Send` story**, and the callback boxes are not `Send`.
7. **OSC 1337 `File=` is swallowed, even at main.**
8. **The published build.rs prints `rerun-if-changed=crates/libghostty-vt-sys/build.rs`**, a
   workspace-relative path that does not exist in the packaged crate. It was harmless here
   (no-op builds stayed `Fresh`).
9. **Without `ZIG_GLOBAL_CACHE_DIR`, Zig writes `~/.cache/zig`.**

## Unsafe policy per gap (what I'd do)

- **Contribute to libghostty-rs.** That covers the safe wrappers for `UNKNOWN_SEQUENCE`,
  `PROGRAM_STATUS`, `SEMANTIC_PROMPT` and `RENDER_HOLD`, the binding bump to main, and `Send`.
  Notification, progress and snapshot are already on master.
- **Keep one small FFI module in temor-vt meanwhile.** It holds the `Send` wrapper and those four
  callbacks, roughly 150 lines, and is deleted when upstream lands.
- **Do it in Rust:** our own PNG decoder, a safe trait impl with no `unsafe`.
- **Do it in Ghostty upstream:** report unimplemented OSC 1337 keys to UNKNOWN_SEQUENCE.

## Files

- `Cargo.toml`, `rust-toolchain.toml` (1.98.1), `env.sh`
- `src/main.rs`: the probe, 47 checks
- `src/ffi_gap.rs`: the only `unsafe`
- `src/bin/send_check.rs`: the expected compile failure
- `main-pin/`: part B, raw FFI against Ghostty main
  - `env.sh`: Zig 0.16 and `target-b`
  - It uses the copy of libghostty-rs master at `/mnt/data/cargo/p0-temor/rs-main`, with
    regenerated `bindings.rs`.
- Outside the repo, under `/mnt/data/cargo/p0-temor/`:
  - `ghostty-src`: a887df4
  - `ghostty-main`: a worktree at 7ec9e26
  - `zig-0.15.2`
  - `zig-global-cache/p`: the offline store for a887df4
  - `zig-global-cache-016`
  - `target-a`, `target-b`, `target-a-gc`
