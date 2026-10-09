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

## quire v0.3.0 (960ac8fa) — bump in R1, features in P3
- [ ] Bump the pin to `rev = "960ac8faa0166eba83f5f09a72ba063f79d35c06"  # v0.3.0` (breaking: `launch`
      returns a Result, seams behind `ds-blitz/testing`, settings setters; v0.2.31 shipped
      SplitView axis, RawKey/RawKeySurface, terminal palette tokens, public `focus_soon_told` /
      `focus_landing_told`, RootExtent::Window, IME composition). Drop temor's IME machine copy.
- [ ] IME candidate area: `ImeHost::cursor_area(surface, Rect)` via the document host's `.ime()`
      with the grid's MountedData and caret cell rect; repro to whopper if it doesn't move. §6
- [ ] Theme: re-apply the palette in an effect on `ds_settings::use_environment(..)`; Space theme and
      runtime window title are queued in quire. §5
- [ ] Add quire's coherence test (examples/consumer/tests/it/coherence.rs).
- [ ] Keys (PLAN §6, answered by whopper): bind each ⌘ action to both Ctrl+Shift+key and Super+key;
      ⌘ chord = `command_keys(mods)` has META without CONTROL, never `is_command` in a pane. Tests:
      Ctrl+Shift+T and Super+T open a tab, Ctrl+C reaches the PTY. temor-core terminal/chord.rs and
      keymap.rs.

## temor-ui / temor (lane/app, P1, reviewed at f783a2a)
- [ ] quire v0.2.31: replace plain layout divs and the 100vh root with quire layout components and
      `RootExtent::Window`; terminal palette tokens; re-apply the palette on theme change. §5
- [ ] quire asks (whopper offline; saved in /mnt/data/cargo/temor-lanes/pending-quire-asks.md): IME
      cursor area API, runtime window title, theme-change signal, consumer coherence test. §6
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
