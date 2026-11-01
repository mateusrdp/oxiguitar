#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Makes the history a new repository starts from (#165): a clone of
# oxiguitar keeping only the commits that touch what moves there, each with
# only those files, at the same paths. Issue and pull request numbers in
# commit messages ("#123") become "mateusrdp/oxiguitar#123", so they still
# point at oxiguitar's. Tags are left out: they name oxiguitar's releases.
#
# oxiguitar itself is never changed: the clone is rewritten, not the source.
# scripts/new-repo.sh NAME --history runs this and pushes the result.
#
# Usage:
#   scripts/split-history.sh NAME OUT [--ref REF] [--from REPO]
#
#   NAME        the new repository (oxiguitar-guitarpro, oxiguitar-midi, ...)
#   OUT         a folder that doesn't exist yet; it becomes a repository
#               whose only branch, main, holds the history
#   --ref REF   the branch to take (default: main, the released code)
#   --from REPO where to clone from (default: https://github.com/mateusrdp/oxiguitar.git)
#   --list      print the paths NAME keeps and stop
#
# Needs git and git-filter-repo (Debian and Ubuntu: apt install
# git-filter-repo; elsewhere: pip install git-filter-repo).

set -euo pipefail

# What moves to each repository, at the same paths (docs: the #165 plan).
paths_of() {
    case "$1" in
        oxiguitar-guitarpro)
            echo plugins/gp3 plugins/gp4 plugins/gp5 plugins/gpx plugins/gp \
                crates/oxiguitar-formats \
                fixtures/gp fixtures/guitar-pro-7.6 fixtures/guitar-pro-check \
                corpus fuzz \
                scripts/guitar-pro-check.sh scripts/interop \
                .github/workflows/interop.yml .github/workflows/fuzz.yml ;;
        oxiguitar-midi)
            echo plugins/midi plugins/soundfont fixtures/sfz .github/workflows/listening.yml ;;
        oxiguitar-nam)
            echo plugins/nam fixtures/nam ;;
        oxiguitar-wav)
            echo plugins/wav ;;
        oxiguitar-pipewire | oxiguitar-alsa | oxiguitar-wasapi | oxiguitar-coreaudio)
            echo "plugins/audio-${1#oxiguitar-}" ;;
        *) return 1 ;;
    esac
}

usage() { sed -n '/^# Usage:/,/^# Needs git/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'; }

name=""
out=""
ref="main"
from="https://github.com/mateusrdp/oxiguitar.git"
list=0
while (($#)); do
    case "$1" in
        --ref) ref="${2:?--ref needs a branch}"; shift 2 ;;
        --from) from="${2:?--from needs a repository}"; shift 2 ;;
        --list) list=1; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
        *)
            if [[ -z "$name" ]]; then name="$1"
            elif [[ -z "$out" ]]; then out="$1"
            else echo "too many arguments" >&2; exit 2
            fi
            shift ;;
    esac
done
[[ -n "$name" ]] || { usage >&2; exit 2; }
if ! read -r -a paths < <(paths_of "$name"); then
    echo "$name has no history to take from oxiguitar (it starts empty)" >&2
    exit 3
fi
if ((list)); then
    printf '%s\n' "${paths[@]}"
    exit 0
fi
[[ -n "$out" ]] || { usage >&2; exit 2; }
[[ ! -e "$out" ]] || { echo "$out already exists" >&2; exit 2; }
git filter-repo --version > /dev/null 2>&1 \
    || { echo "needs git-filter-repo (apt install git-filter-repo, or pip install git-filter-repo)" >&2; exit 1; }

git clone --quiet --no-local --no-tags --single-branch --branch "$ref" "$from" "$out"
args=()
for p in "${paths[@]}"; do args+=(--path "$p"); done
log="$(mktemp)"
if ! git -C "$out" filter-repo --force "${args[@]}" \
    --message-callback '
return re.sub(rb"(?<![\w/.#-])#([0-9]+)\b", rb"mateusrdp/oxiguitar#\1", message)' \
    > "$log" 2>&1; then
    cat "$log" >&2
    rm -f "$log"
    exit 1
fi
rm -f "$log"
[[ "$ref" == main ]] || git -C "$out" branch --quiet -m "$ref" main

count="$(git -C "$out" rev-list --count main)"
echo "$name: $count commits from oxiguitar's $ref, keeping: ${paths[*]}"
