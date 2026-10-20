#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# Builds the release's three musl images on Alpine (linux-2 L2-01), named as the release names them:
#
# - the edge `ghostlight` and the service `ghostlight-service`, static, so they run on any musl
#   system with no loader and no library (the service links no UI toolkit, ADR-0199);
# - the shell `ghostlight-desktop`, linked against Alpine's WebKitGTK 4.1, GTK 3 and D-Bus through
#   musl's loader, so it needs those packages where it runs.
#
# The release workflow runs it in an Alpine container. On an Alpine machine with the build packages
# (build-base, pkgconf, webkit2gtk-4.1-dev, dbus-dev, libayatana-appindicator-dev) and rustup, it
# runs as is from the repository root:
#   sh scripts/build-musl-images.sh dist/raw-x86_64-unknown-linux-musl

set -eu

target="x86_64-unknown-linux-musl"
output=${1:?usage: sh scripts/build-musl-images.sh <output directory>}
static_dir="target/musl-static"
shell_dir="target/musl-shell"

# Rust's musl target links statically by default. The shell cannot: GTK and WebKitGTK exist only as
# shared libraries, so its build turns static linking off. Separate target directories keep the
# two builds from invalidating each other.
cargo build -p ghostlight -p ghostlight-service --release --locked --target "$target" --target-dir "$static_dir"
RUSTFLAGS="-C target-feature=-crt-static" \
  cargo build -p ghostlight-desktop --release --locked --target "$target" --target-dir "$shell_dir"

mkdir -p "$output"
for name in ghostlight ghostlight-service; do
  cp "$static_dir/$target/release/$name" "$output/$name-$target"
done
cp "$shell_dir/$target/release/ghostlight-desktop" "$output/ghostlight-desktop-$target"

# The promise each image makes, checked on the copies that ship.
for name in ghostlight ghostlight-service; do
  image="$output/$name-$target"
  if readelf -l "$image" | grep -q 'program interpreter' || readelf -d "$image" | grep -q NEEDED; then
    echo "build-musl-images: $name is not static" >&2
    exit 1
  fi
done
if ! readelf -l "$output/ghostlight-desktop-$target" | grep -q '/lib/ld-musl-x86_64.so.1'; then
  echo "build-musl-images: ghostlight-desktop does not run through musl's loader" >&2
  exit 1
fi
echo "Built the musl images in $output: the edge and the service static, the shell through musl's loader."
