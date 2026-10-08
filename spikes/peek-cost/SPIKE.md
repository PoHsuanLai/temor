# P0(d): what `anyview-peek` adds to temor's build

Date: 2026-10-09. Plan refs: D13, §10, §11 P0(d).

## Setup

| Project | What |
|---|---|
| `base/` | Minimal quire consumer shaped like `quire/examples/consumer`: `ds`, `ds-settings` (`dioxus`), `ds-blitz`, `dioxus`; one `ds_blitz::launch`, a `Ds` root with `use_environment` and a paragraph. quire by git at v0.2.30 = `366dcd97ebc26003f466d20fa12123f783180653`. `[patch.crates-io]` copied from quire v0.2.30's root (vello/anyrender forks and the dioxus-core fork). |
| `with-peek/` | The same plus `anyview-peek` and `anyview-core` by git at anyview `6f27d117269f514d69c4c4eaa64a5a72e5b96720` (default features, so `media` is on). `main` peeks `argv[1]` with `anyview_peek::peek` and draws `Pane { peeked }`. |
| `with-peek-nomedia/` | Count-only (`cargo tree`, never built): `anyview-peek` with `default-features = false`. |
| `base-pdf/` | Count-only: `base` with `ds-blitz` feature `pdf`, to split pdfrum's share from anyview's. |

Toolchain 1.98.1 everywhere (`rust-toolchain.toml`). Both apps compile. Neither was run.

### The quire version mismatch, and how it was fixed

anyview pins quire at `db321897` (v0.2.28 plus 5 commits, an ancestor of v0.2.30) and spells the URL
`https://github.com/PoHsuanLai/quire.git`. Without a fix the graph has two quires. A
`[patch."…/quire.git"]` pointing at `…/quire` is refused: Cargo canonicalises both URLs to the same
source ("patch for `ds` points to the same source"). What worked, in `with-peek/Cargo.toml`:

```toml
[patch."https://github.com/PoHsuanLai/quire"]   # covers both spellings
ds = { git = "https://www.github.com/PoHsuanLai/quire", rev = "366dcd97…" }
# … same line for ds-core, ds-core-derive, ds-style, ds-motion, ds-behaviour, ds-blitz,
#     ds-settings, ds-settings-derive, ds-desktop, ds-intents
```

The `www.` host is a different canonical source, and GitHub redirects it to the same repo. Every
`ds-*` crate in the lockfile now comes from one source at v0.2.30, and anyview-peek builds against
v0.2.30 unchanged. For temor proper the better fix is for anyview and temor to pin the same quire rev
**and the same URL spelling** (both with or both without `.git`). A rev-only mismatch can then be
fixed with one ordinary patch, or not happen at all.

Feature check: anyview's workspace enables `zbus/tokio`, but nothing `anyview-peek` pulls turns it on.
zbus keeps its default `async-io` backend, as quire's CONSUMING.md §1 requires.

## Numbers

Machine: 32 cores, heavily shared. Load average was 35–71 throughout (`uptime` before and after each
build is in `measure.log`). Builds ran through the agent cargo shim: 8 jobs, `CPUWeight=idle`, one
build at a time. sccache was bypassed (`RUSTC_WRAPPER=.tools/nowrap.sh`), each build had its own new
`CARGO_TARGET_DIR` (`/mnt/data/cargo/p0-temor/target-d-<project>-<profile>`), and the four builds ran
one after another (`measure.sh`). The times are cargo's own "Finished in" (queue wait excluded).
Expect about ±20% noise: a later rebuild of `base` release by cargo-bloat took 1m55s against 2m28s.

### Crates (`cargo tree -e normal`, unique name+version, root excluded)

| | base | with-peek | delta |
|---|---|---|---|
| normal deps | 414 | 585 | **+171** |
| normal + build deps | 464 | 630 | +166 |
| Cargo.lock packages | 600 | 773 | +173 |
| with-peek, `media` off | | 563 | +149 |
| base + `ds-blitz/pdf` only | 461 | | +47 |

So anyview's "about 600" is mostly shared: about 414 come with any quire app. Of the 171 new crates:
- **47** come from quire's own `ds-blitz` feature `pdf` (pdfrum plus its crypto and font-subsetting
  stack). peek turns it on for `PdfThumb`. They are shared if temor ever enables `pdf` or `print` itself.
- **124** come from anyview proper: archive (zip, 7z, tar, bzip2, lzma, zstd), book, font, image
  (jxl-oxide, exr, tiff, psd, qoi, resvg, **ravif + rav1e**, an AV1 *encoder*), text (syntect,
  fancy-regex, calamine, csv, pulldown-cmark).
- **22** of those come from the `media` feature (symphonia ×13, mp4parse, matroska-demuxer).

Added duplicate versions:
- read-fonts 0.39 and 0.44 (base has 0.41)
- skrifa 0.42 and 0.47 (base has 0.44)
- font-types 0.11
- vello_common and vello_cpu 0.2.0: pdfrum's raster pins crates.io vello 0.2, which is **not**
  covered by quire's vello fork patch (0.1). So two vellos are linked.
- moxcms 0.9, weezl 0.2, digest 0.11, thiserror 1, hashbrown 0.13

### Clean build time (cargo "Finished")

| profile | base | with-peek | delta |
|---|---|---|---|
| dev | 1m02s | 1m23s | +21 s (+34%) |
| release | 2m28s | 3m53s | +85 s (+57%) |

### Binary size (bytes)

| | base | with-peek | delta |
|---|---|---|---|
| release, unstripped | 51,337,440 | 74,800,080 | +23.5 MB (+46%) |
| release, stripped | 37,296,648 | 55,635,672 | **+18.3 MB (+49%)** |
| dev, unstripped | 753,918,976 | 1,107,552,096 | +354 MB |
| dev, stripped | 91,299,936 | 144,977,536 | +53.7 MB |

The `.text` section (cargo-bloat) grows from 23.0 to 33.8 MiB, +10.8 MiB. The rest is mostly
`.rodata`: syntect's default syntaxes, font tables, and similar data.

### Largest new code

**In the binary.** These are the top `.text` deltas per crate from cargo-bloat (estimates).
- jxl_render 818K
- image 728K (grew)
- std 512K
- skrifa 508K
- regex_automata 438K (grew; syntect turns on more regex features)
- anyview_peek 343K
- jxl_vardct 337K
- ds 293K (grew: the `pdf` paths)
- vello_cpu 273K
- pdfrum_font 266K
- anyview_archive 241K
- anyview_text 239K
- moxcms 236K
- pdfrum_render 222K
- regex_syntax 220K
- read_fonts 217K
- pdfrum_page 213K
- pdfrum 205K
- exr 205K
- jxl_frame 185K
- aho_corasick 179K
- symphonia_metadata 174K
- symphonia_format_isomp4 160K
- vello_common 134K
- pulldown_cmark 128K
- calamine 119K
- tiff 118K
- mp4parse 113K
- pdfrum_parser 110K

rav1e and ravif link to ~0: the AVIF encoder is dead code for peek and costs build time only.

**Compiled artefacts.** These are release rlib/proc-macro growth versus base, which is what costs
build time.
- read_fonts 28.9 MB
- write_fonts 15.2
- pulp 12.4
- skrifa 10.0
- moxcms 8.8
- regex_automata 7.8
- exr 7.8
- rav1e 7.6
- regex_syntax 7.3
- pdfrum_edit 6.8
- jxl_render 6.1
- symphonia_core 5.3
- raw_cpuid 5.1
- nom 4.8
- anyview_image 4.3
- pdfrum_page 4.1
- anyview_peek 4.0
- thiserror_impl 3.9
- syntect 3.9
- calamine 3.8
- pdfrum_doc 3.7
- typenum 3.7
- ravif 3.7
- pdfrum_font 3.5
- arg_enum_proc_macro 3.4
- vello_cpu 3.4
- equator_macro 3.4
- num_bigint 3.2
- pdfrum 3.1
- pulldown_cmark 3.0

Total rlib growth is 1226 → 1527 MB, +301 MB.

### Native / system libraries

`ldd` of the two release binaries is **identical**: libc, libm, libgcc_s, fontconfig, freetype,
harfbuzz, graphite2, glib, pcre2, png16, xml2, z, bz2, lzma, brotli. All of them come from the quire
baseline. peek adds **no** new shared library: no libasound, libdav1d, ffmpeg or libmpv. Every new
codec is pure Rust; libbz2-rs-sys and zlib-rs are Rust ports. None of the new crates declares `links`.
The new build scripts are rav1e, av-scenechange, pulp, ahash, getrandom, paste, pulldown-cmark,
thiserror and two pdfrum crates. None of them needs nasm or a C toolchain in this configuration.

### Licences of the 171 new crates (`cargo metadata`)

All are MIT, Apache-2.0, BSD, ISC, Zlib, Unicode or CC0, except:
- **MPL-2.0**: symphonia (13 crates) and mp4parse. This is file-level copyleft and allowed by the
  brief's list. Turning `media` off removes all of them.
- **`bzip2-1.0.6`**: libbz2-rs-sys. This is the permissive bzip2 licence (BSD-like), not on the list
  by name. Flagged for a `deny.toml` allow entry, not a blocker.

anyview-* and pdfrum-* are `MIT OR Apache-2.0`. Nothing GPL or LGPL, and nothing unlicensed.

## Recommendation

**Link `anyview-peek` in-process behind a default-on feature (D13 as written), on and off the
desktop. Keep D-Bus `Open` as the fallback when the feature is off. Don't build a D-Bus Peek path.**

Why:
- The cost is real but bounded and harmless: +18 MB stripped, +171 crates, about +1.5 min on a clean
  release build, and +20 s dev. No native libraries and no licence problems.
- Incremental rebuilds of temor won't touch these crates.
- A D-Bus Peek would save the 18 MB only on our desktop. It would add a process hop and a
  serialisation format for `AnyPeeked` (pixels included) on the hover path. It would also do nothing
  for `anyview-pane`, which must be in-process anyway (§10 viewer pane).
- Two paths would mean two behaviours to test.
- D16 (single binary vs dylib at desktop level) is where the 18 MB gets shared with anyview, not here.

Follow-ups, cheapest first:
1. Pin temor's quire to the same rev and URL spelling anyview uses, or ask anyview to drop `.git`. The
   `www.` patch above is a spike workaround, not something to keep.
2. Ask anyview to make the AVIF encoder (ravif/rav1e) optional in `anyview-image`, or move it out of
   the peek path. It is build time with no linked code.
3. Ask quire/pdfrum to move pdfrum's raster onto the patched vello (0.1 fork). The vello 0.2 copy is
   a quire-side duplicate that any `pdf` user pays for.
4. Optional: temor's `peek` feature can forward `anyview-peek/media`. With it off, 22 crates and all
   MPL code go away, and audio and video peek as facts only.
5. Measure `anyview-pane` again when it lands. Expect it to share nearly all of this.

## Files

- `base/`, `with-peek/`: the two measured apps (own `[workspace]`, `Cargo.lock`, `rust-toolchain.toml`).
- `with-peek-nomedia/`, `base-pdf/`: count-only variants.
- `measure.sh`, `measure.log`: the timed sequential clean builds and their `uptime` lines.
- `.tools/`: `cargo-bloat` 0.12.1 and the passthrough `RUSTC_WRAPPER`.
