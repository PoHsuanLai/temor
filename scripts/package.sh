#!/bin/sh
# Build temor's Linux release tarball: temor and temor-remote built on an old glibc (Ubuntu 22.04,
# glibc 2.35) so they run on most current distributions, with the desktop files beside them.
#
#   scripts/package.sh <work-dir> [<out-dir>]
#
# The build runs in a podman container with its storage, Rust, Zig, Ghostty and target folders all
# under <work-dir> (several GB; the first run fetches about 1 GB). The container builds through
# scripts/dev-setup.sh, so a release also checks that the from-source setup works on a clean
# system. It links with the system linker, never mold or clang. Writes <out-dir>/temor-<arch>-linux.tar.gz and its .sha256 (default <work-dir>/out),
# plus install.sh beside them, which is what a release carries.
set -eu

[ $# -ge 1 ] && [ $# -le 2 ] || { echo "usage: $0 <work-dir> [<out-dir>]" >&2; exit 2; }
command -v podman >/dev/null || { echo "package.sh needs podman" >&2; exit 1; }

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$1"
work=$(CDPATH='' cd -- "$1" && pwd)
out=${2:-$work/out}
mkdir -p "$out"
out=$(CDPATH='' cd -- "$out" && pwd)

version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml")
[ -n "$version" ] || version=$(git -C "$root" describe --tags --always)
arch=$(uname -m)
name=temor-$version-$arch-linux
image=docker.io/library/ubuntu:22.04

# The checkout is copied, not mounted, so the build sees exactly what is committed at HEAD.
src=$work/src
rm -rf "$src"
mkdir -p "$src" "$work/home" "$work/deps" "$work/target" "$work/storage" "$work/run"
git -C "$root" archive HEAD | tar -x -C "$src"
# dev-setup.sh writes its ignore line into the checkout's git; the copy is a checkout of its own.
git -C "$src" init -q

pod() { podman --root "$work/storage" --runroot "$work/run" "$@"; }

pod run --rm --network=host \
  -v "$src:/src:Z" -v "$work/home:/home/build:Z" -v "$work/deps:/deps:Z" -v "$work/target:/target:Z" \
  -e HOME=/home/build -e CARGO_TARGET_DIR=/target -e DEBIAN_FRONTEND=noninteractive \
  "$image" sh -euc '
    apt-get update -qq >/dev/null
    apt-get install -y -qq --no-install-recommends ca-certificates curl git build-essential python3 \
      pkg-config libfontconfig-dev libfreetype-dev xz-utils unzip >/dev/null
    git config --global --add safe.directory "*"
    cd /src
    sh scripts/dev-setup.sh --deps /deps --no-build --system-linker
    . /deps/env.sh
    cargo build -q --locked --release -p temor -p temor-remote --bins
  '

# A release links with the system linker (GNU ld); mold and lld sign the binaries they link.
for bin in temor temor-remote; do
  if readelf -p .comment "$work/target/release/$bin" 2>/dev/null | grep -qiE 'mold|lld'; then
    echo "$bin was not linked with the system linker" >&2
    exit 1
  fi
done

stage=$work/stage/$name
rm -rf "$work/stage"
mkdir -p "$stage/bin"
cp "$work/target/release/temor" "$work/target/release/temor-remote" "$stage/bin/"
cp -R "$root/dist" "$stage/share"
rm -rf "$stage/share/windows" "$stage/share/icons/source"
cp "$root/LICENSE"* "$stage/" 2>/dev/null || true

# The asset name carries no version, so releases/latest/download/<asset> always names the newest.
asset=temor-$arch-linux.tar.gz
tarball=$out/$asset
tar -C "$work/stage" -czf "$tarball" "$name"
(cd "$out" && sha256sum "$asset" >"$asset.sha256")
cp "$root/scripts/install.sh" "$out/install.sh"

glibc=$(objdump -T "$stage/bin/temor" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
echo "$tarball (temor needs $glibc or later)"
