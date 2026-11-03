#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Puts the plugin files of the latest release of the named plugin
# repositories (guitarpro, midi, wav, nam, ...) in one folder, for the tests
# that need real plugins (#165: the plugins live in repositories of their
# own, mateusrdp/oxiguitar-NAME). Each release's zip for this system is
# checked against the release's SHA256SUMS before it is unpacked. Prints the
# folder and, on stderr, the versions used. The tests download our own
# releases; OxiGuitar itself never downloads anything.
#
# Usage: scripts/released-plugins.sh OUT-DIR NAME...
#   OUT-DIR  where the .oxp files go (replaced)
#   NAME     guitarpro, midi, wav, nam, alsa, pipewire, ...
# Environment:
#   GH_TOKEN or GITHUB_TOKEN   used for the GitHub API when set (the rate
#                              limit without one is 60 requests an hour)
#   OXIGUITAR_RELEASES_FRESH   seconds an earlier download of the same names
#                              counts as current (default 600; 0 always asks)
# Needs curl and python.
set -euo pipefail
trap 'echo "released-plugins.sh line $LINENO: $BASH_COMMAND" >&2' ERR

out="${1:?usage: released-plugins.sh OUT-DIR NAME...}"
shift
[[ $# -gt 0 ]] || { echo "released-plugins.sh: no plugin named" >&2; exit 2; }
fresh="${OXIGUITAR_RELEASES_FRESH:-600}"
token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"

# Python is python3 on Linux and macOS, and often only python on Windows.
if command -v python3 >/dev/null 2>&1; then py=python3; else py=python; fi

mkdir -p "$out"
stamp="$out/.stamp"
want="$*"
if [[ "$fresh" -gt 0 && -f "$stamp" && "$(head -n 1 "$stamp")" == "$want" ]]; then
    age=$(($(date +%s) - $("$py" -c 'import os,sys; print(int(os.path.getmtime(sys.argv[1])))' "$stamp")))
    if [[ $age -lt $fresh ]]; then
        tail -n +2 "$stamp" >&2
        echo "$out/files"
        exit 0
    fi
fi

case "$(uname -s)" in
    Linux) system=linux ;;
    Darwin) system=macos ;;
    *) system=windows ;;
esac
case "$(uname -m)" in
    x86_64 | AMD64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) arch="$(uname -m)" ;;
esac
case "$system" in
    macos) target=macos-universal ;;
    windows) target="windows-$([[ "$arch" == aarch64 ]] && echo arm64 || echo x86_64)" ;;
    *) target="linux-$arch" ;;
esac

api() {
    local headers=(-H "Accept: application/vnd.github+json")
    [[ -n "$token" ]] && headers+=(-H "Authorization: Bearer $token")
    curl -fsSL --retry 5 "${headers[@]}" "$1"
}
sha256() {
    "$py" -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$1"
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
rm -rf "$out/files"
mkdir -p "$out/files"
versions=()
for name in "$@"; do
    repo="mateusrdp/oxiguitar-$name"
    # The latest release that is not a draft (a pre-release counts while the
    # plugins are 0.x).
    tag="$(api "https://api.github.com/repos/$repo/releases?per_page=10" \
        | "$py" -c 'import json,sys
for r in json.load(sys.stdin):
    if not r["draft"]:
        print(r["tag_name"]); break')"
    [[ -n "$tag" ]] || { echo "released-plugins.sh: $repo has no published release" >&2; exit 1; }
    version="${tag#v}"
    zip="oxiguitar-$name-$version-$target.zip"
    base="https://github.com/$repo/releases/download/$tag"
    curl -fsSL --retry 5 -o "$work/$zip" "$base/$zip"
    curl -fsSL --retry 5 -o "$work/SHA256SUMS" "$base/SHA256SUMS"
    wanted="$(awk -v n="$zip" '$2 == n || $2 == "*" n { print $1 }' "$work/SHA256SUMS")"
    [[ -n "$wanted" && "$wanted" == "$(sha256 "$work/$zip")" ]] \
        || { echo "released-plugins.sh: $zip does not match SHA256SUMS" >&2; exit 1; }
    "$py" - "$work/$zip" "$out/files" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    for n in z.namelist():
        if n.endswith(".oxp") and "/" not in n:
            open(f"{sys.argv[2]}/{n}", "wb").write(z.read(n))
PY
    versions+=("oxiguitar-$name $version")
done

{
    echo "$want"
    printf 'plugins from releases: %s\n' "${versions[*]}"
} > "$stamp"
tail -n +2 "$stamp" >&2
echo "$out/files"
