#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the .rpm of one plugin: oxiguitar-NAME, which puts the plugin file
# in /usr/lib/oxiguitar/plugins. Runs where rpmbuild is (Ubuntu's "rpm"
# package will do). Used by .github/workflows/plugin-release.yml (#165).
#
# Usage: scripts/plugin-package/rpm.sh NAME VERSION ARCH OUT-DIR SRC[:DEST]...
# Arguments and environment as deb.sh, except:
#   PLUGIN_DEPENDS_RPM     extra Requires, such as "oxiguitar-midi >= 1.0.0"
#   PLUGIN_RECOMMENDS_RPM  Recommends (default: oxiguitar)
# Writes: OUT-DIR/oxiguitar-NAME-VERSION-1.ARCH.rpm

set -euo pipefail
trap 'echo "::error::rpm.sh line $LINENO: $BASH_COMMAND"' ERR

name="${1:?usage: rpm.sh NAME VERSION ARCH OUT-DIR SRC[:DEST]...}"
version="${2:?}"
arch="${3:?}"
out_dir="${4:?}"
shift 4
[[ $# -gt 0 ]] || { echo "rpm.sh: no plugin file given" >&2; exit 2; }
case "$arch" in
    x86_64 | aarch64) ;;
    *) echo "rpm.sh: no RPM architecture for $arch" >&2; exit 2 ;;
esac

title="${PLUGIN_TITLE:-$name}"
description="${PLUGIN_DESCRIPTION:?PLUGIN_DESCRIPTION is required}"
homepage="${PLUGIN_HOMEPAGE:-https://github.com/mateusrdp/oxiguitar}"
license="${PLUGIN_LICENSE:-GPL-3.0-or-later}"
recommends="${PLUGIN_RECOMMENDS_RPM-oxiguitar}"
package="oxiguitar-$name"
# RPM versions can't hold a hyphen or a tilde-free pre-release: 1.0.0-rc.1
# becomes 1.0.0~rc.1.
rpm_version="${version//-/~}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
top="$work/rpmbuild"
mkdir -p "$top"/{BUILD,RPMS,SOURCES,SPECS,SRPMS} "$out_dir"

files=""
install_lines=""
for spec in "$@"; do
    src="${spec%%:*}"
    dest="$name.oxp"
    [[ "$spec" == *:* ]] && dest="${spec#*:}"
    [[ -f "$src" ]] || { echo "rpm.sh: $src isn't a file" >&2; exit 2; }
    cp "$src" "$top/SOURCES/$dest"
    install_lines+="install -D -m644 %{_sourcedir}/$dest %{buildroot}/usr/lib/oxiguitar/plugins/$dest"$'\n'
    files+="/usr/lib/oxiguitar/plugins/$dest"$'\n'
done

{
    echo "Name: $package"
    echo "Version: $rpm_version"
    echo "Release: 1"
    echo "Summary: $title plugin for OxiGuitar"
    echo "License: $license"
    echo "URL: $homepage"
    echo "BuildArch: $arch"
    [[ -n "${PLUGIN_DEPENDS_RPM:-}" ]] && echo "Requires: $PLUGIN_DEPENDS_RPM"
    [[ -n "$recommends" ]] && echo "Recommends: $recommends"
    echo
    echo "%description"
    echo "$description"
    echo
    echo "# The files are built already, and stripping or debug packages would"
    echo "# only touch the plugin."
    echo "%global debug_package %{nil}"
    echo "%global __strip /bin/true"
    echo "%global _binaries_in_noarch_packages_terminate_build 0"
    echo
    echo "%install"
    printf '%s' "$install_lines"
    echo
    echo "%files"
    printf '%s' "$files"
} > "$top/SPECS/$package.spec"

rpmbuild -bb --target "$arch" \
    --define "_topdir $top" \
    --define "_rpmdir $out_dir" \
    --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
    "$top/SPECS/$package.spec"
out="$out_dir/$package-$rpm_version-1.$arch.rpm"
rpm -qpi "$out"
rpm -qpl "$out"
echo "$out"
