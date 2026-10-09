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
