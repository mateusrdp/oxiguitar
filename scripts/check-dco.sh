#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Checks that every commit in BASE..HEAD carries a Developer Certificate of
# Origin sign-off (git commit -s) matching its author's email.
#
# Commits made by the Claude GitHub app are skipped: the DCO is a statement by
# a person. They are recognised two ways:
#   - authored by noreply@anthropic.com (commits on Claude's branches);
#   - committed by GitHub (noreply@github.com) with a Co-Authored-By trailer
#     for noreply@anthropic.com: the squash merges of Claude's pull requests
#     into develop, which GitHub attributes to the account that merged them.
# The maintainer's approval of the pull request into main is the human
# sign-off for them. See CONTRIBUTING.md.
#
# Usage: scripts/check-dco.sh BASE_SHA HEAD_SHA

set -euo pipefail

base="$1"
head="$2"
bad=0

for sha in $(git rev-list --no-merges "$base..$head"); do
    email="$(git show -s --format='%ae' "$sha")"
    subject="$(git show -s --format='%s' "$sha")"
    if [[ "$email" == "noreply@anthropic.com" ]]; then
        echo "skip  ${sha:0:10} (Claude app commit) $subject"
        continue
    fi
    committer="$(git show -s --format='%ce' "$sha")"
    if [[ "$committer" == "noreply@github.com" ]] \
        && git show -s --format='%(trailers:key=Co-authored-by,valueonly)' "$sha" \
            | grep -qiF "<noreply@anthropic.com>"; then
        echo "skip  ${sha:0:10} (squash merge of a Claude pull request) $subject"
        continue
    fi
    if git show -s --format='%(trailers:key=Signed-off-by,valueonly)' "$sha" \
        | grep -qiF "<$email>"; then
        echo "ok    ${sha:0:10} $subject"
    else
        echo "FAIL  ${sha:0:10} $subject"
        echo "      no 'Signed-off-by: ... <$email>' line; amend with: git commit --amend -s"
        bad=1
    fi
done

if [[ "$bad" -ne 0 ]]; then
    echo
    echo "Some commits are not signed off. See CONTRIBUTING.md, section 'Sign-off'."
fi
exit "$bad"
