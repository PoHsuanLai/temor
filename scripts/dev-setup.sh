#!/bin/sh
# Set up a computer to build temor from source: check the tools, fetch the pinned Zig and Ghostty,
# write an env file, and build.
#
#   scripts/dev-setup.sh [--deps <dir>] [--no-build] [--system-linker]
#   curl -fsSL https://raw.githubusercontent.com/PoHsuanLai/temor/main/scripts/dev-setup.sh | sh
#
# Run from a checkout, or from anywhere to clone one into ./temor first. Linux, macOS, and Windows
# from Git Bash (MSYS2). Only Linux has a working desktop layer today; on macOS and Windows the
# workspace builds against stubs (crates/temor-desktop/src/platform), which is where that work goes.
#
# Zig and Ghostty go to <deps> (default <checkout>/.deps, about 1 GB, ignored by git), the build
# to $CARGO_TARGET_DIR (default <checkout>/target). Afterwards, in a new shell:
#
#   . <checkout>/.deps/env.sh
#   cargo run -p temor
#
# Running it again is cheap: what is already fetched at the pin is kept. --system-linker links with
# the system's cc in place of mold through clang (.cargo/config.toml), so neither is needed; the
# release build uses it.
set -eu

REPO="https://github.com/PoHsuanLai/temor.git"

deps=""
build=1
system_linker=0
while [ $# -gt 0 ]; do
  case "$1" in
    --deps) deps=$2; shift 2 ;;
    --no-build) build=0; shift ;;
    --system-linker) system_linker=1; shift ;;
    -h | --help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

say() { printf '==> %s\n' "$*"; }
die() { printf 'dev-setup: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  *) die "no setup for $(uname -s) yet" ;;
esac

# What to install for each missing piece, by the package manager this computer has.
hint() {
  case "$os" in
    macos) echo "xcode-select --install   (git, a C compiler, python3); brew install pkg-config" ;;
    windows) echo "install Visual Studio Build Tools (C++ workload), Python 3, and, in Git Bash, unzip" ;;
    linux)
      if have dnf; then echo "sudo dnf install git gcc clang mold python3 pkgconf-pkg-config fontconfig-devel freetype-devel xz unzip"
      elif have apt-get; then echo "sudo apt-get install git build-essential clang mold python3 pkg-config libfontconfig-dev libfreetype-dev xz-utils unzip"
      elif have pacman; then echo "sudo pacman -S --needed git base-devel clang mold python pkgconf fontconfig freetype2 xz unzip"
      elif have zypper; then echo "sudo zypper install git gcc clang mold python3 pkg-config fontconfig-devel freetype2-devel xz unzip"
      else echo "install git, a C compiler, clang, mold, python3, pkg-config, and the fontconfig and freetype development files"
      fi ;;
  esac
}

missing=""
# python3 runs the style crate's code generator (stylo).
for tool in git curl python3; do have "$tool" || missing="$missing $tool"; done
have cc || have gcc || have clang || [ "$os" = windows ] || missing="$missing cc"
case "$os" in
  linux)
    have pkg-config || missing="$missing pkg-config"
    have xz || missing="$missing xz"
    # .cargo/config.toml links x86_64 Linux with mold through clang.
    if [ "$(uname -m)" = x86_64 ] && [ "$system_linker" = 0 ]; then
      have clang || missing="$missing clang"
      have mold || missing="$missing mold"
    fi
    if have pkg-config; then
      for lib in fontconfig freetype2; do
        pkg-config --exists "$lib" || missing="$missing $lib(dev)"
      done
    fi ;;
  macos) have xz || tar --version 2>/dev/null | grep -q bsdtar || missing="$missing xz" ;;
  windows) have unzip || missing="$missing unzip" ;;
esac
[ -z "$missing" ] || die "missing:$missing
  $(hint)"

# Rust comes from rustup; the checkout's rust-toolchain.toml picks the version.
if ! have cargo; then
  [ -x "$HOME/.cargo/bin/cargo" ] || {
    say "installing rustup (the toolchain manager)"
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
  }
  PATH="$HOME/.cargo/bin:$PATH"
fi

# The checkout: this script's own, the current folder, or a fresh clone.
root=""
here=$(CDPATH='' cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || true)
for dir in "${here%/scripts}" "$PWD"; do
  if [ -n "$dir" ] && [ -f "$dir/crates/temor-vt-sys/scripts/fetch-ghostty.sh" ]; then root=$dir; break; fi
done
if [ -z "$root" ]; then
  [ -e temor ] && die "./temor is there but is not a temor checkout"
  say "cloning temor into $PWD/temor"
  git clone "$REPO" temor
  root=$PWD/temor
fi
root=$(CDPATH='' cd -- "$root" && pwd)
deps=${deps:-$root/.deps}
mkdir -p "$deps"
deps=$(CDPATH='' cd -- "$deps" && pwd)
case "$deps" in "$root"/*) grep -qxF "/${deps#"$root"/}" "$root/.git/info/exclude" 2>/dev/null \
  || echo "/${deps#"$root"/}" >>"$root/.git/info/exclude" ;; esac

scripts=$root/crates/temor-vt-sys/scripts
zig_dir=$(sh "$scripts/fetch-zig.sh" "$deps/zig" | sed -n 's/^export PATH=\(.*\):\$PATH$/\1/p')
[ -n "$zig_dir" ] || die "fetch-zig.sh did not say where Zig went"
PATH="$zig_dir:$PATH"

pin=$(cat "$root/crates/temor-vt-sys/ghostty.pin")
if [ "$(git -C "$deps/ghostty/src" rev-parse HEAD 2>/dev/null)" = "$pin" ] && [ -d "$deps/ghostty/zig-store/p" ]; then
  say "Ghostty $pin is already fetched"
else
  say "fetching Ghostty $pin and its Zig packages (about 540 MB)"
  sh "$scripts/fetch-ghostty.sh" "$deps/ghostty" >/dev/null
fi

env_file=$deps/env.sh
cat >"$env_file" <<EOF
# temor's build environment, written by scripts/dev-setup.sh. Source it: . $env_file
export PATH="$zig_dir:\$HOME/.cargo/bin:\$PATH"
export GHOSTTY_SOURCE_DIR="$deps/ghostty/src"
export GHOSTTY_ZIG_SYSTEM_DIR="$deps/ghostty/zig-store/p"
export ZIG_GLOBAL_CACHE_DIR="$deps/zig-cache"
EOF
# Cargo's environment form of target.<triple>.linker and .rustflags wins over the config file (an
# empty one does not), and the last -fuse-ld wins over rustc's own lld: GNU ld, the system's.
[ "$system_linker" = 0 ] || cat >>"$env_file" <<EOF
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=bfd"
EOF
. "$env_file"
say "wrote $env_file"

if [ "$build" = 1 ]; then
  say "building temor (the first build takes a while: it compiles Ghostty too)"
  (cd "$root" && cargo build -p temor -p temor-remote --bins)
fi

cat <<EOF

temor is set up in $root. In a new shell:

  . $env_file
  cd $root && cargo run -p temor
EOF
[ "$os" = linux ] || cat <<EOF

On $os the desktop layer is still stubs (crates/temor-desktop/src/platform); the workspace
builds and its tests run, and the window is the next piece of work there.
EOF
