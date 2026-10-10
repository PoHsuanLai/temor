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

## quire v0.3.4, v0.3.6
- [x] Bumped to v0.3.4 (e0d1ed48; v0.3.3 lacked `ds-blitz/src/layout_chain.rs`). The hover-card guard fixes the Restart panic; `restarting_an_ended_shell_gives_a_new_prompt_and_keeps_the_window_alive` runs again. quire now styles `.ds-texture-layer`, so the coherence exemption is gone.
- [ ] Adopt in Batch 2: `SwitchChord::Unbound` for the Space switch when Spaces reach the window; `Column`; `ds::focus::soon::focus_landing_told`; `ds::components::editor::write_soon::write_soon`; RawKeySurface `style` and pointer/wheel props; ds-harness `WindowHosting::Recorded` (`window_titles()`, `window_icon()`); `WindowIcon` + `HostWindow::set_icon` (X11 and Windows; Wayland takes the desktop entry's icon).

## Keys on chordkit (lane/keys, chordkit d6fed65)
- [x] chordkit 338b717 (ba094c6 fixes the fallback where the default is plain Ctrl in a terminal): temor forgoes "show fonts" and "new", so ⌘T is new tab (⌘N free); equalize has the fallback ⌥⌘0 (Ctrl+Shift+Alt+0 where Primary is Ctrl+Shift); a person's chord revives an action refused at start (focus left/right on "Other" Linux, where Ctrl+Alt arrows switch Spaces).
- [ ] chordkit FINDINGS: a fallback applies only where the default has no live form, not where it collides; focus left/right stay refused by default on "Other" Linux.
- [x] keycap 0d9abbf adds the `keymap-aware` profile (⌘ arrives as Super); temor's desktop entry asks for it with `X-Keycap-Profile=keymap-aware`. A still-remapped window falls back to chordkit's Ctrl+Shift alias, where ⌘⇧D reads as ⌘D.
- [ ] Forward Primary-form chords to the program under the kitty keyboard flags (route.rs) instead of the drop.
- [ ] Desktop shortcut sources: keycap on our desktop, `chordkit-kde`, `chordkit-gnome` (edge/keymap.rs uses `ConventionSource` only), plus `KeymapSource::watch` to reload and re-register.
- [x] quire v0.3.6 (8ce6d4b, chordkit ba094c6, maps ALT_GRAPH): one chordkit in the tree. temor still routes its own window keys; moving to quire's `ds::keys` resolver is a later step.
- [ ] Wire `keys::{shortcut, terminal_shortcut}` into menus when they show shortcuts.
- [ ] The old `keys.<action> = ctrl+shift+c` lines now report "no setting called"; say so in the user docs when settings are documented.
- [ ] Space switch: `SelectSpace(n)` is chordkit's switch-to-space standard action; no platform binds it live in a terminal, so it is reached from the bar and the Spaces menu only.
- [ ] temor-ssh `files::an_awkward_path_is_fetched_edited_and_written_back` flaked once with ETXTBSY (a fake `ssh` script written while another test thread forked). Retry the spawn on ExecutableFileBusy in the test helper, or write the fakes before the threads start.

## Batch 2 (2026-10-10: Spaces, ⌘T bar and quick terminal, desktop, SSH Tier A, shelf, agents in the window)
Written without builds by Sonnet lanes; merged, compiled, fixed and gated together by the orchestrator.

### Orchestrator decisions (2026-10-10)
- Agent- or intent-opened panes do not take the keyboard; a second launch raises the window (not done yet: both are defaults to apply).
- Dock badge should count agents waiting on the person (`waiting_count`), not unread tabs; switch the desktop badge to it.
- Downloads land in XDG Downloads (lane choice) while PLAN D12 says temor's data dir: owner to decide.
- One `TailnetFeed` (edge/desktop_feed.rs) serves the SSH host list; the bar's own `HostRows` seam is still `NoHosts`: wire it to the hub's host rows.
- Hosts (consent book, panes on hosts) live in the first Space's session file.
- Two openers exist (platform/files.rs from shelf, platform/open_file.rs from ssh): merge into one.

### spaces

## Needs the orchestrator / other lanes
- Desktop lane: provide the account store with `with_context(temor_ui::edge::Credentials(Arc::new(impl CredentialSource)))` on the launch config. The window reads it with `try_consume_context`; without one `NoCredentials` is used and a profile with a credential prints "No saved key for <account> was found" per new shell. Secrets cross as plain `String` (no zeroizing).
- Bar lane: the quick terminal's pane must already be in the hub's slots when it calls `Hub::adopt_into_space(space, pane, cwd)` / `HubRef::adopt_into_space`. It does not switch the Space; call `desk.spaces.switch(space)` after if wanted. Routes for "Run in a new tab": `HubRef::space_for_command(cmd)` / `space_for_folder(dir)` return the Space; the bar then switches and opens the tab.
- Merge: `Desk` has a new field `spaces`; `Hub` a new field `book`. Anyone using `self.ws` directly sees the Space on screen, as before.

## Known gaps
- All Spaces' sessions are restored and their shells started at launch (pane ids are only unique across Spaces if every Space restores together). Lazy start needs pane-id remapping at restore.
- A pane's title change in a Space that is not on screen is not saved as the tab's title (the row uses the live title anyway); `Hub::deliver` finds the tab in the current workspace only.
- Starting a Space's ssh-agent blocks the window thread for up to 2 s on that Space's first new shell. Move to a worker. The agent edge is untested (no test starts `ssh-agent`/`ssh-add`).
- A passphrase-protected key cannot be added (no terminal, no askpass): the person is told, the Space opens without the agent.
- Project root, profile (env, dotenv, key, credentials) and routes have no editor yet: they live in `spaces.json`. Wants a Space settings page (and a warning for empty or all-star route patterns).
- The delete question from the kit says "Everything on disk is not affected. Only X's look and settings go." It does not say that the Space's tabs close. Ask quire for a hook on that text.
- "Move Tab Here" is in the menu of the Space you right-click; a "Move tab to Space" submenu on tab rows (listing the other Spaces) would be nicer, and the bar's "Move to Space" row.
- The kit's `use_keys()` inside `use_spaces` runs outside the `Ds` root (hook is called in `App`), so it makes a default keymap that is never read (`SwitchChord::Unbound`, no `on_key`).
- Space `link` (desktop Space following) is stored by the kit but unused here.
- Windows: the key agent is a named pipe there; `edge/space_agent.rs` has `TODO(platform:windows)` and spawns `ssh-agent -a` as on Unix.
- A hidden Space's shells count as "unfocused" only at launch and on switch; a long command finishing in a hidden Space notifies like any unfocused pane.
- Spaces' `recall` (place) is refreshed in `spaces.json` at each save from every workspace, so losing `session-<id>.json` still restores tabs (without their folders and focus).
- Session files: Space 0 keeps `session.json` (a pre-Spaces session becomes Space 0's), others `session-<id>.json`. A Space deleted while temor is not running leaves its file; ids are never reused within one list, but delete the file by hand if the list is rebuilt.

### bar

## PLAN / FOLLOWUPS text to update (the lane may not edit them)
- PLAN §6 default keys: `temor.open-bar` is ⌘T (forgoes the offered-only "show fonts"); `temor.new-tab` moves to ⌘N (forgoes "new"), the plain tab; `temor.toggle-quick` is ⇧⌘Y. Why: ⌘T must be the bar (D11) and the tab chord the person already knows from other terminals is ⌘N here; ⌘T Enter is also a new tab. The old line "⌘T opens a plain tab until the command bar lands" is done.
- PLAN §7 quick terminal: "one hotkey" is an in-app chord now; a global one is TODO(platform) (below).

## Open
- Older recently-closed tabs: the bar offers only the newest ("Reopen <title>", `Target::Closed(Seq)`), because `WorkspaceIn::Reopen` takes the newest. Reopening a chosen one needs a `ReopenSeq(Seq)` input in `workspace_step`.
- Routes (`route`, `route_folder`) are not applied to the Run row or to folders: they belong to the spaces lane (it knows each Space's routes). Hook: `Hub::run_bar_row` (`hub/bar_run.rs`), before `command_in_new_tab`.
- Space rows: `SpaceRows::go_to` is a seam the spaces lane implements (`BarSeams::spaces`, set with `Hub::set_bar_seams`). Until then no Space rows exist.
- Host rows: `HostRows` is the ssh lane's; `connect` defaults to a new tab running the system `ssh <alias>` (D17), refusing names unsafe for a command line. The ssh lane should override it to use its shared connection.
- AI rows: only when `ModelRows::available()` says On (default `NoModel`: Off, rows absent, no stub). One switch covers all three rows; the PLAN wants "Ask about @tab" wording (core says "Ask about this tab").
- Plugin rows (`open.provider`): no plugin host exists; `BarOut::AskProviders` is ignored and `BarAction::Plugin` is a no-op. Saved layouts: none are stored, so none are offered.
- Folder frecency (`BarSlot::visits`) lives in memory only and is seeded from the panes' folders; persist it with the session when someone owns that file.
- Recent commands are gathered per pane (newest first within a pane), not merged by time across panes: `Block::started` is a per-session `Stamp`.
- Moving a recent-command row into a tab types it without Enter (the person's Enter approves, D6); host rows and the Run row send Enter because the row says it connects/runs.
- Quick terminal: "Move to Space" calls the stub `Hub::adopt_quick_pane` (`hub/quick.rs`) which adopts into the one workspace via `quick::adopt`; the orchestrator replaces its body with the spaces lane's `Hub::adopt_into_space(space, pane, cwd)`. With no Spaces the overlay offers one button, "Keep as a tab" (`SpaceId(0)`, unread).
- Quick terminal global hotkey: TODO(platform): a desktop/portal shortcut that works while another app has the keys. Only the in-app chord exists.
- Quick terminal size is fixed (CSS percentages in `view/window_style.rs`); no resize, no per-screen placement.
- The bar's field takes the keyboard, so ⌘T does not close an open bar (Escape and a click outside do). If wanted, a `claim` on `CommandPalette` can take the chord.
- quire ask: a `PaletteIn::Select(PaletteIndex)` so a controlled palette's `on_select` goes through the machine; today `BarModel::selecting` rebuilds `PaletteState::open(query, index)`.
- Focus return after Escape relies on the pane's existing "take the keyboard back when it becomes the keyboard pane" effect (`pane_view.rs` `had_keyboard`); the scenario `the_bar_opens_ranks_runs_and_hands_the_keys_back` checks it, nothing here proves it on a real window.

### desktop

- Credentials (item 5): not done. temor-desktop exposes nothing over porter for secrets, and the
  spaces lane's `CredentialSource` trait is not on this base. Needed from porter: a client for
  the secrets store through `porter-client` (a `Secrets`-style handle over `DbusTransport::over(conn)`
  with `get(name)` for a Space-scoped item, plus its `Denied` error), and then a `SystemSecrets` in
  temor-desktop::platform::linux that maps it to `CredentialSource`. Wire it in
  `app/desktop.rs::serve` next to the tailnet feed, and add a `credentials: Arc<dyn CredentialSource>`
  to `Desktop` (default no-op), read from the window with `try_consume_context::<Desktop>()`.
- `TailnetFeed` (edge/desktop_feed.rs) is pull-only: `hosts()` returns the last list. The ssh lane
  reads it with `try_consume_context::<temor_ui::app::Desktop>()` (`.tailnet`). If the host list
  must redraw on a change, add a `watch` tick to `LiveTailnet` and bump the desk from a loop.
- Dock badge counts tabs with news (`ws.tabs.unread`). Agent panes that are waiting for an answer
  (the agents lane's presence) should add to the count and set `Attention::Needed`; hook point is
  `Hub::desktop_snapshot` returning the count, and `badge_of` in app/desktop.rs.
- `Snapshot.context.space` is `SpaceRef { local: 0, link: None }` and `selection` is `None`:
  the spaces lane should fill the Space (local id and link) in `Hub::desktop_snapshot`; a selection
  needs a public `Session::selected_text()`.
- Reads of a pane's text (`temor.pane.read`, `temor.command.read`) and every action go to the
  window thread and wait up to 5 s (`ASK_TIMEOUT`); only `context`, `panes`, `commands` come from
  the published snapshot. Publishing happens in the window's render (`view/window.rs`), so a
  window that is not redrawing publishes late.
- `temor.pane.open` supports terminals only (tab, split right, split down); agent and viewer panes
  answer "can only open a terminal for now". The opened pane takes the keyboard (the workspace step
  focuses a new tab or split), which an agent can use to pull the person's focus; consider opening
  it in the background once the workspace step has a way.
- `temor.command.run_sandboxed` answers "not available yet" until the trial module (P8) exists.
- A second launch's NewWindow opens a new tab (temor has one window); it does not raise or focus
  the window. Raising needs an `AppHandle`/activation token from ds-blitz.
- `Exit::Unknown` was added to temor-desktop (command ended, status not reported); it reads as
  `null` in `temor.command.list`, same as Running. If agents need to tell them apart, add an
  `"ended"` field in `perform.rs::command_list`.
- `SystemIntents::serve` and the Linux `SystemTailnet` futures are spawned with `Handle::spawn`
  (app/desktop.rs); if either is not `Send` the compiler will say so. A dedicated
  `current_thread` runtime on its own thread is the fallback.
- A tailnet read that is `Denied` is retried only when `Changed` ticks (and the watch itself fails
  with Denied too, so the feed stays empty until the next launch). Launching through the unit
  (DBusActivatable, orchestrator decision) is what makes porter grant it.
- Cargo.lock was edited by hand (temor: temor-desktop, tokio; temor-ui: temor-desktop, temor-ssh);
  cargo will normalise it.

### ssh

## Wiring the orchestrator or other lanes must do
- [ ] Mount `view::host_list::HostList { desk }` where the host list should show (the bar lane's ⌘T, or a
      sidebar section). Nothing in `App` mounts it yet. The bar reads the same rows through
      `hub::HostRows` (`HubRef`/`Hub` implement it); if the bar lane defined its own `HostRows`, drop mine
      in `hub/hosts.rs` and implement theirs. Same for `TailnetFeed`.
- [ ] The desktop lane provides the tailnet: `hub::TailnetHandle(Arc<dyn TailnetFeed>)` as a root context
      (read in `view/window.rs` with `try_consume_context`), or call `Hub::set_tailnet`. It maps porter's
      `Machine` to `temor_ssh::TailnetHost`.
- [ ] ⌘T: a new tab or split of a host pane is a session on the same host (spec). With the bar, ⌘T opens the
      bar instead; decide whether the bar's "new tab" from a host pane means the host or a local shell.
      Today `Hub::ssh_prepare` makes every `NewTab { cwd: None }` from a host pane a host session.
- [ ] `Desk::open_host(&HostRow)` / `Hub::open_host_named(text)` are the entries for the bar's host rows and
      for a typed destination.
- [ ] "Edit locally" from a block: the entry is `Hub::edit_remote(Some(pane), "host:/path" | "path")`; the
      block bar has no such button yet (a path in a block's output needs picking out of text). The host
      banner has a "File on the computer" field + "Edit locally" button meanwhile.

## Known gaps
- [ ] The check URL is found by reading the pane's screen text (`Session::screen_text`) after each repaint,
      not the raw bytes (the pane engine does not hand raw bytes out). Stopped at the first folder report
      (OSC 7), the connection coming up, or the session ending; OSC 133 alone does not stop it. A raw tap in
      temor-pane (`PaneEvent::Raw` or an `EventSink` hook, before `Term::write`) would match the spec exactly.
- [ ] Established is detected by polling `ssh -O check` every ~900 ms while the first session connects (a
      probe thread per poll), not from the host key being verified in the pane. A master that dies later is
      only noticed when a joined `ssh` exits 255.
- [ ] Fingerprint of a plain alias: `ssh-keygen -F <hostname> -l` on `known_hosts` after the connection is up
      (`keygen_lookup_argv` + `fingerprint_from_keygen`). It is a lookup by name, not the key the handshake
      presented; with several key types recorded for a name the first line wins. An `ssh -v` parse of
      "Server host key: … SHA256:…" would be exact.
- [ ] A plain session (consent refused) cannot start in the pane's folder (no remote command to carry it).
- [ ] The restored tab of a host reconnects on launch and may ask for a password then.
- [ ] A reopened closed tab (`Start::Scrollback`) loses its host (the mapping is dropped with the pane).
- [ ] Offline tailnet rows are `Availability::Disabled` (quire dims them and takes no press); a machine that
      is offline in the tailnet but reachable another way cannot be opened from the list.
- [ ] The forward row's local port is any free port when the same number is taken; the person is not asked.
- [ ] `ForwardFault::LocalPortTaken` has no message path in practice (a free port is picked first).
- [ ] Local copies of edited files stay in `<runtime>/edit/<id>/` until the runtime dir is cleared; "Done"
      stops watching but does not delete them.
- [ ] Remote edit conflicts show Replace theirs / Use theirs / Decide later; no diff.
- [ ] HostRow/Edit/Forward banners are drawn above the status line of each host pane; they are not in the
      sidebar. No unread mark for a pending question in a background tab.
- [ ] `Hub::edit_remote` needs the host's connection up; there is no "connect, then edit" for `host:path`
      typed in the bar for a host with no open pane.
- [ ] OSC 52 / transfers / notifications consent stays in `remote::HostConsent` (FOLLOWUPS Batch 1 ssh).

## Quire asks
- [ ] A `Toolbar`/row layout component for the host banner's text field + buttons (uses `.temor-host-line`).

## Platform TODOs
- `platform/open_file.rs`: `UnsupportedFile` stub for macOS/Windows (`TODO(platform:macos)`,
  `TODO(platform:windows)`); Linux uses `xdg-open`.

## Tests not run (nothing compiled in this lane)
- hub/ssh_tests/* (stand-in `ssh`/`scp` in /tmp/tmr-<pid>-<name>, setsid needed), tests/it/ssh.rs (ds-harness).

### shelf

## Decisions made (confirm)
- A program's `ac=receive` (temor get) is asked about every time (banner "wants your newest download" /
  "the file X"), never remembered: a remote program could otherwise pull any Downloads file by printing
  one escape sequence. `Allow` for an offer to save is remembered per stream (host, or the pane's own
  output) until temor quits; `Don't allow` is remembered the same way (core `transfer_step` says
  Remember(Refused)). A refusal that is remembered has no undo short of restarting; consider remembering
  only Allowed, or listing remembered answers in settings.
- Files land in the XDG Downloads folder (`XDG_DOWNLOAD_DIR`, then `user-dirs.dirs`, then `~/Downloads`),
  not temor's data dir (PLAN D12 says the data dir by default; the lane brief said XDG download dir).
  The shelf index (`shelf.json`, through `ShelfStore`) stays in temor's data dir (`Ambient::shelf_dir`).
- Staging: each pane has `<shelf_dir>/incoming/pane-<id>/offer-<n>/`; the window moves the file with
  an exclusive create and removes the staged copy and its folder. Empty `pane-<id>` folders stay.

## Gaps
- "Save output" (block bar) still saves into `shelf_dir` through `ShelfStore::place` and does not make a
  row on the new shelf; route it through `Incoming{source: TemporSave}` and the same placement.
- `Origin::Remote` is never produced: every pane's files are `LocalStream`. Once ssh host sessions exist
  (`hub/transfers.rs::origin_of`, TODO(ssh)), the host name keys the remembered answer and the row says
  "from <host>". Remote files' tab id is not kept across a restart (the index stores no tab): "Show tab"
  on a restored row names tab 0 and the workspace says it is no longer open.
- The remembered answers (`ShelfHub::grants`) are not persisted; they could live beside the host consent
  of the ssh lane (`HostConsent`).
- Drop: `ReceiveSupport` is always `Unknown`, so a drop only pastes quoted paths. `Send` (a program that
  announced receive) is carried out as a notice only ("can't send files to this program yet"). `PaneSite`
  is always `Local` (`Session::set_site` is the hook for the ssh lane).
- OSC 1337 `inline=1` images are parsed and then dropped (not drawn); needs a kitty-graphics-like path in
  temor-render.
- Sending (`ac=receive`) streams 16 frames per I/O turn when the program's input has drained; files that
  cannot be opened cancel the transfer silently (the program sees `ac=cancel`). No progress is shown for
  files sent out.
- `Wanted` serves only regular shelf files (`kitty::serve`); folders are not offered.
- Shelf UI: a `Disclosure` in the sidebar foot with `Row`s; a long list grows the foot instead of scrolling
  (needs quire's scroll container for the foot, CONSUMING §11). Thumbnails and drag-out are not drawn.
  Reveal is `xdg-open` on the folder (TODO(platform:linux): FileManager1.ShowItems); macOS and Windows
  are stubs in `platform/files.rs`.
- `Harness::file_drag` is `pub(crate)` in ds-harness at 8ce6d4b, so the drop is tested through
  `Session::drop_files` (real bash), not by a harness drag. Ask quire to make it public.
- The banner is one question at a time (queue); there is no badge for questions waiting in other tabs.
- `temor save` over a non-temor terminal now fails after ~3 s with "nothing answered" (was 60 s); the
  next wait is 60 s so a person has time to answer. A terminal that never says STARTED but does ask
  (an older temor) would fail the same way.

### agents

Nothing was compiled or run (batch rule). The orchestrator's first build will find the typos.

## Not built (PLAN §8.2 / D19)
- Worktree name is not editable at creation: it is `<repo folder>-<n>`. The Space profile's setup command is not run; the Space is the constant `main` (`hub/agents/state.rs` `SPACE`) until Spaces reach the window.
- Closing an agent tab does not ask Keep / Merge / Discard (`LifeIn::CloseTab` exists in core). A closed tab's worktree just becomes a "Leftover branch".
- Merge back offers Squash only. Merge, Rebase onto base and Open pull request need the review view (P10) first. A leftover can be opened or discarded, not merged.
- A merge that stops on conflicts opens a plain shell tab in the checkout (not a split) and leaves the checkout mid-merge as git left it; the conflicted file list is in the notice only.
- After a successful merge the worktree is removed, but its tab's shell is still in the removed folder.
- Tier B / SSH worktrees; D20 mode line (Sandbox / Asks) needs docket readers; hook bridge, agent view.
- Restore is offered from the tab's focused pane's run only; marks are labelled "Before turn N".
- The tab's tooltip (state words, mode) is not set; the orb has no words of its own besides quire's aria label.

## Decisions I took, to check
- Checkpoints are taken when a pane known to run an agent (foreground process name, or the `app` it reported over 7501 or the socket, in `KNOWN_AGENTS`) goes Idle/Done/Failed -> Working. A build tool reporting 7501 (`app=cargo`) shows its status but gets no checkpoints. Every turn takes one ref; nothing prunes them (`refs/temor/checkpoints/*` grow). Consider skipping a snapshot whose tree equals the last one.
- The radar's focus = the pane that has the keyboard on the page (`EditFocus::In/Out` in `pane_view`), plus "no longer the keyboard pane" after a workspace change. A window that loses the OS focus is only unfocused if the page reports `Out`; verify in a real window. `pane_blurred` must not redraw (a redraw makes the pane take the keyboard back).
- Bell and notifications reach the radar only for panes with an agent process or any 7501/socket report (orchestrator decision). The `Alert`s the radar wants made are in `Hub::take_agent_alerts()` for the desktop lane (notification text = the agent's own note, raise through xdg-activation); nothing calls it yet.
- The waiting count: `hub::WaitingCount` (trait, on `Hub` and `HubRef`) and `Hub::waiting_count()`. Counts panes in `Waiting` only, not unseen Done/Failed.
- Foreground process is polled once a second by the pane's I/O thread (`temor-pane/src/engine/watch.rs`, `/proc` root fixed), and sent as `PaneEvent::Foreground(Option<String>)` on change. macOS/Windows: temor-pty returns no name, so no events (`TODO(platform:macos)`, `TODO(platform:windows)` live in temor-pty).
- `.worktreeinclude` copying lists every ignored file (`ls-files --others --ignored`), including `target/`; fine for small repos, slow for huge ones. Pass the include patterns as pathspecs or cap the list.
- Default `Primary+Shift+U` on platforms where Primary is Ctrl+Shift becomes Ctrl+Shift+Alt+U; plain Ctrl+Shift+U is IBus/GTK's Unicode input anyway.
- `PaneEvent` gained `Program`, `Foreground` and (unix) `Reported`; `ShellPaneInput::socket`; `Action::JumpToWaiting` (`temor.jump-to-waiting`).

## Verify first in the harness
- The tab menu (`AgentTabMenu`) is rendered inside the Row's content; if a pick bubbles to the Row and selects the tab, move it out next to the Row.
- `Menu::<AgentPick>` / `Sheet` / `Button { onclick: EventHandler::new(..) }` props were written from the quire sources at 8ce6d4b, not compiled.
- `ShortcutKey::Super + Shift + u` in `tests/it/agents_window.rs` reaches the keymap as `Super+Shift+U`.
- A socket path must stay under 104 bytes: harness scratch names are short for that (`ag-orb`, `ag-sock`).

## quire asks
- A multi-line body for `Alert` (the restore plan is a `Sheet` with one `Label` per file for now).
- A `Menu` anchored to a `Row` that does not bubble its picks to the row.

## Agent checkpoints move to docket (decided 2026-10-10)
- [ ] docket owns per-turn checkpoints: it uses temor's git-ref approach (private index, hidden ref), keeps the last 20 per run, drops them after 14 days, and skips agents that keep their own (Claude Code /rewind). When the interface is frozen (a list per run, plus restore with a dry-run plan), temor stops snapshotting (hub/agents/checkpoints.rs, temor-core checkpoint git) and shows docket's list and restore. Until then temor's refs stay, and temor adds no pruning.
      Update (2026-10-10): the interface is frozen (~/rs-wt/checkpoints/DESIGN.md §3, §5, §8, §9). Migration starts when the agent session sends the docket sha:
      - delete temor-core checkpoint/* and the hub's checkpoint jobs, and call docket's CheckpointList / CheckpointPlan / `checkpoints.restore`;
      - move undo_ref to refs/temor/undo/<n>;
      - at start, prune refs/temor/checkpoints/* older than 14 days, with no import;
      - add `_` arms for the non-exhaustive SessionEntry.

## Look batch: Warp-style cards (merged 2026-10-10)
Studied Warp's design (AGPL; no code copied). D5 stays: the shell keeps its own line editor, and
there is no input box of temor's own (Warp's needs its own vim emulation and completer, all AGPL).
- [ ] Hover goes stale after a wheel scroll or when the pointer leaves the pane. Clear it on
      pointer leave of `.temor-keys` and recompute on wheel.
- [ ] `card_bands` reads the cursor row from the previous paint, so the last block's extent lags
      one frame. Compute the bands after the capture.
- [ ] `TOOLS_HEIGHT_LOGICAL` (28.0) is an estimate; measure it with `use_rect`.
- [ ] No GPU pixel test for bands (needs an adapter); the CPU tests cover the shared scene path.
- [ ] Prompt: nushell is left alone (there's no reliable stock-prompt test). fish 3 and older were not run.
      In fish, Enter on an empty line before any command has no blank line above it. The branch is
      read from .git only, with no dirty marker (the context chips batch adds facts).
- [ ] zsh: without a terminal description that can move the cursor up (an unknown TERM), the
      typing line starts with one space, because zsh draws no prompt whose last line is empty.
- [ ] quire asks: a compact icon-button group surface (the toolbar uses `.temor-block-tools`
      CSS); a sticky overlay surface with an opaque material and a bottom hairline;
      `text-overflow: ellipsis` in Blitz (the header cuts with `clip_chars`).
- [ ] Warpify-style offer: for a typed `ssh`, `docker exec` or `sudo -i` inside a tab, offer
      once, from a fixed command list, to load temor's shell integration in the inner shell (so
      its commands become cards). Later, after the context chips batch.
- [ ] Next, queued (owner OK): the context chips batch
      (/mnt/data/cargo/temor-lanes/QUEUE-facts.md).

## Batch 3 (merged 2026-10-10: P16 context chips, P17 shell offer, P18 finish Batch 2, P14 wire revision and Tier B in the window)
Written by Sonnet lanes without builds. The orchestrator merged, compiled, fixed and gated them
together.

### Orchestrator decisions
- The virtualenv chip comes from a `.venv` folder only. A venv activated in the shell needs the
  shell to report `VIRTUAL_ENV` (an integration mark); until then it doesn't show.
- Chips sit on the live prompt's header line (the line above the typing line, or the prompt's own
  line for a one-line prompt), including before the first command.
- `org.quire.Temor1` methods take an activation token (`NewWindow(s)`, `NewTab(ss)`). Nothing
  outside temor calls them.
- Remote calls send the default cell size (8×16) and no palette for now.
- The offer bar stays a bar under the pane (it resizes the pane), like the host banners. An
  overlay waits for a quire sticky surface.

### Next (most visible first)
- [x] (Batch 4) Tier B sessions get no shell integration, so no blocks, cards, chips or folder reports.
      `OpenSession` needs an argument list, or the daemon applies `plan_integration`.
- [x] (Batch 4) Check the quit path: if quitting runs `end_pane` for every pane, it ends host sessions
      instead of detaching them, which defeats persistence.
- [x] (Batch 4) Wire the real values into Tier B:
      - the pane's cell size on open and resize;
      - the window's palette after attach and on theme change;
      - `Client::ping` on an abandonable thread, plus `LinkEvent::PingTimedOut` and
        `LinkEffect::DropLink` (kill the stalled ssh first);
      - the `TODO(batch3-wire)` slots in `edge/remote_*.rs`.
- [x] (Batch 4) Session ids aren't saved, so a restart opens new sessions. Adopt them through
      `list_sessions` on restore.
- [x] (Batch 5) A reconnect after the shared connection died can't ask for a password (it uses
      BatchMode). Retry should repeat the in-pane connect step.
- [ ] Remote edit and forwards still use scp and `ssh -O`; move them to the helper's file service.
      Input typed while the link is down is dropped.
- [x] (Batch 5) Helper consent is in its own `remote.json`; fold it into the consent book.
- [ ] The helper binary comes from `TEMOR_REMOTE_HELPER` or the folder beside temor, for the same
      CPU kind only. Packaging a helper per architecture is open.
- [ ] Offer:
      - fish and nushell inner shells: Yes does nothing yet (`temor.fish` can't be sourced in
        place);
      - shells with no line editor cut the roughly 14 KB line at 4096 bytes, so shrink the payload
        or split it;
      - history hiding needs `ignorespace` / `HIST_IGNORE_SPACE`.
- [ ] Facts:
      - host panes get no chips;
      - `.node-version` and `.tool-versions` aren't read;
      - untracked files aren't counted;
      - a repo with no commit shows no branch;
      - a long typed command can run under the chips.
- [ ] Desk:
      - the ⌘T bar re-reads ~/.ssh/config on every keystroke (cache it);
      - the X11 token is `DESKTOP_STARTUP_ID` and isn't read;
      - check that the radar treats an agent-opened pane as unfocused.
- [ ] Wire:
      - staged `.temor-part-*` files aren't swept;
      - `Error::TooLarge` is unused;
      - there's no test that reads the pixel size back, and none for a slow file request.
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b3-*.md`.

## Batch 14 (merged 2026-10-10: pins and P19 checkpoints on docket, first part)
Pins: quire v0.3.9 (7ce3a85), docket bd83d5e, porter 9c01699 (Machine.link / Machine.ssh, ported
in temor-desktop), and almanac 0fdbcfd and stoker 4a0bd9e through the lock. One copy each of quire,
porter-core and docket.

- Done:
  - temor's own snapshotting is gone (temor-core `checkpoint/`, the hub's jobs);
  - merge-undo uses `refs/temor/undo/<n>` (`worktree/undo.rs`);
  - `refs/temor/checkpoints/*` older than 14 days are swept at start, with no import;
  - List, Plan and Perform `checkpoints.restore` sit behind temor-desktop's `CheckpointPort`;
  - Restore is held back while the pane's agent is Working (a stopgap).
- [x] Batch 15 (docket DESIGN §11 and §12, with the shas coming through sill). Pane agents get
      docket watches, not full sessions:
      - `CheckpointWatch { workspace, label, rewind }` when an agent starts in a pane
        (`rewind = Agent` for Claude Code, so the list says "not saved: this agent keeps its
        own");
      - `CheckpointMark { session }` on Idle→Working;
      - `SessionTurnEnded { session, turn, how }` on Working→Idle;
      - `Session.Close` when the agent exits.

      Never Session.Open or Session.Turn for pane agents. Call `Hub::set_agent_session` with the
      watch's id.
- [x] Batch 15: grey Restore out from `CheckpointList.turn: TurnState`, and drop the pane-local
      Working stopgap. A CheckpointList literal needs `turn`.
- [x] Batch 15: temor shows the plan only, with no confirm of its own. sill's hold-to-confirm
      sheet is the one confirmation.
- [ ] Outside temor: `org.quire.Temor` must be listed under `cli` in docket's intentd.toml (for the
      docket side to add). Perform uses `Origin::InWindowField` for now; check it against the cli role.
- [ ] quire's context menu drops disabled items and has no per-item tooltip, so "Wait for the
      agent to finish" is an info row (quire ask).
- Lane notes: `/mnt/data/cargo/temor-lanes/followups-b14-ckpt.md`.

## Batch 4 (merged 2026-10-10: Tier B finishing)
- Helper sessions start as a login shell with temor's integration (`temor-remote/src/daemon/launch.rs`).
  `TEMOR_SHELL_INTEGRATION=0` in the open request's env turns it off, so no wire field was needed.
- Quitting lets go of the links without ending sessions. Each pane's helper session id is saved
  in the hosts file (`PaneHost.session`) and adopted on restore when the host still has it;
  otherwise a fresh session opens.
- The pane's cell size goes out on open and resize. The window's palette goes out on open, after
  every attach and on theme change. A ping per link drops a stalled one and redials it at once;
  `ServerAlive*` stays as a second check.
- [x] (Batch 5) `edge/remote.rs` (328 lines) and `hub/remote_line.rs` (322) are over 300 lines; split them.
- Lane notes: `/mnt/data/cargo/temor-lanes/followups-b4-*.md`.

## Batch 15 (merged 2026-10-10: pins and P19 part 2, checkpoint watches)
Pins: quire v0.3.12 (faecc7a), docket 864c03d, porter a83b575, and almanac 20bf1a4 through the lock.

- Pane agents get docket watches:
  - Watch on first sight (rewind = Agent for agents that keep their own history, such as
    `claude`; a table in temor-core);
  - Mark on Idle→Working;
  - SessionTurnEnded on Working→Idle (Answered or Failed);
  - Close when the agent exits or the pane ends.
- Calls run off the UI thread, one at a time per pane. Failures are logged once per pane.
- Restore greys out from the list's `turn`; the pane-local stopgap is gone.
- temor's sheet is a preview ("What restoring changes", with Close and "Restore…"). sill's
  hold-to-confirm sheet is the one confirmation.
- Orchestrator decision: an exited agent's Restore list stays until the pane ends, since that's
  when undoing is wanted.
- [x] (Batch 5) Agent exit is detected only for agents first seen as the foreground process; report-only
      agents close their watch when the pane ends.
- [x] (landed with anyview 397012b) P10 viewers are written and pass their tests on branch `b15/view` (anyview-pane 3d52a9b,
      zeroize 1.9.0 for anyview's pdf crate). They wait for anyview's Batch 15 sha, which uses
      quire v0.3.12; with today's sha the lock would hold two quires. Peek isn't built (it needs
      `anyview-fs` and anyview-peek's `pane` feature).
- Lane notes: `/mnt/data/cargo/temor-lanes/followups-b15-*.md`.

## P10 Viewers (merged 2026-10-10, anyview 397012b)
- A viewer pane in a split, opened by ⌘-clicking a file path in the output or with the toolbar's
  View button. A tab holds one viewer, and the next file replaces it.
- The viewer's commands join the ⌘T bar while it has the keyboard. A listing's files form a
  Sequence (`SequenceOrigin::Output`), so the arrow keys step through them.
- Esc returns the keyboard to the terminal and ⌘W closes the viewer. Viewer tiles are saved and
  reopened if the file still exists.
- Orchestrator fix: the viewer opens on the press's release. Opening mid-press split the pane
  and dropped the press's pointer capture.
- [x] (Batch 5) Peek (⌘-hover preview): needs `anyview-fs` and anyview-peek's `pane` feature.
- [ ] Recordings in the pane: anyview-pane's `player` feature with `PaneEdge::with_player`
      (off for now).
- [x] (anyview 121db23) When anyview #94 lands, take `PaneEdge::with_store` (views remembered through the store;
      Remember is in memory now), and the single Unfocus on blur.
- [ ] No D-Bus Open fallback. OpenElsewhere uses the FileShell opener. Files on another
      computer (Tier A/B panes) can't be viewed yet.
- [x] (Batch 5) A Spaces-file recall of only a Place brings viewer panes back as shells.
- Lane notes: `/mnt/data/cargo/temor-lanes/followups-b15-view.md`.
- Viewer store: `PaneEdge::with_store($XDG_DATA_HOME/anyview)` (else `~/.local/share/anyview`),
  shared with anyview's own windows. It's used only by runs that keep state.
  Uses `anyview_pane::STORE_FOLDER` (anyview e74e5de).

## Batch 5 (merged 2026-10-11: open follow-ups)
Six Sonnet lanes from main 16b28dc (peek, remote, edit, offer, facts, desk); merged, fixed and gated
together.

- Peek: holding ⌘ over a file path shows anyview's preview card beside the pointer (local panes).
  It never takes the keyboard or the pointer capture; a press hides it before the click opens the
  viewer. anyview-fs and anyview-peek's `pane` feature are in the lock.
- Remote: Retry repeats the in-pane connect step (password and key prompts show in the pane).
  Typing while the link is down is held (64 KiB, in order) and sent after re-attach; past that
  the banner says part was dropped. The helper answer is in the consent book; `remote.json` is
  read once per start for carry-over and never written.
- Edit: a Tier B pane's remote edit reads and writes through the helper with
  `WriteCheck::Unchanged`, so a file changed on the host is a conflict question. Tier A keeps scp.
  The helper sweeps `.temor-part-*` older than a day at start. `Error::TooLarge` is gone.
- Offer: Yes works in fish (in place, as a function) and nushell (the marks only). The load text
  is lines under 4096 bytes. bash and fish delete the lines from history; zsh keeps one short
  first line in history (the rest is ignored through HIST_IGNORE_SPACE, which is put back).
- Facts: `.node-version` and `.tool-versions` (node, python, ruby, go), the untracked count
  ("N new"), the branch in a repo with no commit, and chips that step aside for a long command on
  their own row.
- Desk: the ⌘T bar caches ~/.ssh/config by mtime; `DESKTOP_STARTUP_ID` is the X11 launch token;
  report-only agents end on an OSC 7501 `clear` or when a shell comes to the front; a place-only
  Spaces recall brings viewer panes back as viewers.

### Orchestrator decisions
- Automatic redials stay in BatchMode; only Retry runs the in-pane connect step, so a password
  prompt never appears in a pane unasked.
- `remote.json` is left in place (never written again); temor does not delete it.
- A shell in front ends a report-only agent. An agent's own tool commands stay under the agent's
  foreground group, so this should not flap; revisit if it does.
- No "ended" message on the agent socket for now.
- Tier B edits read whole files in memory for now; streaming large files waits.
- Merge fix: the zsh prelude sets its flag with `typeset -g`, which fish parses (a bare `x=1`
  line was a fish parse error, printed to the person).
- Merge fix: the chips overlap scenario was dropped. The harness never paints, so the cursor the
  rule reads (`Session::cursor_logical`, from the painter's frame) is unknown there. The rule is
  tested in temor-core.

### Next
- [ ] Host panes: no peek, no chips (both need the helper to read or report from the host).
- [ ] Offer: nushell history isn't cleaned and the zsh prelude prints a parse error there; dash
      prints "source: not found" once; nushell has no pty scenario; fish history deletion is only
      checked with piped input.
- [ ] Remote: no ds-harness scenario for Retry (needs a fake ssh that asks for a password);
      `edge/remote_install.rs` is 307 lines; a half-received send can repeat after a reconnect.
- [ ] Edit: stream large files; conflict wording could name the "same time, other contents" case.
- [ ] Facts: `.python-version` and `pyproject.toml` aren't read; a huge untracked tree is slow.
- [ ] Peek: OSC 8 `file://` links show no card; the card's size is estimated; `view/pane_view.rs`
      is over 300 lines (split the pointer and key wiring).
- [ ] Desk: an `Include`d ssh config edited in place isn't noticed; quire ask: confirm
      `focus_with_token` acts on an X11 startup id; a place-only recall starts shells in the
      Space's folder, not each pane's.
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b5-*.md`.

## Batch 16 pins (2026-10-11)
quire v0.3.14 (c404fb0), docket 312cd27, porter 83ac0fa, anyview 6c325c9, and almanac ac4bc03,
stoker 4a0bd9e, blitz-kit 25b5099 and chordkit ba094c6 through the lock. One copy each; no code
changes.

- [ ] anyview #100: find in a viewer pane. `PaneHandle::find(query)` / `end_find()` and
      `hits(HitList::Brief | Whole)` (an "In This File" palette group whose rows run through
      `run`). anyview suggests ⌘T feeds its query to the viewer in a "find in pane" mode; ⌘F stays
      temor's.
