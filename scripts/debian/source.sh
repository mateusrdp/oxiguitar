#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# The Debian source of the version in debian/changelog, from HEAD:
#
#   OUT/oxiguitar_UPSTREAM.orig.tar.xz         HEAD's tree without what
#                                              debian/copyright's
#                                              Files-Excluded names
#   OUT/oxiguitar_UPSTREAM.orig-vendor.tar.xz  the crates Cargo.lock names,
#                                              from `cargo vendor`, until
#                                              Debian has them all
#                                              (docs/debian.md)
#   OUT/oxiguitar-UPSTREAM/                    both unpacked, with debian/
#                                              from the working tree:
#                                              dpkg-buildpackage runs there
#
# Needs git, cargo (and the network, for the crates), xz, and dpkg-dev for
# dpkg-parsechangelog. The build itself needs no network.
#
# Usage: scripts/debian/source.sh OUT

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
[[ $# -eq 1 ]] || { echo "usage: scripts/debian/source.sh OUT" >&2; exit 2; }
mkdir -p "$1"
out="$(cd "$1" && pwd)"

version="$(dpkg-parsechangelog -l "$root/debian/changelog" -S Version)"
upstream="${version%-*}"
upstream="${upstream#*:}"
# The changelog's upstream version is the workspace's, plus +ds (repacked).
workspace="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n1)"
if [[ "$upstream" != "$workspace+ds" ]]; then
    echo "debian/changelog says $version; the workspace is $workspace (expected $workspace+ds-N)" >&2
    exit 1
fi

# Every file the tarball and the build tools write carries the last
# commit's time, so the same commit gives the same bytes.
when="$(git -C "$root" log -1 --format=%ct)"
export SOURCE_DATE_EPOCH="$when"

excluded=()
while read -r path; do
    [[ -n "$path" ]] && excluded+=(":(exclude)$path")
done < <(sed -n '/^Files-Excluded:/,/^[^ ]/{/^ /s/^ *//p}' "$root/debian/copyright")

orig="$out/oxiguitar_$upstream.orig.tar.xz"
git -C "$root" archive --format=tar --prefix="oxiguitar-$upstream/" HEAD -- . "${excluded[@]}" \
    | xz -T0 -6 > "$orig"
echo "source.sh: $orig"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
(cd "$root" && cargo vendor --locked --versioned-dirs --quiet "$work/vendor" > /dev/null)
# As Debian does with vendored crates: no list of each crate's files to
# check, only the crate's own checksum, which Cargo.lock pins. Cargo
# packages keep their manifest as Cargo.toml.orig, which dh_clean deletes
# with every other *.orig; nothing is built from those, so they go.
find "$work/vendor" -name Cargo.toml.orig -delete
# No prebuilt binaries: the import libraries of the Windows crates, the
# DLLs of libloading's tests, wit-bindgen's WebAssembly objects and
# winit's minified web worker belong to other targets and are never
# built here. pdf-writer's example ICC profile doesn't allow changes, so
# Debian can't carry it (krilla's own profiles, which it builds in, are
# CC0).
find "$work/vendor" -type f \( -name '*.dll' -o -name '*.lib' -o -name '*.a' -o -name '*.o' \
    -o -name '*.wasm' -o -name '*.min.js' \) -delete
rm -f "$work"/vendor/pdf-writer-*/examples/*.icc
for sum in "$work"/vendor/*/.cargo-checksum.json; do
    sed -i 's/"files":{[^}]*}/"files":{}/' "$sum"
done
vendor="$out/oxiguitar_$upstream.orig-vendor.tar.xz"
tar -C "$work" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$when" \
    -cf - vendor | xz -T0 -6 > "$vendor"
echo "source.sh: $vendor ($(ls "$work/vendor" | wc -l) crates)"

tree="$out/oxiguitar-$upstream"
rm -rf "$tree"
tar -C "$out" -xf "$orig"
tar -C "$tree" -xf "$vendor"
cp -a "$root/debian" "$tree/debian"
echo "source.sh: $tree"
