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

## Platform (owner): OS-specific stubs to implement on real machines
Each one sits behind a `platform/` edge trait, marked `// TODO(platform:<os>)`.
- [ ] macOS keymap: the primary modifier is Cmd (bind Cmd chords, not keycap's Ctrl+Shift). §6
- [ ] macOS emoji: Apple Color Emoji is sbix (bitmap); add a bitmap glyph path to temor-render. §5
- [ ] macOS pty: `Pty::spawn` and `run_session_leader` (platform/unsupported.rs); process name, cwd,
      current user and `is_executable` (platform/macos.rs) through `proc_pidinfo`. temor-pty
- [ ] macOS trial sandbox: none (bwrap is Linux-only), so proposals say "not checked". Decide whether
      anything replaces it. §8.3
- [ ] Ubuntu 24.04+: unprivileged user namespaces are restricted by AppArmor; detect this, report
      `NoSandbox`, and document the profile. §8.3
- [ ] macOS: verify quire windows and IME (winit) and fontique's CoreText path; consider a
      `cargo check --target aarch64-apple-darwin` CI job (Zig can cross-build libghostty).
- [ ] Windows: PTY (ConPTY; temor-pty platform/windows.rs and unsupported.rs) and the plugin process
      side are unsupported today.
