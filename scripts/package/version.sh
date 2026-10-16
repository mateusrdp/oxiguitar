#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Prints the version the release packages carry.
#
# - For a v* tag (GITHUB_REF_TYPE=tag): the workspace version, which must
#   match the tag (v0.1.0 needs version = "0.1.0" in Cargo.toml).
# - Otherwise (a build started by hand, or a pull request): the workspace
#   version plus "~dev.DATE.gCOMMIT", which sorts before that version in
#   dpkg and rpm, so the real release later replaces it.
#
# Usage: scripts/package/version.sh

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cargo="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")"
[[ -n "$cargo" ]] || { echo "no version in Cargo.toml's [workspace.package]" >&2; exit 1; }

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
    tag="${GITHUB_REF_NAME:?}"
    if [[ "$tag" != "v$cargo" ]]; then
        echo "tag $tag does not match the workspace version $cargo (expected v$cargo)" >&2
        exit 1
    fi
    echo "$cargo"
else
    commit="$(git -C "$root" rev-parse --short=7 HEAD)"
    echo "$cargo~dev.$(date -u +%Y%m%d).g$commit"
fi
