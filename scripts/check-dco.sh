#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Checks that every commit in BASE..HEAD carries a Developer Certificate of
# Origin sign-off (git commit -s) matching its author's email.
#
# Commits made by automated bots are skipped: the DCO is a statement by a
# person, and bots cannot satisfy it. They are recognised by their author
# email or by being committed by GitHub on behalf of the bot:
#   - Claude GitHub app (noreply@anthropic.com, or noreply@github.com with
#     a Co-Authored-By for noreply@anthropic.com — squash merges into develop);
#   - Dependabot (noreply@github.com with a Co-Authored-By for dependabot[bot],
#     or authored directly by dependabot[bot]@users.noreply.github.com);
#     Dependabot adds a Signed-off-by line, but its email (support@github.com)
#     never matches the author email, so the DCO check would always fail.
# The maintainer's review and merge of the pull request is the human sign-off
# for bot commits. See CONTRIBUTING.md.
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
    # Dependabot: authored directly by the bot, or committed by GitHub with
    # a Co-Authored-By for dependabot[bot]. Dependabot's Signed-off-by email
    # (support@github.com) never matches its author email, so the DCO check
    # would always fail; the maintainer's review is the human sign-off.
    if [[ "$email" == *dependabot* ]]; then
        echo "skip  ${sha:0:10} (Dependabot commit) $subject"
        continue
    fi
    if [[ "$committer" == "noreply@github.com" ]] \
        && git show -s --format='%(trailers:key=Co-authored-by,valueonly)' "$sha" \
            | grep -qiF "<dependabot[bot]@users.noreply.github.com>"; then
        echo "skip  ${sha:0:10} (Dependabot squash merge) $subject"
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
