#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the plugin's release file for the system this runs on:
#
#   dist/NAME-VERSION-SYSTEM-ARCH.oxp
#
# such as dist/example-0.1.0-linux-x86_64.oxp, named the way OxiGuitar's own
# plugins are: an uncompressed tar archive of manifest.json, the library (as
# plugin.so, plugin.dll or plugin.dylib) and the ui and assets folders, if
# there are any (the plugin guide's "Building for every system"). On macOS it builds both processors and joins them into one
# universal file (needs both Rust targets:
# rustup target add aarch64-apple-darwin x86_64-apple-darwin).
# On Windows, run it from Git Bash. .github/workflows/build.yml runs it on
# each system.

set -euo pipefail

# The plugin's file name, and the crate's library name (the package's name
# with _ for -). Change both when you rename the plugin.
NAME=example
LIBRARY=example_plugin

cd "$(dirname "$0")"
version="$(cargo pkgid)"
version="${version##*[#@]}"
host="$(rustc -vV | sed -n 's/^host: //p')"
mkdir -p dist

build() {
    cargo rustc --release --features oxp --crate-type cdylib "$@"
}

# pack LIBRARY OUT: the plugin file OUT, holding manifest.json first, the
# library second, then the ui and assets folders. COPYFILE_DISABLE keeps
# macOS's tar from adding ._ files.
pack() {
    local ext="${1##*.}" stage folder
    local more=()
    stage="$(mktemp -d)"
    cp manifest.json "$stage/manifest.json"
    cp "$1" "$stage/plugin.$ext"
    for folder in ui assets; do
        if [[ -d "$folder" ]]; then
            cp -R "$folder" "$stage/"
            more+=("$folder")
        fi
    done
    (cd "$stage" && COPYFILE_DISABLE=1 tar --format=ustar -cf - manifest.json "plugin.$ext" ${more[@]+"${more[@]}"}) > "$2"
    rm -rf "$stage"
}

case "$host" in
    *apple*)
        out="dist/$NAME-$version-macos-universal.oxp"
        for target in aarch64-apple-darwin x86_64-apple-darwin; do
            build --target "$target"
        done
        lipo -create -output "target/lib$LIBRARY-universal.dylib" \
            "target/aarch64-apple-darwin/release/lib$LIBRARY.dylib" \
            "target/x86_64-apple-darwin/release/lib$LIBRARY.dylib"
        pack "target/lib$LIBRARY-universal.dylib" "$out"
        ;;
    *windows*)
        arch="${host%%-*}"
        [[ "$arch" == aarch64 ]] && arch=arm64
        out="dist/$NAME-$version-windows-$arch.oxp"
        build
        pack "target/release/$LIBRARY.dll" "$out"
        ;;
    *)
        out="dist/$NAME-$version-linux-${host%%-*}.oxp"
        build
        pack "target/release/lib$LIBRARY.so" "$out"
        ;;
esac
echo "$out"
