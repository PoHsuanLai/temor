# Spike P0(b): libghostty grids and kitty images in quire `TextureLayer`s

Date: 2026-10-09. Plan refs: D1, D4, D13, §4b, §5.

- **Machine.** Fedora 44, KDE Plasma (KWin, Wayland). One 3840x2160 output at scale 2.
- **GPU.** The window got the RTX 5070 Ti (Vulkan, discrete).
- **Toolchain.** Rust 1.98.1, release builds.
- **Load.** The box was shared, with a load average of 21–29 during the runs (logged per run in
  `measure.log`).

## Verdict

**The risky path works.** In one real window there are:

- a quire `SplitView` with two panes
- a libghostty-vt terminal in each pane, painted into its own `TextureLayer`
- kitty images
- an anyview peek in the third region, drawing its own `TextureLayer`

Both renderer strategies draw the same picture.

**Recommendation: build the GPU renderer, but not on the terminal's I/O thread. Ask quire for a
render hook.**

- **The GPU path costs far less CPU.**
  - Process CPU: 13–22% of one core, against 34–49% for the CPU path.
  - Per-pane thread: 2–7%, against 12–20%.
  - Upload: 50–470 KB per frame, against 4.6–6.2 MB.
  - The CPU path's cost grows with pixels; the GPU path's does not.
- **But submitting from a pane thread stalls.** `queue.submit` on the window's queue blocks for
  up to one display refresh (p50 1–7 ms, p95 10–16 ms). The window's own present seems to hold
  the queue (see Problems 1).
- **The CPU strategy is the safe fallback.** It works today with predictable latency: a
  keystroke reaches a ready texture in about 0.1 ms. It is fine at these sizes.

## What was built (`/home/pohsuanlai/temor/spikes/render`)

### The project

- Standalone Cargo project with its own `[workspace]` and toolchain 1.98.1.
- `.cargo/config.toml` copied from temor.
- Build output: `CARGO_TARGET_DIR=/mnt/data/cargo/p0-temor/target-render`, set by `env.sh`.
- Zig's cache stays under `/mnt/data/cargo/p0-temor`.
- Nothing was written under `$HOME`. Runs point `XDG_CONFIG_HOME` and `XDG_CACHE_HOME` at a
  scratch directory, and the app has no settings store.

### Quire and anyview pins

- quire is pinned as asked: `git = "https://github.com/PoHsuanLai/quire.git", rev = 366dcd9`.
- The `[patch.crates-io]` block is copied from CONSUMING.md §1 and `docs/workspace-deps.toml`.
- `anyview-peek` and `anyview-core` come from `github.com/PoHsuanLai/anyview@6f27d11`.
- anyview pins quire `db32189` under the same `.git` spelling. P0(d)'s `www.github.com`
  redirect patch unifies the two: `Cargo.lock` holds **one** `ds`, at 366dcd9.

### `src/lib.rs`: the window

- `ds_blitz::launch` → `Ds` → `SplitView` with two `SplitPane`s, each holding a `TermPane`. The
  rest region holds `PeekRegion`, which is anyview `Pane` on `sample.png`.
- Each `TermPane`:
  - is a parent `div` with the `onclick`
  - wraps its `TextureLayer` (`TextureFit::Actual`, `Sampling::Nearest`) in a
    `pointer-events:none` wrapper
  - measures its box with `use_rect` × `HostSignals.scale`
  - starts one worker thread when `use_gpu()` has a device
- A watchdog calls `AppHandle::quit` at `--lifetime`, and forces an exit 2 s later.

### The worker thread (one per pane)

Because the crate's terminal types are `!Send` (P0(a)), one thread owns the Terminal, the
RenderState and the renderer. It:

1. installs our PNG decoder (the decoder slot is thread-local)
2. sizes the terminal to whole cells of its box
3. turns mode 2027 on
4. writes the canned stream (`stream.rs`):
   - 256-colour and truecolour SGR
   - bold, italic, bold-italic, underline, double and curly underline, strike, faint, inverse
   - wide CJK
   - emoji, including skin tone and ZWJ
   - a box-drawing table
   - a kitty PNG (`a=T`, chunked, `c=16,r=7`) placed at row 10, column 30
5. runs a 60 Hz timeline:
   - scroll one line per frame (pane 2 only, for 1 s, or both panes for 3 s)
   - then 36 single-character "keystrokes"
   - then idle
6. reflows when the box changes.

### `grid.rs`: the frame model

- `begin_update` then `end`, timed separately.
- Only the rows the render state marks dirty are walked, plus the old and new cursor rows.
- For each cell it reads the graphemes, style, and resolved fg and bg, and skips spacer cells.
- Kitty placements are collected by layer through `placement_render_info`, with image pixels
  premultiplied once and cached per (id, generation).

### `font.rs`: fonts and atlases

- Faces:
  - quire's code face, Space Mono 400 and 700, taken from `ds::style::fonts::FACES`
  - fallbacks found through fontique: Adwaita Mono, Noto Sans Mono CJK, Noto Color Emoji,
    Symbola
- swash shapes multi-codepoint graphemes and rasterises, with synthetic italic as a skew.
- Glyphs are shelf-packed into a 2048² alpha atlas and a 1024² premultiplied colour atlas.
- Box-drawing characters (light, heavy and double, U+2500 subset) are drawn procedurally to
  fill the cell.

### `cpu.rs`: strategy (i)

- Composes only the dirty rows into a premultiplied `Rgba8Unorm` buffer, in this order:
  1. default background
  2. `BELOW_BG` images
  3. cell backgrounds
  4. `BELOW_TEXT` images
  5. glyphs and decorations
  6. cursor
  7. `ABOVE_TEXT` images
- Then uploads either way:
  - **`--upload rows` (default).** Our own texture is registered with `handle.replace`. Each
    contiguous run of dirty rows goes up with `queue.write_texture`, then `handle.redraw()`.
  - **`--upload full`.** `handle.update(&Pixels::new(Rgba8Premultiplied, …))`, the example's
    way.

### `gpu.rs`: strategy (ii)

- Renders on the window's own device (`gpu.device()` and `queue()`).
- The target is a `TextureBinding | RenderAttachment` texture registered with the layer.
- One instanced quad pipeline (WGSL) with four kinds: solid, alpha-atlas glyph, colour-atlas
  glyph, and image sampled from its own bind group.
- Per-row instance lists are rebuilt only for dirty rows.
- Each frame:
  1. sync new atlas regions to the device
  2. one `write_buffer` of the instances
  3. one render pass: clear, `BELOW_BG` images, backgrounds, `BELOW_TEXT` images, text, cursor,
     `ABOVE_TEXT` images
  4. `submit`, then `handle.redraw()`
- With timing on, it waits for `on_submitted_work_done`. `--no-gpu-wait` turns that off.

**So quire's API does let an app render into a layer's texture on the window's device.**
`use_gpu()` gives the device, `gpu.register` or `handle.replace` registers the texture, and
`handle.redraw` shows it. That is a full GPU path, not only CPU pixels.

### The other files

- `stats.rs`: per-frame records, CPU from `/proc` (process, pane thread, UI thread = main tid),
  and memory.
- `src/bin/vtbench.rs`: a headless `vt_write` benchmark.
- `tests/harness.rs`: the same `App` on ds-harness with `Backend::Hybrid` (the window's
  vello_hybrid renderer on the real GPU). It covers:
  - the composition picture
  - clicks in both modes
  - a resize
  - 150% scale
- `measure.sh`: the 14-window matrix. Its output is `measure.log`.

## Screenshots (in `shots/`)

**`spectacle -b -n -a` did not capture our window.** The active window was the owner's
fullscreen app, and KWin's focus-stealing prevention kept our new window behind it. So:

- `-a` captured the owner's screen. **I deleted that file at once**; it is not in the repo.
- `-f` would show only the owner's window.
- Raising our window would mean a focus action or KWin scripting, and the coordinator ruled out
  any synthetic input.

The evidence is therefore:

- **Read-backs from the live window's own device.** `shots/live-{cpu,gpu}-{80x24,200x60}-steady-pane{1,2}.png`
  are each layer's texture copied back (`copy_texture_to_buffer`) from the real window's wgpu
  device, mid-run. They show:
  - text, colours, bold, italic and bold-italic
  - underline, double and curly underline, strike, faint, inverse
  - wide CJK
  - the box table
  - the kitty image beside it
  - the cursor

  CPU and GPU match.
- **The whole document in quire's harness, on the same renderer and the real GPU.** These
  pictures show the split, both terminal layers and the anyview peek (its picture is anyview's
  own `TextureLayer`):
  - `shots/harness/harness-{cpu,gpu}-80x24.png`
  - `shots/harness/resized-harness-{cpu,gpu}-80x24.png`, after a resize
  - `shots/harness/harness-gpu-80x24-scale150.png`, at 1.5x

  I checked every one with the Read tool.

**What the window itself does show:**

- The window opened and ran 14 times with no panic.
- Each run exited 0 within 7–8 s.
- The UI thread used 6–14% of a core while the panes animated, so the window was painting, not
  throttled.
- nvidia-smi listed its GPU memory.

## Numbers

**How the runs were set up:**

- Per pane, both panes animating at 60 Hz (180 frames each).
- Scale 2. At 80x24 the cell is 16x40 device px (13 px Space Mono) and the texture is
  1280x960. At 200x60 the cell is 7x19 (6 px) and the texture is 1400x1140.
- "frame" is the pane thread's CPU time from the start of `vt_write` to the texture being ready
  with `redraw` called. It excludes any GPU wait.
- Two runs per configuration. The ranges cover both panes.

**Frame cost per strategy:**

| | CPU, dirty rows uploaded | CPU, `handle.update` full frame | GPU (`--no-gpu-wait`) |
|---|---|---|---|
| **80x24, scroll 1 line/frame** | p50 2.0 ms, p95 2.8–3.0 | p50 3.0, p95 4.0–4.1 | p50 4.3–4.6, p95 9.8–15 |
| — paint / build instances | 1.35–1.39 | 1.66–1.68 | 0.03 |
| — upload (+ submit for GPU) | 0.48–0.50 (4.6 MB) | 1.05–1.08 (4.8 MB) | 4.7–5.8 (82 KB), of which submit is nearly all |
| **200x60, scroll 1 line/frame** | p50 3.0–3.3, p95 4.4–4.9 | p50 3.2–3.5, p95 4.7–4.9 | p50 3.8–5.7, p95 14.5–16 |
| — paint / build instances | 1.46–1.50 | 1.48–1.50 | 0.10 |
| — upload | 0.41–0.63 (4.8 MB) | 0.71–0.97 (6.2 MB) | 5.4–7.0 (408 KB) |
| **200x60, 20 lines/frame (`cat`)** | p50 3.25, p95 4.8 | | p50 6.8–7.3, p95 16.7 |
| **Keystroke (1 dirty row), vt→ready** | p50 0.07–0.12 ms, p95 ≤ 0.26 | p50 0.42–1.14 | p50 1.4–14 (the submit stall) |

**Process CPU, % of one core:**

| | CPU, dirty rows | CPU, full frame | GPU |
|---|---|---|---|
| 80x24, scroll 1 line/frame | 34% (pane threads 12% each, UI 10%) | 47% (17%, 14%) | 13–15% (2%, 8–9%) |
| 200x60, scroll 1 line/frame | 41% (15%, 9%) | 45% (18%, 10%) | 20–21% (6%, 8–9%) |
| 200x60, 20 lines/frame | 49% (20%, 9%) | | 22% (7%, 8%) |

**The terminal core is cheap** once it is built ReleaseFast (see Problems 2):

- `vt_write` is 9–18 µs per scrolled line, and 70 µs for 20 lines.
- `begin_update` is 7–17 µs. That is the part held under the pane lock in §4b.
- `end` is 1–4 µs.
- Walking every row costs 0.16–0.18 ms at 80x24 and 0.67–0.70 ms at 200x60. That is mostly
  per-cell FFI getters.
- The demo stream, kitty PNG decode included, takes 0.2 ms.

**Actual GPU work is small.** With a single pane rendering (the screenshot runs), submit took
0.05 ms and GPU completion 0.21–0.24 ms. The 4–15 ms "gpu" figures in the wait-mode runs
(`measure.log`) are queue waiting, not rendering.

**Latency from `vt_write` to the presented frame:**

- **Measured:** `vt_write` to texture ready.
  - CPU strategy: p50 2 ms while scrolling, about 0.1 ms per keystroke.
  - GPU strategy: GPU done 0.5 ms when uncontended, otherwise up to 16 ms.
- **Not measurable:** presentation. quire has no presented-frame signal, and wgpu timestamp
  queries need a feature that `AppConfig` cannot request (§5 known limit).
- **Expected:** add the window's next paint (at most one refresh, 16.7 ms at 60 Hz) plus
  KWin's compositing.

**Memory:**

| | CPU strategy | GPU strategy |
|---|---|---|
| VmHWM (whole process: Blitz, fonts, anyview, two terminals) | 343–410 MB | 327–350 MB |
| VmRSS at exit | 245–291 MB | 233–270 MB |
| nvidia-smi GPU memory (whole process) | 115–149 MiB | 115–121 MiB |
| Ours per pane, on the CPU side | 13.5 MB (frame buffer 4.9 MB at 80x24 plus atlases 8.4 MB) | 8.6 MB (atlases) |
| Ours per pane, on the device | 4.9–6.4 MB (one texture) | 13.4–15.3 MB (atlases, target, instances) |

The atlases are oversized for this content (100 glyphs). A shared, growable atlas per window
would cut both columns.

**Build:**

- Clean release dependency build: 3 m 58 s.
- Rebuilding libghostty ReleaseFast: 54 s.

## Checks

### Resize and reflow

- **The window's own resize did nothing.** `use_window_sizer().request_size()` asked for
  1608x482 → 1286x401. KWin never answered: `SizeRequest::Expired`, origin `None`, and the box
  never changed. That was for a background, unfocused window, and it was tried 4 times.
- **Window sizes set in code work.** Launching at the sizes computed for 80x24 and 200x60 gave
  exactly 80x24 and 200x60 per pane, with 4x4 device px of padding.
- **Reflow works.** I drove it through ds-harness `resize_window` (a person's drag, which the
  coordinator allows). Both panes went from 80x24 to 68x20 in a 1093x812 device-px box, with
  5x12 px of padding. `vt resize` took 60–150 µs, and the 256-colour rows and scrollback
  reflowed (`resized-harness-*.png`).
- **The kitty image moved after reflow.** It slid left over the table: its anchor, on a
  trailing blank cell at column 30, did not survive the reflow. This needs a note to Ghostty or
  libghostty.

### Fractional scale

- **The owner's output is a whole 2x.** I did not change the owner's display scale.
- **2x is crisp.** Glyphs are rasterised at device size, and `TextureFit::Actual` maps one texel
  to one device pixel.
- **1.5x, checked in the harness:**
  - the cell is 12x30 and the box 963x723 → 80x24 with 3x3 of padding
  - at 500% zoom, both panes' text is pixel-identical and sharp
  - `Actual` floors its centring offset, so a fractional box origin did not blur

### The click bug (Blitz custom widget, quire v0.2.30)

I tested in ds-harness, clicking the centre of each pane.

| Mode | Result |
|---|---|
| **direct** (layer hit-tested, handler on the parent) | **0 of 2** clicks fired |
| **workaround** (`pointer-events:none` on the layer's wrapper, handler on the parent) | **2 of 2** fired |

The bug is confirmed, and so is the workaround. `pointer-events:none` is inherited by the
`<object>` the layer renders.

### Several `TextureLayer`s next to anyview's

One window held four layers with no conflict:

- two terminal layers, each replaced or resized at runtime
- anyview's picture layer (made with `gpu.upload` on the same device)
- quire's `GpuProbe`

Our registrations and anyview's coexist, and resizing replaced our textures cleanly. No
validation errors and no lost frames appeared in any run.

## Problems

1. **`queue.submit` from a worker thread stalls on the window's present.**
   - A one-row frame's submit takes 0.05 ms when uncontended, but 1–16 ms otherwise. That range
     matches one refresh.
   - Staggering the two panes by half a frame did not help (p95 still 14–16 ms), so the
     contention is the window's render thread (acquire and present under FIFO), not the panes.
   - `queue.write_texture` from the same threads does not stall (0.03–0.6 ms).
   - **Consequence:** never submit GPU work from a terminal I/O thread. Encode and submit on
     the window's render path. Until quire offers that, use one per-window render thread.
2. **A Debug libghostty passes silently for "release".**
   - libghostty-vt-sys's build.rs picks Zig `Debug` whenever cargo sets `DEBUG=true`, and any
     debuginfo setting does that, even in the `release` profile.
   - The result: `vt_write` costs **8.6 ms per scrolled line**, against **1 µs** at
     ReleaseFast. The `.a` is 42.8 MB against 15.6 MB.
   - The fix here is `[profile.release.package.libghostty-vt-sys] debug = false`.
   - **For temor-vt-sys (D1):** choose Zig's optimize mode from `OPT_LEVEL` and never from
     `DEBUG`.
3. **Colour emoji are missing.**
   - Fedora ships Noto Color Emoji as **COLRv1 only**, and swash cannot draw COLRv1.
   - fontique lists "Noto Emoji" (monochrome) with **no fonts** in the family.
   - The spike falls back to Symbola: monochrome, and 👍🏽 and ZWJ families split into pieces.
   - **For temor:** paint COLRv1 through skrifa's colour painter into vello_cpu (already in the
     tree), or bundle a CBDT emoji font.
4. **quire's code face is not a terminal face.**
   - The Space Mono subset has no box drawing, CJK or emoji.
   - Its line height is about 1.5 (the cell is 16x40 at 26 device px).
   - The spike draws box drawing procedurally (needed anyway, as Ghostty and kitty do) and falls
     back for the rest.
   - Ask quire for a terminal code-face token with a tighter line height, or let temor set its
     own.
5. **The `Ds` root is only as tall as its content.**
   - The split was 381 px tall in a 482 px window (sized by the peek's content).
   - `height:100vh` on the app's own container fixed it.
   - Either document this or make the root fill the window.
6. **`placement_render_info` needs `&Terminal`, not the render-state snapshot.**
   - So in the §4b split, image placements must be read during the locked `begin` phase.
   - The cost is small, but it is not lock-free.
7. **The spike leaves out some things a real renderer needs:**
   - SGR 58 underline colour
   - font ligatures, and shaping across cells
   - atlas eviction
   - cursor styles beyond a block
   - selection
   - blink
   - hyperlink hover
   - per-window shared atlases
8. **Threading in the spike is simplified.** Terminal and render share one thread (the
   crate's types are `!Send`), so the §4b I/O-thread/render-path split was not exercised here;
   P0(a) proved it headless.

## Quire asks

1. **A render hook for a texture layer.** Let an app encode into its texture on the window's
   render thread, inside the frame, before the window's own submit. For example
   `TextureHandle::on_frame(|device, queue, encoder| …)`, or `Gpu::before_submit`. This removes
   Problem 1 and orders the terminal's draw with the frame.
2. **A presented-frame signal per handle or per window**, with the present time. temor needs it
   for real latency numbers and for RENDER_HOLD (mode 2026) pacing.
3. **`AppConfig` requesting wgpu features and limits**, for `TIMESTAMP_QUERY` at least. This is
   already a known limit.
4. **The custom-widget hit fix (v0.2.31).** Confirmed: until it lands temor keeps
   `pointer-events:none` on the layer.
5. **A root that fills the window,** or documentation of `100vh`.
6. **A terminal code-face token** (see Problem 4).
7. **`request_size` on KWin.** It expired for a background window here; worth a check with a
   focused one.

## Reproduce

```sh
cd /home/pohsuanlai/temor/spikes/render
. ./env.sh && cargo build --release
$CARGO_TARGET_DIR/release/render-spike --strategy gpu --grid 80x24 --animate both --lifetime 7   # closes itself
./measure.sh                                                    # 14 windows, about 2 min, writes measure.log
cargo test --release --test harness -- --nocapture --test-threads=1   # composition, clicks, resize, 1.5x
$CARGO_TARGET_DIR/release/vtbench                               # the vt_write cost headless
```

Flags:

- `--strategy cpu|gpu`
- `--grid CxR`
- `--animate both|second`
- `--anim-secs N`
- `--lines N`
- `--upload rows|full`
- `--no-gpu-wait`
- `--resize`
- `--shots DIR`
- `--click parent|direct`
- `--stagger`
- `--font PX`
- `--scale F`
