#!/bin/sh
# Install temor from its latest release:
#
#   curl -fsSL https://github.com/PoHsuanLai/temor/releases/latest/download/install.sh | sh
#
# Options (after `sh -s --` when piped):
#   --prefix <dir>      where to install (default /usr/local; asks for sudo when it is not yours)
#   --version <tag>     a release other than the latest, such as v0.1.0
#   --from <tarball>    install a tarball already on disk instead of downloading one
#   --uninstall         remove what an earlier install put under <prefix>
#
# Linux only for now. temor trusts a `temor -e` hand-off only from a root-owned temor in /usr/bin
# or /usr/local/bin, so install there (the default) for `-e` and for starting from the desktop's
# bus; another prefix gives a temor that runs, without those.
set -eu

REPO=PoHsuanLai/temor
prefix=/usr/local
version=latest
from=""
uninstall=0
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix=$2; shift 2 ;;
    --version) version=$2; shift 2 ;;
    --from) from=$2; shift 2 ;;
    --uninstall) uninstall=1; shift ;;
    -h | --help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

say() { printf '==> %s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

case "$(uname -s)" in
  Linux) ;;
  Darwin | MINGW* | MSYS* | CYGWIN*)
    die "there is no temor release for $(uname -s) yet. To build it from source:
  curl -fsSL https://raw.githubusercontent.com/$REPO/main/scripts/dev-setup.sh | sh" ;;
  *) die "no temor for $(uname -s)" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) die "no temor release for this processor ($(uname -m))" ;;
esac

case "$prefix" in /*) ;; *) die "--prefix must be an absolute path" ;; esac
prefix=${prefix%/}
manifest=$prefix/share/temor/installed-files

# Root only when the prefix is not ours to write.
probe=$prefix
while [ ! -e "$probe" ]; do probe=$(dirname "$probe"); done
if [ -w "$probe" ] && { [ ! -e "$manifest" ] || [ -w "$manifest" ]; }; then
  as_root() { "$@"; }
elif [ "$(id -u)" = 0 ]; then
  as_root() { "$@"; }
elif have sudo; then
  say "installing to $prefix needs sudo"
  as_root() { sudo "$@"; }
else
  die "$prefix is not writable and there is no sudo; pass --prefix <a folder of yours>"
fi

# Refresh what the desktop caches about installed apps; none of it is required to run temor.
refresh() {
  have gtk-update-icon-cache && [ -f "$prefix/share/icons/hicolor/index.theme" ] \
    && as_root gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" || true
  have update-desktop-database && as_root update-desktop-database -q "$prefix/share/applications" || true
  # The user's systemd reads units only from the standard prefixes.
  case "$prefix" in
    /usr | /usr/local) have systemctl && [ "$(id -u)" != 0 ] && systemctl --user daemon-reload 2>/dev/null || true ;;
  esac
}

if [ "$uninstall" = 1 ]; then
  [ -f "$manifest" ] || die "nothing installed by this script under $prefix"
  say "removing temor from $prefix"
  while IFS= read -r file; do
    case "$file" in "$prefix"/*) as_root rm -f "$file" ;; esac
  done <"$manifest"
  as_root rm -f "$manifest"
  as_root rmdir "$prefix/share/temor" 2>/dev/null || true
  refresh
  say "temor is removed"
  exit 0
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

asset=temor-$arch-linux.tar.gz
if [ -n "$from" ]; then
  [ -f "$from" ] || die "no such file: $from"
  cp "$from" "$tmp/$asset"
  [ ! -f "$from.sha256" ] || sed "s|  .*|  $asset|" "$from.sha256" >"$tmp/$asset.sha256"
else
  have curl || die "needs curl"
  case "$version" in
    latest) base=https://github.com/$REPO/releases/latest/download ;;
    *) base=https://github.com/$REPO/releases/download/$version ;;
  esac
  say "downloading $asset"
  curl -fL --proto '=https' --tlsv1.2 -# -o "$tmp/$asset" "$base/$asset"
  curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/$asset.sha256" "$base/$asset.sha256"
fi
if [ -f "$tmp/$asset.sha256" ]; then
  (cd "$tmp" && sha256sum -c --quiet "$asset.sha256") || die "$asset does not match its checksum"
elif [ -z "$from" ]; then
  die "the release has no checksum for $asset"
fi

mkdir "$tmp/x"
tar -xzf "$tmp/$asset" -C "$tmp/x"
pkg=$(find "$tmp/x" -mindepth 1 -maxdepth 1 -type d | head -1)
[ -x "$pkg/bin/temor" ] || die "$asset holds no bin/temor"
share=$pkg/share

# The activation files and the user unit name the program by its full path.
for file in "$share/org.quire.Temor.service" "$share/org.quire.Temor1.service" "$share/systemd/org.quire.Temor.service"; do
  sed -i "s|=/usr/bin/temor\$|=$prefix/bin/temor|" "$file"
done

# Every file this installs, as `<mode> <from> <to>`.
{
  echo "755 $pkg/bin/temor $prefix/bin/temor"
  echo "755 $pkg/bin/temor-remote $prefix/bin/temor-remote"
  echo "644 $share/org.quire.Temor.desktop $prefix/share/applications/org.quire.Temor.desktop"
  echo "644 $share/org.quire.Temor.service $prefix/share/dbus-1/services/org.quire.Temor.service"
  echo "644 $share/org.quire.Temor1.service $prefix/share/dbus-1/services/org.quire.Temor1.service"
  echo "644 $share/systemd/org.quire.Temor.service $prefix/lib/systemd/user/org.quire.Temor.service"
  echo "644 $share/intents/org.quire.Temor.toml $prefix/share/quire/intents/org.quire.Temor.toml"
  for icon in "$share"/icons/hicolor/*/apps/*; do
    echo "644 $icon $prefix/share/icons/hicolor/${icon#"$share/icons/hicolor/"}"
  done
} >"$tmp/plan"

say "installing temor to $prefix"
: >"$tmp/installed-files"
while read -r mode src dst; do
  as_root install -D -m "$mode" "$src" "$dst"
  echo "$dst" >>"$tmp/installed-files"
done <"$tmp/plan"
as_root install -D -m 644 "$tmp/installed-files" "$manifest"
refresh

release=${pkg##*/temor-}
say "temor ${release%-"$arch"-linux} is installed"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "    $prefix/bin is not on PATH; add it to run \`temor\`" ;; esac
case "$prefix" in
  /usr | /usr/local) ;;
  *) echo "    temor -e and starting from the desktop need an install to /usr/local (the default)" ;;
esac
more=""
[ "$prefix" = /usr/local ] || more=" --prefix $prefix"
echo "    to remove it: curl -fsSL https://github.com/$REPO/releases/latest/download/install.sh | sh -s -- --uninstall$more"
