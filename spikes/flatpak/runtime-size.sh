#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# How big a mirror of the Flatpak runtime would be (#163): the roadmap asks for
# the runtime's size against GitHub Pages' 1 GB site limit before anything is
# built. Usage: runtime-size.sh ARCH VERSION OUT_DIR
#
# For each ref the app needs (the Platform and its GL driver extension) and
# one it may want (the Platform's translations), prints Flathub's own download
# and installed sizes, then measures the ref as a static mirror would hold it:
# installed alone (--no-related) into a throwaway Flatpak installation, which
# checks Flathub's signature, and its commit copied with `ostree pull-local`
# into an archive-mode repository, the form a Pages site serves. Flathub's
# server refuses plain `ostree pull` (HTTP 403), hence the detour. The refs go
# into one repository in turn, so the last column is the running total
# (objects shared between refs count once). Writes OUT_DIR/sizes-ARCH.md.
set -euo pipefail

arch="$1"
version="$2"
out="$3"
mkdir -p "$out"
table="$out/sizes-$arch.md"

refs=(
    "runtime/org.freedesktop.Platform/$arch/$version"
    "runtime/org.freedesktop.Platform.GL.default/$arch/$version"
    "runtime/org.freedesktop.Platform.Locale/$arch/$version"
)

human() { numfmt --to=iec-i --suffix=B --format='%.1f' "$1"; }
bytes() { du -sb "$1" | cut -f1; }

# A Flatpak installation of its own, so the runner's user installation (which
# the app build uses) isn't touched.
export FLATPAK_USER_DIR="$out/installation"
rm -rf "$FLATPAK_USER_DIR"
flatpak --user remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

mirror="$out/mirror-$arch"
rm -rf "$mirror"
ostree init --repo="$mirror" --mode=archive

{
    echo "### $arch, runtime $version"
    echo
    echo "| Ref | Flathub download | Flathub installed | Mirrored alone | Mirror so far |"
    echo "| --- | --- | --- | --- | --- |"
} >"$table"

for ref in "${refs[@]}"; do
    info="$(flatpak --user remote-info flathub "$ref" 2>&1 || true)"
    download="$(sed -n 's/^ *Download: *//p' <<<"$info")"
    installed="$(sed -n 's/^ *Installed: *//p' <<<"$info")"
    alone="-"
    total="-"
    if flatpak --user install -y --noninteractive --no-related flathub "$ref" >"$out/install.log" 2>&1; then
        commit="$(ostree --repo="$FLATPAK_USER_DIR/repo" rev-parse "flathub:$ref")"
        one="$out/mirror-one"
        rm -rf "$one"
        ostree init --repo="$one" --mode=archive
        ostree pull-local --repo="$one" "$FLATPAK_USER_DIR/repo" "$commit" >/dev/null
        alone="$(human "$(bytes "$one/objects")")"
        rm -rf "$one"
        ostree pull-local --repo="$mirror" "$FLATPAK_USER_DIR/repo" "$commit" >/dev/null
        total="$(human "$(bytes "$mirror/objects")")"
    else
        alone="install failed: $(tail -1 "$out/install.log")"
    fi
    echo "| \`$ref\` | ${download:-?} | ${installed:-?} | $alone | $total |" >>"$table"
done

echo >>"$table"
rm -rf "$mirror" "$FLATPAK_USER_DIR"
cat "$table"
