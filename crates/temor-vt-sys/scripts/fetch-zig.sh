#!/bin/sh
# Fetch the Zig that builds the pinned Ghostty, from ziglang.org, checked against its pinned SHA-256.
# For hosts whose package manager has no Zig 0.16 (Ubuntu 24.04 has none; others drift).
#
#   scripts/fetch-zig.sh <dest>
#
# Afterwards <dest>/zig-<arch>-<os>-<version>/zig is the compiler. Put its folder first on PATH:
#
#   export PATH=<dest>/zig-<arch>-<os>-<version>:$PATH
#
# Keep ZIG_VERSION in step with ZIG_MAJOR_MINOR in build.rs. The sums are ziglang.org's own
# (https://ziglang.org/download/index.json).
set -eu

ZIG_VERSION="0.16.0"

if [ $# -ne 1 ]; then
  echo "usage: $0 <dest>" >&2
  exit 2
fi

case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) echo "no pinned Zig for this machine ($(uname -m))" >&2; exit 1 ;;
esac
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) echo "no pinned Zig for this system ($(uname -s))" >&2; exit 1 ;;
esac

case "$arch-$os" in
  x86_64-linux) sum=70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00 ;;
  aarch64-linux) sum=ea4b09bfb22ec6f6c6ceac57ab63efb6b46e17ab08d21f69f3a48b38e1534f17 ;;
  x86_64-macos) sum=0387557ed1877bc6a2e1802c8391953baddba76081876301c522f52977b52ba7 ;;
  aarch64-macos) sum=b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489 ;;
esac

name="zig-$arch-$os-$ZIG_VERSION"
url="https://ziglang.org/download/$ZIG_VERSION/$name.tar.xz"

mkdir -p "$1"
dest=$(CDPATH='' cd -- "$1" && pwd)
if [ -x "$dest/$name/zig" ]; then
  echo "export PATH=$dest/$name:\$PATH"
  exit 0
fi

tarball="$dest/$name.tar.xz"
curl --fail --location --proto '=https' --tlsv1.2 --output "$tarball.part" "$url"
if command -v sha256sum >/dev/null 2>&1; then
  have=$(sha256sum "$tarball.part" | cut -d' ' -f1)
else
  have=$(shasum -a 256 "$tarball.part" | cut -d' ' -f1)
fi
if [ "$have" != "$sum" ]; then
  rm -f "$tarball.part"
  echo "the download's SHA-256 is $have, not the pinned $sum; nothing was unpacked" >&2
  exit 1
fi
mv "$tarball.part" "$tarball"
tar -xJf "$tarball" -C "$dest"
rm -f "$tarball"
test "$("$dest/$name/zig" version)" = "$ZIG_VERSION"

echo
echo "export PATH=$dest/$name:\$PATH"
