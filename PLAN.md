# temor: plan

temor is the desktop's terminal. Settled with the owner on 2026-10-09, after briefings from the
whopper, agent and anyview sessions and two feasibility spikes. Everything below is one plan,
ordered by dependency (§11), not split into versions.

## 0. Context

### What temor is
- **A terminal built like a browser.** It takes Arc's and Dia's model:
  - Spaces are projects.
  - Persistent vertical tabs sit in the sidebar.
  - A tab shows its splits.
  - ⌘T opens a command bar.
  - There is a downloads shelf.
- **Built on libghostty-vt.** We get the modern protocols (kitty graphics, the kitty keyboard
  protocol, OSC 133/7/8/9/52/777/7501, synchronized output) and draw everything with our own
  renderer.
- **The best place to host agents.** That covers:
  - agent panes, with a shared status radar
  - git checkpoints with Undo all
  - command proposals that land in your own prompt and are checked first by a sandboxed trial run
- **IDE-like viewing through anyview:**
  - viewer panes in splits
  - Quick Look peeks on paths
  - stepping through `ls`/`rg` results in the viewer

### Decisions
| # | Decision |
|---|---|
| D1 | Terminal core: **libghostty-vt**, at **Ghostty main pinned by commit** (spike P0(a): `7ec9e26`, with Zig 0.16). The crates.io `libghostty-vt` 0.2.2 pins a July commit that lacks the notification, progress, 7501, unknown-sequence, OSC 133-with-exit-code and render-hold callbacks and the snapshot API. temor owns its FFI: a `temor-vt-sys` crate (bindgen output generated from the pinned headers and checked in; build.rs adapted from libghostty-rs's `-sys`, MIT, with attribution; offline through `GHOSTTY_SOURCE_DIR`, `GHOSTTY_ZIG_SYSTEM_DIR` and `ZIG_GLOBAL_CACHE_DIR`). The safe API is our own `temor-vt`, whose single `ffi` module is the only `unsafe` and includes the `Send` wrapper. Ghostty's full embedded API is macOS/iOS only, so we render ourselves. Fixes go upstream (libghostty-rs, Ghostty) only with the owner's OK. |
| D2 | **Spaces** use quire's kit (`ds_style::space::list::Spaces<P,R>`, `SpacesSource::Local`). A Space is a project and a mode switch: the sidebar shows only its tabs, and ⌃1–9 switches between Spaces. |
| D3 | **Tabs persist** until closed. They can be pinned or grouped in folders. "Recently closed" keeps their scrollback. |
| D4 | **Nested splits.** The tree is ported from Ghostty's `split_tree.zig` (MIT) and owned by temor. Dividers come from quire `SplitView`, extended with an axis, share-based sizes, nesting and an unfocused-dim token (whopper has queued this). |
| D5 | **Blocks** come from OSC 133. temor keeps the shell's own line editor, with no Warp-style input replacement. |
| D6 | **The AI never guesses a mode.** The shell prompt is only ever shell. AI appears only (a) as a proposal you asked for, typed into your prompt, where your Enter is the approval, (b) as ⌘T rows you pick, (c) in agent panes, (d) through the desktop companion. |
| D7 | **Proposals are trial-run first** in a bwrap overlay sandbox (spike: 21/21 bad commands caught, 0 false failures, about 10 ms overhead). There are no per-tool rules: flag checks against `--help`, path guessing and dry-run tables were dropped. |
| D8 | **"New agent" opens the gated agent view** (an ACP session through docket, drawn with quire companion components). An agent's own CLI in a pane is opt-in and labelled "ungated, sandboxed". Claude Code's pre-tool hook is bridged to our gate as an agent-adapter plugin, which gates the CLI pane. |
| D9 | **Checkpoints:** a git snapshot of tracked and untracked files is taken before an agent writes. Ignored files are labelled as not covered. Writes inside the project then count as `UndoableWrite`. |
| D10 | **Spaces carry profiles:** env vars, an SSH identity through a per-Space agent, and an optional dotenv. They apply to new shells only. |
| D11 | **The ⌘T command bar** ranks results by fixed rules, with AI rows always last. |
| D12 | **Downloads shelf:** the path is configurable, defaulting to temor's own data dir. It is fed by kitty file transfer (OSC 5113), iTerm2 `File=` (OSC 1337), `temor get`/`save`/`view -`, and "Save output". |
| D13 | **anyview:** `anyview-peek` is linked in-process. An embeddable viewer pane is being proposed by anyview to its owner. Both sit behind a default-on feature, and the fallback is D-Bus `Open`. |
| D14 | **Plugins** follow `agent-spec/plugins.md`, using the shared crate in the new public repo **bayonet** (extracted by anyview). temor's capabilities: `block.lens`, `open.provider` (shaped to match sill's launcher providers), `agent.adapter`, `row.facts`. temor has no format plugins: viewing belongs to anyview. |
| D15 | **Portable core** (design/36): every desktop extra sits behind the `quire-desktop` feature, and `--no-default-features` must build and pass. Off our desktop the trial says "not checked". |
| D17 | **SSH, Tier A (nothing installed on the host):** hosts are first-class (⌘T, per-Space pins, `host:path` in tabs). Shell integration and terminfo are sent with every connection, as kitty's ssh kitten does. One shared connection per host, so a split or new tab opens instantly in the same remote folder. Remote files support peek, view, and edit-locally-then-write-back, plus upload by drop. Port forwarding from the running row. Consent per host. |
| D18 | **SSH, Tier B (`temor-remote`, an optional helper uploaded to the host):** sessions survive disconnects and reattach with the exact screen; listening ports are detected and forwarded; remote file service for live viewer panes; trial runs on the host; local agents working on remote workspaces (the agent stays local, its file and terminal calls go to the host). Installed after consent per host; the version is pinned by hash, so it updates itself. Hosts that can't run it fall back to Tier A. |
| D19 | **Agent worktrees are temor's, not the agent's.** Starting an agent offers "on a new branch" (first, the default) or "in this checkout"; nothing guesses. temor creates the worktree in its own data dir, sets it up from project facts (an include list for ignored files, the Space's setup command), pins the agent's workspace to it through the gate, warns early about overlapping changes between agents with a `git merge-tree` dry run, and owns review and merge (default **Squash**). It separates files, not processes or ports, and says so. §8.2 |
| D20 | **Every agent pane shows its approval mode,** as two fixed facts read from the actual launch profile and gate session, never inferred: **Sandbox** (on/off) and **Asks** (everything / writes / nothing). A mode changes only by an explicit action on that pane and never carries over silently to other panes or windows. §8.2 |
| D21 | **Nothing goes to a model unseen.** Every AI request temor makes (a proposal, a ⌘T AI row, "Ask about @tab") first shows exactly what it will send, as removable chips with sizes, and the sent list is kept with the request's log record. Nothing is attached implicitly. §8.3 |
| D22 | **Tailscale through porter.** temor never talks to tailscaled. porter's accountd publishes `Tailnet1.Machines()`/`Changed` (typed reader in porter-client); the person's tailnet computers appear as SSH hosts in ⌘T ("Your computers", deduped against `~/.ssh/config`, offline ones dimmed). Connecting is still the system `ssh` to the MagicDNS name with the same sharing, bootstrap and helper; Tailscale SSH hosts need no keys, and its browser check shows as a running row with Open (never auto-opened). Tailnet ports open directly at `name:port` before offering a forward. Design: `~/rs-wt/accounts/tailnet-temor.md`. §9b |
| D16 | **Packaging:** temor is a lib plus a thin `main.rs`. Single binary vs dylib is decided later at desktop level. |

### Rules inherited
- Follow quire `CONVENTIONS.md` and `CONSUMING.md`, including the four coherence rules (§5): no literal
  colours, durations, fonts or radii; quire components only; anything missing goes to quire
  first; motion only through quire timers. Look: Mac-native (design/27).
- No developer words in the UI: never "ACP", "MCP", "OSC", "PTY" or "CLI".
- Tooltips: quire `Button { title }` for every icon-only or terse button, and `Tooltip`/`HoverTarget`
  for everything else; never hand-rolled, never raw `title` attributes. Short Mac wording, no trailing
  period.
- Commit identity comes from env vars (author and committer Po Hsuan Lai <pohsuanlai0208@gmail.com>),
  with no session or co-author trailers.
  Build output goes to `/mnt/data/cargo/<lane>-temor`. Tests never touch the real system
  (private bus, scratch HOME, harness jail). Never write `~/.config` or `/etc`.
- **OS-specific code lives at the edges, and the owner writes the non-Linux parts.**
  - Anything that differs by OS sits behind a small trait in the crate's `platform/` edge module:
    process info, the trial sandbox, the keymap's primary modifier, bitmap emoji, shell paths.
  - The Linux implementation is written; other OSes get a stub returning a typed `Unsupported` or
    absent value, marked `// TODO(platform:<os>)` and listed in `FOLLOWUPS.md` under "Platform (owner)".
  - No `cfg(target_os)` outside `platform/` modules. The owner implements and tests the stubs on
    macOS and other machines.
- Licences are permissive only. GPL code (mpv, ffmpeg, cosmic-term, cmux) is read for facts, never
  linked and never pasted.

## 1. Repositories and pins

| Repo | Role for temor | Pin |
|---|---|---|
| quire | ds, ds-style (Spaces), ds-settings, ds-blitz (TextureLayer, launch), ds-desktop, ds-intents, ds-harness, ds-lint | `git = "https://github.com/PoHsuanLai/quire.git", rev = "<full sha>"  # vX.Y.Z` (the desktop-wide form; batch 7: `366dcd97ebc26003f466d20fa12123f783180653`, v0.2.30) |
| blitz (fork, `quire-pin`) / blitz-kit | through quire's `[patch]` block, copied verbatim from `docs/workspace-deps.toml` | as quire |
| ghostty | libghostty-vt source, through the `libghostty-vt` crate (`GHOSTTY_SOURCE_DIR` for offline builds) | one commit, bumped by hand behind `temor-vt` tests |
| docket | `docket-dbus` proxies (Intents1, Confirm1, Companion1, IntentProvider1), `docket-ds`, docket-launch profiles | batch protocol |
| **bulkhead** (public; split from `docket/crates/docket-shell` by the agent session) | bwrap runner: `AgentRun`, `Overlay`, plus our new trial module | batch protocol |
| bayonet (public) | plugin envelope, discovery, framing, process handling. temor implements `manifest::Provides` and `run::Protocol`, then uses `discover::<P>(&Search::new("temor", …))`, `registry.serving(cap, fit)` and `Runner::new("temor", Timeouts::default()).open(..)`. The process side is Unix-only. Start from `docs/capabilities.md`, `examples/shapes/mod.rs` and `examples/demo_plugin.rs` | v0.1.0, rev `592eea9b69aea9bb9e4d6694d09322595f8e8e0a` |
| anyview | `anyview-peek`, `anyview-core::Sequence`, later the embeddable viewer pane | batch protocol |
| porter | `prov::{SpaceId, Actor, Effect}` types (through docket) | through docket |

## 2. Workspace layout

```
temor/
  crates/
    temor-core/       pure: vocabulary, split tree, tabs, space payload, blocks, presence,
                      command-bar ranking, proposal sanitising, every step machine
    temor-vt-sys/     bindgen output (checked in) plus a build.rs that builds the pinned Ghostty with Zig
    temor-vt/         the only libghostty-vt user: Terminal, RenderState, encoders, callbacks
                      → typed events. Trait `Vt` so tests run on a fake.
    temor-pty/        openpty and spawn, shell integration scripts (bash, zsh, fish), profile env
    temor-render/     fonts (fontique discovery, swash raster), glyph atlas, cell and image
                      passes on wgpu, painted into a quire TextureLayer
    temor-transfer/   OSC 5113 / OSC 1337 File= decoding, the shelf store
    temor-ui/         Dioxus views: window, sidebar (tabs, split minis, foot), panes, blocks,
                      command bar, shelf, proposal chip
    temor-ssh/        Tier A: host list (ssh_config), connection sharing (ControlMaster),
                      bootstrap injection, remote file edit-and-sync, forwards
    temor-wire/       pure: the versioned temor <-> temor-remote protocol (frames, messages)
    temor-remote/     Tier B helper binary (static, Linux x86_64 and aarch64 musl): session
                      daemon, a libghostty-vt per remote session, ports, files, trial, agent relay
    temor-desktop/    feature `quire-desktop`: IntentProvider1, docket and companion clients,
                      LauncherEntry badge, anyview D-Bus fallback, checkpoint and trial callers
    temor/            lib + thin main; dist/ (desktop file `org.quire.Temor.desktop`, D-Bus service, systemd unit
                      `org.quire.Temor.service`,
                      intents manifest, skills, shell integration)
```

**Boundary rules** (checked by `check-boundary.sh` and quire's `check-portable.sh`):
- `temor-core` has no I/O, no Dioxus, and no FFI.
- Only `temor-vt` names libghostty (through `temor-vt-sys`). `unsafe` is allowed only in `temor-vt-sys`
  and in `temor-vt`'s `ffi` module; the rest of the workspace keeps `unsafe_code = "deny"`.
- Only `temor-desktop` names zbus or docket. Nothing in the core reaches `desktop/`.

## 3. Core types (temor-core, pure)

```rust
pub struct PaneId(u64);  pub struct TabId(u64);  pub struct BlockId(u64);

/// What a Space keeps (the `P` of `Spaces<P, R>`). Written flat beside the look.
pub struct Project { root: ProjectRoot, profile: Profile, routes: Vec<Route> }   // the desktop link is the quire kit's
pub struct Profile { env: Vec<(EnvKey, EnvValue)>, ssh_identity: Option<KeyPath>, dotenv: Option<RelPath> }
pub struct Route { pattern: RoutePattern }            // e.g. `ssh prod-*`, a path prefix

/// What a Space recalls (the `R`): its tabs and their trees.
pub struct Place { tabs: Vec<Tab>, focused: TabId }
pub struct Tab { id: TabId, title: TabTitle, pin: Pin, folder: Option<FolderId>, tree: SplitTree }
pub enum Pin { Pinned { relaunch: Relaunch }, Loose }

/// Ported from ghostty split_tree.zig: a flat node array, handles instead of pointers, and
/// ratios relative to the parent. Every operation returns a new tree.
pub struct SplitTree { nodes: Vec<Node>, zoomed: Option<Handle> }
pub enum Node { Leaf(PaneId), Split { axis: Axis, ratio: Share, first: Handle, second: Handle } }
pub enum Axis { Row, Column }
pub enum Goto { Previous, Next, Spatial(Direction) }

pub enum PaneKind { Terminal(TerminalPane), AgentView(AgentSessionRef), Viewer(ViewerRef) }
pub enum TerminalRole { Shell, AgentCli { program: AgentLabel, gate: CliGate } }
pub enum CliGate { HookBridged, Sandboxed }           // never "free"; both are labelled in the UI

/// OSC 133 A/B/C/D.
pub struct Block { id: BlockId, prompt: GridSpan, command: CommandText, output: GridSpan,
                   exit: Option<ExitCode>, by: Author, started: Stamp, ended: Option<Stamp> }
pub enum Author { Person, Proposal { turn: TurnRef, taint: Taint }, Agent(AgentLabel) }

/// Per-pane agent state, mapped to quire's CompanionPresence for display.
pub enum AgentState { Idle, Working, Waiting(WaitKind), Done(Seen), Failed(Seen) }
pub enum WaitKind { Permission, Question, SignIn }
pub enum Seen { Unseen, Seen }
pub struct StatusReport { pane: PaneId, state: AgentState, seq: Seq, source: StatusSource, note: Option<Note> }
pub enum StatusSource { ProgramStatus /* OSC 7501 */, Adapter(PluginId), Docket, Notification }

pub struct Proposal { text: SanitisedLine, turn: TurnRef, taint: Taint, verdict: TrialVerdict }
pub enum TrialVerdict { Clean { changes: FileChanges }, Failed { exit: ExitCode, first_error: ErrorLine },
                        Unchecked(UncheckedWhy) }
pub enum UncheckedWhy { TimedOut, NeededNetwork, NoSandbox /* off Linux, or user namespaces blocked */ }

/// A host from ~/.ssh/config, or one typed in ⌘T.
pub struct Host { alias: HostAlias, target: SshTarget, space_pin: Option<SpaceRef>, consent: HostConsent, tier: RemoteTier }
pub enum RemoteTier { Plain /* Tier A */, Helper { version: HelperHash } /* Tier B */ }
pub struct HostConsent { clipboard: Grant, transfers: Grant, notifications: Grant, helper: Grant }
pub enum Grant { Ask, Allowed, Refused }
pub struct RemoteSession { host: HostAlias, id: RemoteSessionId, cwd: RemotePath }
pub enum Link { Attached, Detached { since: Stamp }, Reattaching { attempt: Attempt } }

pub struct Download { id: DownloadId, path: ShelfPath, from: Origin, size: Bytes, at: Stamp, state: Transfer }
pub enum Origin { Remote { host: HostName, tab: TabId }, Local { tab: TabId, block: Option<BlockId> }, Pipe }
```

All closed sets derive `Word`. There are no bool fields: `Seen`, `Pin` and `CliGate` are enums. Every unit and
id is a newtype.

## 4. State machines (pure `step`, effects at the edge)

| Machine | States | Inputs |
|---|---|---|
| `tree_step` | a SplitTree | Split(at, axis), Close(pane), Resize(handle, delta), Equalize, Zoom(pane), Unzoom, Goto |
| `block_step` | Idle → Prompting → Typing → Running → Finished(exit) | the OSC 133 marks, the shell exiting |
| `presence_step` | AgentState per pane | StatusReport (stale `seq` ignored), focus (Done/Failed become Seen), a new prompt (OSC 133 A clears Working/Waiting as in 7501), the process exiting |
| `proposal_step` | Off → Asking → Trialling → Shown(verdict) → Typed / Dismissed | Ask, ModelAnswer, TrialResult, Accept, Escape, the shell leaving the prompt |
| `bar_step` | Closed → Open(query, rows, highlighted) → Picked(row) | Typed, ProviderRows, Tab/arrows, Enter, Escape |
| `transfer_step` | Offered → Receiving(bytes) → Saved / Refused / Broken | protocol frames, the person accepting (remote offers ask once per host) |
| `link_step` | Connecting → Attached → Detached → Reattaching → Attached / Lost | the ssh process exiting, network change, wake from sleep, the backoff timer, the helper's Hello and snapshot |
| `helper_step` | Absent → Offered → Uploading → Ready(hash) / Refused / Unsupported | the host probe (`uname`, writable cache dir, not mounted noexec), person's consent, upload result, hash mismatch |
| `checkpoint_step` | None → Taken(ref) → RestoredTo(ref) | AgentWillWrite, UndoAll, UndoTo(block) |
| `worktree_step` | Creating → SettingUp → Working(status) → Reviewing → Merging → Merged / Conflicted / Kept / Discarded | add and setup results, a command finishing (re-check), the dry-run merge result, Review, Merge(style), the merge result, CloseTab(choice), Discard |

`Escape` and Ctrl-C keep their shell meaning in every state. A machine never takes a key the
shell would get.

## 4b. Threading

- **Each terminal pane has an I/O thread.** It reads the PTY and calls `vt_write` while holding the
  pane's terminal lock. libghostty's callbacks run inside `vt_write`: `WRITE_PTY` replies go straight
  back to the PTY, and everything else becomes a typed event sent over a channel to the UI. Callbacks
  never block.
- **One render path per window.** On each frame, for each dirty pane: `begin_update` under the lock,
  then `end_update` outside it. Only dirty rows are walked, then the pane's texture is uploaded and
  `handle.redraw()` is called.
  `RENDER_HOLD` (mode 2026) defers the frame.
- **Workers** (tokio, injected through quire's `Spawner`) handle the model calls, trial runs, plugin
  processes, transfers and peeks. Library crates never spawn threads themselves.

## 5. Rendering (temor-render)

Spike P0(b): two libghostty panes plus kitty images plus an anyview peek, each in its own
`TextureLayer`, run in one real window with no conflicts. Both strategies below were measured.

- **Two renderers behind one `Renderer` trait:**
  - **CPU (the default until quire's render hook lands):**
    - swash glyphs go into an in-memory atlas
    - only dirty rows are composed into a premultiplied RGBA buffer, then `TextureLayer` uploads it
    - measured p50 2–3 ms per pane per frame and about 0.1 ms from a key to a ready texture
    - costs 35–49% of one core while both panes scroll at 60 Hz
  - **GPU (the target):**
    - one instanced WGSL pass over glyph atlases, with images by z-layer, into a texture registered
      on the window's device (`use_gpu`, `handle.replace`/`register`, `redraw`)
    - 13–22% CPU, and 82–408 KB uploaded per frame instead of about 5 MB
    - **Never `queue.submit` from a pane thread:** it stalls up to one refresh against the window's
      present. It turns on when quire ships a render hook that encodes into the layer's texture on the
      window's render thread before its submit.
- **Fonts:**
  - fontique finds the fonts; the default is the system `monospace` until quire adds a terminal
    code-face token (Space Mono has no box drawing, CJK or emoji, and a line height of about 1.5).
  - Box drawing and block elements are drawn procedurally.
  - **Colour emoji:** Fedora's Noto Color Emoji is COLRv1 only, which swash can't draw. Rasterise
    COLRv1 through skrifa's colour paint with vello_cpu (already in quire's tree) into the colour atlas.
    Fall back to monochrome.
- **Kitty images:**
  - read `placement_render_info` in the locked begin phase, since it needs `&Terminal`
  - textures cached per image `generation`, drawn by z-layer (`BELOW_BG`, `BELOW_TEXT`, `ABOVE_TEXT`)
  - an image's anchor after reflow follows libghostty
- **Grid colours** come from quire's terminal palette tokens (v0.2.31), so light/dark and the Space
  theme apply.
- **Sizing:**
  - panes snap to whole cells, and the leftover pixels are padding
  - fractional scale comes from winit (checked crisp at 1.5x and 2x)
  - the app's container sets full height itself until quire's root fills the window
- **Build:** temor-vt-sys chooses Zig's optimize mode from `OPT_LEVEL`, never from `DEBUG`. A Debug
  libghostty makes `vt_write` about 8000x slower.
- **Known limits:**
  - `AppConfig` cannot request wgpu features (no timestamp queries), and there is no presented-frame
    signal; both are asked of quire.
  - Clicks on custom widgets are lost until v0.2.31, so each pane's input is caught on the containing
    element, and the drawn grid gets `pointer-events: none` (verified in the harness).

## 6. Input

- **Keys:** quire `EditSurface`-style focus, IME and clipboard hosting, then libghostty's `KeyEncoder`,
  which honours the kitty keyboard flags.
- **Mouse:** libghostty's `MouseEncoder`, using pointer capture from `use_pointer_capture`.
  `data-keys="capture"` and `data-wheel="capture"` keep the scroll engine away from the grid.
- **Paste:** bracketed paste, with libghostty's unsafe-paste check.
- **Clipboard:** OSC 52 through `ds_blitz::clipboard`; a read asks once per tab.
- **Keyboard profile:** the desktop file says `Categories=TerminalEmulator` so keycap applies its
  terminal profile, and ⌘ is never remapped to Ctrl in temor.
- **Spike P0(c) findings:**
  - libghostty's encoder plus real fcitx5 IME (text-input-v3) work in a v0.2.30 window.
  - temor currently carries copies of quire's private IME machine and focus helper; these are asked
    upstream.
  - The unshifted key comes from a US table until the Blitz fork passes winit's
    `key_without_modifiers`.
  - temor corrects the modifier lag in its own keymap.
  - Keys are **configurable** (ds-settings). On KDE, KWin takes Meta+T/W/D/V globally; the desktop's
    reserved chords have been asked of whopper.
- **The desktop key rule** (whopper):
  - Apps never bind Super. Super chords belong to the shell and compositor (⌘Space, ⌘Tab, ⌘`,
    screenshots).
  - keycap's `terminal` profile (picked through `Categories=TerminalEmulator`; sill's keyboard source
    maps that category) delivers the physical ⌘ + letter as **Ctrl+Shift + letter**, so ⌘C is never an
    interrupt. temor binds Ctrl+Shift chords and the UI shows them as ⌘ chords.
  - Ctrl+Space stays with the input method everywhere.
- **Default keys** (shown with ⌘; bound as the Ctrl+Shift equivalent):
  - ⌘T bar; ⌘W close; ⌘D / ⌘⇧D split (Ctrl+Shift+D, and Ctrl+Shift+Alt+D for down); ⌘⌥ arrows move
    focus
  - ⌘⇧↩ zoom; ⌃⌘= equalize; ⌘C/⌘V copy/paste; ⌘F searches the scrollback; ⌘↑/↓ move between prompts
  - ⌘1–9 tabs; ⌃1–9 Spaces (quire `SwitchChord`); ⌘⇧U jumps to the waiting agent
  - Every binding is configurable; the defaults never use Super or Ctrl+Space.

## 7. Browser-like structure

**Sidebar** (quire `Sidebar`, at least `SIDEBAR_MIN_WIDTH`):
- **Head:** `SpaceHead`.
- **Body:** pinned tabs, then folders, then the other tabs.
- **A tab row** shows title, cwd and agent status (quire `CompanionPresence` orb at Inline size), plus an
  unread dot.
- **A tab with splits** shows a miniature of its layout. Each tile has a title and status, and clicking a
  tile focuses that pane.
- **Drag and drop:** a tab onto a tab makes a split; a tile dragged out tears that pane off into its own
  tab.
- **Running row:** background jobs, listening ports (click to open in the browser), and progress from
  OSC 9;4 / 7501.
- **Foot:** `SpacesFoot`, plus the Downloads button.

**Tabs:** stay until closed.
- Pinned tabs relaunch their command on start.
- Recently closed keeps the last N tabs with their scrollback, restored through libghostty's `snapshot`.
- A session is restored on launch: trees, cwds and snapshots.

**Spaces:**
- A Space is a project root plus a profile plus routes.
- New tabs open at the root with the profile's environment. A per-Space ssh-agent holds only that
  Space's identity.
- Credentials (API keys) come from porter's account store, never from plain text in the profile.
- Routes send new sessions to a Space, for example `ssh prod-*` to Ops.

**⌘T bar** (fixed ranking, `bar_step`). It is built on quire v0.2.31's generic palette machine
(`ds::prelude::{PaletteState, PaletteIn, PaletteOut, PaletteParams}`):
- `bar_step` produces the ranked rows. The palette machine never re-ranks.
- When provider rows change, temor sends `PaletteIn::Refreshed`.
- `Run(row)` and `Closed` come back. ⌘T sends `Close`.
- Selection stops at the ends and does not wrap.

The ranking rules:
1. Exact matches: Spaces, tabs, folders, SSH hosts (`~/.ssh/config`), saved layouts, recent
   commands, directories (frecency).
2. If the first word resolves on PATH, the top row is "Run `…` in a new tab". This is a lookup, not a
   guess.
3. Plugin rows (`open.provider`).
4. AI rows, always last:
   - "Suggest a command for: …" (§8.3)
   - "Ask an agent in a new pane: …" (opens the agent view)
   - "Ask about @tab…" (hands off to the companion with tab context chips)

**Other:**
- **Quick terminal:** one hotkey opens a small throwaway terminal over the current window, and
  "Move to Space" promotes it into a real tab.
- **Tab names** come from OSC 0/2, otherwise the command plus the cwd. A model renames a tab only when
  asked.

## 8. Agents

### 8.1 Status radar (all panes)
- **Sources, strongest first:**
  1. OSC 7501 program status (parsed by libghostty)
  2. `agent.adapter` plugins reporting over temor's socket (`TEMOR_PANE_ID`, `TEMOR_SOCKET`, monotonic
     `seq`)
  3. docket's `Companion1` roster and `roster_changed` for agent-view sessions
  4. OSC 9/99/777 notifications and BEL
- **Display:** status uses quire `CompanionPresence`, so a waiting agent in temor looks like a waiting
  companion anywhere else.
- **Attention:**
  - Done and Failed stay until the pane is seen.
  - ⌘⇧U jumps to the next waiting agent.
  - A desktop notification fires only when the pane is unfocused, carries the agent's own message, and
    raises the window through xdg-activation.
  - The dock badge shows the waiting count through `LauncherEntry`.

### 8.2 Agent panes
- **Agent view (default for "new agent"):**
  - A `PaneKind::AgentView` drives an ACP session through docket (the `docket-agent` / `docket-launch`
    profiles `claude-code` and `agy`; `agents.toml` labels).
  - It is drawn with quire companion components: `AnswerCard`, `PlanList`, and `RunRow` controls
    (Watch, Take over, Hand back, Pause, Stop, Undo all). The agent's commands show as blocks.
  - Every file, terminal and permission request goes through the router and `Confirm1`.
  - Built together with docket and quire, as a desktop-wide agent view.
- **Agent CLI pane (opt-in):**
  - The agent's own interface runs in a terminal pane, launched through docket-launch (bwrap).
  - It is labelled `Sandboxed`, or `HookBridged` once the Claude Code adapter forwards each
    PreToolUse to docket.
  - **Hook bridge (agreed with the agent session):**
    - The adapter calls `Intents1.Request.Session.Open` once, then
      `JudgeToolCall(session, call) -> Allow | Deny{reason} | Ask` for each tool call.
    - `call` is `{program, label?, tool, kind: PermissionKind, input, cwd, space}`.
    - `Ask` raises `Confirm1` inside docket, and the hook waits for the person's answer.
    - The caller is `CallerRole::AcpAgent`, so every `acpagent.*` rule and the R10–R12 taint rules
      apply.
    - The adapter must set the CLI's own permissions to "ask for everything", since pre-allowed
      tools bypass hooks.
    - docket builds its side after the agy lane, once the adapter's shape is fixed.
- **Approval mode, always visible (D20):**
  - Every agent pane (agent view and CLI pane) has a mode line in its header with two facts:
    - **Sandbox:** On (docket-launch bwrap) or Off.
    - **Asks:** Everything, Writes, or Nothing, from the gate's session for that agent.
  - Both come from the launch profile and docket's session, never from what the agent says. If temor
    can't read them (an unbridged CLI), the line says **Not gated**.
  - Changing a mode is an explicit action on that pane (a menu on the mode line, with the
    consequence spelled out). It applies to that pane only; a new agent starts from the Space's
    default, never from the last pane's choice.
  - The same two facts appear on the radar row and in the tab's tooltip.
- **Checkpoints (D9):**
  - Before an agent's first write in a Space, temor snapshots tracked and untracked files to a hidden
    ref (`refs/temor/checkpoints/<n>`), never a branch, and never in the person's index or stash.
  - Every agent block offers "Undo to here", and the run offers "Undo all".
  - Ignored files are labelled as not covered.
- **Agent worktrees (D19).** temor owns them; agents never decide where they work.
  - **Choosing:** ⌘T "New agent" offers two rows, "on a new branch" (first) and "in this checkout".
    A Space setting can put "in this checkout" first. The tab shows the choice.
  - **Creating:**
    - `git worktree add <data>/worktrees/<space>/<name> -b agent/<name> <base>`. The base commit is
      recorded. `<data>` is temor's data dir, outside the repo, so the person's editor, watchers and
      builds never see it, and `git clean` can't touch it.
    - The name is a short slug from the task, editable at creation.
    - Over SSH (Tier B), the same steps run on the host through temor-remote; Tier A hosts get
      "in this checkout" only.
  - **Setting up (project facts, not agent decisions):**
    - Ignored files the project needs (`.env`, local config) are copied from a gitignore-style
      include list in the repo. If Claude Code's desktop app really reads a `.worktreeinclude` file
      (unverified), temor reads the same file name.
    - Then the Space profile's optional setup command runs (for example `pnpm install`) as a block in
      a pane, so its output and exit are visible.
  - **Pinning:** the agent's pane cwd and its workspace root are the worktree. docket's workspace
    path check then refuses or asks for writes outside it, including an agent that tries to make its
    own worktree elsewhere. The CLI pane gets the cwd only; the agent view is told its root. Processes
    and ports stay shared, and the tab's details say so ("separate files, shared ports").
  - **Early overlap warnings (deterministic, no AI; spike `spikes/worktree/SPIKE.md`):**
    - **When:** after a command finishes in an agent worktree, debounced, off the render path.
    - **Snapshot:** each changed worktree is snapshotted with a temporary index
      (`GIT_INDEX_FILE=<tmp> read-tree HEAD; add -A; write-tree; commit-tree -p HEAD`), so uncommitted
      and untracked work counts. The agent's own index is never touched. The snapshot's tree id is the
      cache key; only pairs whose trees changed are recomputed.
    - **Checks** (refs validated with `rev-parse --verify` first, since a bad ref also exits 1):
      `merge-tree --write-tree --name-only -z` against the base and each sibling (exit 1 = conflict,
      with the paths), plus a same-file signal from `diff --no-renames --name-only` on both sides of
      the merge base (catches rename-vs-edit), and ahead/behind from `rev-list --left-right --count`.
    - **Tab shows, two severities only:** red "Conflicts with <tab> in <file>"; amber "Both touch
      <file>"; amber "Base moved: N files, M overlap yours"; then plain ahead/behind and files changed.
      Labelled "text overlap only": semantic conflicts (an API changed in another file) aren't seen.
    - **Cost measured:** about 1 ms per pair; a full round for five agents is 40 ms (commits only)
      to 350 ms (five dirty snapshots on a 2,500-file repo). Snapshots add loose objects to the repo,
      which `git gc` cleans up.
  - **Review and merge:**
    - "Review" opens the diff against the base in the viewer pane (§10), with accept or reject per
      file.
    - Buttons: **Squash** (default), **Merge**, **Rebase onto base**, **Open pull request** (through
      the host's tool when present). temor runs git itself.
    - A conflict is never resolved silently: temor opens a shell split in that worktree with the
      conflicted files listed.
    - After a merge, the worktree and branch are removed, and a checkpoint ref is kept so the merge
      can be undone (same hidden-ref rule as checkpoints).
  - **Lifecycle:** worktrees persist with their tab. Closing the tab asks Keep / Merge / Discard.
    Kept worktrees with no tab are listed in the Space ("Leftover branches") for cleanup.
  - **Maintenance budget:** one git module (worktree add/remove, merge-tree, diff, merge) and one
    review view. No per-agent rules: any agent just gets a cwd.

### 8.3 Proposals in your prompt
- **What's sent, shown first (D21):** before any model call (a proposal, a ⌘T AI row, "Ask about
  @tab"), a context card lists exactly what will be sent as chips: the request text, cwd, the last
  block's command and its last N output lines, the selection, file names. Each chip shows its size and
  can be removed. Defaults are fixed per entry point (no guessing what's relevant). The sent list is
  stored with the request's log record, so "what did the model see?" always has an answer.
- **Asked for, never offered by itself.** Two entry points:
  - the "Explain · Fix" chip on a failed block (it does not retry by itself)
  - the ⌘T row "Suggest a command for: …"
- **Model:** inferd's `text` slot (local, private). The context is the block, cwd, shell, the installed
  tool's version, and earlier successful commands in this Space (almanac).
- **Before it is shown:**
  1. Sanitise: strip newlines and control characters; it will be typed by bracketed paste.
  2. Syntax check (`bash -n`, or the shell's own check) and command resolution through the live
     shell.
  3. **Trial run** (D7), as the trial module in bulkhead:
     - bwrap with an overlay on `$HOME` and the cwd, everything else read-only
     - no network, fresh `/tmp` and `/run`, a separate session keyring, stdin from `/dev/null`
     - a 2 s limit with a process-group kill, plus rlimits (process count, file size, address space)
     - **verdict rule:** a timeout gives Unchecked(TimedOut); a non-zero exit plus moved netns
       counters gives Unchecked(NeededNetwork); otherwise the exit code decides, and a clean run
       shows the file changes.
  4. A failed trial is shown to the model once, then the chip offers "Ask an agent".
- **Shown:**
  - The proposal is typed into the prompt with its verdict badge.
  - A marker shows when the model's turn read untrusted text ("suggested from the output above").
  - **The person's Enter is the approval**, under Cedar rule `user-launcher`, so no `Confirm1`.
- **Logged:** to the companion session record as the person's act. The record is `ProposalRun`,
  added by docket in companion-wire:
  - `space`, `pane`
  - `text` (stored after sanitising) and `sanitised: Changed{removed} | Unchanged`
  - `suggested_by: TurnRef` and `model: ModelRef`
  - `taint`, as prov's `TaintNote`/`Integrity`
  - `verdict`
  - `ran_at: prov::UnixSeconds`, from the injected clock
- One switch turns off all AI. When it is off, nothing reaches a model.

### 8.4 temor's actions (ds-intents manifest `dist/intents/<app-id>.toml`, `IntentProvider1`)
| Action | Effect | Notes |
|---|---|---|
| `terminal.pane.list` / `pane.cwd` | Read | |
| `terminal.pane.read` / `command.list` / `command.read` | Read | taints the session (`ReadUntrusted`); bounded ranges |
| `terminal.pane.open` (terminal, agent, viewer; split or tab) | UndoableWrite | |
| `terminal.command.run_sandboxed` | UndoableWrite with no network (the overlay is the undo); Outbound with network | |
| `terminal.command.run_in_pane` | Destructive | runs as the person; always asks; never a standing grant |
| `context(scope)` | | focused pane, cwd, last block, selection, Space id |

- **Space mapping** (owner decision: per app, linkable):
  - temor's Spaces are its own. Each one can be linked to a desktop-wide Space through quire's kit
    (v0.2.31: an opaque link per Space and a "Link to…" menu item). Only linked Spaces share memory
    and consent with other apps.
  - Every request and `context()` carry
    `porter_core::SpaceId::for_app(&AppName, LocalSpace(space.0), space.link.as_deref())`
    (porter `209aaaf`, on the desktop in batch 8). temor never formats the u64 itself.
  - The desktop Spaces available for linking come from `org.quire.Spaces1` on accountd, through
    porter-client `DbusTransport::spaces()` (`list`, `create`, `watch`). This is a desktop extra in
    `temor-desktop`.
  - When a linked desktop Space is removed, temor clears the link and falls back to its own Space.
- **Companion @tab:** the companion can take any tab's blocks as `ContextChip`s. What isn't shown
  isn't sent.

## 9. Transfers and the downloads shelf
- **Receive:**
  - OSC 1337 `File=` is swallowed by Ghostty's parser, so `temor-vt` pre-scans PTY bytes for it before
    `vt_write`
  - kitty file transfer (OSC 5113, including directories), iTerm2 `File=` (OSC 1337), and a tiny
    `temor get <file>` shipped for hosts with neither
  - libghostty passes these sequences through `UNKNOWN_SEQUENCE`
  - a remote offer asks once per host
- **Send:** drop a file onto an SSH pane to upload it to the remote cwd (kitty transfer).
- **Local:**
  - "Save output" on a block (text, HTML through libghostty's formatter, raw image)
  - `temor save` and `temor view -` from pipes
  - a pane screenshot
- **Shelf:**
  - newest first, each item with its thumbnail (the freedesktop cache, otherwise a peek) and its
    origin
  - "Show in its tab" jumps back to where it came from
  - the path is configurable (`ds-settings`), defaulting to temor's own data dir

## 9b. SSH
- **Tailscale (D22):** tailnet machines come from porter's `Tailnet1.Machines()` (node id, name,
  MagicDNS name, addresses, online/last seen, Tailscale SSH on/off, host keys, owner) and its
  `Changed()` signal. They join the ⌘T host list as "Your computers"; an ssh_config alias whose
  HostName is a machine's MagicDNS name or 100.x address is shown once (the person's alias wins).
  The `HostId` also carries the node id, so a renamed machine keeps its pins and consent. Tailscale
  SSH needs no key or identity picker; the check-mode `https://login.tailscale.com/…` line in the
  pre-shell output becomes a "Confirm it's you in your browser" row with Open. Tier B ports offer
  "Open at <name>:<port>" before "Forward port…". With no Tailscale account, the group is absent.
  Tests use a fake `Tailnet1` on a private bus and the throwaway sshd printing the check URL.

### Tier A: nothing installed on the host
- **Hosts.** They come from `~/.ssh/config` (`Host` blocks, `Include`, `ProxyJump`) into ⌘T rows,
  can be pinned per Space, and show a reachability dot. Tabs and split tiles show `host:path`.
  - A Space profile can carry the identity and jump host (D10).
  - Hosts in a Space can carry a tinted marker, for example Prod.
- **Opening.** temor runs the system `ssh` (never a reimplementation) with a temor-owned
  `ControlMaster`/`ControlPath` per host, plus a bootstrap:
  - **Bootstrap:** a small POSIX `sh` script, sent with the connection the way kitty's ssh kitten does
    and never installed. It:
    1. sets up OSC 133/7 integration for bash, zsh and fish
    2. installs or falls back to a compatible terminfo (`xterm-ghostty`, else `xterm-256color`)
    3. defines `temor get` / `temor open` shell functions that speak OSC 5113 or OSC 1337 over
       the terminal stream
  - **OSC 7 cwd** carries the hostname, so temor always knows which host and folder a pane is in.
- **Split and new tab inherit the host.** ⌘D in an SSH pane opens a new channel on the shared
  connection, in the same remote cwd, in about the time one round trip takes.
- **Remote files:**
  - Peek, open in a viewer pane, and drag onto the pane to upload.
  - Edit locally and write back: the file is fetched to the shelf, opened in the local editor or
    viewer, and written back on save after a check that the remote file hasn't changed.
  - In Tier A, transfers run over the terminal stream (§9).
- **Port forwarding:** "Forward port…" in the running row uses `ssh -O forward` on the shared
  connection, then opens the port in the browser. In Tier A, ports are found from URLs printed in
  output (OSC 8 links and `localhost:NNNN` text) or added by hand.
- **Consent per host** (`HostConsent`): clipboard reads, incoming transfers and notifications ask
  once per host, and the answer is remembered per Space.
- **Proposals in SSH panes** say "not checked (remote)" unless Tier B is active.

### Tier B: `temor-remote`
- **What it is:**
  - A static Rust binary for Linux x86_64 and aarch64 (musl), built and shipped with temor, the same
    version as temor.
  - It speaks `temor-wire` over an ssh channel on stdio. It never listens on a network port; its
    local socket is `$XDG_RUNTIME_DIR/temor-remote/<hash>.sock` with mode 0600.
- **Install:**
  - After consent per host, temor probes the host (architecture, a writable `~/.cache`, not
    `noexec`) and uploads the binary into `~/.cache/temor-remote/<hash>/`.
  - The version is the content hash, so updating means uploading a new hash and removing the old.
  - "Remove from host" is one action.
  - Hosts that can't run it (another OS, noexec, refused) stay on Tier A with no error.
- **Durable sessions:**
  - The helper runs a per-user daemon that owns the remote PTYs, with **one libghostty-vt instance
    per session** that keeps the screen state.
  - On reattach it sends a libghostty `snapshot`, then the live stream, so the exact screen, scrollback
    and modes come back. This is the same core on both ends, which is the approach Superlogical's Rex
    takes.
  - A dropped connection, sleep or a network change runs `link_step`: temor reconnects with backoff
    and reattaches every pane of that host.
  - Detached sessions are kept until an idle limit (setting, default 7 days) or until they exit.
- **Ports:** the helper watches the listening sockets owned by the person's sessions (`/proc/net/tcp*`
  plus the process's inodes) and reports them. temor shows them in the running row and forwards them
  on the shared connection, optionally automatically, as VS Code does.
- **Files:** list, stat, read, write and watch. Viewer panes on remote files reload live, and
  write-back uses the remote file's mtime and hash as the check.
- **Trial runs on the host:** the helper links bulkhead's trial module. If the host has bwrap and
  unprivileged user namespaces, proposals in SSH panes get real verdicts; otherwise "not checked
  (remote)".
- **Remote agents: the agent runs locally, and only its hands are on the host.**
  - In the agent view, a session's workspace can be `host:path`. The agent process, its sign-in,
    docket's gate, `Confirm1`, memory and checkpoints all stay on the person's machine.
  - ACP already lets the client serve the agent's file and terminal calls. docket routes that
    session's `fs/*` and `terminal/*` to temor, which carries them out on the host through
    temor-remote:
    - file reads and writes go to the file service
    - commands run in a remote PTY and appear as blocks in the agent's pane
  - The agent's own built-in tools that would touch the local disk (local shell, grep, glob) are
    turned off for remote sessions through the launch profile. Search tools come back through our tool
    server as remote actions (`terminal.remote.search`, `terminal.remote.list`), so everything the
    agent sees is the host.
  - Checkpoints are taken on the host through the helper, with the same hidden-ref rule.
  - **Agreed shape with the agent session (for P14):**
    - **Executor:** a second performer in docket-acp, next to `client/perform.rs`, chosen when the
      session opens. It talks to temor-remote through temor, over a typed transport, and can report
      "the host refused".
    - **Guarantees on the host:** the workspace path check, output limits and redaction run on the
      host, from bulkhead's shared code linked into temor-remote, so remote calls get the same
      guarantees as local ones.
    - **Gate:** a new `site: Site::Local | Remote { host: HostId }` on docket's `Call` and on the ACP
      session's opening, never encoded in the cwd. `HostId` is the ssh host-key fingerprint plus the
      shown name. A remote write counts as `UndoableWrite` only while a host-side checkpoint is held.
      The host's network is outside our reach rules, so Cedar can say "remote destructive always
      asks".
    - **Local tools off:** `permissions.deny` (Bash, Grep, Glob, LS, WebFetch and the built-in
      Read/Edit/Write) through the launch profile's settings, so the adapter uses the fs and terminal
      the client serves.
    - **Check:** smoke flows include a host-side canary, a local file the agent must never see. Each
      adapter version is checked live before remote workspaces are offered for it.
  - Agents that can't hand their tools to the client (the CLI pane) aren't offered for remote
    workspaces. As an explicit fallback, the CLI can run *on* the host through the helper, with
    `JudgeToolCall` relayed back to local docket and labelled "runs on host".
- **Maintenance budget:**
  - two Linux targets, one protocol crate with an explicit version handshake (bayonet's framing rules)
  - no per-distro packaging
  - macOS and BSD hosts are a later decision (§14)

## 10. Viewers (anyview)
- **Peek:** hover or ⌘-click a path or link in the output. The path comes from the block's text and
  OSC 8. `anyview_peek::peek` runs on a worker and is shown in a `Pane` popover.
- **Sequence:** the paths in an `ls`/`find`/`rg` block become anyview's `Sequence`
  (`SequenceOrigin::Results`), so you can arrow through matches.
- **Viewer pane:** `PaneKind::Viewer` hosts `anyview_pane::ViewerPane` (design:
  `~/av-wt/pane/design.md`; approved, building after anyview's bug-fix lane).
  - Props: `file`, `sequence`, `edge` (`PaneEdge::portable(workers, on_request)`), `focused`,
    `on_request`, `look`.
  - **Keys:** temor owns every modifier chord, ⌘K, ⌘W and split navigation. The pane takes plain keys
    only while focused. Esc unwinds inside the pane, then sends `PaneRequest::Unfocus`, which returns
    focus to the terminal.
  - **Commands:** `PaneHandle::commands()` and `run(cmd)` merge into temor's quire `CommandPalette`.
  - **Requests temor handles:**
    - `SizeHint`
    - `ClosePane`
    - `OpenElsewhere`
    - `File(..)` mutations, which temor carries out or declines
    - drag and drop, by passing a new `file`
  - **Video** opens a viewer window until anyview extracts its player into a library.
  - **Fallback:** without the pane (feature off), ⌘-click opens anyview through D-Bus `Open`, or its
    socket off the desktop.
- **Inline:** kitty images in blocks are native. `ls` thumbnails are opt-in.

## 11. Phases (dependency order)

| Phase | Content | Needs |
|---|---|---|
| **P0 Spikes** | (a) build: Zig 0.16 plus the `libghostty-vt` crate pinned, offline through `GHOSTTY_SOURCE_DIR`, in the lane's build dir. (b) render: two panes side by side, libghostty output plus a kitty image in two `TextureLayer`s, with an `anyview-peek` texture beside them in one window. (c) input: IME plus the key encoder through an `EditSurface`-style host. (d) measure what `anyview-peek` actually adds to the build. | quire v0.2.29 |
| **P1 Single terminal** | PTY, `temor-vt`, renderer, keys/mouse/paste/clipboard, scrollback, search, selection, hyperlinks, fonts, settings, desktop file, keycap profile | P0 |
| **P2 Shell integration and blocks** | bash/zsh/fish scripts (OSC 133/7), blocks and their actions, jump between prompts, exit marks, command-finished notifications, progress | P1 |
| **P3 Window structure** | sidebar tabs (persistent, pinned, folders), split tree plus `SplitView`, split miniatures, drag to split and tear off, recently closed through snapshots, session restore | P1; quire `SplitView` batch |
| **P4 Spaces** | kit integration, `Project` payload, profiles (env, ssh-agent, dotenv), routes, ⌃1–9 | P3 |
| **P5 Command bar and quick terminal** | providers, fixed ranking, Run row, quick terminal | P4 |
| **P6 Desktop integration** | intents manifest, `IntentProvider1`, `context()`, D-Bus activation and systemd unit, dock badge, `quire-desktop` boundary gate | P2 |
| **P7 Agents** | `presence_step` and the radar, OSC 7501, `TEMOR_PANE_ID` socket, checkpoints, the CLI pane through docket-launch, the agent view (with docket and quire), the hook bridge (with docket), the approval mode line (D20), agent worktrees (D19; overlap spike done) | P3, P6, P10 (review view); docket |
| **P8 Proposals** | the context card (D21), the trial module in bulkhead, sanitising, the model call, the verdict UI, the Fix chip, the taint marker, the log record | P2, P6; bulkhead split |
| **P9 Transfers and shelf** | OSC 5113/1337, `temor get/save/view`, shelf UI, upload by drop | P2 |
| **P10 Viewers** | peek, Sequence, viewer pane | P2; anyview pane API |
| **P11 Plugins** | bayonet; `block.lens`, `open.provider`, `agent.adapter`, `row.facts`; the built-in adapters (Claude Code status and hook bridge, git facts) written as plugins | P5, P7; bayonet |
| **P13 SSH Tier A** | host list (plus tailnet machines from porter, D22), connection sharing, bootstrap (integration, terminfo, `temor get/open`), splits on the same host, remote files and write-back, forwards, consent per host | P2, P3, P9 |
| **P14 SSH Tier B** | `temor-wire`, `temor-remote` (session daemon with libghostty-vt, snapshot and reattach, `link_step`), port detection (direct tailnet open first), the file service, remote trial, remote workspaces for local agents (plus the on-host CLI fallback) | P13; bulkhead; docket's `JudgeToolCall` |
| **P12 Companion context** | @tab chips, `context()` scopes, the ⌘T "Ask about" row with the context card (D21) | P6; companion |

P3 to P5 can run alongside P6 and P9 once P2 lands.

## 12. Work in other repos

| Repo | Ask | Status |
|---|---|---|
| quire | `SplitView` axis plus fractional shares plus nesting tests plus unfocused-dim token | queued, v0.2.31 |
| quire | terminal palette token set (16 ANSI colours plus foreground, background, cursor, selection), for each appearance and following the Space look | queued, v0.2.31 |
| quire | v0.2.31: clicks on custom widgets | queued |
| docket | `JudgeToolCall` hook bridge; `ProposalRun` in companion-wire; the agent view as a desktop component | bridge and record agreed; view to propose |
| quire / porter | the kit's per-Space link and "Link to…" (v0.2.31); porter `SpaceId = Desktop(id) \| App{app, local}` plus a helper | decided by the owner; account confirming the shape |
| docket → bulkhead | split out `docket-shell`, keeping `Overlay{from,to}`, `resolver_binds()` and `AgentRun` stable; temor adds the trial module there | agent session splitting; trial module after it lands |
| anyview | `anyview-pane` (`ViewerPane`); `SequenceOrigin::Output` for block output; the resume store safe across two processes | approved; after anyview's bug-fix lane |
| quire | generic palette machine (anyview writes the quire patch), shared by temor's palette and ⌘T bar | queued, v0.2.31 |
| anyview → bayonet | extract the plugin crate; shape `open.provider` to match sill's launcher providers | done: v0.1.0, with temor's four capabilities in its examples |
| docket | remote workspaces: a remote performer in docket-acp, `Site` on `Call`, profiles that deny local tools, a canary smoke flow, `JudgeToolCall` with `Site` (fallback) | shape agreed; after batch 7 and agy |
| bulkhead | the trial module plus the path check, output limits and redaction build for musl and run inside `temor-remote` | with the trial module |
| detent | Extensions page | queued after bayonet |
| keycap | none if `Categories=TerminalEmulator` is enough; otherwise `X-Keycap-Profile` | check in P1 |

## 13. Verification

- **Lane gate:**
  - `cargo fmt --check`, `clippy -D warnings`, tests (one `tests/it/main.rs` per crate)
  - `check-boundary.sh`, quire `check-portable.sh`, `cargo check --workspace --no-default-features`
  - `cargo deny check licenses`, `check-intents.sh`
  - coherence tests from CONSUMING.md §5
- **temor-core:** tree, presence, proposal, bar and transfer machines tested on `Clock::Virtual`.
  The split tree gets property tests: equalize, resize clamps, spatial goto is symmetric.
- **temor-vt:** conformance against recorded byte streams (vttest captures, kitty graphics and
  keyboard fixtures, 7501 and 133 sequences). A pin bump must pass them.
- **temor-render:** golden images from a headless wgpu adapter for a fixed grid and image set.
- **UI:** `ds-harness` scenarios with a fake `Vt` and a fake PTY: sidebar, split minis, bar
  ranking, shelf.
- **Trial module:** the 50-command corpus from the spike (kept as a fixture) runs under a scratch
  HOME. The fixture hash stays unchanged, and verdicts are identical across three runs.
- **SSH:** tests use a throwaway `sshd` in a podman container (scratch HOME, a generated key, loopback
  only).
  - **Tier A:** bootstrap marks, splits on a shared connection, transfers and write-back conflicts.
  - **Tier B:** kill the ssh process mid-output, reattach, and require the snapshot to restore the
    exact screen; port detection; helper upgrade by hash; fallback on a `noexec` home.
- **Desktop:** a private session bus with fake docket and companion services. Never the real bus.

## 14. Open decisions (defaults chosen, not blocking)

| Question | Default |
|---|---|
| Shells with integration | bash, zsh and fish in P2; nushell later |
| Font | quire's code face; any installed monospace font can be chosen |
| Zig in packaging | Zig 0.16 pinned together with Ghostty; offline builds need the full Zig package store (Ghostty's `fetch-zig-cache.sh`, 39 packages / about 540 MB with Zig 0.16, which extracts into `zig-pkg/`; temor-vt-sys's `scripts/fetch-ghostty.sh` builds the store) through `GHOSTTY_ZIG_SYSTEM_DIR` |
| cargo-deny allow list | quire's list plus `bzip2-1.0.6` (copied from anyview's deny.toml) |
| Trial limits | 2 s, 256 processes, 1 GiB file size; settings under R13 "at your own risk" |
| Remote transfer consent | ask once per host, remembered per Space |
| Recently closed depth | 20 tabs per Space |
| Helper on macOS and BSD hosts | later; Tier A only until then |
| Detached remote session lifetime | 7 days idle, a setting |
| Automatic port forwarding | off; detected ports are offered in the running row |
| Hook bridge for agents other than Claude Code | when an agent exposes pre-tool hooks |
| Agent worktree defaults | "on a new branch" first; worktrees in temor's data dir; Squash merge; Tier A hosts in-checkout only |
