#!/usr/bin/env bash
# The end-to-end workflow test (crates/temor-ui/tests/it/workflow.rs): a local project, rsync to
# a scratch sshd on 127.0.0.1, a host tab, an agent, uploads and downloads. Not part of CI.
#
# Needs sshd, ssh, rsync and ssh-keygen, and the Ghostty build settings the rest of the build
# uses (GHOSTTY_SOURCE_DIR, GHOSTTY_ZIG_SYSTEM_DIR, ZIG_GLOBAL_CACHE_DIR). Nothing reads the real
# ~/.ssh or reaches another computer. `WF_STEPS=W3,W9` runs only those steps.
set -euo pipefail
cd "$(dirname "$0")/.."
# The host helper the test installs on the scratch host; `cargo test` does not build it.
cargo build -p temor-remote --features ghostty
exec env -u DISPLAY -u WAYLAND_DISPLAY -u XAUTHORITY \
    cargo test -p temor-ui --test it workflow -- --ignored --nocapture "$@"
