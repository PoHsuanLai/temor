# Follow-ups

Review leftovers that didn't block a merge. Each one names the lane it came from and the PLAN section.

## temor-core (lane/core, reviewed at 03031e9)
- [ ] `TrialId` is only as unique as `TurnRef`: every Ask must get a fresh `TurnRef` (document it, or
      add a counter). §8.3
- [ ] `Origin::LocalStream` emits `Remember(Grant)`, but no consent store exists for local panes
      (`HostConsent` is per host). Add one at the edge, or always ask. §9
- [ ] A typed proposal deleted by hand and replaced by the person's own command still logs
      `Ran{Edited}`. Tell "edited" apart from "replaced". §8.3
- [ ] U+2800 (braille blank) looks like a space but isn't default-ignorable. Consider treating it as
      a space. §8.3

## temor-vt (lane/vt, reviewed at 8d586df)
- [ ] iTerm2 `MultipartFile=` / `FilePart=` / `FileEnd` (imgcat inside tmux) are swallowed by Ghostty
      and not yet pre-scanned. §9
- [ ] `RENDER_HOLD`: capture a frame when a hold starts, as Ghostty recommends; the hold timeout is the
      UI's job. §4b
- [ ] OSC 52 clipboard reads (`CLIPBOARD_READ`) are not wired. §6, §9
- [ ] No vttest captures in the conformance fixtures yet; DA2/DA3 use libghostty's defaults. §13
- [ ] Upstream (only with the owner's OK): Ghostty reporting OSC 1337 `File=` through
      UNKNOWN_SEQUENCE; a per-cell hyperlink id; libghostty-rs taking the `Send` and new-callback
      wrappers. D1

## temor-pty (lane/pty, reviewed at 0f49c7b)
- [ ] The app owns reaping: poll `try_exit` after `hang_up()` and escalate with `kill()` after a grace
      period, or children stay zombies. §4b
- [ ] nushell (the owner's shell) and other shells get no integration. nushell can emit the marks
      itself (`shell_integration` config); turn that on. §6
- [ ] fish 3 marks path untested (fish 4+ emits marks natively); bash 3.2 has no PS0, so no C/D marks.
- [ ] A person's own `trap … EXIT` replaces the emulated-login `.bash_logout` trap; an old-style
      `zle -N zle-line-init` drops zsh's first-prompt B fallback under themes.
- [ ] Foreground name/cwd come from the group leader, so a pipeline reports its first command. §7
- [ ] Send `integration_scripts()` with the SSH Tier A bootstrap. §9b

## temor-wire (lane/wire, reviewed at 5e2ffd8)
- [ ] Callers (temor-ssh, temor-remote) must check the first frame is `Hello`, answer
      `UnknownRequest`/`UnreadableRequest`, surface `UnreadableEvent`, and add request timeouts. §9b, §4b
- [ ] `Unknown` catch-alls on wire enums and the `u32::MAX` no-payload marker are additions to §3;
      a request carrying a nested newer enum is answered `Unreadable` (safe, not precise).
- [ ] Tune snapshot and file-size limits and the retry policy (1 s base, 60 s cap, 30 tries) against
      real hosts. §9b, §14

## temor-transfer (lane/transfer, reviewed at 39cc6df)
- [ ] Edge contract for the app: feed back every write failure (`WriteFailed{op}`) and every
      successful Finish (`WriteDone`); hold the `Root` for the transfer's lifetime. §9
- [ ] Upload by drop (`SendSession`) is not built. §9
- [ ] Kitty `prm` is ignored (files 0o644, folders 0o755); in-root symlinks are refused, not made.
- [ ] Verify the kitty `st` reply encoding and `tt` values against a real kitty. §9
- [ ] A malformed frame with no open files only replies; the transfer continues.

## temor-render (lane/render, reviewed at f4ead2c)
- [ ] The app must call `Palette::apply` on every new terminal before its first frame and on theme
      change; grid colours from quire's terminal tokens once v0.2.31 is tagged. §5
- [ ] GPU path waits for quire's render hook (v0.2.32); no device-level tests for atlas retries or
      the oversize-image skip. §5
- [ ] A capture error after a resize leaves the new size applied; harmless while libghostty reports
      Full after resize. §5
- [ ] Kitty unicode-placeholder (virtual) placements are not drawn; selection changes recompose the
      whole pane; CPU clips glyph overhang to the row, GPU does not. §5

## quire v0.3.1 (b9b9c03b) — bump in R1, features in P3
- [x] Bump the pin to `rev = "b9b9c03bf166f0e2be834518b2dd1ee76b4de7f5"  # v0.3.1` (done in R1a; the
      IME machine copy is dropped in 5f852bf, now `ds::edit::composition` and `RawKeySurface`).
- [x] IME candidate area: `ime().switch(On)` then `cursor_area` with the layer origin plus the caret
      cell, through the host seam with a Busy retry (5f852bf; `view/window_help.rs`, `view/ime.rs`).
      Not seen on a real fcitx5 yet, and ds-harness has no IME host to test it (leftover below).
- [x] Theme: the palette is re-applied on each `use_environment` change and on a Space look change
      (5bccf7d; `view/theme.rs`; the follower is `view/palette.rs`, a child inside the `Ds` root that
      reads `use_space_look()` there). `use_space_frame()` is not followed: its frame
      variables are sidebar ink, and P3a has no sidebar (P3b). Window title: `use_window_title` (fefac1e).
- [x] IME on the pane's TextureLayer: `switch(On)` first, `cursor_area` with the caret in window-logical
      px, Busy retried (5f852bf). quire's `EditSurface` retry is private, so `view/ime.rs` keeps its own.
- [x] Add quire's coherence test (examples/consumer/tests/it/coherence.rs), adapted (e318913).
- [x] Keys (PLAN §6, answered by whopper): every ⌘ action is bound to Ctrl+Shift+key and Super+key;
      a pane chord is ⌘ by its form, never `is_command` (2f9cf18). Tests at input level:
      Ctrl+Shift+C and Super+C copy (New Tab does not exist yet, so Copy stands in), Ctrl+C and an
      unbound Ctrl+Shift combo reach the program, Super alone and Super+Space match no binding.
      The keymap is in temor-pane `keys`, routing in temor-ui `input/`.

## temor-ui / temor (lane/app, P1, reviewed at f783a2a)
- [ ] quire v0.2.31: replace plain layout divs and the 100vh root with quire layout components and
      `RootExtent::Window`; terminal palette tokens; re-apply the palette on theme change. §5
      (partly: `RootExtent::Window` and quire's terminal tokens in 91eaf50, the theme change in 5bccf7d.
      The column is still a flex element: quire v0.3.1 has no column component, asked.)
- [x] quire asks (whopper offline; saved in /mnt/data/cargo/temor-lanes/pending-quire-asks.md): IME
      cursor area API, runtime window title, theme-change signal, consumer coherence test. Now in
      quire v0.3.1 and used (5f852bf, fefac1e, 5bccf7d, e318913). §6
- [ ] Bell, OSC 0/2 title, OSC 52 writes, notifications, cwd and progress are received but not shown
      (P2/P3). Hyperlink hover target preview not built. §6, §7
- [ ] Settings are temor's own typed file at $XDG_CONFIG_HOME/temor/terminal.conf; move to
      ds-settings when quire provides it. §6
- [ ] Per-cell text reads cost ~3 FFI calls per cell: measure search on a large scrollback. §5
- [ ] Output read just before a resize may be parsed at the new size (the usual SIGWINCH window).
- [ ] Without XDG_RUNTIME_DIR the runtime folder is ~/.cache/temor/run (0700, owner checked).

## temor-ssh (lane/ssh, reviewed at f3116b1 plus orchestrator fixes)
- [ ] P13 wiring: host list in ⌘T (plus tailnet machines, D22), opening panes through the shared
      master, consent per host, `HostId::same_machine` for the gate. §9b
- [ ] HostAlias rejects `:`, `[`, `]`: IPv6 literals need a HostName in ssh_config; say so in the UI.
- [ ] Nushell on the host: the bootstrap places `temor.nu` under `nu/nushell/vendor/autoload`; P13 adds the `nu` case that starts nushell with `XDG_DATA_DIRS` pointing there (same as temor-pty's local plan). §9b
- [ ] IdentityFile and ProxyJump values must be validated before any `-i`/`-J` builder uses them.
- [ ] fish 4.9 prints no marks on piped stdin, so fish marks over SSH aren't asserted; no real sshd
      test yet (only `ssh -G`); csh/tcsh login shells untested. §13
- [ ] A bootstrap dir owned by us with mode 0300/0000 fails closed (EACCES) instead of tightening.

## Platform (owner): OS-specific stubs to implement on real machines
Each one sits behind a `platform/` edge trait, marked `// TODO(platform:<os>)`.
- [ ] macOS keymap: the primary modifier is Cmd (bind Cmd chords, not keycap's Ctrl+Shift);
      temor-core terminal/chord.rs and keymap.rs defaults. §6
- [ ] Link opener and private runtime dir: temor-ui platform/opener.rs and private_dir non-unix
      stubs. §6
- [ ] macOS emoji: Apple Color Emoji is sbix (bitmap); implement `SystemBitmapEmoji` in
      temor-render platform/other.rs. §5
- [ ] macOS pty: `Pty::spawn` and `run_session_leader` (platform/unsupported.rs); process name, cwd,
      current user and `is_executable` (platform/macos.rs) through `proc_pidinfo`. temor-pty
- [ ] macOS trial sandbox: none (bwrap is Linux-only), so proposals say "not checked". Decide whether
      anything replaces it. §8.3
- [ ] Ubuntu 24.04+: unprivileged user namespaces are restricted by AppArmor; detect this, report
      `NoSandbox`, and document the profile. §8.3
- [ ] macOS: verify quire windows and IME (winit) and fontique's CoreText path; consider a
      `cargo check --target aarch64-apple-darwin` CI job (Zig can cross-build libghostty).
- [ ] macOS/Windows transfer edge: `temor-transfer` platform/unsupported.rs refuses all writes
      (needs an openat2-like safe open), `sync_folder` is a no-op, and the duplicate-name key must be
      case-folded/normalised; Windows `RelPath` must refuse `:` components. §9
- [ ] temor-ssh private runtime dir and regular-file reads: platform/unsupported.rs stubs
      (macOS, Windows). §9b
- [ ] Windows: PTY (ConPTY; temor-pty platform/windows.rs and unsupported.rs) and the plugin process
      side are unsupported today.

## App icon (in dist/, wire up when packaging)
- Linux: `dist/icons/hicolor/<N>x<N>/apps/org.quire.Temor.png` (16–512) installs under `$prefix/share/icons/hicolor/`; the desktop entry already says `Icon=org.quire.Temor`. Window icon at runtime: set from the 256/512 PNG once quire lands its window-icon setter (queued: launch-config field plus `WindowHost::set_icon`).
- macOS (owner): build `.icns` from `dist/icons/source/org.quire.Temor-1024.png`.
- Windows (owner): `dist/windows/temor.ico` (6 sizes) into the exe resources.

## Blocks (lane/blocks, P2, reviewed at 08c5ce2 plus orchestrator fixes)
- [ ] Mark cuts: libghostty reports an OSC 133 mark when it reads the ESC that ends it; the cut
      sits right after that ESC (scan/marks.rs). If a Ghostty bump changes where it dispatches,
      the blocks tests time out at "the first prompt"; check `marks_split_at_every_offset`.
- [ ] Nushell marks are tested through the env only; a real nu block test once nu is on the gate PATH.
- [ ] view/window.rs is near 300 lines; P3's split tree replaces most of it.

## R1a (lane/r1a)
- [ ] temor-vt-sys: generated bindings carry indented C doc comments that rustdoc runs as ~100 failing
      doctests. Have `scripts/gen-bindings.sh` fence them as `text` (or set `doctest = false` on the
      -sys lib), then add a crate-root example.
- [ ] temor-vt `row_wraps` still returns `bool` (temor-vt can't see temor-core's `Wrap`); give temor-vt
      its own two-variant enum when R1b touches rows.

## R1b (lane/r1b)
- [ ] Blocks stay in temor-core (L2) because they use `vocab` ids and quire's `Stamp`. An embedder of
      temor-pane gets raw OSC 133 events (`MarkBegin`/`Prompt`/`Anchor`) but no block model. Move the
      block machine into temor-pane with its own id/time types once an embedder (anyview, sill) needs it.

## P3a (lane/p3a: keys, layout and tokens, theme, title, IME, coherence)
- [ ] quire asks: (1) a column or stack layout component (v0.3.1 has only SplitView); (2)
      `focus_landing_told` is `pub(crate)` at v0.3.1 (the v0.3.1 notes say public); (3) EditSurface's
      Busy retry (`write_soon`) is private, so `view/ime.rs` keeps its own 16-try loop; (4)
      `RawKeySurface` takes no style, pointer or wheel handlers and sizes nothing: the grid needs an
      inner element and a sheet rule for the surface's class (`view/window.rs` `WINDOW_CSS`); (5)
      `.ds-texture-layer` has no rule in quire's sheet (the coherence test allows that one class). §6
- [x] A keymap setting replaces both command forms of its action, which gives the chord back to the
      program; a bare Ctrl chord is refused with the ⌘ forms named (review fixes). §6
- [ ] Unbound Super+letter reaches the program as text (as before P3a). Ghostty drops unbound ⌘ keys;
      decide the same here. §6
- [ ] KWin takes Meta+T/W/D/V globally on KDE, so those Super forms never reach temor there; the
      desktop's reserved chords are with whopper. §6
- [ ] IME candidate placement is untested: ds-harness has no IME host, and the owner's fcitx5 is the only
      real check. §6
- [ ] The window title is untested in the harness (no window host there); only the rule is tested. §7
- [ ] Per-workspace Space looks: the window draws `SpaceLook::default()` until sill publishes them. §5
- [ ] The palette follows the Space look (read inside `Ds`, `view/palette.rs`) but not
      `use_space_frame()` (the sidebar's ink; P3b). The look itself is `SpaceLook::default()` until
      sill publishes per-workspace looks, so no change reaches it yet. §5
- [ ] `real_shells_*` and `a_five_mebibyte_paste_arrives_intact` time out under a parallel
      `cargo test -p temor-ui` (fish startup, a 15 s wait); they pass alone. §13

## P3b (lane/p3b: tabs, splits, session restore)
- [ ] Window integration is not done: the window still shows one session. The workspace model
      (temor-core `workspace`) is not driven by the window yet, and the sidebar (quire `Sidebar`
      with tab rows), the split miniature, the `SplitView` tree, drag to split and tear, and the
      `use_space_frame()` sidebar ink are not built. §7
- [ ] The session file (`temor-ui::edge::session_file`) is written and read, but the window does
      not load or save it yet, so nothing is restored on launch. Saves are paced (once a second,
      plus one on quit). §7
- [ ] Restoring scrollback needs a temor-pane entry point that starts a pane from a
      `temor_vt::Snapshot`. temor-vt's `Terminal::restore` exists and nothing calls it. Until then
      no snapshot sidecars are written. When added: the state dir is 0700, files 0600, each
      snapshot is capped at 4 MiB, a pane on the alternate screen is never snapshotted, and a
      tab's sidecars are deleted when it is forgotten. §7
- [ ] ⌘T opens a plain new tab for now (the ⌘T bar is P5). §6
- [ ] The pane ignores the workspace actions (`Session::run` no-op) until the window routes them
      to the workspace step. §6
- [ ] quire asks: `SplitView` needs a starting share and a finished-drag report, so the tree's
      ratio round-trips (if absent, the tree keeps its ratio and the screen uses equal shares,
      and quire gets this ask). The unread dot has no named component at v0.3.1 (a token-styled
      dot is the fallback). Check both at window integration.
- [ ] ⌃⌘= is bound to Ctrl+Shift+= and Ctrl+Super+=. The Ctrl+Super form is not a ⌘ chord under
      the terminal rule (it has Ctrl), so the owner should confirm this binding. §6

## Batch 1 (2026-10-10: window, spaces, desktop, ssh, remote, transfers, agents)
Written without builds by Sonnet lanes; compiled, fixed and gated together by the orchestrator.
Items below are each lane's leftovers and the orchestrator's decisions.

### window


- quire ask: `SplitView` takes a starting share (used) but reports no finished drag, so a divider the
  person drags is not written back to the tree. It stays where it was dragged until the tab's
  structure changes (`view/tree_view.rs`). Ask: an `on_resized(index, Share)` handler.
- quire ask: a named unread badge (a token-styled `Badge` dot is used, `view/tab_row.rs`).
- Snapshot sidecars: libghostty's snapshot is one opaque block, so "cap at 4 MiB, keep the newest
  part" cannot cut it. A snapshot over the cap is not stored (`edge/sidecar.rs`, `Session::snapshot`).
  A real cap needs the pane to trim its scrollback first (temor-vt) or a text-based store.
- Sidecars are written when the hub is dropped (quit). If the process is killed, none are written
  and the next launch restores trees and folders only. Dioxus gives no explicit quit hook, so the
  drop of the window's `Hub` is the quit save; confirm on the real window that it runs.
- Blitz panic on Restart (node.rs:1573) was not reproduced (nothing runs here). Temor-side changes:
  the restart now happens a frame after the press, element handles of the input method are dropped
  when a pane leaves the page (`ImeGlue::release`), and no pane measures a handle across an unmount.
  If it still panics, the node left behind is a quire/Blitz one (a hovered node of the removed
  banner); report it upstream with the harness test `restarting_an_ended_shell_*`.
- Pinning a tab, folders and renaming have no controls yet (the model and sidebar support them).
- A restored pinned tab types its command into the new shell (`hub/act.rs` `type_command`); it does
  not replace the shell with the command.
- Tab titles follow the program's title in the pane that has the keys; they are saved (paced).
- Keyboard focus after a sidebar press: the pane takes the keyboard back on the next window update
  (`view/pane_view.rs` `had_keyboard`); verify on a real window.
- `ReopenTab` (⌘⇧T) is a new temor-pane action; PLAN §6 should list it.

### spaces


- temor-ui must change to compile with this lane (not touched, another lane owns it):
  - `session/keys.rs:75`: add `| Action::SelectSpace(_)` to the arm of window-owned actions.
  - `edge/session_file.rs:48-53`: `Restored` and `FreshWhy` are now `#[non_exhaustive]`; add a `_` arm.
  - `edge/save_pace.rs`: `Pace` is in temor-ui, so its `#[non_exhaustive]` (review item 7) is for the window lane.
- Wiring for the window lane:
  - Space switching: `Action::SelectSpace(n)` -> `Spaces::at(n-1)`.
  - New tabs: `space::launch(space, project, dotenv_text, runtime)` gives cwd, env, agent plan and credential list.
  - The edge reads `dotenv_path(project)`, starts the agent at `AgentPlan::socket` (folder `<runtime>/temor`, mode 0700), asks porter for each `Credential`, and spawns.
  - Routes: `routes_of(spaces.list())` then `route(command, &routes)` / `route_folder(cwd, &routes)`.
  - Quick terminal: `quick_step`, then on `Promote` call `quick::adopt(&workspace_of(space), pane, cwd)`. `adopt` exists because `WorkspaceIn` has no "adopt a running pane" input.
- Palette: `bar_step` keeps its own highlight and `OpenBar.rows` (kept from P3). To sit on quire's `PaletteState`, temor-ui feeds `OpenBar.rows` as `PaletteParams { rows }` and sends `Refreshed` on `BarOut::Refreshed`. The adapter lives in temor-ui because temor-core cannot depend on the `ds` views crate. Once that is wired, the duplicate highlight in `OpenBar` can go.
- Ctrl+1..9 Space chords:
  - Plain Ctrl+digit is the only form. Super+digit is reserved, and Ctrl+Shift+digit is the tab chord.
  - It is a bare Ctrl chord and so takes Ctrl+2..7 (NUL, ESC..US) from programs that want them. Owner decision: keep, or move to a chord with Alt.
  - quire's own note says Ctrl+digit is the desktop's workspace switch "left free of apps"; check on KDE that KWin does not take Ctrl+1..9 first.
  - The keymap can't rebind them (not in `Action::ALL`, and bare Ctrl is refused); add settings keys if wanted.
- Quick terminal closing ends the shell (throwaway). A "hide, keep running" mode is a different state machine; ask the owner.
- The quick terminal's hotkey is a model input only (`QuickIn::Toggle`). A global hotkey needs the desktop/portal edge (TODO(platform)); no `Action` was added.
- Bar: no "Move to Space" row yet; a Target for it needs the quick terminal open. The "Ask about" row reads "Ask about this tab: ..." (existing wording) while PLAN says "@tab".
- Dotenv: multi-line quoted values, `${VAR}` expansion and `export -n` are out of scope by the rules in `space/dotenv.rs`.
- Routes: patterns are not validated when saved (an empty or all-star pattern is allowed; `*` alone matches every command at lowest specificity). A settings UI should warn.
- `closed.rs` `next_seq` now saturates instead of overflowing; a pure sequence number, so no refusal path.

#### Orchestrator decisions (2026-10-10)
- ⌃1–9 stays the Space switch (PLAN §6), but must use quire's `SwitchChord` definition, not a hand-coded Ctrl+digit table; fix at merge.
- Quick terminal hotkey toggles: hide and keep the shell while temor runs; it isn't saved with the session unless promoted ("Move to Space").
- At merge: temor-ui session/keys.rs add `Action::SelectSpace(_)`; edge/session_file.rs add `_` arms for `Restored`/`FreshWhy`.

### desktop


- Window wiring (another lane): implement `temor_desktop::WindowPort` over the window's shared
  state (reads must return from a snapshot, they run on the bus task), then at startup
  `SystemInstance::new(env).claim(&Request::NewWindow)`; on `Claim::Primary` call
  `SystemIntents::serve(&env, window)`, keep the `Primary` and `SystemIntents` alive, and drive
  `primary.next()`. Needs a tokio runtime and `ds-settings/tokio` (zbus is built on tokio here,
  as docket and porter build it).
- temor-ui must depend on temor-desktop with `default-features = false` and forward its own
  `quire-desktop` feature, or the portable build pulls zbus.
- Action names are `temor.*`, not `terminal.*` as PLAN §8.4 writes them: docket's validator
  requires the app name's last element as prefix. PLAN should say so.
- `context()`: docket's ContextSnapshot has no field for the focused pane's folder or the last
  block, and temor declares no entities. cwd and the last command are served by the actions
  `temor.pane.cwd` / `temor.command.read` (they taint the session). If the companion should get
  them in `context()`, temor needs entity kinds (`temor.pane`, `temor.command`) in the manifest.
- The Space in `Invocation.space` is not mapped to the window's Spaces yet; the window decides
  which Space an action acts in. `SpaceRef` is only used to label `context()`.
- Dock badge: nothing in ds-desktop or docket sends `LauncherEntry.Update` (sill only reads it), so
  `SystemDock` emits the signal itself at object path `/com/canonical/unity/launcherentry/1`.
  A dock that keys on the path (none known) would need the libunity hash.
- Porter identifies the terminal by its systemd unit (`org.quire.Temor.service`). A temor started
  from the launcher or a shell is not that unit, so Tailnet1 may answer `Denied`. `SystemTailnet`
  returns that as an error (not an empty list); the window should show the group as absent.
- Dependency copies: docket abbb62c pins porter 55d1ade, the Tailnet reader is porter a7c11d2, so
  two copies of porter-core are in the tree. temor only touches the a7c11d2 copy through
  `porter_client::Machine`. When docket has a rev pinning a7c11d2 or later, bump docket and
  prov/porter-core to it and the duplicate goes.
- `porter-client` (feature `dbus`) still pulls porter-provider, porter-secrets and porter-service
  (oo7 keyring etc.). Ask porter for a `tailnet`-only feature if the build weight matters.
- `DBusActivatable=true` plus `org.freedesktop.Application` is not implemented; activation is
  by the two bus names in dist/. Add it if the launcher should activate temor that way.
- Platform TODOs: macOS dock badge and single instance, Windows taskbar badge and single
  instance (stubs in crates/temor-desktop/src/platform/unsupported.rs).
- The `quire-desktop` boundary gate is in scripts/check-layers.sh (metadata level). A build-level
  check (`cargo tree --no-default-features -p temor-desktop` has no zbus) is for the orchestrator's
  gate if wanted.
- Add `temor-desktop` and its tests (need `dbus-daemon`) to the TEMOR_REQUIRE_TOOLS list.

#### Orchestrator decisions (2026-10-10)
- Two porter-core copies (docket abbb62c pins porter 55d1ade; porter-client is a7c11d2): accepted until docket moves; asked the agent session for a docket rev on porter ≥ a7c11d2.
- `temor.pane` / `temor.command` entity kinds and richer `context()`: P12 (companion context).
- Launcher starts must run as the unit so porter grants Tailnet1: set `DBusActivatable=true` in dist/org.quire.Temor.desktop (the launcher then activates org.quire.Temor, and D-Bus starts it through `SystemdService=`). Do at merge.
- UPDATE: pin docket 1b905edc5303c9f2241f8a5f6358968ece8dde36 and porter (prov, porter-core, porter-client) b12886e4d047d9763a4dccfa9fa3ce92084483ca (includes a7c11d2) at merge, for one porter-core. Batch 10 later: porter a1a07f5 (additive).

### ssh


#### For the window (temor-ui / temor-desktop) to wire
- Host list: `merge_hosts(&load_host_list(..), &tailnet_hosts)` -> `HostRow`s; group by `HostRow::group()`,
  dim `presence() == Offline`, connect with `destination()`. The desktop lane maps porter's `Machine`
  to `temor_ssh::TailnetHost` (`online`/`ssh` are `Presence`/`TailscaleSsh` enums, not bools).
- Session start: `consent::gate(book, Who{key, alias})` -> ask one question at a time (`Gate::Ask`),
  then `RunMode::Bootstrap(policy)` -> `BootstrapScript::build_with(.., BootstrapOptions::with_terminfo(policy))`,
  `RunMode::Plain` -> plain `ssh` with no remote command. After the first connection verifies the host key,
  send `ConsentIn::Identified` so answers recorded under the alias join the machine's record.
- Panes: `master_step` decides First vs Join; run `prepare_control_dirs` once, `probe_master` on `MasterOut::Probe`,
  `session_argv` on `MasterOut::Open`. Feed `MasterIn::Established` when the first session's host key is verified.
- Where the fingerprint comes from for `HostKey`/`HostId` on a plain alias is not built (needs the live
  handshake: e.g. `ssh -v` parse or `ssh-keygen -l` on the key the session presented).
- Persist `ConsentBook` (serde, a list of records) in the session file; not wired.
- `CheckScan` must be fed raw pre-shell bytes and stopped at the first OSC 7/133; the window shows the row
  and opens the link only on click.
- Local copy watching: poll the local copy (e.g. every 300 ms) into `LocalStamp`, `watch_step`, feed
  `edit_step`; after the app rewrites the copy (fetch), restart with `Watch::new(stamp)`.

#### Known gaps
- Remote file edge uses `scp -s` (OpenSSH 8.7+ for the flag; older clients only have the legacy mode). Add an
  sftp-batch fallback or a version probe.
- Stat granularity is whole seconds (`stat -c %Y`): an edit that keeps the size and lands in the same second
  as the fetch goes unnoticed. GNU `%.9Y` / BSD `%Fm` would fix it.
- Write-back copies over the file in place (keeps owner and mode, not atomic). A temp-file-and-rename mode
  with mode/owner copy is not built.
- Remote paths with `* ? [ ] { } \` are refused (scp globs them); `~/` paths are refused (use the OSC 7 cwd).
- tailnet alias matching is by MagicDNS name or address only; a bare short `HostName` equal to the machine
  name does not match.
- Forwards bind the default (loopback) address and always target the host's `localhost`; remote-host and
  bind choices are in `Forward` but the step does not take them. Port detection from output is P14/UI.
- `ForwardFault` mapping from `ssh -O forward` stderr text is not written (app must classify).
- Consent: only `Integrate` and `Terminfo` are modelled. Clipboard/transfers/notifications stay in
  `remote::HostConsent` (per Space, not per machine); merging the two stores is open.
- Pins per Space keyed by `HostKey` are not built (the consent book is the pattern to copy).
- The nu case is tested with a stand-in `nu` that reports its start; no real nushell test yet (needs nu on the
  gate PATH, then a marks assertion like zsh).
- No test against the throwaway sshd printing the check URL (only `CheckScan` units); no real sshd test.
- Cargo.lock: added `temor-ssh` to temor-core's dependency list by hand.

#### Orchestrator decisions (2026-10-10)
- `TailnetHost`: keep the ssh lane's shape (Presence/TailscaleSsh enums plus `node: NodeId`); adapt temor-desktop's mapping to it at merge (both lanes wrote crates/temor-ssh/src/tailnet.rs).
- `scp -s` (OpenSSH ≥ 8.7) is the default; an sftp fallback for older hosts is a follow-up.

### remote


#### Not done (needs something outside this lane)
- **Trial runs on the host and the agent relay** (`Request::RunTrial`, `Request::Relay`): need
  bulkhead's trial module and its shared path/limit/redaction code. The daemon does not advertise
  `Capability::Trial` / `Capability::Relay`; both requests answer `RemoteError::Unsupported`.
  `src/daemon/dispatch.rs` is the one place to add them.
- **Chunked uploads**: `WriteFile` carries the whole file in one frame, so `Client::upload` stops at
  about 63 MiB (`Error::TooLarge`). Wire addition wanted: a write at an offset (or an append) with a
  final commit step, so big uploads can stream. I did not touch temor-wire.
- **Keepalive**: the wire has no ping. A client too slow to read is hung up on by the daemon (its
  queue overflowed) and sees end-of-stream; a stalled but open link is only noticed by ssh. A
  `Ping` request in the wire would let `link_step` notice dead links sooner.
- **Cell size and colours of the host-side screen**: the screen the daemon keeps uses libghostty
  defaults (8x16 px cells, default palette, dark colour scheme). Colour queries (OSC 10/11) and
  pixel reports from programs on the host are answered from those, not the window's. `OpenSession`
  and `ResizeSession` would need to carry the cell size, and a palette message would be needed.
- **`OpenSession` has no argument list** (only a program). Login-shell flags and shell-integration
  injection (temor-shell `plan_integration`) are not applied on the host; Tier A's bootstrap env is
  what a client can put in `env`.
- **Default shell** is `$SHELL`, else `/bin/sh`; no passwd lookup (temor-pty has none).

#### Known limits
- Requests on one connection are handled one at a time on its reader thread, so a big file read
  delays keystrokes sent on the same connection. Fix: hand file requests to a worker thread.
- Watches poll with `stat` once per `tick` (about a second). A folder's mtime shows entry changes,
  but not changes inside sub-folders. inotify (rustix `fs::inotify`) is the refinement.
- The port scan reads all of `/proc` each tick; throttle it if hosts with thousands of processes
  matter.
- The write-back check and the replace are two steps; a change between them is lost (documented on
  `FileService::write`).
- A symlink is listed and `stat`ed as a symlink and never followed (confinement). The wire's doc says
  `stat` may report what it points at; it does not here.
- The daemon does not survive its own death (no persistence of PTYs, by design: a PTY cannot be
  re-owned). An updated helper (new hash) listens on a new socket, so the old daemon and its
  sessions stay until "Remove from host" or until its sessions end. A `Shutdown` request is not in
  the wire; `Daemon::shutdown` exists for it.
- Terminal replies queued for a program that never reads are capped at 1 MiB (dropped beyond),
  as in temor-pane.

#### For the window (client side use)
- Command to run over ssh: `ssh host ~/.cache/temor-remote/<hash>/temor-remote stdio` (starts the
  daemon when needed). `Client::connect(child.stdout, child.stdin, hello(expected_hash, &DAEMON_CAPS))`.
- After a lost link: `link_step` -> `Dial` -> `Client::connect` again -> `attach` each session
  (`Attached::screen()` -> `temor_vt::Terminal::restore`), then feed `ClientEvent::Output`.
- `port_actions(&HostReach, Port)` gives "Open at <name>:<port>" first for tailnet hosts, then
  "Forward port...".

#### Platform (owner)
- `src/platform/unsupported.rs`: `Beneath` (confined files), `listening_ports`, the unix socket
  (`Listener`, `Half`, `connect`, `ensure_private_dir`) and `spawn_detached` refuse with
  `Error::Unsupported` off Linux. `TODO(platform:macos)` / `TODO(platform:windows)` marked there.
  Hosts are Linux per PLAN; the client-side types still build everywhere.

#### Build notes for the orchestrator
- New workspace deps (root Cargo.toml): `temor-wire` (path) and `sha2 = "0.10"`; Cargo.lock needs
  updating (sha2 0.10.x is in the offline registry; I did not resolve it).
- `scripts/check-layers.sh` already listed `temor-remote 3`; no change was needed.

#### Orchestrator decisions (2026-10-10)
- Yes to all three as one temor-wire revision (P14 follow-up): WriteFile with offset/append for streamed uploads; Ping for stalled-link detection in link_step; cell pixel size on Open/Resize plus a Palette message.

### transfers


#### Decisions to confirm
- `temor get` uses a temor dialect of kitty's receive: the client sends `ac=receive;id=X[;n=<name>]`;
  the terminal answers by offering the file with `ac=send` under the same id (`kitty::serve` +
  `SendSession`), and the client takes it with `ReceiveSession` (`drive_receive`). A refusal is an
  `ac=status` failure for the request id. Not wire-compatible with kitty's own `receive` (which lists
  several paths in `file` frames); fine for temor-to-temor.
- Two shelf models exist: temor-core `shelf::Shelf` (live model, `shelf_step`, view) and
  temor-transfer `shelf::ShelfStore` (index file + exclusive file placement). The app maps between
  them: core's `ShelfOut::Place{path}` is created with `create_new` (answer `NameTaken` on EEXIST);
  `ShelfStore::receive_consented` / `receive_saved` are the one-call versions for headless use.

#### For the window (temor-ui lane, after the other lane lands)
- Terminal side of the protocols is not wired: temor-vt reports `UnknownKind::Osc1337File` and the
  OSC 5113 payloads; the pane must feed `Osc1337Reader` (keep one per pane, map temor-vt truncation to
  `Completeness`), `kitty::ReceiveSession` (one per offer; edge contract in its `Input` docs; the
  fid for `WriteDone` is the `file` of the frame that produced the `Finish`, see
  `drive/receive.rs`), and answer `ac=receive` with `kitty::serve` then a `SendSession` (or run
  `drive_send` on a helper thread over the pty as a `Link`).
- Ask once per host/stream via core `transfer::offer`; then `shelf_step(Incoming)`.
- Drop: build `shelf::Dropped` (support: pane has answered a transfer offer or announced; quoting
  from the pane's shell) and carry out `DropAction`. `Send` needs a terminal-initiated send to the
  program; with the current protocol only a program that started the exchange can receive, so
  "announced" support has no source yet. Needs a decision: an `ac=receive`-style advert from
  `temor` on the host (shell integration sets it), or `Unknown` always.

#### Known gaps
- Directories: `SendSession`/`temor save` send regular files only; the receiver already takes folders.
- `temor view -` and `temor save -` (pipes) are not built: they need a temp file/stream with unknown
  size (PLAN §9 "from pipes").
- `temor view` over SSH sends to the shelf and tells the person to open it there; no remote "open".
- `temor save` waits up to 60 s for the first answer (the person may be asked). A terminal that is
  not temor therefore hangs for a minute. Fix: ReceiveSession replies `STARTED` for the offer id
  before asking (changes `an_offer_asks_for_consent...` test), then `first` can be ~3 s.
- Compression (`zip=zlib`), rsync transmission, symlinks, `prm` permissions: still refused/ignored.
- Verify the `st` reply encoding (base64) and `CANCELED` spelling against real kitty (parse_reply
  accepts what encode_reply writes; unknown codes read as EIO).
- Error text from the other side (`Declined`) is passed through; only "User refused the transfer"
  is mapped to a friendlier sentence.
- macOS/Windows: cli `platform/unsupported.rs` (tty line, opener `open`/`start`); transfer edge notes
  already in FOLLOWUPS.
- Dropped-path pasting of names with control characters is refused (a tell), not escaped.

#### Orchestrator decisions (2026-10-10)
- The temor-only `ac=receive` handshake for `temor get` is accepted; document it as temor's variant in temor-transfer's docs.
- "Program announced receive": default Unknown, so the drop pastes quoted paths. Shell integration advertises it later (P9 follow-up).
- `temor save`: answer STARTED before asking consent, so the CLI doesn't wait 60 s; update the test that pins the old order.

### agents


Nothing here was compiled or run (batch rule). The orchestrator's first build will find the typos.

#### Found already done (told in the report)
- OSC 7501 was already parsed by libghostty into `VtEvent::ProgramStatus` (temor-vt `ffi/reports.rs`), with `ProgramStatus` in `event/status.rs`. I added only a pure reader (`scan/status.rs`) so `FakeVt` emits the same event; it is `testing`-feature only.
- `presence_step` and `checkpoint_step` existed. Added `agent::radar` over `presence_step` (sources, precedence, timing, title hints, process facts) and `checkpoint::git` (the refs and the restore plan).

#### Assumptions to check
- OSC 7501 grammar in `scan/status.rs` is taken from the libghostty header example only (spec page not in the repo): fields split on `:`, `key=value`, `title`/`msg` base64, limits 256 bytes (`id`, `app`) and 1024 bytes decoded (`title`, `msg`). Compare with the real spec at superlogical.com/rex/docs/build/program-status. The reply to `OSC 7501 ; ?` is not produced by the fake.
- `title_hint`: a leading Braille pattern char means busy (Claude Code's spinner). Unverified for other agents.
- Radar timings (`RadarParams::default`): precedence hold 60 s, title hold 4 s, stale working 15 min. Tune with real agents.
- A notification or bell makes an unfocused pane Waiting(Question) until focus or a prompt; maybe too loud for bells in plain shells. The app should send `Notified` only for panes that are agents or have 7501 records.
- Pane socket: per-pane file `<dir>/<pane id>.sock`, mode 0600, directory 0700. Path must be under 104 bytes. Protocol v1 lines are in `temor-pane/src/report/protocol.rs`.

#### Needs the owner or another lane
- `ProcessGit` lives in temor-core (`worktree/process.rs`), the only code there that starts a process. PLAN says the core has no I/O. If that is not wanted, move that one file to temor-desktop and keep the `Git` trait in core.
- Restore deletes: `checkpoint::restore_files` writes files back but the core cannot delete files made since; the caller deletes `RestorePlan::delete`.
- Checkpoint numbers: `Checkpoints::default()` starts at 0 but refs are per repository; seed `next` from `checkpoint::next_free` when a run starts or two runs overwrite each other.
- Snapshot index: one fixed private index per worktree (`<gitdir>/temor-snapshot-index`); the runner must not snapshot one worktree twice at once.
- Setting names: the pane socket env needs `TEMOR_SOCKET` to be this per-pane path (temor-pty's `EnvContract.socket` already carries it).
- Platform TODOs: peer uid and private dir on macOS (`getpeereid`) and Windows (named pipe with ACL) in `temor-pane/src/platform/unsupported.rs`; the whole `report` module is `cfg(unix)`.
- The `.worktreeinclude` name is unverified (PLAN says so). The matcher is a small gitignore subset (no `!`).
- Not built: the "Leftover branches" list (needs `git worktree list` parsing), SSH/Tier B worktrees, `Open pull request` (outcome only), the shell split on conflict.
- Semantic overlap is invisible by design; the UI must show `TEXT_OVERLAP_ONLY`.
- D20: `LaunchFact`/`GateFact` need real readers (docket-launch profile, docket session). `ModeChange::Asks` has no gate API yet.
- D21: sizes are bytes of the text; no token estimate. The sent list stores the chip text (the log is local); decide retention.
- Cargo.lock was edited by hand (tempfile dev-dep on temor-core and temor-pane, rustix on temor-pane); let cargo rewrite it.

#### Orchestrator decisions (2026-10-10)
- Move `temor-core/src/worktree/process.rs` (ProcessGit) to temor-ui `edge/` at merge; core keeps the `Git` trait (L2 stays I/O-free).
- Braille-spinner title hint: keep as the weakest fallback (OSC 7501 and process facts win).
- Bell/notification sets Waiting only for agent panes; the app sends `Notified` only there.
- Duplicate commit message 6cf65ed/71ed891: fine, leave history.

### orchestrator (merge)
- [ ] Space switch: no default chord until the desktop rule settles (quire leaves Ctrl+digit to the desktop; the shell owns Super+digit); asked the agent session.
- [ ] Unbound Ctrl+Shift chords are dropped. Once a program turns on kitty keyboard flags they are distinct keys; forward them then instead of dropping.
- [ ] `temor save` STARTED-before-consent (transfers decision) not yet done.
- [ ] The Restart panic (Blitz node.rs:1573) was not reproduced; the regression test runs it in the harness. If it still happens on the desktop, take it to quire/Blitz with a backtrace (RUST_BACKTRACE=1).

## quire v0.3.4
- [x] Bumped to v0.3.4 (e0d1ed48; v0.3.3 lacked `ds-blitz/src/layout_chain.rs`). The hover-card guard fixes the Restart panic; `restarting_an_ended_shell_gives_a_new_prompt_and_keeps_the_window_alive` runs again. quire now styles `.ds-texture-layer`, so the coherence exemption is gone.
- [ ] Adopt in Batch 2: `SwitchChord::Unbound` for the Space switch when Spaces reach the window; `Column`; `ds::focus::soon::focus_landing_told`; `ds::components::editor::write_soon::write_soon`; RawKeySurface `style` and pointer/wheel props; ds-harness `WindowHosting::Recorded` (`window_titles()`, `window_icon()`); `WindowIcon` + `HostWindow::set_icon` (X11 and Windows; Wayland takes the desktop entry's icon).
