#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds OxiGuitar's Linux packages from release binaries already built for
# this machine's architecture: a .deb, an .rpm and an AppImage. Used by
# .github/workflows/release.yml; runs on Ubuntu 24.04 (dpkg-deb and
# dpkg-shlibdeps are there; rpmbuild comes from the "rpm" package, the
# AppImage needs squashfs-tools and the AppImage runtime, see below).
#
# Usage: scripts/package/linux.sh deb|rpm|appimage|all VERSION
#   VERSION  the package version, from scripts/package/version.sh
# Reads:  target/release/oxiguitar, target/release/oxiguitar-cli,
#         dist/THIRD-PARTY-LICENSES.html (scripts/package/licenses.sh)
#         and, for the AppImage, $APPIMAGE_RUNTIME (the type 2 runtime for
#         this architecture, from github.com/AppImage/type2-runtime).
# Writes: dist/oxiguitar_VERSION_ARCH.deb, dist/oxiguitar-VERSION-1.ARCH.rpm,
#         dist/OxiGuitar-VERSION-ARCH.AppImage
#
# Only the .deb and the .rpm register the Guitar Pro file types (UI-4); the
# AppImage registers nothing (the manual shows how to do it by hand).

set -euo pipefail

what="${1:?usage: linux.sh deb|rpm|appimage|all VERSION}"
version="${2:?usage: linux.sh deb|rpm|appimage|all VERSION}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
id="io.github.mateusrdp.OxiGuitar"
bin="$root/target/release"
dist="$root/dist"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$dist"

arch="$(uname -m)" # x86_64 or aarch64
case "$arch" in
    x86_64) deb_arch=amd64 ;;
    aarch64) deb_arch=arm64 ;;
    *) echo "unsupported architecture $arch" >&2; exit 1 ;;
esac

summary="Guitar tablature and score viewer"
description="OxiGuitar opens Guitar Pro files (.gp, .gpx, .gp5, .gp4, .gp3) and
shows them as standard notation and tablature in four layouts, with any
set of tracks together, and exports them to PDF. Editing and playback
follow in later versions. No telemetry: it never uploads anything."

# Libraries the program loads at run time rather than links, so the
# packaging tools cannot find them (checked with grep -ao 'lib.*\.so' on the
# binary): Wayland, keyboard maps and EGL are needed; X11, GLX and D-Bus only
# in X11 sessions and for some desktop services. The file dialogs go through
# the XDG desktop portal.
deb_runtime="libwayland-client0, libwayland-egl1, libxkbcommon0, libegl1"
deb_recommends="libxkbcommon-x11-0, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrender1, libgl1, libdbus-1-3, xdg-desktop-portal"
rpm_runtime=(
    "libwayland-client.so.0()(64bit)" "libwayland-egl.so.1()(64bit)"
    "libxkbcommon.so.0()(64bit)" "libEGL.so.1()(64bit)"
)
rpm_recommends=(
    "libxkbcommon-x11.so.0()(64bit)" "libX11.so.6()(64bit)" "libX11-xcb.so.1()(64bit)"
    "libXcursor.so.1()(64bit)" "libXi.so.6()(64bit)" "libXrender.so.1()(64bit)"
    "libGL.so.1()(64bit)" "libdbus-1.so.3()(64bit)" "xdg-desktop-portal"
)

for f in oxiguitar oxiguitar-cli; do
    [[ -x "$bin/$f" ]] || { echo "missing $bin/$f: build it first" >&2; exit 1; }
done
[[ -f "$dist/THIRD-PARTY-LICENSES.html" ]] || {
    echo "missing dist/THIRD-PARTY-LICENSES.html: run scripts/package/licenses.sh" >&2
    exit 1
}

# The files every package installs, under a /usr prefix.
stage() {
    local to="$1"
    install -Dm755 "$bin/oxiguitar" "$to/usr/bin/oxiguitar"
    install -Dm755 "$bin/oxiguitar-cli" "$to/usr/bin/oxiguitar-cli"
    install -Dm644 "$root/assets/linux/$id.desktop" "$to/usr/share/applications/$id.desktop"
    install -Dm644 "$root/assets/linux/$id.xml" "$to/usr/share/mime/packages/$id.xml"
    for s in 16 24 32 48 64 128 256 512; do
        install -Dm644 "$root/assets/icon/png/oxiguitar-$s.png" \
            "$to/usr/share/icons/hicolor/${s}x${s}/apps/$id.png"
    done
    install -Dm644 "$root/assets/icon/oxiguitar-placeholder.svg" \
        "$to/usr/share/icons/hicolor/scalable/apps/$id.svg"
    local doc="$to/usr/share/doc/oxiguitar"
    install -Dm644 "$root/README.md" "$doc/README.md"
    install -Dm644 "$root/CHANGELOG.md" "$doc/CHANGELOG.md"
    install -Dm644 "$root/CREDITS.md" "$doc/CREDITS.md"
    install -Dm644 "$dist/THIRD-PARTY-LICENSES.html" "$doc/THIRD-PARTY-LICENSES.html"
    install -Dm644 "$root/assets/fonts/bravura/OFL.txt" "$doc/fonts/Bravura-OFL.txt"
    install -Dm644 "$root/assets/fonts/noto-sans/OFL.txt" "$doc/fonts/NotoSans-OFL.txt"
}

build_deb() {
    local tree="$work/deb"
    stage "$tree"
    # Debian keeps the licence as doc/<package>/copyright.
    {
        echo "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
        echo "Upstream-Name: OxiGuitar"
        echo "Source: https://github.com/mateusrdp/oxiguitar"
        echo
        echo "Files: *"
        echo "Copyright: 2026 Mateus Rocha de Paula and the OxiGuitar contributors"
        echo "License: GPL-3.0-or-later"
        echo " The distributed program is under GPL-3.0 (Slint is GPL-3.0-only)."
        echo " On Debian systems the full licence is in /usr/share/common-licenses/GPL-3."
        echo " Third-party licences: /usr/share/doc/oxiguitar/THIRD-PARTY-LICENSES.html"
    } > "$tree/usr/share/doc/oxiguitar/copyright"

    # Linked libraries, as dpkg-shlibdeps finds them (it wants a debian/
    # folder to stand in).
    mkdir -p "$work/shlibs/debian"
    touch "$work/shlibs/debian/control"
    local linked
    linked="$(cd "$work/shlibs" && dpkg-shlibdeps -O -e"$tree/usr/bin/oxiguitar" -e"$tree/usr/bin/oxiguitar-cli" 2> /dev/null \
        | sed -n 's/^shlibs:Depends=//p')"
    local size
    size="$(du -sk --exclude=DEBIAN "$tree" | cut -f1)"
    mkdir -p "$tree/DEBIAN"
    {
        echo "Package: oxiguitar"
        echo "Version: $version"
        echo "Architecture: $deb_arch"
        echo "Maintainer: Mateus Rocha de Paula <https://github.com/mateusrdp/oxiguitar/issues>"
        echo "Installed-Size: $size"
        echo "Depends: ${linked:+$linked, }$deb_runtime"
        echo "Recommends: $deb_recommends"
        echo "Section: sound"
        echo "Priority: optional"
        echo "Homepage: https://github.com/mateusrdp/oxiguitar"
        echo "Description: $summary"
        # Continuation lines start with a space; empty lines are " .".
        sed 's/^/ /; s/^ $/ ./' <<< "$description"
    } > "$tree/DEBIAN/control"
    # The mime, desktop and icon databases are rebuilt by the triggers of
    # shared-mime-info, desktop-file-utils and the icon themes: no scripts.
    local out="$dist/oxiguitar_${version}_${deb_arch}.deb"
    dpkg-deb --root-owner-group -Zxz --build "$tree" "$out"
    dpkg-deb --info "$out"
    echo "Built $out"
}

build_rpm() {
    local tree="$work/rpm-root"
    stage "$tree"
    install -Dm644 "$root/LICENSE" "$tree/usr/share/licenses/oxiguitar/LICENSE"
    local spec="$work/oxiguitar.spec"
    {
        echo "Name: oxiguitar"
        # rpm versions may not contain '-'; '~' sorts before the release.
        echo "Version: ${version//-/.}"
        echo "Release: 1"
        echo "Summary: $summary"
        echo "License: GPL-3.0-or-later AND GPL-3.0-only"
        echo "URL: https://github.com/mateusrdp/oxiguitar"
        for r in "${rpm_runtime[@]}"; do echo "Requires: $r"; done
        for r in "${rpm_recommends[@]}"; do echo "Recommends: $r"; done
        # The binaries are built and stripped by Cargo.
        echo "%global debug_package %{nil}"
        echo "%global __os_install_post %{nil}"
        echo "%global _build_id_links none"
        echo
        echo "%description"
        echo "$description"
        echo
        echo "%install"
        echo "cp -a '$tree/.' %{buildroot}/"
        echo
        echo "%files"
        echo "%license /usr/share/licenses/oxiguitar/LICENSE"
        echo "/usr/bin/oxiguitar"
        echo "/usr/bin/oxiguitar-cli"
        echo "/usr/share/applications/$id.desktop"
        echo "/usr/share/mime/packages/$id.xml"
        echo "/usr/share/icons/hicolor/*/apps/$id.*"
        echo "%doc /usr/share/doc/oxiguitar"
    } > "$spec"
    # Fedora's shared-mime-info and desktop-file-utils rebuild their
    # databases with file triggers, so the package needs no scriptlets.
    rpmbuild -bb --target "$arch" \
        --define "_topdir $work/rpmbuild" \
        --define "_rpmdir $work/rpms" \
        "$spec"
    local built
    built="$(find "$work/rpms" -name '*.rpm' | head -n1)"
    local out
    out="$dist/$(basename "$built")"
    mv "$built" "$out"
    rpm -qip "$out"
    echo "Requires:"
    rpm -qp --requires "$out"
    echo "Built $out"
}

build_appimage() {
    local runtime="${APPIMAGE_RUNTIME:?set APPIMAGE_RUNTIME to the type 2 runtime for $arch}"
    local dir="$work/OxiGuitar.AppDir"
    stage "$dir"
    # The desktop entry and icon at the top, as the AppImage format wants,
    # and AppRun starting the program. The mime file stays inside, unused.
    cp "$dir/usr/share/applications/$id.desktop" "$dir/$id.desktop"
    cp "$root/assets/icon/png/oxiguitar-256.png" "$dir/$id.png"
    cp "$root/assets/icon/png/oxiguitar-256.png" "$dir/.DirIcon"
    ln -s usr/bin/oxiguitar "$dir/AppRun"
    mksquashfs "$dir" "$work/image.squashfs" -root-owned -noappend -comp zstd -quiet
    local out="$dist/OxiGuitar-$version-$arch.AppImage"
    cat "$runtime" "$work/image.squashfs" > "$out"
    chmod 755 "$out"
    echo "Built $out"
}

case "$what" in
    deb) build_deb ;;
    rpm) build_rpm ;;
    appimage) build_appimage ;;
    all) build_deb; build_rpm; build_appimage ;;
    *) echo "unknown package type $what" >&2; exit 1 ;;
esac
