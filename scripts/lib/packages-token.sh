# shellcheck shell=bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Sourced by new-repo.sh and enable-pages.sh. GitHub never shows a secret's
# value, not even to its owner, so PACKAGES_TOKEN can't be copied from one
# repository to another: the value has to come from you. This asks for it
# once per run and keeps it in a variable, never in a file or an argument
# list.
#
#   packages_token_value   sets PACKAGES_TOKEN_VALUE and returns 0, or
#                          returns 1 when there is no value to give
#
# The value comes from the environment variable OXIGUITAR_PACKAGES_TOKEN
# (the token that lets a publish workflow send the `plugin-published` event
# to oxiguitar-packages: a token of the maintainer's with contents: write on
# that repository), or, when that is empty and a terminal is attached, from
# a hidden prompt.

PACKAGES_TOKEN_VALUE=""
PACKAGES_TOKEN_ASKED=0

packages_token_value() {
    [[ -z "$PACKAGES_TOKEN_VALUE" ]] || return 0
    if [[ -n "${OXIGUITAR_PACKAGES_TOKEN:-}" ]]; then
        PACKAGES_TOKEN_VALUE="$OXIGUITAR_PACKAGES_TOKEN"
        return 0
    fi
    ((PACKAGES_TOKEN_ASKED == 0)) || return 1
    PACKAGES_TOKEN_ASKED=1
    [[ -t 0 && -t 2 ]] || return 1
    local answer=""
    printf 'PACKAGES_TOKEN (typed text is hidden; empty to skip the secret): ' >&2
    IFS= read -rs answer || true
    printf '\n' >&2
    [[ -n "$answer" ]] || return 1
    PACKAGES_TOKEN_VALUE="$answer"
}

# set_packages_token OWNER/REPO: gives the repository's Actions secret
# PACKAGES_TOKEN the value, over stdin so it never shows in a process list.
set_packages_token() {
    printf '%s' "$PACKAGES_TOKEN_VALUE" | gh secret set PACKAGES_TOKEN --repo "$1" --app actions > /dev/null
}
