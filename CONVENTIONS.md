# Conventions

temor follows quire's `CONVENTIONS.md` and `CONSUMING.md` (the four coherence rules) without
local exceptions. The plan is `PLAN.md`; decisions there win over anything older.

Additions for temor:
- **Layers (PLAN §2, D23).** L0 standalone libraries, L1 headless engines, L2 the pure app model,
  L3 the app. A crate depends only on its own or lower layers; `scripts/check-layers.sh` is part of
  the gate. L0/L1 never depend on quire, Dioxus or docket.
- **Every L0/L1 crate is a library someone else could use:**
  - A small public API re-exported at the crate root; `pub mod` only for real namespaces; nothing
    app-specific (no temor policy baked in: defaults are injected or have a builder).
  - Crate docs (`//!`) say what it is and include a runnable example (doctest).
  - Public enums that can grow are `#[non_exhaustive]`; public types implement `Debug`, and `Clone`,
    `PartialEq`, `Eq`, `Hash`, `Display` where they make sense; errors implement `std::error::Error`
    and `From` for the errors they wrap.
  - Constructors with more than three parameters take a builder or a params struct.
  - Closed sets use plain `FromStr`/`Display`/serde, not quire's `Word` (the library-layer
    exception to quire's convention). L2/L3 keep quire's rules.
  - `Send`/`Sync` is stated in the docs of every type that crosses threads. Libraries never spawn
    threads; they expose a `pump`/`step` that the caller drives.
  - Optional dependencies sit behind features; `--no-default-features` builds.
- Only `temor-vt` names libghostty; only `temor-desktop` names zbus or docket (`PLAN.md` §2).
- `cargo check --workspace --no-default-features` must pass (portable core, design/36).
- Build output goes to `/mnt/data/cargo/<lane>-temor`, never under `~`.
- Commits: author and committer `Po Hsuan Lai <pohsuanlai0208@gmail.com>` through `GIT_*` env vars,
  with no session, co-author or URL trailers.
- Tests never touch the real system: private bus, scratch `HOME`, no real `~/.config`.
