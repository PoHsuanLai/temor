# Follow-ups

Review leftovers that didn't block a merge. Each one names the lane it came from and the PLAN section.

## temor-core (lane/core, reviewed at 03031e9)
- [x] `TrialId` is only as unique as `TurnRef`: every Ask must get a fresh `TurnRef` (document it, or
      add a counter). §8.3 (Batch 26: an ask counter, `AskNo`)
- [x] `Origin::LocalStream` emits `Remember(Grant)`, but no consent store exists for local panes
      (`HostConsent` is per host). Add one at the edge, or always ask. §9
      (done: Batch 2 shelf, `ShelfHub` grants per stream in `hub/shelf.rs`, kept until quit)
- [x] A typed proposal deleted by hand and replaced by the person's own command still logs
      `Ran{Edited}`. Tell "edited" apart from "replaced". §8.3 (Batch 26: `AsRun::Replaced`, rule in proposal/as_run.rs)
- [x] U+2800 (braille blank) looks like a space but isn't default-ignorable. Consider treating it as
      a space. §8.3 (Batch 26)

## temor-vt (lane/vt, reviewed at 8d586df)
- [x] iTerm2 `MultipartFile=` / `FilePart=` / `FileEnd` (imgcat inside tmux): temor-pane reads the
      pieces when they arrive as an unknown OSC (`engine/xfer/state.rs`), but temor-vt's pre-scan
      catches only `File=`, and its notes say Ghostty drops the rest. §9 (check) (Batch 26: the pre-scan takes all four)
- [x] `RENDER_HOLD`: capture a frame when a hold starts, as Ghostty recommends; the hold timeout is the
      UI's job. §4b (done: the hold hook)
- [x] OSC 52 clipboard reads (`CLIPBOARD_READ`) are not wired. §6, §9 (done: ffi/clipboard_read.rs)
- [ ] No vttest captures in the conformance fixtures yet; DA2/DA3 use libghostty's defaults. §13
- [ ] Upstream (only with the owner's OK): Ghostty reporting OSC 1337 `File=` through
      UNKNOWN_SEQUENCE; a per-cell hyperlink id; libghostty-rs taking the `Send` and new-callback
      wrappers. D1

## temor-pty (lane/pty, reviewed at 0f49c7b)
- [x] The app owns reaping: poll `try_exit` after `hang_up()` and escalate with `kill()` after a grace
      period, or children stay zombies. §4b (done: temor-pane `engine/reap.rs`, R1b)
- [x] nushell (the owner's shell) and other shells get no integration. nushell can emit the marks
      itself (`shell_integration` config); turn that on. §6 (done: nushell through a vendor
      autoload file, bb36e33)
- [x] fish 3 marks path untested (fish 4+ emits marks natively); bash 3.2 has no PS0, so no C/D marks.
      (done 2026-10-15: fish 3.7 tested; bash before 4.4 marks through a DEBUG trap)
- [x] A person's own `trap … EXIT` replaces the emulated-login `.bash_logout` trap; an old-style
      `zle -N zle-line-init` drops zsh's first-prompt B fallback under themes. (Batch 26: `_temor_chain_exit`; zsh already re-hooks zle-line-init)
- [x] Foreground name/cwd come from the group leader, so a pipeline reports its first command. §7 (done: `pipeline_last`, by start time since Batch 26)
- [x] Send `integration_scripts()` with the SSH Tier A bootstrap. §9b (done: Batch 2 ssh,
      `hub/ssh_connect.rs`)

## temor-wire (lane/wire, reviewed at 5e2ffd8)
- [x] Callers (temor-ssh, temor-remote) must check the first frame is `Hello`, answer
      `UnknownRequest`/`UnreadableRequest`, surface `UnreadableEvent`, and add request timeouts. §9b, §4b
      (done: Batch 1 remote, `client/connect.rs`, `client/calls.rs`, `daemon/serve.rs`; temor-ssh
      never speaks the wire)
- [ ] `Unknown` catch-alls on wire enums and the `u32::MAX` no-payload marker are additions to §3;
      a request carrying a nested newer enum is answered `Unreadable` (safe, not precise).
- [x] Tune snapshot and file-size limits and the retry policy (1 s base, 60 s cap, 30 tries) against
      real hosts. §9b, §14 (2026-10-16, over a simulated line of 150 ms and 1 MB/s: file pieces
      are 256 KiB, two in flight, so a keystroke waits about 0.5 s, not 3.8 s; the retry and ping
      policies hold at that latency; `temor-remote/tests/it/slow_link.rs`)

## temor-transfer (lane/transfer, reviewed at 39cc6df)
- [x] Edge contract for the app: feed back every write failure (`WriteFailed{op}`) and every
      successful Finish (`WriteDone`); hold the `Root` for the transfer's lifetime. §9
      (done: Batch 2, temor-pane `engine/xfer/receive.rs`)
- [x] Upload by drop (`SendSession`) is not built. §9 (done: kitty/drive/send.rs)
- [x] Kitty `prm` is ignored (files 0o644, folders 0o755); in-root symlinks are refused, not made. (done: prm applied; in-root symlinks made under `LinkPolicy::Make`)
- [x] Verify the kitty `st` reply encoding and `tt` values against a real kitty. §9 (unpadded; fixed)
- [x] A malformed frame with no open files only replies; the transfer continues. (done: it ends the transfer)

## temor-render (lane/render, reviewed at f4ead2c)
- [x] The app must call `Palette::apply` on every new terminal before its first frame and on theme
      change; grid colours from quire's terminal tokens once v0.2.31 is tagged. §5
      (done: P3a 91eaf50 and 5bccf7d; temor-pane `engine/term.rs`)
- [ ] No device-level tests for atlas retries or the oversize-image skip (the GPU path itself is
      wired through `use_gpu` in `view/pane_view.rs`). §5
- [ ] A capture error after a resize leaves the new size applied; harmless while libghostty reports
      Full after resize. §5
- [x] Kitty unicode-placeholder (virtual) placements are not drawn; selection changes recompose the
      whole pane; CPU clips glyph overhang to the row, GPU does not. §5 (done: Batch 23)

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
- [ ] The window and pane columns are still flex divs (`.temor-window`, `.temor-pane` in
      `view/window_style.rs`). quire has `Column` now (the sidebar uses it); move them to it. §5
      (the root extent, palette tokens and theme change are done: 91eaf50, 5bccf7d)
- [x] quire asks (whopper offline; saved in /mnt/data/cargo/temor-lanes/pending-quire-asks.md): IME
      cursor area API, runtime window title, theme-change signal, consumer coherence test. Now in
      quire v0.3.1 and used (5f852bf, fefac1e, 5bccf7d, e318913). §6
- [x] OSC 52 writes are received and dropped (`TODO(P9)` in `session/lifecycle.rs`), and the
      hyperlink hover target preview is not built. Bell, title, notifications, cwd and progress
      are shown (P2, P3). §6, §7 (done: session/osc52.rs; link hover tip in view/link_tip.rs)
- [ ] Settings are temor's own typed file at $XDG_CONFIG_HOME/temor/terminal.conf; move to
      ds-settings when quire provides it. §6 (check: ds-settings is already used for the theme in
      `view/theme.rs`; is it ready for temor's own keys?)
- [ ] Per-cell text reads cost ~3 FFI calls per cell: measure search on a large scrollback. §5
- [ ] Output read just before a resize may be parsed at the new size (the usual SIGWINCH window).
- [ ] Without XDG_RUNTIME_DIR the runtime folder is ~/.cache/temor/run (0700, owner checked).

## temor-ssh (lane/ssh, reviewed at f3116b1 plus orchestrator fixes)
- [x] P13 wiring: host list in ⌘T (plus tailnet machines, D22), opening panes through the shared
      master, consent per host, `HostId::same_machine` for the gate. §9b (done: Batch 2 ssh, and
      the bar's host rows in P18, 79ebb19)
- [x] HostAlias rejects `:`, `[`, `]`: IPv6 literals need a HostName in ssh_config; say so in the UI. (done: the HostName hint, also for `host:path` since Batch 26)
- [x] Nushell on the host: the bootstrap places `temor.nu` under `nu/nushell/vendor/autoload`; P13 adds the `nu` case that starts nushell with `XDG_DATA_DIRS` pointing there (same as temor-pty's local plan). §9b
      (done: the `nu|nushell` case in temor-ssh `sh/bootstrap.sh`, tested in `tests/it/nu.rs`)
- [ ] IdentityFile and ProxyJump values must be validated before any `-i`/`-J` builder uses them.
- [ ] fish 4.9 prints no marks on piped stdin, so fish marks over SSH aren't asserted; no real sshd
      test yet (only `ssh -G`); csh/tcsh login shells untested. §13
- [x] A bootstrap dir owned by us with mode 0300/0000 fails closed (EACCES) instead of tightening. (done in temor-ssh; temor-remote's socket folder too since Batch 26)

## Platform (owner): OS-specific stubs to implement on real machines
Each one sits behind a `platform/` edge trait, marked `// TODO(platform:<os>)`.
- [x] macOS keymap: the primary modifier is Cmd (bind Cmd chords, not keycap's Ctrl+Shift);
      temor-core terminal/chord.rs and keymap.rs defaults. §6 (done: keys on chordkit, Primary per
      platform; `macos_uses_command_only` in temor-ui input tests)
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
- [x] Nushell marks are tested through the env only; a real nu block test once nu is on the gate PATH.
      (done: the block tests run nu where it is installed, temor-ui `tests/it/blocks/support.rs`)
- [x] view/window.rs is near 300 lines; P3's split tree replaces most of it. (done: Batch 1, the
      tree is `view/tree_view.rs`)

## R1a (lane/r1a)
- [x] temor-vt-sys: generated bindings carry indented C doc comments that rustdoc runs as ~100 failing
      doctests. Have `scripts/gen-bindings.sh` fence them as `text` (or set `doctest = false` on the
      -sys lib), then add a crate-root example. (done: `doctest = false`; PLAN R1 exempts the
      bindings from the example, d5621cd)
- [x] temor-vt `row_wraps` still returns `bool` (temor-vt can't see temor-core's `Wrap`); give temor-vt
      its own two-variant enum when R1b touches rows. (done: `Wrap`)

## R1b (lane/r1b)
- [ ] Blocks stay in temor-core (L2) because they use `vocab` ids and quire's `Stamp`. An embedder of
      temor-pane gets raw OSC 133 events (`MarkBegin`/`Prompt`/`Anchor`) but no block model. Move the
      block machine into temor-pane with its own id/time types once an embedder (anyview, sill) needs it.

## P3a (lane/p3a: keys, layout and tokens, theme, title, IME, coherence)
- [x] quire asks: (1) a column or stack layout component (v0.3.1 has only SplitView); (2)
      `focus_landing_told` is `pub(crate)` at v0.3.1 (the v0.3.1 notes say public); (3) EditSurface's
      Busy retry (`write_soon`) is private, so `view/ime.rs` keeps its own 16-try loop; (4)
      `RawKeySurface` takes no style, pointer or wheel handlers and sizes nothing: the grid needs an
      inner element and a sheet rule for the surface's class (`view/window.rs` `WINDOW_CSS`); (5)
      `.ds-texture-layer` has no rule in quire's sheet (the coherence test allows that one class). §6
      (done in quire by v0.3.6; temor's side is the "Adopt" item under quire v0.3.4, v0.3.6)
- [x] A keymap setting replaces both command forms of its action, which gives the chord back to the
      program; a bare Ctrl chord is refused with the ⌘ forms named (review fixes). §6
- [x] Unbound Super+letter reaches the program as text (as before P3a). Ghostty drops unbound ⌘ keys;
      decide the same here. §6 (done: dropped, f689008)
- [x] KWin takes Meta+T/W/D/V globally on KDE, so those Super forms never reach temor there; the
      desktop's reserved chords are with whopper. §6
      (2026-10-14, owner's choice: on Plasma temor gets keycap's `terminal` profile, Cmd+letter as
      Ctrl+Shift+letter, which chordkit's KDE keymap reads; Cmd+D and Cmd+Shift+D arrive as one
      chord there. Before, its class matched no list and it got `default`, so Cmd+T reached the
      shell as Ctrl+T. Installed in ~/.local/bin/keycap-focus; keycap branch
      `temor-plasma-terminal` (b6d5594) for its repo. The installed keycap binary (Oct 3)
      predates the `keymap-aware` profile; reinstalling keycap is the owner's.)
- [x] The cursor never blinked: the renderer ignored libghostty's blink flag and nothing timed
      it. Done: the terminal's default cursor blinks (`DEFAULT_CURSOR_BLINK`, applied with a
      cursor-style reset at start, which also applies `cursor.style` to the first prompt),
      programs can ask for a steady one, the focused pane's cursor is left out in each off half
      (600 ms) and shows solid from each key, paste or focus; `cursor.blink = off` turns it off.
- [x] IME candidate placement is untested: ds-harness has no IME host, and the owner's fcitx5 is the only
      real check. §6 (done 2026-10-16 on fcitx5: quire 3cfe32c4 carries the blitz-shell fix)
- [x] The window title is untested in the harness (no window host there); only the rule is tested. §7 (done: tests/it/window_title.rs)
- [x] Per-workspace Space looks: the window draws `SpaceLook::default()` until sill publishes them. §5
      (done: Batch 2 spaces, the root takes `spaces.look()` in `view/window.rs`)
- [x] The palette follows the Space look (read inside `Ds`, `view/palette.rs`) but not
      `use_space_frame()` (the sidebar's ink; P3b). The look itself is `SpaceLook::default()` until
      sill publishes per-workspace looks, so no change reaches it yet. §5 (done: Batch 1 and 2,
      the sidebar reads `use_space_frame()` in `view/tab_sidebar.rs`, the look comes per Space)
- [ ] `real_shells_*` and `a_five_mebibyte_paste_arrives_intact` time out under a parallel
      `cargo test -p temor-ui` (fish startup, a 15 s wait); they pass alone. §13

## P3b (lane/p3b: tabs, splits, session restore)
- [x] Window integration is not done: the window still shows one session. The workspace model
      (temor-core `workspace`) is not driven by the window yet, and the sidebar (quire `Sidebar`
      with tab rows), the split miniature, the `SplitView` tree, drag to split and tear, and the
      `use_space_frame()` sidebar ink are not built. §7 (done: Batch 1 window)
- [x] The session file (`temor-ui::edge::session_file`) is written and read, but the window does
      not load or save it yet, so nothing is restored on launch. Saves are paced (once a second,
      plus one on quit). §7 (done: Batch 1 window, restore on launch)
- [x] Restoring scrollback needs a temor-pane entry point that starts a pane from a
      `temor_vt::Snapshot`. temor-vt's `Terminal::restore` exists and nothing calls it. Until then
      no snapshot sidecars are written. When added: the state dir is 0700, files 0600, each
      snapshot is capped at 4 MiB, a pane on the alternate screen is never snapshotted, and a
      tab's sidecars are deleted when it is forgotten. §7 (done: e5fc6ca and Batch 1
      `edge/sidecar.rs`; the 4 MiB cap caveat is under Batch 1 window)
- [x] ⌘T opens a plain new tab for now (the ⌘T bar is P5). §6 (obsolete: ⌘T is the bar and ⌘N
      the plain tab since Batch 2)
- [x] The pane ignores the workspace actions (`Session::run` no-op) until the window routes them
      to the workspace step. §6 (done: Batch 1, `Session::take_asked` hands them to the hub)
- [x] quire ask: `SplitView` reports no finished drag, so a dragged divider is not written back
      to the tree (`view/tree_view.rs`; the starting share is used, and the unread dot is quire's
      `Badge`). (check: v0.3.14's `SplitView` has `on_shown`; is that the report?)
      (done: quire v0.3.20's `SplitView` has `on_resized`; the share goes to the tree as
      `WorkspaceIn::SetShare`, the split found by the panes on either side of its divider;
      `resizing::a_dragged_divider_keeps_its_place_through_a_tab_switch`)
- [x] ⌃⌘= is bound to Ctrl+Shift+= and Ctrl+Super+=. The Ctrl+Super form is not a ⌘ chord under
      the terminal rule (it has Ctrl), so the owner should confirm this binding. §6 (obsolete: keys
      moved to chordkit; equalize is `Primary+Ctrl+=`, else ⌥⌘0 where Primary is Ctrl+Shift)

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
- [x] Space switch: no default chord until the desktop rule settles (quire leaves Ctrl+digit to the desktop; the shell owns Super+digit); asked the agent session.
      (done: settled as `SwitchChord::Unbound`, `view/space_ui.rs`; see Keys on chordkit)
- [x] Unbound Ctrl+Shift chords are dropped. Once a program turns on kitty keyboard flags they are distinct keys; forward them then instead of dropping.
      (obsolete: the same item lives under Keys on chordkit, "Forward Primary-form chords")
- [x] `temor save` STARTED-before-consent (transfers decision) not yet done. (done: 8e55667)
- [x] The Restart panic (Blitz node.rs:1573) was not reproduced; the regression test runs it in the harness. If it still happens on the desktop, take it to quire/Blitz with a backtrace (RUST_BACKTRACE=1).
      (done: quire v0.3.4's hover-card guard fixed it; the restart test runs again)

## quire v0.3.4, v0.3.6
- [x] Bumped to v0.3.4 (e0d1ed48; v0.3.3 lacked `ds-blitz/src/layout_chain.rs`). The hover-card guard fixes the Restart panic; `restarting_an_ended_shell_gives_a_new_prompt_and_keeps_the_window_alive` runs again. quire now styles `.ds-texture-layer`, so the coherence exemption is gone.
- [ ] Still to adopt from quire: `Column` for the window and pane columns (the sidebar has it);
      `ds::focus::soon::focus_landing_told`; `write_soon` (`view/ime.rs` keeps its own loop);
      RawKeySurface `style` and pointer/wheel props (the grid still uses an inner div); ds-harness
      `WindowHosting::Recorded` (`window_titles()`, `window_icon()`); `WindowIcon` and
      `with_icon` (X11 and Windows; Wayland takes the desktop entry's icon).
      `SwitchChord::Unbound` is in (`view/space_ui.rs`).

## Keys on chordkit (lane/keys, chordkit d6fed65)
- [x] chordkit 338b717 (ba094c6 fixes the fallback where the default is plain Ctrl in a terminal): temor forgoes "show fonts" and "new", so ⌘T is new tab (⌘N free); equalize has the fallback ⌥⌘0 (Ctrl+Shift+Alt+0 where Primary is Ctrl+Shift); a person's chord revives an action refused at start (focus left/right on "Other" Linux, where Ctrl+Alt arrows switch Spaces).
- [ ] chordkit FINDINGS: a fallback applies only where the default has no live form, not where it collides; focus left/right stay refused by default on "Other" Linux.
- [x] keycap 0d9abbf adds the `keymap-aware` profile (⌘ arrives as Super); temor's desktop entry asks for it with `X-Keycap-Profile=keymap-aware`. A still-remapped window falls back to chordkit's Ctrl+Shift alias, where ⌘⇧D reads as ⌘D.
- [x] Forward Primary-form chords to the program under the kitty keyboard flags (route.rs) instead of the drop. (done: input/route.rs)
- [x] Desktop shortcut sources: keycap on our desktop, `chordkit-kde`, `chordkit-gnome` (edge/keymap.rs uses `ConventionSource` only), plus `KeymapSource::watch` to reload and re-register. (done: edge/keymap_source.rs)
- [x] quire v0.3.6 (8ce6d4b, chordkit ba094c6, maps ALT_GRAPH): one chordkit in the tree. temor still routes its own window keys; moving to quire's `ds::keys` resolver is a later step.
- [x] Wire `keys::{shortcut, terminal_shortcut}` into menus when they show shortcuts. (Batch 26: the sidebar and more menus; quire draws none in context menus)
- [ ] The old `keys.<action> = ctrl+shift+c` lines now report "no setting called"; say so in the user docs when settings are documented.
- [x] Space switch: `SelectSpace(n)` is chordkit's switch-to-space standard action; no platform binds
      it live in a terminal, so it is reached from the Spaces menu only. The bar has no Space rows
      yet: nothing sets `BarSeams::spaces`, so it stays `NoSpaces` (Batch 2 bar). (done: `KitSpaces` rows in the bar)
- [x] temor-ssh `files::an_awkward_path_is_fetched_edited_and_written_back` flaked once with ETXTBSY (a fake `ssh` script written while another test thread forked). Retry the spawn on ExecutableFileBusy in the test helper, or write the fakes before the threads start.
      (done: d5a9d6f, the spawn retries on ExecutableFileBusy in `tests/it/harness.rs`)

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

- Credentials (item 5): done since (Secret Service; see the Settings section). temor-desktop exposes nothing over porter for secrets, and the
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
- [ ] `view::host_list::HostList { desk }` is still mounted nowhere (a sidebar section, if wanted).
      The ⌘T bar lists the hub's host rows through `hub::HostRows` (P18, 79ebb19).
- [x] The desktop lane provides the tailnet: `hub::TailnetHandle(Arc<dyn TailnetFeed>)` as a root context
      (read in `view/window.rs` with `try_consume_context`), or call `Hub::set_tailnet`. It maps porter's
      `Machine` to `temor_ssh::TailnetHost`. (done: `view/window.rs` calls `Hub::set_tailnet`)
- [ ] ⌘T: a new tab or split of a host pane is a session on the same host (spec). With the bar, ⌘T opens the
      bar instead; decide whether the bar's "new tab" from a host pane means the host or a local shell.
      Today `Hub::ssh_prepare` makes every `NewTab { cwd: None }` from a host pane a host session.
- [x] `Desk::open_host(&HostRow)` / `Hub::open_host_named(text)` are the entries for the bar's host rows and
      for a typed destination. (done: `hub/bar_run.rs` uses both)
- [x] "Edit locally" from a block: the entry is `Hub::edit_remote(Some(pane), "host:/path" | "path")`; the
      block bar has no such button yet (a path in a block's output needs picking out of text). The host
      banner has a "File on the computer" field + "Edit locally" button meanwhile. (Batch 26: Edit Locally in a host pane's block menu)

## Known gaps
- [x] The check URL is found by reading the pane's screen text (`Session::screen_text`) after each repaint,
      not the raw bytes (the pane engine does not hand raw bytes out). Stopped at the first folder report
      (OSC 7), the connection coming up, or the session ending; OSC 133 alone does not stop it. A raw tap in
      temor-pane (`PaneEvent::Raw` or an `EventSink` hook, before `Term::write`) would match the spec exactly. (done: `ByteTap` in temor-pane, `RawCheck`)
- [ ] Established is detected by polling `ssh -O check` every ~900 ms while the first session connects (a
      probe thread per poll), not from the host key being verified in the pane. A master that dies later is
      only noticed when a joined `ssh` exits 255.
- [ ] Fingerprint of a plain alias: `ssh-keygen -F <hostname> -l` on `known_hosts` after the connection is up
      (`keygen_lookup_argv` + `fingerprint_from_keygen`). It is a lookup by name, not the key the handshake
      presented; with several key types recorded for a name the first line wins. An `ssh -v` parse of
      "Server host key: … SHA256:…" would be exact.
- [ ] A plain session (consent refused) cannot start in the pane's folder (no remote command to carry it).
- [ ] The restored tab of a host reconnects on launch and may ask for a password then.
- [x] A reopened closed tab (`Start::Scrollback`) loses its host (the mapping is dropped with the pane). (done: `ssh_note_closed`/`ssh_reclaim`)
- [ ] Offline tailnet rows are `Availability::Disabled` (quire dims them and takes no press); a machine that
      is offline in the tailnet but reachable another way cannot be opened from the list.
- [ ] The forward row's local port is any free port when the same number is taken; the person is not asked.
- [ ] `ForwardFault::LocalPortTaken` has no message path in practice (a free port is picked first).
- [x] Local copies of edited files stay in `<runtime>/edit/<id>/` until the runtime dir is cleared; "Done"
      stops watching but does not delete them. (done: Done deletes them, and keeps one while a conflict waits since Batch 26)
- [ ] Remote edit conflicts show Replace theirs / Use theirs / Decide later; no diff.
- [ ] HostRow/Edit/Forward banners are drawn above the status line of each host pane; they are not in the
      sidebar. No unread mark for a pending question in a background tab.
- [x] `Hub::edit_remote` needs the host's connection up; there is no "connect, then edit" for `host:path`
      typed in the bar for a host with no open pane. (done: `edit_when_up`)
- [ ] OSC 52 / transfers / notifications consent stays in `remote::HostConsent` (FOLLOWUPS Batch 1 ssh).

## Quire asks
- [ ] A `Toolbar`/row layout component for the host banner's text field + buttons (uses `.temor-host-line`).
      (check: quire v0.3.14 has a `Toolbar` in `chrome/`; is it usable inside a banner?)

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
- [x] docket owns per-turn checkpoints: it uses temor's git-ref approach (private index, hidden ref), keeps the last 20 per run, drops them after 14 days, and skips agents that keep their own (Claude Code /rewind). When the interface is frozen (a list per run, plus restore with a dry-run plan), temor stops snapshotting (hub/agents/checkpoints.rs, temor-core checkpoint git) and shows docket's list and restore. Until then temor's refs stay, and temor adds no pruning.
      Update (2026-10-10): the interface is frozen (~/rs-wt/checkpoints/DESIGN.md §3, §5, §8, §9). Migration starts when the agent session sends the docket sha:
      - delete temor-core checkpoint/* and the hub's checkpoint jobs, and call docket's CheckpointList / CheckpointPlan / `checkpoints.restore`;
      - move undo_ref to refs/temor/undo/<n>;
      - at start, prune refs/temor/checkpoints/* older than 14 days, with no import;
      - add `_` arms for the non-exhaustive SessionEntry.
      (done: P19, Batches 14 and 15)

## Look batch: Warp-style cards (merged 2026-10-10)
Studied Warp's design (AGPL; no code copied). D5 stays: the shell keeps its own line editor, and
there is no input box of temor's own (Warp's needs its own vim emulation and completer, all AGPL).
- [x] Hover goes stale after a wheel scroll or when the pointer leaves the pane. Clear it on
      pointer leave of `.temor-keys` and recompute on wheel. (done; harness test in Batch 26)
- [x] `card_bands` reads the cursor row from the previous paint, so the last block's extent lags
      one frame. Compute the bands after the capture. (done: it reads the live cursor first)
- [x] `TOOLS_HEIGHT_LOGICAL` (28.0) is an estimate; measure it with `use_rect`. (done: measured, session/marks/tools_height.rs)
- [ ] No GPU pixel test for bands (needs an adapter); the CPU tests cover the shared scene path.
- [ ] Prompt: nushell is left alone (there's no reliable stock-prompt test). fish 3 and older were not run.
      In fish, Enter on an empty line before any command has no blank line above it. The branch is
      read from .git only, with no dirty marker on the prompt line (the chips show changes now).
- [ ] zsh: without a terminal description that can move the cursor up (an unknown TERM), the
      typing line starts with one space, because zsh draws no prompt whose last line is empty.
- [ ] quire asks: a compact icon-button group surface (the toolbar uses `.temor-block-tools`
      CSS); a sticky overlay surface with an opaque material and a bottom hairline;
      `text-overflow: ellipsis` in Blitz (the header cuts with `clip_chars`).
- [x] Warpify-style offer: for a typed `ssh`, `docker exec` or `sudo -i` inside a tab, offer
      once, from a fixed command list, to load temor's shell integration in the inner shell (so
      its commands become cards). Later, after the context chips batch. (done: P17, Batch 3)
- [x] Next, queued (owner OK): the context chips batch
      (/mnt/data/cargo/temor-lanes/QUEUE-facts.md). (done: P16, Batch 3)

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
- [x] Remote edit and forwards still use scp and `ssh -O`; move them to the helper's file service.
      Input typed while the link is down is dropped. (done: Batch 5, Tier B edits go through the
      helper and typing is held while the link is down; Tier A keeps scp and forwards stay on
      `ssh -O` by design)
- [x] (Batch 5) Helper consent is in its own `remote.json`; fold it into the consent book.
- [x] The helper binary comes from `TEMOR_REMOTE_HELPER` or the folder beside temor, for the same
      CPU kind only. Packaging a helper per architecture is open. (done: Batch 24, `temor-remote-<arch>`)
- [x] (Batch 5) Offer:
      - fish and nushell inner shells: Yes does nothing yet (`temor.fish` can't be sourced in
        place);
      - shells with no line editor cut the roughly 14 KB line at 4096 bytes, so shrink the payload
        or split it;
      - history hiding needs `ignorespace` / `HIST_IGNORE_SPACE`.
      (what is left is under Batch 5 Next)
- [x] (Batch 5) Facts:
      - `.node-version` and `.tool-versions` aren't read;
      - untracked files aren't counted;
      - a repo with no commit shows no branch;
      - a long typed command can run under the chips.
- [x] Facts: host panes get no chips (also under Batch 5 Next). (done: 8c9fce8; folder chip Batch 23)
- [x] (Batch 5) Desk:
      - the ⌘T bar re-reads ~/.ssh/config on every keystroke (cache it);
      - the X11 token is `DESKTOP_STARTUP_ID` and isn't read;
      - check that the radar treats an agent-opened pane as unfocused (a560d57, test in
        `hub/desktop_tests.rs`).
- [x] (Batch 5) Wire:
      - staged `.temor-part-*` files aren't swept;
      - `Error::TooLarge` is unused;
      - there's no test that reads the pixel size back, and none for a slow file request.
      (5977500)
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
- [ ] docket lists `org.quire.Temor` under `cli` in its intentd.toml now (Batch 15 notes). Left:
      Perform still sends `Origin::InWindowField` (temor-desktop `platform/linux/checkpoints.rs`);
      check it against the cli role.
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
- [x] Host panes: no peek, no chips (both need the helper to read or report from the host). (done: 8c9fce8 and peek through the helper; folder chip Batch 23)
- [ ] Offer: nushell history isn't cleaned and the zsh prelude prints a parse error there; dash
      prints "source: not found" once; nushell has no pty scenario; fish history deletion is only
      checked with piped input.
- [x] (Batch 17) Remote: no ds-harness scenario for Retry (needs a fake ssh that asks for a password);
      `edge/remote_install.rs` is 307 lines; a half-received send can repeat after a reconnect.
- [x] (Batch 17) Edit: stream large files; conflict wording could name the "same time, other contents" case.
- [x] Facts: `.python-version` and `pyproject.toml` aren't read; a huge untracked tree is slow. (done)
- [x] (Batch 17) Peek: OSC 8 `file://` links show no card; the card's size is estimated; `view/pane_view.rs`
      is over 300 lines (split the pointer and key wiring).
- [x] (Batch 17) Desk: an `Include`d ssh config edited in place isn't noticed; quire ask: confirm
      `focus_with_token` acts on an X11 startup id; a place-only recall starts shells in the
      Space's folder, not each pane's.
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b5-*.md`.

## Batch 16 pins (2026-10-11)
quire v0.3.14 (c404fb0), docket 312cd27, porter 83ac0fa, anyview 6c325c9, and almanac ac4bc03,
stoker 4a0bd9e, blitz-kit 25b5099 and chordkit ba094c6 through the lock. One copy each; no code
changes.

- [x] anyview #100: find in a viewer pane. `PaneHandle::find(query)` / `end_find()` and
      `hits(HitList::Brief | Whole)` (an "In This File" palette group whose rows run through
      `run`). anyview suggests ⌘T feeds its query to the viewer in a "find in pane" mode; ⌘F stays
      temor's. (done: "Find in This File"; ⌘F in a viewer pane since Batch 26)
- [ ] Batch 17: take anyview be8c91c (same lock as 6c325c9). It adds #98 (a recording plays in a pane
      given a player, else opens elsewhere: pairs with the `player` follow-up above) and #99 (one
      remembering policy, `anyview_machines::{Remembering, Noted, REMEMBER_EVERY}`; a pane flushes
      on unmount, so far fewer `Job::Keep`s).

## Batch 17 (merged 2026-10-11: bundled follow-ups)
Four Sonnet lanes from main 26799e4, each with a bundle (viewer, remote, shell, desk); merged,
fixed and gated together. anyview be8c91c.

- Viewer:
  - the viewer remembers places through anyview's one `Remembering` policy (temor's write-only
    `resumes` map is gone);
  - "Find in This File" in the ⌘T bar while a viewer has the keyboard (`bar/viewer_find.rs`);
  - peek and ⌘-click both follow `file://` links;
  - the peek card is placed by its measured size;
  - `view/pane_view.rs` is split (pane_keys, pane_pointer, pane_focus, pane_footer).
- Remote:
  - `remote_install.rs` is split;
  - a held chunk cut off by a dying link is dropped, never sent twice;
  - a Retry scenario with a fake ssh that asks for a password;
  - remote edits stream (1 MiB reads, 4 MiB uploads), and "same time, other contents" has its
    own words;
  - Tier A: Done deletes the local copy, a reopened tab keeps its host, a typed `host:path`
    connects then edits, a swapped forward port is named, host banners mark the tab unread;
  - IdentityFile and ProxyJump checks; an IPv6 hint; a control folder of mode 0300 or 0000 is
    tightened.
- Shell:
  - the offer asks the inner shell its name first, then types that shell's text (dash and the
    like get "could not tell which shell this is");
  - `.python-version` and `requires-python`;
  - the untracked count streams and stops at the cap (it counts entries);
  - zsh under an unknown TERM prints the folder line from precmd;
  - a person's exit trap and `zle-line-init` keep temor's hooks;
  - a pipeline reports its newest command.
- Desk:
  - unbound Primary chords reach a program that turned on kitty keyboard flags;
  - shortcuts on bar rows and the New tab tooltip;
  - Space rows in the ⌘T bar;
  - a place-only recall restores each pane's folder;
  - block hover follows the wheel and clears on leave;
  - card bands use this frame's cursor row;
  - the toolbar height is measured;
  - `Include`d ssh config files are watched;
  - the window icon;
  - checkpoint restore sends `Origin::Cli`.

### Orchestrator decisions
- A cut-off held chunk is dropped rather than numbered (no wire change; keys are never repeated).
- A tab opened by connect-then-edit takes the keyboard: the person asked for it.
- No dirty marker on the prompt line: it would need a blocking `git status`; the chips show it.
- The probe line stays in history for now.
- Merge wiring: ⌘-click opens a `file://` link in the viewer; hover clears on leave and redraws
  after a wheel (the hooks the desk lane wrote for `pane_view.rs`).

### Next
- [x] Recordings in the pane: skipped for now (owner's decision, relayed by the agent session
      2026-10-11). Recordings open in their own app. Revisit when mpv-wgpu settles or the owner
      wants video inside temor: an additive lane (the `player` feature, one `MediaHub` per window
      on temor's tokio runtime, an mpv locator).
- [x] "Edit locally" from a block's output in a host pane: the hub entry exists
      (`Desk::host_edit`); the block menu item and which text it passes are open. (Batch 26)
- [ ] Find in the bar: Esc closes the whole bar; no harness scenario (needs a mounted viewer).
- [ ] quire asks: `Column` with a zero gap and a fill option; a `write_soon` that returns its value.
      `focus_landing_told` no longer calls `told`, so temor keeps its own.
- [ ] Offer: nushell history isn't cleaned (no delete command); the probe line stays in history;
      the one-text `InnerLoadLine::build` is used only by tests.
- [ ] zsh under an unknown TERM: a screen clear drops the folder line until the next prompt.
- [ ] temor-ssh has no `-i`/`-J` builder yet to use the new checks.
- [ ] pyproject: Poetry's python constraint isn't read.
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b17-*.md`.

## Batch 18 (merged 2026-10-11: close the open items)
Five Sonnet lanes from main 5f01230 (hostview, tiera, term, files, shell), each with a bundle;
merged, fixed and gated together. anyview c91ec0a.

- Host files:
  - ⌘-click on a host pane opens the file in the viewer, fetched to `<runtime>/view/<n>/` (helper
    `ReadFile` or scp) and removed when the viewer closes;
  - peek on Tier B panes (big files fetch their first 64 KiB);
  - chips on Tier B panes through a new helper `Facts` request (the capability guards it; the
    helper sends raw text and temor-core parses it in the window);
  - an edit conflict's "Show changes" opens a unified diff (`similar`, MIT/Apache);
  - "Open in another app" for a file the viewer can only describe.
- SSH Tier A:
  - the check link is read from raw bytes (`ByteTap` on the pane engine);
  - "Established" when the master's socket appears (one connection at a time), `ssh -O check`
    otherwise;
  - fingerprints by `ssh -G` and `ssh-keygen -F` on the resolved name and port;
  - a restored host tab connects when first shown;
  - offline tailnet rows say why and offer "Try anyway";
  - clipboard, transfer and notification permits are in the consent book;
  - the unused `HostList` view is gone; new tabs and splits already open on the same host.
- Terminal:
  - OSC 52 writes, and reads that ask once per tab;
  - a hovered link's target shows in the status line;
  - `RENDER_HOLD` frames are captured where a hold begins;
  - a failed capture after a resize changes nothing;
  - `row_wraps` returns `temor_vt::Wrap`;
  - a search bench (ignored);
  - the resize window is documented.
- Files:
  - dropping files on a host pane uploads them to its folder (never over a file), listed on the
    shelf;
  - kitty `prm` (masked to 0o777), in-root symlinks, and a malformed frame ends the transfer;
  - "Edit here" on a host pane's block toolbar;
  - PLAN §3 lists the wire's catch-alls and the no-payload marker.
- Shell:
  - bash and fish forget the probe line; the one-text load is gone;
  - zsh redraws its folder line after Ctrl-L under an unknown TERM;
  - Poetry's python constraint;
  - nushell gets temor's prompt when its own is the stock one;
  - KDE and GNOME shortcut sources, reloaded on change;
  - old `keys.` lines say what to write instead;
  - Esc in the bar's find steps back;
  - a window-title scenario; heavy tests share a lock.
- Merge work:
  - a host's clipboard-write answer is kept in the consent book and applied to its panes;
  - the launch token is Wayland's only (quire reads `DESKTOP_STARTUP_ID` itself);
  - a hold whose end straddles the inline-file scanner's pieces is seen;
  - test fixes: hold offsets, a scratch runtime for viewer copies, the socket and fingerprint
    tests wait for the first session, titles held by a sleep, the keys watch reads its first
    time before its thread starts.

### Orchestrator decisions
- terminal.conf stays temor's own file: its parser is in temor-pane (L1, no quire) and its key
  lines are chordkit's. ds-settings would need a line-form format; not asked for now.
- A plain session (setup refused) never has a `cd` typed: it starts in the host's home.
- The first OSC 52 read gets an empty answer rather than blocking the pane's I/O thread.
- quire answers (whopper, 2026-10-11): v0.3.16 brings SplitView's finished-drag report, a
  zero-gap `Column`, a returning `write_soon`, an icon-button group, an offer-bar overlay,
  menus that keep disabled items with tooltips, and a banner action row. `chrome::Toolbar` is
  the window's toolbar, not a banner row. `text-overflow: ellipsis` waits on Blitz (#893): use
  `.ds-truncate` or `text::clip_chars` until then.

### Next
- [ ] quire v0.3.16 (one re-pin): adopt the finished-drag report, `Column`, `write_soon`, the
      icon-button group, the offer-bar overlay, the menu tooltips and the banner row.
- [x] Byte progress for uploads (a progress hook on temor-remote's `Client::upload`). (done: Batch 20)
- [x] anyview ask: a "could not be opened" request, so a failed file offers another app (anyview 75f1f86, `PaneRequest::NotOpened`; done 2026-10-11).
- [ ] quire ask: tell Esc from a click outside on the palette (`onclose` covers both).
- [x] keycap: our desktop's shortcuts come through keycap-client's `KeycapSource` (95f41dc, the rev casement reads).
- [ ] keycap-client is gone (2026-10-19): the desktop's shortcuts come from quire's
      `system_key_source`, as mailo's do, so temor builds on macOS and Windows past it. On our desktop
      a person's own keycap bindings no longer reach temor's keymap; the ⌘-style conventions do, and
      `X-Keycap-Profile=keymap-aware` still keeps ⌘ apart from Ctrl. temor builds its keymap on
      `system_key_source` itself, so a source a launcher injects (quire's `with_keymap_source`) does
      not reach it; bringing keycap's bindings back means building on the injected `KeySource`.
- [x] Mark host panes remote where `ssh.panes` gets its entry (today the facts read does it). (done: Batch 20)
- [x] Run `search_bench` and decide on a row-text path for search (2026-10-15: glance as text).
- [ ] Offer: zsh keeps the probe line; nushell history isn't cleaned. (2026-10-16: no probe at all
      when the command line names the shell, as Warp does; plain `ssh host` still asks.)
- [ ] OSC 52 primary/selection clipboards (ds_blitz has no target).
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b18-*.md`.

## Open now (updated 2026-10-14)
Everything still open, one line each, with the section it lives in. The boxes in the batch
sections above are the lanes' own notes; this list is the one to read.

### Doing now (owner, 2026-10-14: "do 1 2 3")
1. ~~Sidebar: edge peek, pinned tile drags, squeezed splits.~~ Done 2026-10-14.
2. ~~Cards: bottom anchor, bold prompt, more menu, DropTab minimum.~~ Done 2026-10-14.
3. ~~Chips on older blocks' headers.~~ Done 2026-10-14.

### Done 2026-10-14: the terminal follows the page zoom (quire#19, pinned with anyview 4280bc2)

### Doing now (owner, 2026-10-14: "do 1 2 3, match mailo with 4")
1. ~~One Space: no Space name in the head and no lone dot; a right click on the foot is the
   Space's menu.~~ Done 2026-10-14.
2. ~~Pin tab / Unpin tab and Downloads in the command bar (chordless actions a keys file can
   bind).~~ Done 2026-10-14.
3. ~~Downloads at the right end of the head row, its list opening below.~~ Done 2026-10-14.
4. ~~No hover close on a split's panes: a right click on a pane is the tab's menu with Close
   Pane.~~ Done 2026-10-14.

### Waiting on another project
- [x] blitz-shell hard-wired keys before the app heard them: Ctrl or Super with `=`, `-` and `0`
  zoomed the page whatever else was held (Equalize zoomed too, Ctrl+_ zoomed out), and Alt+D,
  Alt+H and Alt+T drew its debug overlays. Fixed upstream: blitz-shell's `BuiltinKeys::Off`,
  ds-blitz's `ShellKeys::App` with zoom as a window request (`WindowHost::page_zoom`), and
  chordkit's Bigger, Smaller and Actual Size as Ctrl or ⌘ with `=` (labelled `+`), `-` and `0`,
  also in a terminal. temor binds them as Zoom In, Zoom Out and Actual Size (also in the command
  bar), and Equalize moved to ⌥⌘= (Ctrl+Shift+Alt+0 where Primary is Ctrl+Shift). Pins: quire
  163957e0, anyview 776abe1, chordkit 6ef5493, keycap 794f9de, Blitz 531fb43d, blitz-kit 2ecef10.
  Done 2026-10-15.
- Ctrl+_ (Ctrl+Shift+-) never reaches the shell as undo off our desktop: it is dropped as an
  unbound Primary chord. chordkit#3 (55a62b2) folds Shift+- into `_`, so it is the program's;
  Whopper bundles it with the next quire bump. (keycap's ⌘= and ⌘- on our desktop already work:
  a raw press keeps Shift apart from the key, so keycap's Ctrl+Shift is read as ⌘ first.)
- quire: tell Esc from a click outside on the palette. (Batch 18 Next)
- Blitz: `text-overflow: ellipsis` (#893). (Batch 18 decisions)
- chordkit: a fallback where the default collides. (Keys on chordkit)
- ds_blitz: primary and selection clipboards for OSC 52. (Batch 18 Next)
- P8 Proposals: bulkhead split (fresh `TurnRef` per Ask, "edited" vs "replaced", U+2800).
  (temor-core)
- P11 Plugins (bayonet), P12 Companion context (companion). (PLAN §11)
- Ghostty upstream, with the owner's OK: OSC 1337 `File=` through UNKNOWN_SEQUENCE, per-cell
  hyperlink id, libghostty-rs wrappers; iTerm2 multipart files dropped before temor (check).
  (temor-vt)

### Settings window (owner, 2026-10-15: settings.toml, keys in their own file, quire's page first, all four pages)
- [x] Batch 21: settings are ds-settings tables in `~/.config/temor/settings.toml` (font, cursor,
  scrollback, shell, notify, offer), key changes in `keys.conf` (bind refuses a held chord and
  names its action; reset one or all), `terminal.conf` converted once and kept as
  `terminal.conf.old`, both files followed live (font and cursor reach open panes), and
  `temor --write-schema <dir>`. Lane notes: `/mnt/data/cargo/temor-lanes/followups-b21-settings.md`
  (PLAN §6/§14 text to fold in, the key and label table).
- [x] The window: General, Appearance, Keyboard, Spaces on quire's `ds-settings-page`
  (quire c62994d0), opened by ⌘, and a command bar row; open-or-raise copied from mailo's
  `SettingsOpened`. Waits for anyview, keycap and sill to re-pin to quire c62994d0 and chordkit
  55a62b2 (Whopper, after the owner's login testing). (landed: 5b7cb01)
- Open: quire's `Page` has only the app, so General and Appearance are told apart by section.
  Closed (Batch 24): `notify.after` reaches open panes; the converter's comments vanishing at the
  next save is accepted (`terminal.conf.old` keeps everything).
- [x] Space credentials are kept in the desktop's password store (Secret Service through
  porter-secrets, namespace "temor"); a missing or locked store says so. The unlock prompt is
  untested on a real desktop.
- [x] A new tab from a host pane opens on the host, as a split does; the bar's "New tab on this
  computer" opens a local one.

### Owner's call (closed 2026-10-15, owner: "close them out fully autonomously"; Batch 24)
- [x] quire f47b05c8 (anyview f4c8608): the chips draw quire's Folder, GitBranch and Diff glyphs.
  Pinned tabs keep temor's own tile: `PinTiles` has no orb face, counts instead of the news dot,
  its own drag (temor's moves tabs between tiles and rows), and only the solid terminal glyph.
- [x] Agent note: the tab's hover text carries it (it already did; now tested).
- [x] The 720 px breakpoint stays: the sidebar hides under 720 px, as Mac sidebars collapse in
  narrow windows, and ⌘⇧L shows it at any width.
- [x] keycap `temor-plasma-terminal` merged as keycap d2eae05 (Whopper, owner approved); temor pins it.
- [x] Recordings in the pane stay deferred: they open in their own app until mpv-wgpu settles.
- [x] "New tabs start in" (General > Shell): as usual (tabs at home, splits in the pane's folder),
  the current folder, or the home folder. A Space's project folder still wins.
- [x] Helper builds per CPU kind: `temor-remote-<arch>` beside the program (`x86_64`,
  `aarch64`), chosen by the probed host's CPU; a host with no build for its CPU stays on a plain
  connection and is told so. `temor-remote-aarch64` is built static with
  `cargo zigbuild --target aarch64-unknown-linux-musl` (`temor-lanes/build-helpers.sh`) and sits
  in temor-lanes/bin; not yet run on a real ARM host (no emulator here). Packaging must ship the
  extra builds (dist/ not done).

### Platforms (owner)
- macOS: pty, keys (Cmd), emoji (sbix), windows/IME/CoreText check, trial sandbox, opener and
  runtime-dir stubs, transfer edge, temor-ssh dir. (Platform)
- Windows: ConPTY, plugin processes, opener, transfer edge. (Platform)
- Ubuntu 24.04+: AppArmor and user namespaces; detect and document. (Platform)

### Needs hardware, a real host, or a shell this machine lacks
Run on this machine 2026-10-15 (RTX 5070 Ti, a scratch sshd, tools in /mnt/data/cargo/hw-tools;
the gate now runs the GPU tests headless and has hw-tools on its PATH):
- [x] GPU device tests: atlas clears between frames, an image past the device's limit, bands
      against the CPU. A frame whose own glyphs outgrow the atlas is drawn in groups of rows
      (Batch 25); only a single row that cannot fit is still refused (`GlyphsDoNotFit`).
- [x] A real sshd test (`temor-ssh/tests/it/real_sshd.rs`). It found: a held local port read as
      "no connection" (fixed), and a port held on one loopback only let ssh listen on the other
      (fixed: `free_local_port` wants both free).
- [x] Tune wire limits and the retry policy: done over a simulated thin line (see temor-wire above).
      Uploads on this machine's /mnt/data are slow for another reason: one fsync of 8 MiB takes
      8-17 s on that full, busy disk.
- [x] kitty: `st` is base64 without padding and the kitten refused temor's padded replies, so
      every `kitten transfer` into temor failed (fixed). `tt` is `simple`/`rsync`, as parsed.
      The kitten also read a session-level STARTED as a refusal; temor now sends it first only
      to its own sender (`temor=1`, `FirstAnswer`). `kitty_interop.rs` runs the real kitten end
      to end, by hand (ignored), and passes.
- [x] fish 3.7 marks; tcsh/csh as the host's login shell; zsh's Ctrl-L redraw (checked to fail
      without the widget).
- [x] bash before 4.4 (macOS's /bin/bash) marks C/D as Warp's bash-preexec does: a DEBUG trap
      put in from PROMPT_COMMAND that chains a person's own trap (tested on bash 3.2).
- [x] `search_bench`: 100,000 rows read cell by cell 1.8 s, as text 20 ms. Find now glances at
      32 rows as text and reads cells only where a match could be (a rare query: 60 ms). It also
      showed scrollback kept ~600 lines whatever the setting (libghostty's 10 KB byte cap; fixed).
- [x] The aarch64 helper runs under qemu-aarch64 (its own argument handling answers); its plain
      start re-runs itself, which qemu without binfmt can't follow. Not run on real ARM.
- [x] Space secrets in a real Secret Service: `temor-desktop/tests/it/secrets.rs`, by hand in a
      private bus with the scratch gnome-keyring (see the module notes). Passes.
- [x] IME candidate placement, tried with fcitx5 and Bopomofo on a private display: the
      candidates sat at the window's corner. blitz-shell turns the IME on with no capabilities,
      so winit drops every cursor area. A one-line fix (enable with the cursor area) moved them
      under the caret; sent to Whopper (`temor-lanes/blitz-ime-cursor-area.patch`). Close when
      quire's blitz rev carries it. (P3a) (done: quire 3cfe32c4, blitz 5c755366; the candidates
      open under the caret on the private display)

### Small temor items left
- [x] Host panes have chips and peek through the helper; the folder chip since Batch 23 (the helper reports the host's home). No hub scenario test of host facts yet.
- Adopt quire v0.3.16 (pinned 2026-10-11 with anyview 75f1f86): SplitView `on_resized`, `Column`
  (`ColumnGap::None`), `write_soon_then`, `IconButtonGroup`, `PinnedBar`, `BannerActionRow`, menu
  tips, `DotFace::Foot`. Planned with the look work (block tools as an icon group, a pinned typing
  line). (UI pass, 2026-10-11)
- [x] Byte progress for uploads (Batch 20; scp uploads have none).
- [x] Host panes are remote from their session's start (Batch 20).
- [x] Offer: zsh keeps the probe line out of history with a `zshaddhistory` hook (Batch 24), no
  rewriting; the held rewrite (b20-remote) is dropped. Left: the hook runs only once temor.zsh is
  loaded, so a probe typed into a plain zsh (before integration) can still land in history;
  nushell history isn't cleaned (no delete in nushell).
- [x] Kitty virtual placements, selection recompose, glyph overhang (Batch 23: placeholder cells
  decoded in temor-render as libghostty doesn't expose them; a selection recomposes only its
  changed rows; the CPU path draws overhang whole, as the GPU's). Left: each CPU band builds its
  neighbours' scenes too (about 3x the shaping per composed row; build them only when a
  neighbour overhangs; done since), overhang into the outer padding (drawn since Batch 25), and the lane's notes
  in `/mnt/data/cargo/temor-lanes/followups-b23-render.md` (one of `c=`/`r=` given, placement id
  collisions, virtual images' layer).
- Mark cuts depend on where libghostty dispatches OSC 133. (Blocks)
- Blocks stay in temor-core (a design choice for now). (R1b)
- KWin takes Meta+T/W/D/V on KDE (the desktop's binding). (P3a)

## UI pass and Warp comparison (2026-10-11)
Warp and temor were run side by side in a private virtual KWin (offline, scratch home, never the
owner's display). Done: temor's prompt in every shell whatever PS1 the startup files set, as Warp's
default (Fedora's /etc/bashrc set PS1, so the cards never showed; `TEMOR_PROMPT=shell` keeps the
person's own); a split pane no longer goes blank (the old view's drop took the new view's
texture); a click leaves nothing selected; the status line and tab rows are spaced and fade
long text; anyview's `NotOpened` offers "Open in another app"; quire v0.3.16 pinned.
- [x] The look (owner to confirm): bolder cards with padding and the run time and exit code in
  the header; icon-only block tools; no status footer; the typing line pinned to the pane's bottom
  while the screen is not full; one surface for window, sidebar and pane (a quire ask for a flat
  app-shell look); dark by default (owner, 2026-10-11).
  (done in Batch 19 and the sidebar pass; the pinned typing line is under Open now.)
- [x] Design audit (2026-10-11, private artifact https://claude.ai/artifact/QZJUJwoLf5cSKodekfSdAh): 3 critical (five text colours under 4.5:1, the find field not taking the keyboard, no collapse or pane minimum below ~800 px), 8 high (four grounds in two temperatures, accent on six jobs, a ring around the focused pane, flat cards, the floating toolbar sentence, the typing line repeating the card, a 10 px status strip, `user@host:path` tab names), 6 medium (sidebar foot, actions missing from the bar, find bar layout, sans versus mono, the miniature, first launch).
- [x] Sizes (same audit, measured with the ignored `size_audit` harness test): three control sizes in the sidebar foot (New tab is `ControlSize::Small`), tab rows at 40 (155 when split) off quire's 28/32/36 ladder with no vertical padding, a 28 px tools bar on an 18 px terminal row with `--r-panel`, seven type sizes (10 to 16), the command bar's rows not concentric (quire), paddings off the 4 px grid, the closed Downloads hint overlapping the foot buttons.
  (the audit and sizes: done in Batch 19 and the sidebar pass.)
- [x] ds-harness break-it tests: window size sweep, divider drags past both ends, collapse and
  restore, keyboard-only use, resize with a menu or the bar open.
  (done, `tests/it/resizing.rs`: a dragged divider kept through a tab switch, dragged past both
  ends, every control inside the window from 400x360 to 1920x1080 with typing after, keys alone
  for tab, split, bar and close with no click, the bar resized while open. All pass; the kept
  divider failed before `SetShare`. Not covered yet: sidebar collapse and restore, resize with a
  menu open, scale 150/200%.)

## Batch 19: the design audit (2026-10-11)
Four lanes (access, sidebar, cards, bar) from the audit; merged with one combined gate. Done:
card header colours without dim (folder 5.92:1, branch 6.81:1 dark; 5.33 and 5.77 light); dark
by default when appearance.toml names no theme; the sidebar hides under 720 px and Super+Shift+L
(⌘⇧L) shows or hides it at any width; a side-by-side split keeps 40 columns per pane (60 refused
splits in most laptop windows); a 16 px terminal inset (8 top); tabs named by program or folder
with "branch · state"; tab rows on quire's 36 ladder; the miniature a trailing hairline glyph;
the foot's New tab at Regular size and the closed Downloads hint laid out no more; no status
strip (a hovered link shows in a tooltip, progress overlays the pane's bottom edge); bold
command rows; "exit 1 · 4 ms" meta and three hover tools (copy output, re-run, more) in the card's
header row; no typing-line fill and no repeated branch chip; no pane focus ring; find takes the
keyboard, sits top right with a "1 of 21" count and chevrons; every action in the command bar
with its shortcut, under section headers; a first-launch hint line.
Merge fixes: a finished card keeps its input anchor (bold rows, and the anchor leaked); the find
count redraws after the delayed search; tab rows set plain text (quire's Code tone is a chip);
the bar runs the sidebar action through the desk.
- [x] Bottom-anchor the primary screen (the typing line pinned to the pane's bottom): needs one
  row offset in the render view and the fit, used by every grid-to-pixel map; spike with a window.
  (done 2026-10-14: each paint counts the empty rows under the text and the cursor
  (`PaneFrame::blank_rows_below`; none while an image is placed or a full-screen program holds the
  screen) and draws the grid that many rows lower (`GridFit::lowered`, `PaneView::lower`); the
  session's hit tests, mouse reports, gutter, chips, metas and tools read the same lowered fit
  (`Session::placed`). The grid's size never changes, so the program is not resized. The
  first-launch hint moved over the prompt. Checked in a private window: new pane, cards, a click
  picking the block under it. ds-harness doesn't paint, so it has no test there; the renderer has
  unit tests.)
- [x] `gutter_geometry` returns one padding: the meta, tools and chips rows ignore the 8 px top inset. Done: it returns the side and top insets apart (`Gutter`), and the meta and tools sit on the header line.
- [x] Card text sat low in its card (owner): the rule ran along the top of the blank line above the header. Done: a band can begin and end across a row's middle (`Cut`), so the rule crosses that blank line and each card has half a line above its header and below its last row. Warp keeps about 14 px; ours is half a row (about 9 px).
- [x] With `TEMOR_PROMPT=shell` and a one-line prompt, the prompt text on the command row is bold
  too (the band is whole rows; start it at the input mark's column).
  (done 2026-10-14: `RowWeight::BoldFrom(col)` bolds the band's first row from the input mark's
  column and its other rows whole.)
- [x] The tab's "running 2m" doesn't tick (read when the sidebar draws).
  (done: `hub/running_tick.rs` reads each tab's words every second and redraws when one changed;
  `sidebar::a_running_tab_counts_up_with_no_output`)
- [x] DropTab doesn't check the 40-column minimum.
  (done 2026-10-14: the hub tells the drop the width of the pane it lands on, `DropTabIn`, and a
  side-by-side drop under 80 columns is refused as a split is.)
- [x] In a narrow pane the hover tools cover the header's folder text; the find bar can cover a
  card's meta.
  (done: a card's meta shows only where it clears its header line's text, counting each
  character a terminal cell wide (`HeaderRow::fits`), and the tools leave their meta out by the
  same rule; while the find bar is open, metas and tools on the rows under it are hidden.
  `cards::a_card_meta_steps_aside_…`, `cards::the_find_bar_hides_…`. The tools' buttons still
  show over a long header when hovered: they were asked for.)
- [x] `Chrome.cwd`, `title` and `link` are unread; remove. (done)
- [x] The more menu unmounts with the hover tools if the pointer leaves the pane.
  (done 2026-10-14: while it is up its block is `held`, and the tools stay for it.)
- [x] The selected card fill is still the accent at 12%; neutral once quire's selection lands.
  Done: the foreground at 8%.
- [x] quire asks (q20, q21 → v0.3.17): flat sidebar (`SidebarFill::Flat`), neutral source-list
  selection, concentric palette rows, `RunTone::Mono` (tab names and branches), and
  `RowLeading::Element` (the orb leads a tab while an agent works or waits; a terminal glyph
  otherwise). Adopted at the v0.3.17 re-pin. The sidebar's branch lost its changes dot (it read
  as a stray mark beside "· idle"; the card's chips show the counts).
- [x] Porter/docket batch 18 (docket 1695cff, porter ee3c63e, almanac 5678345): taken with
  anyview 5ec7c56 (#109).
- [x] Porter/docket batch 20 (docket afe9314, porter bfd909a, almanac 4eabd9e): taken with
  anyview d571cb0 (#115). No CheckpointList literal here, so docket's new restore-point label
  needs no change; showing it is open.
- [x] The nushell offer load is quadratic: the load text is one `sh -c '...'` program over many
  lines, and nushell's line editor repaints the whole open quote for every line it receives
  (6.4 MB of output for a few KB of script). Alone it finishes in well under a second; under
  the gate's load `nushell_offer` passes its 30 s read twice in four runs. A person's slow ssh
  link would see the same flood. Fix in the offer, not the test: give nushell a load text it
  takes in one line (a nushell-native `decode base64 | save` line, then `source`), or send the
  text as one bracketed paste.
  (done: nushell is given its script itself as one bracketed paste and one Enter, no `sh`
  program, file or `source`, `inner::nu_text`. The gate failures were a second cause: nushell
  asks the terminal for its cursor after every command, and lines typed ahead of the answer
  stalled it after the first one; `nushell_offer` now sends in slow pieces and bounds the
  output. Also found: the app typed the offer with `\n`, which nushell's line editor reads as
  Ctrl+J, so the offer never ran in nushell; it now types Enter's `\r`, checked in a terminal
  for bash, zsh and fish too.)
- [x] The offer types the load on the tick that reads the probe's answer, which can be before
  nushell's next prompt (it is still waiting on its cursor answer then). One paste is less
  exposed than many lines were, but a slow ssh link could still drop it; waiting for the
  prompt needs a sign of it in a shell that has no marks yet (the screen settling, or a
  second probe word).
  (done: the answer counts once a non-blank line is drawn below it and the whole screen is
  unchanged between two ticks, `Answer::Named` until then; a shell that named itself but shows
  no prompt by `PROBE_MS` is loaded anyway. `temor_ssh::prompt_after_answer`, `hub/offer.rs`.)
- [x] Chips, Warp's layout (2026-10-14): over temor's prompt the chips replace the header line
  (folder, branch, `+N −M · F files`, then the tool chips); a one-line or own prompt keeps its
  text and gets the fact chips at the right end. Older blocks' header lines still show the
  shell's dim folder and magenta branch (what was true then is not kept); decide whether the
  block header should carry the same chips from the facts at the block's start.
  (done 2026-10-14, owner: "yes match the new chips": the facts the prompt's chips showed are kept
  for the block it becomes, `Session::keep_header_facts`, and its header line carries the same
  chips on the block's own ground (a failed or picked block's fill) and clear of its meta.)
- [x] quire ask: folder, git-branch and diff glyphs in `Icon`. The chips draw Lucide's (ISC,
  NOTICE) as symbolic SVGs until then (`view/fact_chips.rs`). A long folder is clipped at its end (no ellipsis on Blitz);
  cutting the start (keeping the last folders) needs a start-side ellipsis. (Settled: the Lucide
  glyphs stay; Batch 25 cuts a long folder at its start, `view/folder_fit.rs`.)
- [x] Picking blocks as Warp does (2026-10-14): a click on a block picks it whole, Shift+click a
  run, Ctrl+click (⌘ on a Mac) one in or out; a drag still selects text. With blocks picked, ↑/↓
  move the pick, Shift+↑/↓ grow it, Escape or typing lets go, and copy takes each block's command
  and output (a blank line between blocks). Ctrl+Shift+↑/↓ (Warp's Ctrl+↑/↓) pick the newest
  block, then the next ones, and bring them into view. Picked blocks take the selection colour.
  `temor_core::block::Picks`, `session/marks/picks.rs`. The gutter press picks too, instead of
  selecting the output's text.
- [x] Warp's other block actions (2026-10-14): copy the picked commands only (⌘⇧C, Ctrl+Shift+Alt+C
  elsewhere; commands one per line) or outputs only (⌘⇧O; a blank line between), Select All picks
  every block (the platform's own Select All chord), ⌘B / Ctrl+Shift+B bookmarks the pick (an
  amber stripe that wins over the failed one) and Alt+↑/↓ at the prompt step between bookmarks,
  ⌘I / Ctrl+Shift+I puts the picked commands back in the prompt joined with "; " (pasted, so
  nothing runs), and a right-click on a block opens its menu (Copy, Copy Command, Copy Output,
  Put in Prompt, Bookmark, Select All Commands). The previous/next bookmark steps are in the
  command bar with no chord of their own. `Picks::select_all`, `next_bookmark`,
  `session/marks/picks.rs`, `view/block_menu.rs`.

- [x] Sidebar: mailo's layout and Arc's split row (2026-10-14, owner: "the sidebar is expected to be
  mostly matching mailo, all the icons and layout"; mailo's pass is "a quiet Dia, not an Arc").
  232 px at first (212 to 320); rows on the ladder's 32, one line each (branch, state and the
  agent's note on hover, the run time at the row's end while a command runs); pinned tabs as
  square tiles at the top (Pin Tab / Unpin Tab in the row's and the tile's right-click menu, which
  had no control before; a pinned tab reopens as a shell); Arc's New Tab row heads the loose tabs;
  a split tab is Arc's split row (one pill cut into a segment per pane, each with its glyph, its
  folder and an × that closes that pane, a hairline between, the pane with the keys in ink),
  replacing the miniature; the foot is mailo's: Downloads (a button with a dot, its list in a
  popover) at the start, the Space dots at the centre, and one chevron (New Tab, Reopen Closed Tab,
  New Space, Hide Sidebar) at the end, no `+`. Tooltips on whole rows are quire's `Tooltip` hooked
  to a block `HoverTarget` (`view/row_tip.rs`). `view/split_row.rs`, `pinned_tabs.rs`,
  `side_menu.rs`, `tab_sidebar.rs`, `shelf.rs`.
- [x] Sidebar, the rest of the mailo pass (2026-10-14): hidden, the sidebar peeks from the window's
  edge as mailo's does (quire's `EdgePeek` in a zero-width host at the edge; a click on the edge
  pins it); pinned tiles drag as Arc's do (onto another tile to take its place, onto a row to be
  unpinned there, a row dropped on a tile is pinned there; `TabsIn::Place`, `dnd::resolve`); a
  split of three or more panes keeps the focused pane's name and squeezes the others to their
  glyphs. A pinned tab still never joins a split (the core keeps its panes).
- [x] Sidebar, after the mailo pass (settled: temor keeps its own tiles): (1) quire's `PinTiles` draws accounts only; a glyph face
  (`PinFace::Glyph`) would let temor use it and its drag-to-reorder instead of its own tiles
  (quire change: owner's OK). (2) Hidden, mailo's sidebar peeks from the window's edge (quire's
  `EdgePeek`); temor's still snaps away. (3) Pinned tiles cannot be dragged to reorder or onto a
  row to join a split yet. (4) Arc squeezes a split of three or four panes to glyphs; temor's
  segments only narrow (unconfirmed from screenshots how Arc draws one unselected).
- [x] The tab's glyph (2026-10-14, owner: "too solid"): rows, pinned tiles and split segments draw
  the bare outline prompt `>_`, as Warp's tabs do (Warp's own terminal icon is the prompt with no
  box), not quire's solid boxed terminal. Lucide's `terminal` (ISC, NOTICE) as a symbolic mask,
  `view/tab_glyph.rs`.

## Colours: Apple 2025 palette (owner, 2026-10-15)
Owner picked: ANSI dark = Apple increased-contrast dark hues (normal 8E8E93 FF6165 4AD968 FEDF43 5CB8FF EA8DFF 3BDDEC D1D1D6, brights 30% toward white); light from Apple's increased-contrast light set; accent follows the system, normal value for fills, increased-contrast value for text-like uses (cursor, links, chip text); state colours: done green, waiting orange, error red, git indigo, folder secondary label, Space dots from Apple's 12 hues. Asked Whopper for the quire ds-style PR (accent_table.rs, terminal.rs, status tokens). temor uses the tokens after the next re-pin. Preview: https://claude.ai/artifact/Awot7u7G8teHqa1kTa5jLj
Whopper's colour batch (pending): --ok/--warn/--danger to Apple values (may step if a 4.5:1 gate fails); the person's accent stays macOS's eight; all 12 hues as --tag-* and --signal-*, git = --signal-git; SettingsPage gets `page: Option<Page>` (replace `pages::page_of`). On landing: the sidebar tone (preview-colours 460cd1c) uses --ok/--danger; the git chip moves to --signal-git; Space dots to --tag-*.
Landed (2026-10-15): quire 3f2b194, chordkit 55a62b2 (Ctrl+Shift+- is the program's Ctrl+_), keycap e4dd317, anyview 85d92c4; the Settings window; Apple colours; sidebar glyph green/red by the last command. quire's `SettingsPage { page }` names a page of the desktop's Settings app (`Page::App("temor")` is one page), so temor's own General/Appearance split stays in `settings_window::pages::page_of`. Setting a Space credential is refused until the porter-backed `CredentialSource` implements `set_secret`/`remove_secret`.
- [x] Flaky forward test: it now waits for the refusal among the notices, so a held local port
  doesn't matter (checked with 3000 held).

## Batch 25 (render and UI lanes, merged 2026-10-16)
Landed: the CPU draws glyph overhang into the outer padding; the GPU draws a frame whose own glyphs
outgrow the atlas in groups of rows; a hub test of a host pane's chips; resize tests for the
sidebar, an open menu and 150%/200%; a long folder chip cut at its start; quire's `write_soon_then`,
`Column` and `BannerActionRow`. Left:
- [x] Placement id collisions: Ghostty now comes from our fork (github.com/PoHsuanLai/ghostty,
      branch `temor`), whose `INTERNAL_ID` placement key tells the terminal's ids from the
      program's; placeholders match the program's only, as Ghostty's own renderer does.
- [ ] A grouped GPU frame redraws the whole pane and copies each group's whole atlases; a dirty
      copy per group if a real screen ever needs it.
- [ ] The folder chip's room is counted in grid cells while it is drawn in the smaller code font;
      tune `CHIP_FRAME` (`view/folder_fit.rs`) if a chip is still cut at its end.
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b25-*.md`.

## Scrollback clears (PR #3, #4; 2026-10-16)
A running command's `CSI 3 J` erases only its own rows (Ghostty fork `eraseHistoryFrom`); the
person's `clear` (shell integrations print `ESC ] 7502;clear BEL` first) ends every anchor. Left:
- [x] `clear -x` (keep the scrollback) no longer prints the mark (nushell: `clear -k`); its
      `CSI 2 J` is handled as below.
- [x] Ctrl+L, or a program's `CSI 2 J`, on a screen that does not end at a prompt (libghostty blanks
      it in place): the anchors left on blank rows end, except the prompt being typed at (moved to
      the top, where the shell draws it again) and a running command's output (goes on from the
      top). The block machine takes the input mark the redrawn prompt sends again.
- [ ] Hardening: check an anchor against the row's `semantic_prompt` flag before a card is drawn
      from it, so a misplaced anchor draws nothing.
- [ ] Upkeep: the Ghostty fork (github.com/PoHsuanLai/ghostty, branch `temor`) carries two commits,
      the `INTERNAL_ID` placement key and `ghostty_terminal_erase_history_from`; rebase both when
      the pin moves to a newer Ghostty.

## Batch 26 (merged 2026-10-17: everything left that needs temor code)
Five lanes (core, ssh, ui, term, plugins) plus the orchestrator's clear fixes; most open boxes
above turned out already done and are ticked with where. Landed: proposals count their Asks and
tell an edit from a replacement, U+2800 reads as a space, pipelines named by their last command
(start time), a later `trap … EXIT` chained before .bash_logout, iTerm2 multipart files caught
before Ghostty drops them, ⌘F in a viewer pane, Edit Locally in a host pane's block menu, menu
shortcuts in the sidebar and more menus, a `host:path` with an IPv6 address told to use a HostName,
temor-remote's socket folder tightened, the edit copy kept while a conflict waits, and **P11
plugins** (crate `temor-plugins` on bayonet fe489cf: `row.facts` on the sidebar row, `open.provider`
rows in the bar, `block.lens` and `agent.adapter` host sides). Left:
- [x] Plugins, the drawing side (Batch 27): a header per plugin in the bar (`Section::Plugin`),
      plugin facts as chips (`ChipKind::Plugin`, kept on older headers), "Show with <plugin>" and
      "Hide" in the block menu, and the picture drawn under the block (at most 24 rows).
- [ ] A lens picture covers the rows below its block instead of pushing them down: the grid is one
      texture, so this needs a row gap in temor-render (`followups-b27-plugview.md`).
- [ ] Plugins: the built-in Claude Code and git adapters rewritten as plugins; detent's Extensions
      page (lists `Catalog::plugins()`/`problems()`, never reachable from an agent); a sandbox.
- [ ] Ubuntu 24.04+ user namespaces → `NoSandbox`: belongs beside bulkhead's runner, which temor does
      not link yet (P8).
- [x] Done while a save is checking or uploading waits for it and closes after, as an editor does;
      a save that does not get there keeps the copy and says so (owner: "standard behavior").
- [ ] No end-to-end test of iTerm2 multipart files through temor-pane's reader; ⌘F in a viewer pane
      is unit-tested only without a viewer.
- Lane notes in full: `/mnt/data/cargo/temor-lanes/followups-b26-*.md`.

## quire v0.3.33 (2026-10-18)

- [x] A save that found `settings.toml` damaged moves it aside (`Saved::kept_at`,
      `<name>.bad-<seconds>`) before writing a new one, and the person is told where it went: a
      banner with Show in Folder in the Settings window, the window's notice for the shell offer's
      "never for", and the launch notes for the migration (`settings::kept_aside`).
- [x] The launch notes reach the person: the first window's notice says, in plain words
      (`settings::Told`), that the file could not be read, which settings are back to their
      defaults (by their Settings window labels), how many shortcuts could not be used, and where
      a converted `terminal.conf` went. The exact lines still go to stderr for the log. A key the
      version does not read is not mentioned.
- [ ] The edge strip's click may not pin a hidden sidebar, depending on how the frames fall
      (quire `EdgePeek` with `HoverIntent`). `sidebar::a_hidden_sidebar_peeks_from_the_edge_and_a_click_pins_it`
      passes 10 of 10 when the harness advances 600–900 ms per step; driven in 25 ms steps (waiting
      until the press would land on `.ds-edge`), the click leaves it peeking, unpinned, 8 runs in 10.
      The test keeps its steps and waits on the pin itself. Raised with Whopper for quire.
- [ ] `blocks::shells::real_shells_make_one_block_per_command_with_its_output_and_status` failed once
      at its block comparison (shells.rs:27) in six full `it` runs; 8 runs alone passed. The message
      was not kept; catch it the next time it shows.
- [x] (done 2026-10-20: the test changed the host's file before the save's own stat was read, so the
      new base took the change in; it now waits for the edit to settle first) `hub::ssh_tests::edit::a_saved_copy_is_written_back_and_a_change_on_the_host_asks_first` timed
      out once in a full gate ("timed out waiting for the question", 20 s); the lib suite passed 3 of
      3 and the edit tests 5 of 5 after. Separately, every `hub::ssh_tests::edit` test fails when
      TMPDIR is long (the ssh sockets outgrow 107 bytes), as the window tests did before
      `support::socket_root`; the lib tests want the same fit.
- [x] (2026-10-20, owner: remove the flaky tests) `ports::a_listener_in_a_session_is_reported_and_forgotten`
      (temor-remote) timed out once on Fedora CI waiting for a listener to go after Ctrl-C; it is
      removed. A later connection being told the known ports is still tested; forgetting a stopped
      listener has no end-to-end test now.
- [ ] `temor -e` (run mode): a tab that ran a program comes back as a shell when the session is
      restored after a restart; the session file does not keep what it ran. Decide whether a run
      tab is saved at all.
- [ ] Checkpoint.WatchAgent (agent's docket branch hd4-terminal-handoff): call it in place of
      Checkpoint.Watch when temor starts or recognises an agent-app session, and mark on
      `TerminalRequest.handoff` for a window the companion opened. Waits for hd4 on docket master
      and a batch that carries it.
- [ ] `org.quire.Temor1.Run` answers only temor's installed image (peephole, `Exe`, pinned at
      connect; `instance::CALLERS`). A temor the person built is refused, so a checkout's
      `temor -e` aimed at a running checkout temor is refused too, and that launch then starts as
      a window of its own (`join` treats a failed claim as no desktop). If that gets in the way of
      development, peephole's developer rows are the seam, in debug builds only.
- [ ] `temor -e` waits for its tab (Temor1.Run returns a ticket; `RunEnded(ticket, status)` goes to
      the waiting launch alone). When no temor runs and the bus cannot start one (no activation
      file: a checkout, or no session bus), the launch is the window itself and lives until temor
      quits, not until its tab closes. Installed temor has the activation file, so this is the
      checkout's case only.
- [ ] Releases (`scripts/package.sh`, `scripts/install.sh`): the tarball is built for this
      computer's processor only, so an x86_64 release has no aarch64 tarball and no
      `temor-remote-aarch64` for arm hosts (a cross build, or an arm runner, is the next step).
      There is no macOS or Windows release; `install.sh` sends those to `scripts/dev-setup.sh`,
      which is untested on both (their desktop layer is stubs). `--uninstall` removes the files it
      installed and leaves the folders they were in.
