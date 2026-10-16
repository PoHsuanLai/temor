#!/bin/sh
# Fetch the pinned Ghostty tree and fill an offline Zig package store for it.
#
#   scripts/fetch-ghostty.sh <dest>
#
# Afterwards <dest>/src is the Ghostty source at the pinned commit and <dest>/zig-store/p holds every
# Zig package its build graph resolves, extracted (39 packages, about 540 MB at the pin). Build offline with:
#
#   export GHOSTTY_SOURCE_DIR=<dest>/src
#   export GHOSTTY_ZIG_SYSTEM_DIR=<dest>/zig-store/p
#   export ZIG_GLOBAL_CACHE_DIR=<a writable dir outside $HOME>
#
# This is the only step that needs the network. Keep the commit in step with GHOSTTY_COMMIT in
# src/lib.rs and README.md.
set -eu

GHOSTTY_REPO="https://github.com/PoHsuanLai/ghostty.git"
GHOSTTY_COMMIT="a5df6bc683d47342b773eb113df10e36acd42da9"
ZIG_WANT="0.16"

if [ $# -ne 1 ]; then
  echo "usage: $0 <dest>" >&2
  exit 2
fi
dest=$1

zig_have=$(zig version 2>/dev/null || true)
case "$zig_have" in
  "$ZIG_WANT".*) ;;
  *) echo "need Zig $ZIG_WANT.x on PATH (found '${zig_have:-none}')" >&2; exit 1 ;;
esac

mkdir -p "$dest"
dest=$(CDPATH='' cd -- "$dest" && pwd)
src="$dest/src"
if [ -d "$src/.git" ]; then
  git -C "$src" fetch --filter=blob:none origin "$GHOSTTY_COMMIT"
else
  git clone --filter=blob:none --no-checkout "$GHOSTTY_REPO" "$src"
fi
git -C "$src" -c advice.detachedHead=false checkout --force "$GHOSTTY_COMMIT"
test "$(git -C "$src" rev-parse HEAD)" = "$GHOSTTY_COMMIT"

# Ghostty's script fetches every URL in build.zig.zon.txt, lazy dependencies included; lib-vt's
# build graph needs the whole store, not only its own packages. Zig 0.16 keeps the tarballs in the
# global cache and extracts each package into the project's zig-pkg/, so the extracted packages are
# then moved out of the source tree into the store that `zig build --system` reads.
store="$dest/zig-store"
fetch_cache="$dest/zig-fetch-cache"
rm -rf "$store" "$fetch_cache" "$src/zig-pkg"
mkdir -p "$store" "$fetch_cache"
# `zig fetch` wants a build.zig in the working directory, so run it from the tree.
(cd "$src" && ZIG_GLOBAL_CACHE_DIR="$fetch_cache" sh nix/build-support/fetch-zig-cache.sh)
mv "$src/zig-pkg" "$store/p"
rm -rf "$fetch_cache"

echo
echo "export GHOSTTY_SOURCE_DIR=$src"
echo "export GHOSTTY_ZIG_SYSTEM_DIR=$store/p"
