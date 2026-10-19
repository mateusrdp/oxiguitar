#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds a plugin from the template the way the plugin developer guide
# says (docs/plugins/, "Start from the template"), and checks that it works:
# M2's gate "the developer guide builds a working plugin from the template".
#
#   1. Copies docs/plugins/template/ to a folder outside this repository, as
#      a plugin author would, and points its two OxiGuitar dependencies at
#      this checkout instead of the release tag (the only change).
#   2. cargo fmt, clippy and test in the copy: its unit tests, the test
#      export_plugin! adds, and tests/contract.rs, which builds the .oxp
#      file and runs the contract suite on it.
#   3. oxiguitar-plugin-test --build, the guide's command, and the contract
#      suite again from the command line; then build-oxp.sh, the release file
#      the template's workflow publishes (one universal file on macOS), and
#      the contract suite on that.
#   4. Puts the release file in a plugin folder and opens the template's
#      sample through it with oxiguitar-cli (info, and a PNG of the first
#      page).
#
# Usage: scripts/check-plugin-template.sh [WORK-FOLDER]
#   WORK-FOLDER  where to copy the template (default: a new temporary folder)
# Needs oxiguitar-cli and oxiguitar-plugin-test built (cargo build -p ...);
# it builds them if they are missing.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${1:-$(mktemp -d)}"

# Every example the guide includes from the template is there (mdBook
# includes nothing, silently, for an anchor it can't find).
missing=0
while IFS= read -r include; do
    file="${include%%:*}"
    anchor="${include#*:}"
    path="$root/docs/plugins/src/$file"
    if [[ ! -f "$path" ]]; then
        echo "check-plugin-template: the guide includes $file, which doesn't exist" >&2
        missing=1
    elif [[ "$anchor" != "$include" ]] && ! grep -Eq "ANCHOR: $anchor[[:space:]]*$" "$path"; then
        echo "check-plugin-template: the guide includes $file:$anchor, an anchor it doesn't have" >&2
        missing=1
    fi
done < <(grep -oh '{{#include [^}]*}}' "$root"/docs/plugins/src/*.md | sed 's/{{#include \(.*\)}}/\1/')
[[ "$missing" == 0 ]]

mkdir -p "$work"
plugin="$work/example-plugin"
rm -rf "$plugin"
cp -R "$root/docs/plugins/template" "$plugin"
rm -rf "$plugin/target"

# The template depends on a release tag; built here, it uses this checkout.
# (Cargo fetches a git dependency even when it is patched, so the lines are
# rewritten rather than patched.)
manifest="$plugin/Cargo.toml"
git_dep='git = "https://github.com/mateusrdp/oxiguitar", tag = "v[0-9.]*"'
grep -q "^oxiguitar-plugin = { $git_dep }" "$manifest"
grep -q "^oxiguitar-plugin-test = { $git_dep }" "$manifest"
sdk="$root/crates/oxiguitar-plugin"
suite="$root/crates/oxiguitar-plugin-test"
if command -v cygpath >/dev/null; then
    sdk="$(cygpath -m "$sdk")"
    suite="$(cygpath -m "$suite")"
fi
sed -e "s|^oxiguitar-plugin = { $git_dep }|oxiguitar-plugin = { path = \"$sdk\" }|" \
    -e "s|^oxiguitar-plugin-test = { $git_dep }|oxiguitar-plugin-test = { path = \"$suite\" }|" \
    "$manifest" > "$manifest.new"
mv "$manifest.new" "$manifest"
echo "check-plugin-template: the template copied to $plugin"

cd "$plugin"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# The guide's command line: build and check the .oxp file.
exe=""
[[ "$(rustc -vV | sed -n 's/^host: //p')" == *windows* ]] && exe=".exe"
bin="$root/target/debug"
for program in oxiguitar-plugin-test oxiguitar-cli; do
    if [[ ! -x "$bin/$program$exe" ]]; then
        (cd "$root" && cargo build --locked -p "$program")
    fi
done
"$bin/oxiguitar-plugin-test$exe" --build example-plugin example
test -f "$plugin/target/oxp/example.oxp"

# The release file for this system, as the template's workflow makes it
# (one universal file on macOS), and the contract suite on it.
oxp="$(./build-oxp.sh | tail -n 1)"
test -f "$oxp"
"$bin/oxiguitar-plugin-test$exe" "$oxp"
if [[ "$oxp" == *macos-universal.oxp ]]; then
    lipo -info "$oxp" | grep -q "x86_64 arm64"
fi

# Where a user puts the file: a plugin folder (OXIGUITAR_HOME's plugins/
# here; ~/.local/share/oxiguitar/plugins/ and the like for a user).
home="$work/home"
rm -rf "$home"
mkdir -p "$home/plugins"
cp "$oxp" "$home/plugins/example.oxp"
export OXIGUITAR_HOME="$home"
sample="$plugin/samples/open-strings.extab"
"$bin/oxiguitar-cli$exe" info "$sample" | tee "$work/info.txt"
grep -q "Example tab" "$work/info.txt"
grep -q "Open Strings" "$work/info.txt"
"$bin/oxiguitar-cli$exe" render "$sample" --png "$work/open-strings" > /dev/null
ls "$work"/open-strings*.png
echo "check-plugin-template: the template builds a working plugin"
