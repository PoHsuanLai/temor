# Spike P0(c): the input path

Date: 2026-10-09. Machine: Fedora 44, KDE Plasma (KWin Wayland), fcitx5 as KWin's input method
(keyboard-us + chewing, trigger Ctrl+Space). Rust 1.98.1. Plan refs: D1, §4b, §6, §11 P0(c).

## Verdict

**Go.** Real keyboard and IME events in a quire window reach libghostty's `KeyEncoder`, the bytes
reach a real bash on a PTY, and the output comes back through libghostty onto the screen. The
kitty keyboard protocol, bracketed paste, SGR mouse and DECCKM all follow what the app pushed,
through `set_options_from_terminal`. Nothing needs a quire change to work. A few quire seams are
crate-private, and Blitz drops winit's `text` and `key_without_modifiers`; temor works around both
today (asks below).

The input side is not the risk. The **desktop** is: KWin and fcitx5 take some keys before any
client sees them (see "Desktop findings").

## What was built

`spikes/input/` is a standalone Cargo project with its own `[workspace]` and toolchain 1.98.1. It
pins quire `366dcd97` (v0.2.30) by git with `.git`, with the `[patch.crates-io]` block from
quire's root. It uses `libghostty-vt` 0.2.2 from crates.io (Ghostty a887df4, Zig 0.15.2, offline
through `env.sh`). `CARGO_TARGET_DIR=/mnt/data/cargo/p0-temor/target-input`. No `unsafe`
(`unsafe_code = "deny"`).

| File | What |
|---|---|
| `src/app.rs` | The window: one terminal region. It is the focus, key, IME and paste host, plus the mouse and wheel, and draws cheaply. |
| `src/keymap.rs` | Pure mapping from a Blitz/Dioxus key event to a libghostty key event, plus the routing (PTY, paste, app shortcut). 5 unit tests. |
| `src/term.rs` | libghostty `Terminal` plus its key and mouse encoders, the PTY, and a log of every byte sent and why. |
| `src/pty.rs` | `rustix` openpt/grantpt/unlockpt, then `env -i HOME=<scratch> … setsid -c bash --norc --noprofile -i`. `setsid -c` makes the slave the controlling tty, so no `pre_exec`/`unsafe` is needed. Resize is `tcsetwinsize` (TIOCSWINSZ). A reader thread feeds a tokio channel. |
| `src/ime.rs` | A copy of quire's `on_ime`/`settle` composition machine (it is `pub(crate)` in quire). |
| `tests/harness.rs` | The event-level tier: the same `App` in ds-harness, with a real PTY and bash. 5 tests. |
| `scripts/uinput_keys.py` | The real-OS-input driver: a uinput virtual keyboard with a focus guard. |
| `scripts/probe.py` | Runs inside the shell. It turns a mode on (kitty, kitty 31, mouse, bracketed paste, DECCKM), prints every stdin chunk raw, then turns the mode off. |
| `scripts/run_live.sh`, `steps/*.txt` | One live run: open the window (it closes itself after 6–15 s), drive it, keep the log. |

Logs for each run are in `/mnt/data/cargo/p0-temor/input-runs/<run>/log.txt`. Each one holds every
raw key event, every encoder input and output, every IME event, the status changes, and the final
viewport plus the formatter's plain text.

Commands:

```sh
cd spikes/input && . ./env.sh
cargo build && cargo test --lib                     # keymap tests
cargo test --test harness -- --test-threads=1       # event-level tier, about 7 s
scripts/run_live.sh kitty steps/kitty.txt 7         # one live run on the desktop
```

The shell always runs under `env -i` with HOME, HISTFILE and the working directory in a scratch dir
under `/mnt/data/cargo/p0-temor/input-runs/`. Its environment is `TERM=xterm-256color`,
`PATH=/usr/bin:/bin`, `LANG=C.UTF-8` and `PS1='$ '`. `~/.bash_history` was not touched (its mtime
predates every run).

## Synthetic input: what worked

- **`wtype` fails on KWin:** "Compositor does not support the virtual keyboard protocol". KWin
  offers `zwp_virtual_keyboard_v1` only to its input method.
- **The real-OS path used is uinput instead.** The user is in group `input`, and `/dev/uinput`
  is group-writable. `uinput_keys.py` creates a virtual keyboard, so keys travel kernel → libinput →
  KWin → fcitx5 (when text-input is on) → `wl_keyboard` → winit → Blitz → the region. This is more
  real than wtype: it also exercises fcitx5's key forwarding and KWin's global shortcuts.
- **Focus guard.** uinput keys go to whatever has the keyboard on the owner's live desktop.
  - Nothing but a lone Shift tap is sent until the window reports, in a status file, that it heard
    that Shift.
  - Before every key, the window must report `active=1 focus=1`.
  - After every key, the window must acknowledge it within 0.6 s: its count of key-down events
    must rise. Keys the IME takes are acknowledged by its count of IME events instead.
  - Otherwise the run stops at once.
- **Incident (run `basics2`, before the acknowledgement existed).**
  - About 3 s in, KWin's **Overview** opened. Its search field grabbed the keyboard without the
    window losing activation; the window heard `Ime::Disabled` and deactivation only 3 s later.
  - The rest of that script, about 45 keystrokes, went into the Overview search instead of the
    window. That included several Enters and Meta+Y; the text was "ho two", "echo abc def",
    "echo tail" and similar.
  - The journal shows only `Error requesting matches; org.kde.runners.baloo`, and no launched app.
  - The owner may want to glance at KRunner history. Every run since then used the
    acknowledgement guard, and it stopped `basics3` cleanly after the first unacknowledged key.
- **Mouse was not driven with real OS input.** It would mean moving the owner's pointer, and the
  window's position is unknown on Wayland.
- **Paste was not driven with real OS input.** It would paste the owner's real clipboard into the
  shell and the log.

## Verification

Legend: **Real**: real OS input on the live KDE Wayland desktop (uinput, KWin, fcitx5, winit).
**Harness**: ds-harness dispatching Blitz events into the same `App` with a real PTY.

| Check | Real | Harness | Evidence (run: what the shell got or showed) |
|---|---|---|---|
| `echo hi` + Enter shows output | **yes** | yes | `basics`, `basics3`: `$ echo hi` / `hi` |
| Ctrl-C interrupts `sleep 10` | **yes** | yes | `basics`, `basics3`: `^C` then a new prompt well inside the 10 s (the next command ran 0.4 s later); encoder `\x03` |
| Arrows and readline history | **yes** | yes (+ DECCKM) | `history`: Up Up Enter re-ran `echo one`. Harness: in a program with `CSI ? 1 h`, Up → `\eOA` (from `set_options_from_terminal`) |
| Alt/Meta combos | **yes** | – | `alt`: Alt+B → `\eb` (gave `echo abc Xdef`); Ctrl+A then Alt+F → `\ef` (gave `echoQ tail`) |
| Kitty protocol `CSI > 1 u` | **yes** | – | `kitty`: probe got `a`, `A`, `\e[99;5u` (Ctrl+C), `\e[27u` (Esc), `\e[120;3u` (Alt+x), `\r`, `\t`, `\x7f`, `\e[A`, `\e[97;6u` (Ctrl+Shift+A). All match the kitty spec for flag 1 (Enter, Tab and Backspace stay legacy) |
| Kitty `CSI > 31 u` (all flags) | **yes** | – | `kitty31b`: `\e[97;;97u`, release `\e[97;1:3u`, Shift `\e[57441;2u` … `\e[57441;1:3u`, `\e[97:65;2;65u` (shifted key, alternate and text), `\e[57442;5u`, `\e[99;5:3u`. Matches kitty's own output after the modifier fix (problem 3) |
| Bracketed paste | no | yes | At bash's prompt (readline sets 2004): `\e[200~echo pasted\e[201~`, and it ran. In a raw program without 2004: `a\rb`, flagged `safe=false` by `paste::is_safe`. In a program that sets 2004: probe got `\x1b[200~x y\x1b[201~` |
| SGR mouse from clicks and wheel | no (one stray real click reached the handler, below) | yes | Probe in `?1000h ?1006h`. Click at cell (10,5): `\e[<0;11;6M` / `\e[<0;11;6m`. Right: `\e[<2;11;6M/m`. Wheel up `\e[<64;11;6M`, down `\e[<65;11;6M`. `EncoderSize` comes from the region's measured grid |
| IME preedit and commit | **yes (real composition)** | yes | `ime`: fcitx5 chewing over text-input-v3. Preedit `ㄓ` → `ㄓㄨ` → `住`, the cleared preedit, `Commit("住")`, written as UTF-8, `$ echo 住` / `住`. The keys the IME took never reached `onkeydown`. Harness: preedit drawn at the cursor, a key during composition ignored, commit `注` echoed |
| ⌘/Super chords not sent to the PTY | **yes** (Meta+Y) | yes (Super+T, Super+W) | `alt`: `app shortcut Modifiers(SUPER)+Character("y") (not sent to the shell)`, and no bytes written. ⌘V and Ctrl+Shift+V go to paste instead |
| Resize (TIOCSWINSZ) | yes (at layout) | yes | Region measured as 744×468 with cell 8.41×18, giving 88×26. `stty size` in bash prints `26 88` |
| Focus, and the IME switched on at focus | **yes** | yes | `focus in; ime switch on -> Some(())`, then the real `Ime::Enabled` from KWin |
| Tab stays in the terminal | **yes** | – | `kitty`: Tab → `\t` and later keys still acknowledged. `prevent_default` on keydown stops Blitz's focus traversal |

Real pointer input did reach the handler once. During the first smoke run a real click landed in
the window (the owner's, apparently): `mouse Press Some(Left) at=(688.4,347.3) tracking=false`,
and with no tracking mode the encoder correctly produced nothing. SGR reporting from a real mouse
was not verified.

## How a custom region gets its input (quire v0.2.30)

`EditSurface` is the model, but it cannot be used as is: it hands the app only `Key` plus
`Modifiers`. The region rebuilds it from the same public host seams:

- **Element.** A plain `div` with `tabindex=0`, `data-keys="capture"` and
  `data-wheel="capture"`. Its own `onkeydown` already makes blitz-kit's scroll engine leave keys
  alone (`key_focus`); `data-keys="capture"` says the same explicitly.
- **Focus.** Use `use_document_host().focus().focus(el)`, retried while `Busy`. Blitz's
  `set_focus_to` sends **no focus event**, so the region runs its own focus-in after the write
  succeeds. quire's `focus_soon_told` does exactly this, but it is `pub(crate)`. A press on the
  region focuses it the same way.
- **Raw keys.** Use `onkeydown` and `onkeyup` on the region.
  - Dioxus's `KeyboardEvent` has `key()` (logical), `code()` (physical), `modifiers()`,
    `location()` and `is_auto_repeating()`.
  - It has **no text and no unshifted key**: Blitz's `BlitzKeyEvent.text` is dropped in
    dioxus-native-dom, and `BlitzKeyboardData` is `pub(crate)`, so it cannot be downcast.
  - Call `prevent_default` on every keydown.
- **IME.**
  - `host.edit().ime().listen(&el, sink)` once mounted. The window routes winit `Ime` to the
    surface that has the focus.
  - `switch(&el, On)` at focus, `Off` at blur.
  - `cursor_area(&el, cursor cell rect)` at focus and after every output.
  - Events go through the `on_ime` machine. While composing, keydowns are ignored, as in
    EditSurface; `settle` runs before a key and at blur.
- **Paste.** On ⌘V or Ctrl+Shift+V, `host.edit().paste()` returns `Pasted::Text` or `Html`. Then:
  - check `paste::is_safe` (temor confirms here, §6)
  - `paste::encode(…, bracketed = mode(2004))`
  - write to the PTY

  Ctrl+V stays a key, readline's quoted-insert.
- **Mouse.**
  - Use `onpointerdown`, `onpointerup` and `onpointermove` on the **containing element**; the rows
    have `pointer-events: none`. That is the §5 workaround for the custom-widget click bug, which
    is lost until v0.2.31.
  - Positions are `client_coordinates()` minus the region's measured origin.
  - `EncoderSize` holds whole numbers while CSS cells are fractional (8.41 px). The spike scales
    positions and sizes by 100. The real renderer snaps cells to whole device pixels (§5), which
    removes the trick. SGR-pixels (1016) would then need real pixels.
- **Wheel.** Use `onwheel` with `data-wheel="capture"` and `prevent_default`. Blitz keeps
  winit's sign: positive dy is the content moving down, which is a wheel turned up and mouse
  button 4.

## Key-event mapping (winit → Blitz/Dioxus → libghostty `KeyEvent`)

| libghostty field | Source | Notes |
|---|---|---|
| `key` (`key::Key`, W3C-code-shaped) | Physical `Code`, one-to-one by name (`KeyA` → `A`, `Digit1` → `Digit1`, `BracketLeft`, `Numpad*`, `F1`–`F24`, `ArrowUp`, `ControlLeft`, …). `MetaLeft`/`MetaRight` → `MetaLeft`/`MetaRight` | When `Code::Unidentified` (ds-harness sends that for punctuation), fall back to the logical key on a US layout |
| `mods` | `Modifiers::SHIFT/CONTROL/ALT` → `SHIFT/CTRL/ALT`, plus `CAPS_LOCK`, `NUM_LOCK` | `SUPER`/`META` never reach the encoder: they route to the app. **Modifier keys themselves:** winit sends the key before `ModifiersChanged`, so a Shift press arrives without Shift and its release with it. temor sets the key's own bit on press and clears it on release (kitty: `57441;2u` … `57441;1:3u`) |
| side bits | `Location::Right` of the modifier key itself | The side of *other* held modifiers is unknown: winit has no sides on Wayland |
| `consumed_mods` | `SHIFT` when Shift is held and the event has text | |
| `utf8` (text) | `Key::Character(s)` when Ctrl is not held and `s` has no control chars | Alt keeps its text (Alt+b → `"b"`, and the encoder adds ESC). Ctrl: none (the encoder derives `\x03` from the key) |
| `unshifted_codepoint` | US-layout table by physical key (`Digit1` → `'1'`, `KeyA` → `'a'`, `Enter` → `'\r'`, `Backspace` → `0x7f`) | **Wrong on non-US layouts.** winit's `key_without_modifiers` is what is needed, and Blitz does not pass it |
| `action` | keydown → `Press`, or `Repeat` when `is_auto_repeating`; keyup → `Release` | Legacy encodes nothing for a release; kitty flag 2 does |
| `composing` | `false` (keys never reach the encoder while composing) | |
| Routing | ⌘ (Super) chord, or the Super key alone → app shortcut, never the PTY. ⌘V or Ctrl+Shift+V → paste. Anything else → `set_options_from_terminal`, then `encode`, then the PTY | |

Observed Blitz values (real input): `Key::Character("A")` with `SHIFT` and `KeyA`;
`Character("A")` with `CONTROL|SHIFT` for Ctrl+Shift+A; the Super key is `Key::Meta` with
`MetaLeft`, and chords carry `Modifiers::SUPER`. ds-harness's `ShortcutKey::Super` sets `META`
instead (problem 5).

## Problems

1. **Blitz drops winit's `text` and `key_without_modifiers`.** `BlitzKeyEvent.text` exists, but
   dioxus-native-dom's `BlitzKeyboardData` is `pub(crate)`. Without them the unshifted codepoint
   is a US-layout guess, and dead keys or compose sequences outside the IME have no text.
2. **Blitz's programmatic focus sends no focus event.** The app must run its own focus-in, as
   quire's crate-private `focus_soon_told` does.
3. **Modifier-key state lags** by one event (problem 1's sibling). Fixed in `keymap.rs`; without
   the fix, kitty `REPORT_ALL` reports were wrong.
4. **A focus race at startup.** Keys sent before the host focus write lands are dropped: in the
   harness, the first `p` of a command was lost. temor must focus the pane before it shows a
   prompt it expects typing into.
5. **ds-harness and the window disagree on Super:** `ShortcutKey::Super` sets `Modifiers::META`,
   while Blitz's winit conversion sets `Modifiers::SUPER`. temor checks both.
6. **`EncoderSize` holds integers**; see the mouse notes.
7. **`std::process::exit` from a task aborted** with "thread local panicked on drop". Ending the
   app through `AppHandle::quit()` exits cleanly.
8. **`GeometryHost::find(sel)` returned an element that `measure`/`client_rect` cannot measure**
   (`Unknown`). The span's own `onmounted` handle works.
9. **The window lost activation about 3 s after opening, in every live run** (`Ime::Disabled`,
   `Focused(false)`), probably another agent's window or KWin. Live runs had to fit in about 3 s.

## Desktop findings (for §6)

- **KWin eats ⌘ chords that temor's defaults use.** In the owner's `kglobalshortcutsrc`:
  - Meta+T is the Tiles editor (temor's ⌘T bar)
  - Meta+W is Overview (⌘W close)
  - Meta+D is Peek at Desktop (⌘D split)
  - Meta+V is the clipboard popup (⌘V paste)

  None of these reach a client. Before P1, temor needs the keycap terminal profile or a KDE
  shortcut plan. These chords were not sent live, deliberately.
- **fcitx5 swallows Ctrl+Space** (its trigger) whenever text-input is on, so the shell never gets
  NUL (Emacs set-mark). The terminal profile should cover it.
- **The KWin Overview can grab the keyboard while a window stays "active"** (the incident above).

## Quire asks

1. **Export the composition machine**: `ds::edit::composition::{on_ime, settle, Composing}` is
   `pub(crate)`, so temor copied it.
2. **Export `focus_soon_told`** (or give `FocusHost::focus` a "told" variant), so a custom surface
   hears that focus landed.
3. **A raw-keys surface**, `TerminalSurface` or `EditSurface { raw_keys: true }`. It would hand the
   app full key events: logical key, physical code, modifiers, location, repeat, **key-up**,
   **text**, and **key without modifiers**, plus `on_ime` steps and paste. It would keep the IME
   switching, cursor area and paste that EditSurface already does. That needs the Blitz-fork
   change in ask 4.
4. **Blitz fork (`quire-pin`)**: carry winit's `text` and `key_without_modifiers` into
   `BlitzKeyEvent`, and expose them to Dioxus. Either make `BlitzKeyboardData` public for
   `downcast`, or add accessors.
5. **ds-harness**:
   - `Input::RawKey(BlitzKeyEvent)`, for punctuation codes, F-keys, key-up only, repeat and
     location
   - make `ShortcutKey::Super` set `SUPER`, as the window does
   - `Input::Paste` presses Ctrl+V, which a terminal must not treat as paste; let the chord be
     chosen
6. **Measuring a `find()` result** should work like an `onmounted` handle, or be documented as not
   measurable.
7. Already queued, and confirmed relevant: clicks on custom widgets (v0.2.31) for the
   `TextureLayer` pane, and `use_pointer_capture` for drags that leave the pane (not used here).

## What P1 takes from this

- `keymap.rs` moves into temor as the key adapter, with its tests.
- `pty.rs`'s `setsid -c` trick keeps the PTY spawn free of `unsafe`.
- The status-and-acknowledgement guard in `uinput_keys.py` is the pattern for any future live-input
  test on the shared desktop.
- Display, threading (§4b's I/O thread needs P0(a)'s `Send` wrapper; here `vt_write` runs on the
  UI thread), resize on window resize, and scrollback on the wheel when no mouse mode is on were
  not part of this spike.
