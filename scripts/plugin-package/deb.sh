#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the .deb of one plugin: oxiguitar-NAME, which puts the plugin file
# in /usr/lib/oxiguitar/plugins, the folder the core's own package uses
# (docs/design/plugins.md, "Distribution"). Runs on Debian or Ubuntu
# (dpkg-deb). Used by .github/workflows/plugin-release.yml (#165).
#
# Usage: scripts/plugin-package/deb.sh NAME VERSION ARCH OUT-DIR SRC[:DEST]...
#   ARCH     x86_64 or aarch64
#   SRC      a built .oxp file; DEST the name it is installed under
#            (default NAME.oxp); a second file, such as NAME-x86-64-v3.oxp,
#            is given as SRC:NAME-x86-64-v3.oxp
# Reads, from the environment:
#   PLUGIN_TITLE        shown name, "MIDI (SoundFont, SFZ)" (default NAME)
#   PLUGIN_DESCRIPTION  one line (required)
#   PLUGIN_HOMEPAGE     the repository's address
#   PLUGIN_LICENSE      an SPDX expression
#   PLUGIN_DEPENDS_DEB  extra Depends, such as "oxiguitar-midi (>= 1.0.0)"
#   PLUGIN_RECOMMENDS_DEB  Recommends (default: oxiguitar)
#   PLUGIN_SUGGESTS_DEB    Suggests, such as "oxiguitar-bettersounds-soundfonts"
#   PLUGIN_MAINTAINER   default "Mateus Rocha de Paula <mateusrdp@users.noreply.github.com>"
# Writes: OUT-DIR/oxiguitar-NAME_VERSION_DEBARCH.deb

set -euo pipefail
trap 'echo "::error::deb.sh line $LINENO: $BASH_COMMAND"' ERR

name="${1:?usage: deb.sh NAME VERSION ARCH OUT-DIR SRC[:DEST]...}"
version="${2:?}"
arch="${3:?}"
out_dir="${4:?}"
shift 4
[[ $# -gt 0 ]] || { echo "deb.sh: no plugin file given" >&2; exit 2; }

case "$arch" in
    x86_64) deb_arch=amd64 ;;
    aarch64) deb_arch=arm64 ;;
    *) echo "deb.sh: no Debian architecture for $arch" >&2; exit 2 ;;
esac

title="${PLUGIN_TITLE:-$name}"
description="${PLUGIN_DESCRIPTION:?PLUGIN_DESCRIPTION is required}"
homepage="${PLUGIN_HOMEPAGE:-https://github.com/mateusrdp/oxiguitar}"
license="${PLUGIN_LICENSE:-GPL-3.0-or-later}"
maintainer="${PLUGIN_MAINTAINER:-Mateus Rocha de Paula <mateusrdp@users.noreply.github.com>}"
recommends="${PLUGIN_RECOMMENDS_DEB-oxiguitar}"
package="oxiguitar-$name"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tree="$work/$package"
plugins="$tree/usr/lib/oxiguitar/plugins"
doc="$tree/usr/share/doc/$package"
mkdir -p "$plugins" "$doc" "$tree/DEBIAN" "$out_dir"

size=0
for spec in "$@"; do
    src="${spec%%:*}"
    dest="$name.oxp"
    [[ "$spec" == *:* ]] && dest="${spec#*:}"
    [[ -f "$src" ]] || { echo "deb.sh: $src isn't a file" >&2; exit 2; }
    install -m644 "$src" "$plugins/$dest"
    size=$((size + $(stat -c %s "$src")))
done

# Debian's minimal machine-readable copyright; the licence text itself is
# the plugin's repository's LICENSE (a link, not a copy: the SPDX
# expression says which).
cat > "$doc/copyright" <<COPYRIGHT
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: $package
Source: $homepage

Files: *
Copyright: 2026 Mateus Rocha de Paula and the OxiGuitar contributors
License: $license
 The licence's full text is in the source repository, $homepage
COPYRIGHT
chmod 644 "$doc/copyright"

{
    echo "Package: $package"
    echo "Version: $version"
    echo "Section: sound"
    echo "Priority: optional"
    echo "Architecture: $deb_arch"
    echo "Installed-Size: $(((size + 1023) / 1024))"
    echo "Maintainer: $maintainer"
    echo "Homepage: $homepage"
    [[ -n "${PLUGIN_DEPENDS_DEB:-}" ]] && echo "Depends: $PLUGIN_DEPENDS_DEB"
    [[ -n "$recommends" ]] && echo "Recommends: $recommends"
    [[ -n "${PLUGIN_SUGGESTS_DEB:-}" ]] && echo "Suggests: $PLUGIN_SUGGESTS_DEB"
    echo "Description: $title plugin for OxiGuitar"
    echo " $description"
} > "$tree/DEBIAN/control"

out="$out_dir/${package}_${version}_${deb_arch}.deb"
dpkg-deb --root-owner-group -Zxz --build "$tree" "$out"
dpkg-deb --info "$out"
dpkg-deb --contents "$out"
echo "$out"
