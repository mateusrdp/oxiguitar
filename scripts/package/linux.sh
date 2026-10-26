#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds OxiGuitar's Linux packages from release binaries already built for
# this machine's architecture: a .deb, an .rpm and an AppImage of the core,
# which holds no plugins, and (`plugins`) the basic plugins apart: the
# oxiguitar-plugins-basic .deb and .rpm, which the core Suggests, a zip of
# the plugins and each plugin's own file (Mateus, 18 Oct 2026;
# docs/design/plugins.md, "Distribution"). Used by
# .github/workflows/release.yml; runs on Ubuntu 24.04 (dpkg-deb and
# dpkg-shlibdeps are there; rpmbuild comes from the "rpm" package, the
# AppImage needs squashfs-tools and the AppImage runtime, see below).
#
# Usage: scripts/package/linux.sh deb|rpm|appimage|all|plugins VERSION [wayland|x11]
#   VERSION  the package version, from scripts/package/version.sh
#   wayland  the Wayland build (the default): packages `oxiguitar`
#   x11      the X11 build, deprecated: packages `oxiguitar-x11`, which
#            Provide, Conflict with and Replace `oxiguitar`
#            (docs/design/plugins.md, the two Linux builds)
# Reads:  target/release/oxiguitar (the Wayland build) or
#         target/x11/release/oxiguitar (the X11 build, built with
#         `--no-default-features --features x11 --target-dir target/x11`),
#         target/release/oxiguitar-cli,
#         target/lib/oxiguitar/plugins/*.oxp (scripts/build-plugins.sh;
#         `plugins` only),
#         dist/THIRD-PARTY-LICENSES.html (scripts/package/licenses.sh)
#         and, for the AppImage, $APPIMAGE_RUNTIME (the type 2 runtime for
#         this architecture, from github.com/AppImage/type2-runtime).
# Writes: dist/oxiguitar_VERSION_ARCH.deb, dist/oxiguitar-VERSION-1.ARCH.rpm,
#         dist/OxiGuitar-VERSION-ARCH.AppImage; for the X11 build
#         dist/oxiguitar-x11_…deb, dist/oxiguitar-x11-…rpm and
#         dist/OxiGuitar-x11-VERSION-ARCH.AppImage; for `plugins`
#         dist/oxiguitar-plugins-basic_VERSION_ARCH.deb,
#         dist/oxiguitar-plugins-basic-VERSION-1.ARCH.rpm,
#         dist/OxiGuitar-plugins-basic-VERSION-linux-ARCH.zip (the .oxp
#         files, flat, and a README)
#
# Only the .deb and the .rpm register the Guitar Pro file types (UI-4); the
# AppImage registers nothing (the manual shows how to do it by hand).

set -euo pipefail

what="${1:?usage: linux.sh deb|rpm|appimage|all|plugins VERSION [wayland|x11]}"
version="${2:?usage: linux.sh deb|rpm|appimage|all|plugins VERSION [wayland|x11]}"
build="${3:-wayland}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
id="io.github.mateusrdp.OxiGuitar"
bin="$root/target/release"
case "$build" in
    wayland) app="$root/target/release/oxiguitar"; package=oxiguitar; appimage_name=OxiGuitar ;;
    x11) app="$root/target/x11/release/oxiguitar"; package=oxiguitar-x11; appimage_name=OxiGuitar-x11 ;;
    *) echo "unknown build $build: wayland or x11" >&2; exit 1 ;;
esac
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
if [[ "$build" == x11 ]]; then
    summary="$summary (X11 build, deprecated)"
    description="$description

This is the X11 build, for desktops that still run an X server. It is
deprecated and will be dropped; desktops with Wayland should install the
package oxiguitar instead."
fi

# Libraries the program loads at run time rather than links, so the
# packaging tools cannot find them (checked with grep -ao 'lib.*\.so' on
# each binary; the optimiser can split a name, as with libEGL.so.1). The
# Wayland build needs Wayland, keyboard maps and EGL, and no X11 library
# (the clipboard's XWayland fallback is pure Rust); the X11 build needs
# Xlib and its extensions, keyboard maps and EGL, and GLX where EGL falls
# short. D-Bus serves some desktop services; the file dialogs go through
# the XDG desktop portal.
if [[ "$build" == wayland ]]; then
    deb_runtime="libwayland-client0, libwayland-egl1, libxkbcommon0, libegl1"
    deb_recommends="libdbus-1-3, xdg-desktop-portal"
    rpm_runtime=(
        "libwayland-client.so.0()(64bit)" "libwayland-egl.so.1()(64bit)"
        "libxkbcommon.so.0()(64bit)" "libEGL.so.1()(64bit)"
    )
    rpm_recommends=("libdbus-1.so.3()(64bit)" "xdg-desktop-portal")
else
    deb_runtime="libxkbcommon0, libxkbcommon-x11-0, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrender1, libegl1"
    deb_recommends="libgl1, libdbus-1-3, xdg-desktop-portal"
    rpm_runtime=(
        "libxkbcommon.so.0()(64bit)" "libxkbcommon-x11.so.0()(64bit)"
        "libX11.so.6()(64bit)" "libX11-xcb.so.1()(64bit)" "libXcursor.so.1()(64bit)"
        "libXi.so.6()(64bit)" "libXrender.so.1()(64bit)" "libEGL.so.1()(64bit)"
    )
    rpm_recommends=("libGL.so.1()(64bit)" "libdbus-1.so.3()(64bit)" "xdg-desktop-portal")
fi

# Our basic plugins (scripts/build-plugins.sh), packaged apart.
plugins="$root/target/lib/oxiguitar/plugins"
if [[ "$what" == plugins ]]; then
    compgen -G "$plugins/*.oxp" > /dev/null || {
        echo "missing $plugins/*.oxp: run scripts/build-plugins.sh" >&2
        exit 1
    }
else
    for f in "$app" "$bin/oxiguitar-cli"; do
        [[ -x "$f" ]] || { echo "missing $f: build it first" >&2; exit 1; }
    done
fi
[[ -f "$dist/THIRD-PARTY-LICENSES.html" ]] || {
    echo "missing dist/THIRD-PARTY-LICENSES.html: run scripts/package/licenses.sh" >&2
    exit 1
}

# The files every package installs, under a /usr prefix.
stage() {
    local to="$1"
    install -Dm755 "$app" "$to/usr/bin/oxiguitar"
    install -Dm755 "$bin/oxiguitar-cli" "$to/usr/bin/oxiguitar-cli"
    install -Dm644 "$root/assets/linux/$id.desktop" "$to/usr/share/applications/$id.desktop"
    install -Dm644 "$root/assets/linux/$id.xml" "$to/usr/share/mime/packages/$id.xml"
    for s in 16 24 32 48 64 128 256 512; do
        install -Dm644 "$root/assets/icon/png/oxiguitar-$s.png" \
            "$to/usr/share/icons/hicolor/${s}x${s}/apps/$id.png"
    done
    install -Dm644 "$root/assets/icon/oxiguitar-placeholder.svg" \
        "$to/usr/share/icons/hicolor/scalable/apps/$id.svg"
    install -d "$to/usr/share/man/man1"
    for page in oxiguitar oxiguitar-cli; do
        gzip -9n -c "$root/assets/linux/man/$page.1" > "$to/usr/share/man/man1/$page.1.gz"
    done
    local doc="$to/usr/share/doc/$package"
    install -Dm644 "$root/README.md" "$doc/README.md"
    install -Dm644 "$root/CHANGELOG.md" "$doc/CHANGELOG.md"
    install -Dm644 "$root/CREDITS.md" "$doc/CREDITS.md"
    install -Dm644 "$dist/THIRD-PARTY-LICENSES.html" "$doc/THIRD-PARTY-LICENSES.html"
    install -Dm644 "$root/assets/fonts/bravura/OFL.txt" "$doc/fonts/Bravura-OFL.txt"
    install -Dm644 "$root/assets/fonts/noto-sans/OFL.txt" "$doc/fonts/NotoSans-OFL.txt"
}

# The contact in the packages' Maintainer field and changelog: always the
# GitHub issue tracker, never a personal email (Mateus, 18 Oct 2026).
# Debian Policy 5.6.2 asks for an email address, so lintian reports
# malformed-contact and bogus-mail-host-in-debian-changelog (release.yml
# suppresses those two tags, and only those); dpkg and apt
# accept it. A package in Debian's own archive carries its Debian
# maintainer's address instead (debian/).
maintainer="Mateus Rocha de Paula <https://github.com/mateusrdp/oxiguitar/issues>"

# What every .deb needs beside its files: doc/PACKAGE/changelog.gz in
# Debian's format (one entry for this version, pointing to the full
# changelog), dated by the last commit so a rebuild gives the same bytes,
# and DEBIAN/md5sums.
deb_extras() {
    local tree="$1" name="$2" full="$3"
    local when
    when="$(date -u -R -d "@${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct)}")"
    {
        echo "$name ($version) unstable; urgency=medium"
        echo
        echo "  * OxiGuitar $version. Every change is listed in"
        echo "    $full."
        echo
        echo " -- $maintainer  $when"
    } | gzip -9n > "$tree/usr/share/doc/$name/changelog.gz"
    mkdir -p "$tree/DEBIAN"
    (cd "$tree" && find . -path ./DEBIAN -prune -o -type f -printf '%P\0' \
        | LC_ALL=C sort -z | xargs -0 md5sum) > "$tree/DEBIAN/md5sums"
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
        echo " Third-party licences: /usr/share/doc/$package/THIRD-PARTY-LICENSES.html"
    } > "$tree/usr/share/doc/$package/copyright"
    deb_extras "$tree" "$package" "CHANGELOG.md, beside this file"

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
        echo "Package: $package"
        echo "Version: $version"
        echo "Architecture: $deb_arch"
        echo "Maintainer: $maintainer"
        echo "Installed-Size: $size"
        echo "Depends: ${linked:+$linked, }$deb_runtime"
        echo "Recommends: $deb_recommends"
        # The core opens nothing without format plugins, yet ships alone:
        # apt installs no suggested package by default (Mateus, 18 Oct 2026).
        echo "Suggests: oxiguitar-plugins-basic"
        if [[ "$build" == x11 ]]; then
            # The same program for another display system: one or the other.
            echo "Provides: oxiguitar"
            echo "Conflicts: oxiguitar"
            echo "Replaces: oxiguitar"
        fi
        echo "Section: sound"
        echo "Priority: optional"
        echo "Homepage: https://github.com/mateusrdp/oxiguitar"
        echo "Description: $summary"
        # Continuation lines start with a space; empty lines are " .".
        sed 's/^/ /; s/^ $/ ./' <<< "$description"
    } > "$tree/DEBIAN/control"
    # The mime, desktop and icon databases are rebuilt by the triggers of
    # shared-mime-info, desktop-file-utils and the icon themes: no scripts.
    local out="$dist/${package}_${version}_${deb_arch}.deb"
    dpkg-deb --root-owner-group -Zxz --build "$tree" "$out"
    dpkg-deb --info "$out"
    echo "Built $out"
}

build_rpm() {
    local tree="$work/rpm-root"
    stage "$tree"
    install -Dm644 "$root/LICENSE" "$tree/usr/share/licenses/$package/LICENSE"
    local spec="$work/$package.spec"
    {
        echo "Name: $package"
        # rpm versions may not contain '-'; '~' sorts before the release.
        echo "Version: ${version//-/.}"
        echo "Release: 1"
        echo "Summary: $summary"
        echo "License: GPL-3.0-or-later AND GPL-3.0-only"
        echo "URL: https://github.com/mateusrdp/oxiguitar"
        for r in "${rpm_runtime[@]}"; do echo "Requires: $r"; done
        for r in "${rpm_recommends[@]}"; do echo "Recommends: $r"; done
        echo "Suggests: oxiguitar-plugins-basic"
        if [[ "$build" == x11 ]]; then
            echo "Provides: oxiguitar = ${version//-/.}"
            echo "Conflicts: oxiguitar"
        fi
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
        echo "%license /usr/share/licenses/$package/LICENSE"
        echo "/usr/bin/oxiguitar"
        echo "/usr/bin/oxiguitar-cli"
        echo "/usr/share/applications/$id.desktop"
        echo "/usr/share/mime/packages/$id.xml"
        echo "/usr/share/icons/hicolor/*/apps/$id.*"
        echo "/usr/share/man/man1/oxiguitar.1.gz"
        echo "/usr/share/man/man1/oxiguitar-cli.1.gz"
        echo "%doc /usr/share/doc/$package"
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
    local out="$dist/$appimage_name-$version-$arch.AppImage"
    cat "$runtime" "$work/image.squashfs" > "$out"
    chmod 755 "$out"
    echo "Built $out"
}

# oxiguitar-plugins-basic: the plugins we write and recommend, in the
# program's own plugin folder; then a zip of them, for the AppImage, to
# unzip into a plugin folder; then each plugin on its own.
build_plugins() {
    local name=oxiguitar-plugins-basic
    local summary="Guitar Pro format, sound and audio plugins for OxiGuitar"
    local description="The plugins the OxiGuitar project writes and recommends: one for each
Guitar Pro format (3, 4, 5, 6 and 7-8), so OxiGuitar can open those
files; the SoundFont player, which plays the tracks with a General MIDI
sound set (the recommended package, or one of your own); and the ones
that play its sound through PipeWire or ALSA. OxiGuitar alone opens no
file; it finds these in /usr/lib/oxiguitar/plugins."
    local tree="$work/plugins-deb"
    install -d "$tree/usr/lib/oxiguitar/plugins"
    install -m644 "$plugins"/*.oxp "$tree/usr/lib/oxiguitar/plugins/"
    local doc="$tree/usr/share/doc/$name"
    install -Dm644 "$root/scripts/package/plugins-README.txt" "$doc/README.txt"
    install -Dm644 "$dist/THIRD-PARTY-LICENSES.html" "$doc/THIRD-PARTY-LICENSES.html"
    {
        echo "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
        echo "Upstream-Name: OxiGuitar"
        echo "Source: https://github.com/mateusrdp/oxiguitar"
        echo
        echo "Files: *"
        echo "Copyright: 2026 Mateus Rocha de Paula and the OxiGuitar contributors"
        echo "License: GPL-3.0-or-later"
        echo " On Debian systems the full licence is in /usr/share/common-licenses/GPL-3."
        echo " Third-party licences: /usr/share/doc/$name/THIRD-PARTY-LICENSES.html"
    } > "$doc/copyright"
    deb_extras "$tree" "$name" "CHANGELOG.md, in the package oxiguitar"
    mkdir -p "$work/plugin-shlibs/debian"
    touch "$work/plugin-shlibs/debian/control"
    local args=()
    for f in "$tree"/usr/lib/oxiguitar/plugins/*.oxp; do args+=("-e$f"); done
    local linked
    linked="$(cd "$work/plugin-shlibs" && dpkg-shlibdeps -O "${args[@]}" 2> /dev/null \
        | sed -n 's/^shlibs:Depends=//p')"
    local size
    size="$(du -sk --exclude=DEBIAN "$tree" | cut -f1)"
    {
        echo "Package: $name"
        echo "Version: $version"
        echo "Architecture: $deb_arch"
        echo "Maintainer: $maintainer"
        echo "Installed-Size: $size"
        if [[ -n "$linked" ]]; then echo "Depends: $linked"; fi
        # A General MIDI sound set for the SoundFont plugin to play with.
        echo "Recommends: fluid-soundfont-gm | musescore-general-soundfont-small"
        echo "Enhances: oxiguitar, oxiguitar-x11"
        echo "Section: sound"
        echo "Priority: optional"
        echo "Homepage: https://github.com/mateusrdp/oxiguitar"
        echo "Description: $summary"
        sed 's/^/ /; s/^ $/ ./' <<< "$description"
    } > "$tree/DEBIAN/control"
    local deb="$dist/${name}_${version}_${deb_arch}.deb"
    dpkg-deb --root-owner-group -Zxz --build "$tree" "$deb"
    dpkg-deb --info "$deb"
    dpkg-deb -f "$deb" Recommends | grep fluid-soundfont-gm > /dev/null \
        || { echo "$deb recommends no sound set" >&2; exit 1; }
    echo "Built $deb"

    local rtree="$work/plugins-rpm"
    install -d "$rtree/usr/lib/oxiguitar/plugins"
    install -m644 "$plugins"/*.oxp "$rtree/usr/lib/oxiguitar/plugins/"
    install -Dm644 "$root/LICENSE" "$rtree/usr/share/licenses/$name/LICENSE"
    local spec="$work/$name.spec"
    {
        echo "Name: $name"
        echo "Version: ${version//-/.}"
        echo "Release: 1"
        echo "Summary: $summary"
        echo "License: GPL-3.0-or-later"
        echo "URL: https://github.com/mateusrdp/oxiguitar"
        echo "Recommends: fluid-soundfont-gm"
        echo "Enhances: oxiguitar"
        echo "%global debug_package %{nil}"
        echo "%global __os_install_post %{nil}"
        echo "%global _build_id_links none"
        echo
        echo "%description"
        echo "$description"
        echo
        echo "%install"
        echo "cp -a '$rtree/.' %{buildroot}/"
        echo
        echo "%files"
        echo "%license /usr/share/licenses/$name/LICENSE"
        echo "/usr/lib/oxiguitar"
    } > "$spec"
    rpmbuild -bb --target "$arch" \
        --define "_topdir $work/rpmbuild-plugins" \
        --define "_rpmdir $work/rpms-plugins" \
        "$spec"
    local built
    built="$(find "$work/rpms-plugins" -name '*.rpm' | head -n1)"
    rpm -qp --recommends "$built" | grep fluid-soundfont-gm > /dev/null \
        || { echo "$built recommends no sound set" >&2; exit 1; }
    mv "$built" "$dist/$(basename "$built")"
    echo "Built $dist/$(basename "$built")"

    # The zip: the .oxp files flat, to unzip into a plugin folder.
    local zdir="$work/zip"
    install -d "$zdir"
    install -m644 "$plugins"/*.oxp "$root/scripts/package/plugins-README.txt" "$zdir/"
    mv "$zdir/plugins-README.txt" "$zdir/README.txt"
    local zip="$dist/OxiGuitar-plugins-basic-$version-linux-$arch.zip"
    rm -f "$zip"
    (cd "$zdir" && python3 -m zipfile -c "$zip" ./*)
    echo "Built $zip"
}

case "$what" in
    deb) build_deb ;;
    rpm) build_rpm ;;
    appimage) build_appimage ;;
    all) build_deb; build_rpm; build_appimage ;;
    plugins) build_plugins ;;
    *) echo "unknown package type $what" >&2; exit 1 ;;
esac
