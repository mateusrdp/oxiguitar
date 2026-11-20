#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Turns on GitHub Pages, published from GitHub Actions, in every OxiGuitar
# repository that has a manual (docs/manual/book.toml on its default
# branch), so that its Manual workflow (.github/workflows/docs.yml) can
# deploy. For each repository it makes sure of three things, changing only
# what differs:
#
#   1. Pages exists and its source is "GitHub Actions".
#   2. The github-pages environment lets main deploy (a ruleset doesn't
#      count as a protected branch there, so main is listed by name).
#   3. The Actions secret PACKAGES_TOKEN exists: the publish workflow needs
#      it to tell oxiguitar-packages about a release. A secret's value can't
#      be read back, so it can't be copied from a working repository: give
#      it in the environment variable OXIGUITAR_PACKAGES_TOKEN, or type it
#      when asked (once for the whole run). A secret that already exists is
#      left alone.
#
# Run by the maintainer on his own machine with his gh login (Claude's
# sessions can't touch Pages or secrets). Needs gh and jq, and admin on the
# repositories.
#
# Usage:
#   scripts/enable-pages.sh [--dry-run] [--no-token] [NAME...]
#
#   NAME...      repository names (default: every oxiguitar* repository of
#                the owner that has a manual)
#   --no-token   leave PACKAGES_TOKEN alone
#   --dry-run    read everything, change nothing: print each call that
#                would change something
#
# Run it again at any time: what already matches is left alone. When it is
# done, the Manual workflow runs on the next push to main that touches
# docs/manual/, or by hand: Actions, Manual, Run workflow, branch main.

set -euo pipefail

OWNER="${OXIGUITAR_OWNER:-mateusrdp}"
dry=0
token=1
names=()
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/packages-token.sh
source "$here/lib/packages-token.sh"

usage() { sed -n '/^# Usage:/,/^# Run it again/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'; }

while (($#)); do
    case "$1" in
        --dry-run) dry=1; shift ;;
        --no-token) token=0; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
        *)
            [[ "$1" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "not a repository name: $1" >&2; exit 2; }
            names+=("$1"); shift ;;
    esac
done
for tool in gh jq; do
    command -v "$tool" > /dev/null || { echo "needs $tool" >&2; exit 1; }
done

err="$(mktemp "${TMPDIR:-/tmp}/enable-pages.XXXXXX")"
trap 'rm -f "$err"' EXIT

# get PATH: the JSON on stdout; returns 1 on 404; dies on anything else.
get() {
    local out
    if out="$(gh api "$1" 2> "$err")"; then
        printf '%s\n' "$out"
        return 0
    fi
    if grep -q 'HTTP 404' "$err"; then
        return 1
    fi
    echo "GET $1 failed:" >&2
    cat "$err" >&2
    exit 1
}

# call METHOD PATH [JSON]: a change. Printed, and in a dry run only printed.
call() {
    local method="$1" path="$2" body="${3:-}"
    if ((dry)); then
        echo "  would: gh api -X $method $path${body:+ $(jq -c . <<< "$body")}"
        return 0
    fi
    echo "  gh api -X $method $path"
    if [[ -n "$body" ]]; then
        gh api -X "$method" "$path" --input - <<< "$body" > /dev/null
    else
        gh api -X "$method" "$path" > /dev/null
    fi
}

if ((${#names[@]} == 0)); then
    echo "Looking for $OWNER's repositories that have a manual..."
    while IFS= read -r n; do
        if get "repos/$OWNER/$n/contents/docs/manual/book.toml" > /dev/null; then
            names+=("$n")
        else
            echo "  skipped $n (no docs/manual/book.toml)"
        fi
    done < <(gh repo list "$OWNER" --limit 200 --no-archived --json name --jq '.[].name | select(startswith("oxiguitar"))' | sort)
fi
((${#names[@]})) || { echo "no repository to do" >&2; exit 1; }

((token)) && packages_token_value || true
if ((token)) && [[ -z "$PACKAGES_TOKEN_VALUE" ]]; then
    echo "No PACKAGES_TOKEN value given: the secret is left alone."
    token=0
fi

changed=()
for n in "${names[@]}"; do
    repo="repos/$OWNER/$n"
    echo
    echo "$OWNER/$n"

    # 1. Pages from GitHub Actions.
    if pages="$(get "$repo/pages")"; then
        if [[ "$(jq -r .build_type <<< "$pages")" != workflow ]]; then
            call PUT "$repo/pages" '{"build_type":"workflow"}'
            changed+=("$n: Pages source is GitHub Actions")
        fi
    else
        call POST "$repo/pages" '{"build_type":"workflow"}'
        changed+=("$n: Pages turned on")
    fi

    # 2. The github-pages environment lets main deploy.
    want='{"deployment_branch_policy":{"protected_branches":false,"custom_branch_policies":true}}'
    have="$(get "$repo/environments/github-pages" | jq '{deployment_branch_policy}' || echo '{}')"
    if [[ "$(jq -S -c . <<< "$have")" != "$(jq -S -c . <<< "$want")" ]]; then
        call PUT "$repo/environments/github-pages" "$want"
        changed+=("$n: github-pages environment limited to named branches")
    fi
    policies="$(get "$repo/environments/github-pages/deployment-branch-policies" | jq -r '[.branch_policies[].name] | join(" ")' || true)"
    if [[ " $policies " != *" main "* ]]; then
        call POST "$repo/environments/github-pages/deployment-branch-policies" '{"name":"main","type":"branch"}'
        changed+=("$n: github-pages may deploy from main")
    fi

    # 3. PACKAGES_TOKEN.
    if ((token)); then
        if gh api "$repo/actions/secrets/PACKAGES_TOKEN" > /dev/null 2>&1; then
            echo "  PACKAGES_TOKEN already set"
        elif ((dry)); then
            echo "  would: set the secret PACKAGES_TOKEN"
            changed+=("$n: PACKAGES_TOKEN set")
        else
            set_packages_token "$OWNER/$n"
            echo "  set the secret PACKAGES_TOKEN"
            changed+=("$n: PACKAGES_TOKEN set")
        fi
    fi
done

echo
if ((dry)); then echo "Would change:"; else echo "Changed:"; fi
if ((${#changed[@]})); then printf '  - %s\n' "${changed[@]}"; else echo "  nothing: everything already matches"; fi
