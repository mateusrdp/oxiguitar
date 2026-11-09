#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# How big a mirror of the Flatpak runtime would be (#163): the roadmap asks for
# the runtime's size against GitHub Pages' 1 GB site limit before anything is
# built. Usage: runtime-size.sh ARCH VERSION OUT_DIR
#
# For each ref the app needs (the Platform and its GL driver extension) and
# one it may want (the Platform's translations), prints Flathub's own download
# and installed sizes, then mirrors the refs into a fresh archive-mode OSTree
# repository, the form a static Pages site would serve, and measures that on
# disk: per ref alone, and all together (objects shared between refs count
# once, so the table also gives the running total after each ref). Only
# the latest commit of each ref is mirrored, as the site would keep. Writes a Markdown table to OUT_DIR/sizes-ARCH.md.
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

{
    echo "### $arch, runtime $version"
    echo
    echo "| Ref | Flathub download | Flathub installed | Mirrored alone | Mirror so far |"
    echo "| --- | --- | --- | --- | --- |"
} >"$table"

all="$out/mirror-all-$arch"
rm -rf "$all"
ostree init --repo="$all" --mode=archive
ostree remote add --repo="$all" --no-gpg-verify flathub https://dl.flathub.org/repo/

for ref in "${refs[@]}"; do
    info="$(flatpak --user remote-info flathub "$ref" 2>&1 || true)"
    download="$(sed -n 's/^ *Download: *//p' <<<"$info")"
    installed="$(sed -n 's/^ *Installed: *//p' <<<"$info")"
    one="$out/mirror-one"
    rm -rf "$one"
    ostree init --repo="$one" --mode=archive
    ostree remote add --repo="$one" --no-gpg-verify flathub https://dl.flathub.org/repo/
    if ostree pull --repo="$one" --mirror --depth=0 flathub "${ref}" >/dev/null 2>"$out/pull.err"; then
        alone="$(human "$(bytes "$one")")"
        ostree pull --repo="$all" --mirror --depth=0 flathub "${ref}" >/dev/null
        total="$(human "$(bytes "$all")")"
    else
        alone="pull failed: $(tail -1 "$out/pull.err")"
        total="-"
    fi
    rm -rf "$one"
    echo "| \`$ref\` | ${download:-?} | ${installed:-?} | $alone | $total |" >>"$table"
done

echo >>"$table"
rm -rf "$all"
cat "$table"
