#!/bin/sh
# Build temor.app for macOS: the binary in an app bundle with its icon and Info.plist, signed ad hoc
# (no Developer ID, so Gatekeeper asks the person to allow it once), and zipped.
#
#   scripts/package-macos.sh [<out-dir>]
#
# Run it with the build environment from scripts/dev-setup.sh loaded (`. .deps/env.sh`). It builds
# in the `dist` profile (Cargo.toml); `PROFILE=dev` bundles a debug build in a minute instead, for
# trying the bundle itself. Writes <out-dir>/temor.app and <out-dir>/temor-<version>-<arch>-macos.zip
# with its .sha256 (default ./target/macos).
set -eu

[ "$(uname -s)" = Darwin ] || { echo "package-macos.sh runs on macOS" >&2; exit 1; }
for tool in cargo sips iconutil codesign ditto shasum; do
  command -v "$tool" >/dev/null || { echo "package-macos.sh needs $tool" >&2; exit 1; }
done

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
out=${1:-$root/target/macos}
mkdir -p "$out"
out=$(CDPATH='' cd -- "$out" && pwd)
profile=${PROFILE:-dist}

version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml")
[ -n "$version" ] || version=$(git -C "$root" describe --tags --always)
arch=$(uname -m)
name=temor-$version-$arch-macos

cd "$root"
cargo build -q --locked --profile "$profile" -p temor
target=${CARGO_TARGET_DIR:-$root/target}
dir=$profile
[ "$profile" = dev ] && dir=debug
bin=$target/$dir/temor
[ -x "$bin" ] || { echo "no $bin was built" >&2; exit 1; }

app=$out/temor.app
rm -rf "$app" "$out/$name.zip" "$out/$name.zip.sha256"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/temor"
sed "s/@VERSION@/$version/g" dist/macos/Info.plist > "$app/Contents/Info.plist"

# The icon: the 1024 px source at each size the iconset wants, as `.icns`.
iconset=$(mktemp -d)/Temor.iconset
mkdir -p "$iconset"
src=dist/icons/source/org.quire.Temor-1024.png
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$src" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$src" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Temor.icns"
rm -rf "$(dirname "$iconset")"

codesign --force --sign - --timestamp=none "$app"
codesign --verify --strict "$app"

ditto -c -k --keepParent "$app" "$out/$name.zip"
(cd "$out" && shasum -a 256 "$name.zip" > "$name.zip.sha256")
echo "==> $app"
echo "==> $out/$name.zip"
