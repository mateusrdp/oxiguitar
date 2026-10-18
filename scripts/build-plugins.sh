#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds our plugins (every folder of plugins/ but plugins/test) as .oxp
# files and puts them in the system plugin folder of a cargo build of the
# programs, so `cargo run -p oxiguitar-app` and `oxiguitar-cli` open files.
# That folder is where the host looks beside the program
# (crates/oxiguitar-host/src/folders.rs), under target/ (or target/TRIPLE/
# with --target):
#
#   Linux    lib/oxiguitar/plugins/   (<bin>/../lib/oxiguitar/plugins, for
#                                      both target/debug and target/release)
#   macOS    PlugIns/                 (<bin>/../PlugIns, likewise)
#   Windows  debug/plugins/ and release/plugins/ (plugins\ beside the .exe)
#
# Each plugin is built on its own: built together, Cargo would give every
# plugin the reader features of all of them (docs/design/plugins.md: each
# plugin carries only its own code). They are built in target/oxp/, the
# folder oxiguitar-plugin-test also uses, so tests reuse the builds.
#
# Usage: scripts/build-plugins.sh [--debug] [--target TRIPLE] [--out FOLDER]
#   --debug   debug builds (default: release builds, as shipped)
#   --target  build for another target (packages; macOS builds both
#             architectures and joins them with lipo)
#   --out     put the files there instead

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
profile=release
target=""
out=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug) profile=debug ;;
        --target) target="$2"; shift ;;
        --out) out="$2"; shift ;;
        *) echo "build-plugins: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done

build_dir="$root/target/oxp${target:+/$target}/$profile"
base="$root/target${target:+/$target}"

# The library's file name and the plugin folder on the system the target
# is for.
system="${target:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$system" in
    *windows*) prefix="" ; suffix=".dll" ; folders=("$base/debug/plugins" "$base/release/plugins") ;;
    *apple*) prefix="lib" ; suffix=".dylib" ; folders=("$base/PlugIns") ;;
    *) prefix="lib" ; suffix=".so" ; folders=("$base/lib/oxiguitar/plugins") ;;
esac
if [[ -n "$out" ]]; then
    folders=("$out")
fi

args=(--locked --features oxp --crate-type cdylib)
if [[ "$profile" == release ]]; then
    args+=(--release)
fi
if [[ -n "$target" ]]; then
    args+=(--target "$target")
fi

for dir in "$root"/plugins/*/; do
    name="$(basename "$dir")"
    [[ "$name" == test ]] && continue
    package="oxiguitar-$name-plugin"
    CARGO_TARGET_DIR="$root/target/oxp" cargo rustc --quiet -p "$package" "${args[@]}"
    library="$build_dir/$prefix${package//-/_}$suffix"
    for folder in "${folders[@]}"; do
        mkdir -p "$folder"
        # A new file, not the old one overwritten: a running program that
        # loaded the old one keeps it.
        rm -f "$folder/$name.oxp"
        cp "$library" "$folder/$name.oxp"
        echo "build-plugins: $folder/$name.oxp"
    done
done
