#!/usr/bin/env bash
# Checks that temor's [patch.crates-io] block matches quire's canonical docs/fork-patches.toml at
# the quire rev pinned in Cargo.toml. Nothing is vendored: quire's own fork-patches.sh is run from
# the cargo git checkout of that rev, or, if cargo has not fetched it, from `git show` of the rev.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/Cargo.toml"
quire_repo="${QUIRE_REPO:-$HOME/quire}"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"

rev=$(sed -n 's/^ds-core = { git = ".*quire\.git", rev = "\([0-9a-f]\{40\}\)".*/\1/p' "$manifest")
if [ -z "$rev" ]; then
  echo "check-fork-patches: no quire rev found in $manifest" >&2
  exit 2
fi
short=${rev:0:7}

script=""
for candidate in "$cargo_home"/git/checkouts/quire-*/"$short"/scripts/fork-patches.sh; do
  if [ -f "$candidate" ] && [ -f "$(dirname "$(dirname "$candidate")")/docs/fork-patches.toml" ]; then
    script=$candidate
    break
  fi
done

if [ -z "$script" ]; then
  # The script finds docs/fork-patches.toml beside itself, so both files go into one temp tree.
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  mkdir -p "$tmp/scripts" "$tmp/docs"
  if ! git -C "$quire_repo" show "$rev:scripts/fork-patches.sh" >"$tmp/scripts/fork-patches.sh" 2>/dev/null \
    || ! git -C "$quire_repo" show "$rev:docs/fork-patches.toml" >"$tmp/docs/fork-patches.toml" 2>/dev/null; then
    echo "check-fork-patches: quire $short is neither in the cargo git checkouts nor in $quire_repo" >&2
    exit 2
  fi
  script="$tmp/scripts/fork-patches.sh"
fi

echo "check-fork-patches: quire $short ($script)"
bash "$script" check "$manifest"
