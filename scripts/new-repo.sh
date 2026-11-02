#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Creates github.com/mateusrdp/NAME set up like mateusrdp/oxiguitar for
# working with Claude, or brings an existing one back in line with it.
#
# Every setting is read from oxiguitar through the API when the script runs,
# never written down here: repository settings, the rulesets, Actions
# permissions, Dependabot and security settings, labels, Pages and its
# environment. The starting files (LICENSE, README.md, CONTRIBUTING.md,
# CLAUDE.md, scripts/check-dco.sh, .github/...) take oxiguitar's current
# check-dco.sh, action pins, MSRV, rustfmt.toml, clippy.toml and deny.toml.
#
# Run by the maintainer on his own machine with his gh login (Claude's
# sessions can't write to other repositories). Needs gh, jq and git.
#
# Usage:
#   scripts/new-repo.sh NAME [--description TEXT] [--license ID]
#                            [--history [--ref REF]] [--app SLUG] [--dry-run]
#
#   --description TEXT  the repository's description (kept if not given)
#   --license ID        GPL-3.0-or-later (default), MIT, Apache-2.0 or
#                       "MIT OR Apache-2.0" (LICENSE-MIT and LICENSE-APACHE)
#   --history           start from oxiguitar's history of what moves to NAME
#                       (scripts/split-history.sh; needs git-filter-repo),
#                       the starting files committed on top; only into a
#                       repository that has no main yet
#   --ref REF           with --history: oxiguitar's branch to take (default main)
#   --app SLUG          the GitHub App to give access (default: claude)
#   --dry-run           read everything, change nothing: print each call
#                       that would change something, and leave the files the
#                       first commit would hold in a folder to look at
#
# Run it again at any time: what already matches is left alone, what
# differs is changed back, and nothing else is touched. Files are never
# overwritten once the repository has them (they belong to its history and
# grow with it); a starting file it lacks is offered as a pull request.

set -euo pipefail

OWNER="${OXIGUITAR_OWNER:-mateusrdp}"
SOURCE="${OXIGUITAR_SOURCE:-oxiguitar}"

name=""
description=""
license="GPL-3.0-or-later"
app="claude"
dry=0
history=0
ref="main"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() { sed -n '/^# Usage:/,/^# Run it again/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'; }

while (($#)); do
    case "$1" in
        --description) description="${2:?--description needs a text}"; shift 2 ;;
        --license) license="${2:?--license needs an id}"; shift 2 ;;
        --app) app="${2:?--app needs a slug}"; shift 2 ;;
        --history) history=1; shift ;;
        --ref) ref="${2:?--ref needs a branch}"; shift 2 ;;
        --dry-run) dry=1; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
        *)
            [[ -z "$name" ]] || { echo "one repository at a time" >&2; exit 2; }
            name="$1"; shift ;;
    esac
done
[[ -n "$name" ]] || { usage >&2; exit 2; }
[[ "$name" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "not a repository name: $name" >&2; exit 2; }
[[ "$name" != "$SOURCE" ]] || { echo "that is the source repository" >&2; exit 2; }
case "$license" in
    GPL-3.0-or-later | MIT | Apache-2.0 | "MIT OR Apache-2.0") ;;
    *) echo "unsupported licence: $license" >&2; exit 2 ;;
esac
for tool in gh jq git; do
    command -v "$tool" > /dev/null || { echo "needs $tool" >&2; exit 1; }
done
if ((history)); then
    "$here/split-history.sh" "$name" --list > /dev/null || exit 2
    git filter-repo --version > /dev/null 2>&1 \
        || { echo "--history needs git-filter-repo (apt install git-filter-repo, or pip install git-filter-repo)" >&2; exit 1; }
fi

src="repos/$OWNER/$SOURCE"
dst="repos/$OWNER/$name"
work="$(mktemp -d "${TMPDIR:-/tmp}/new-repo.XXXXXX")"
if ((dry)); then
    echo "Dry run: nothing is changed. Files go to $work/files"
else
    trap 'rm -rf "$work"' EXIT
fi

changed=()
manual=()

# A failed read can happen inside $(...), where exit only leaves the
# subshell: it leaves a mark that stops the run before the next change.
fail() {
    echo "$1 failed:" >&2
    cat "$2" >&2
    touch "$work/failed"
    exit 1
}
stop_if_failed() {
    if [[ -e "$work/failed" ]]; then
        echo "stopped: a read failed (above); nothing more was changed" >&2
        exit 1
    fi
}
note() { changed+=("$1"); }
todo() { manual+=("$1"); }

# --- The API ---------------------------------------------------------------

# get PATH: the JSON on stdout; returns 1 on 404 (and prints nothing), dies
# on anything else.
get() {
    local out err
    err="$work/err"
    if out="$(gh api "$1" 2> "$err")"; then
        printf '%s\n' "$out"
        return 0
    fi
    if grep -q 'HTTP 404' "$err"; then
        return 1
    fi
    fail "GET $1" "$err"
}

# get_all PATH [KEY]: every page of a list, as one JSON array; KEY for
# lists wrapped in an object ({"total_count": n, "installations": [...]}).
get_all() {
    local err="$work/err"
    if ! gh api --paginate "$1" --jq ".${2:+$2}[]" 2> "$err" | jq -s '.'; then
        fail "GET $1" "$err"
    fi
}

# status PATH: the HTTP status of a GET whose answer is only a status
# (204 or 404), such as vulnerability-alerts.
status() {
    if gh api "$1" > /dev/null 2> "$work/err"; then
        echo 204
    elif grep -q 'HTTP 404' "$work/err"; then
        echo 404
    else
        fail "GET $1" "$work/err"
    fi
}

# call METHOD PATH [JSON]: a change. Printed, and in a dry run only printed.
call() {
    local method="$1" path="$2" body="${3:-}"
    stop_if_failed
    if ((dry)); then
        if [[ -n "$body" ]]; then
            echo "  would: gh api -X $method $path --input - <<< $(jq -c . <<< "$body")"
        else
            echo "  would: gh api -X $method $path"
        fi
        return 0
    fi
    echo "  gh api -X $method $path"
    if [[ -n "$body" ]]; then
        gh api -X "$method" "$path" --input - <<< "$body" > /dev/null
    else
        gh api -X "$method" "$path" > /dev/null
    fi
}

# Each section says when it found nothing to change.
mark=0
section() { echo; echo "$1"; mark=${#changed[@]}; }
end_section() { ((${#changed[@]} > mark)) || echo "  already matches"; }

# A dry run on a repository that doesn't exist yet: its lists are empty.
new_in_dry_run() { ((created && dry)); }

same() { [[ "$(jq -S . <<< "$1")" == "$(jq -S . <<< "$2")" ]]; }

raw() { gh api -H 'Accept: application/vnd.github.raw' "$src/contents/$1?ref=$2"; }

# --- Who runs it -------------------------------------------------------------

login="$(gh api user --jq .login)"
echo "Signed in to GitHub as $login; copying $OWNER/$SOURCE to $OWNER/$name"
[[ "$login" == "$OWNER" ]] || echo "warning: $OWNER/$name is created by $login, who must be an admin there" >&2

source_repo="$(get "$src")"
default_branch="$(jq -r .default_branch <<< "$source_repo")"

# --- 1. The repository -------------------------------------------------------

echo
echo "1. The repository"
if target="$(get "$dst")"; then
    echo "  $OWNER/$name exists"
    created=0
else
    echo "  $OWNER/$name does not exist: creating it"
    body="$(jq -n --arg n "$name" --arg d "$description" \
        '{name: $n, description: $d, private: false, auto_init: false}')"
    if [[ "$login" == "$OWNER" ]]; then
        call POST user/repos "$body"
    else
        call POST "orgs/$OWNER/repos" "$body"
    fi
    note "created $OWNER/$name"
    created=1
    if ((dry)); then
        target='{}'
    else
        target="$(get "$dst")"
    fi
fi

# The settings oxiguitar has, as one PATCH, sending only what differs.
keys='["visibility","has_issues","has_projects","has_wiki","has_discussions",
  "allow_squash_merge","allow_merge_commit","allow_rebase_merge",
  "allow_auto_merge","allow_update_branch","delete_branch_on_merge",
  "squash_merge_commit_title","squash_merge_commit_message",
  "merge_commit_title","merge_commit_message",
  "web_commit_signoff_required","allow_forking"]'
patch="$(jq -n --argjson s "$source_repo" --argjson t "$target" --argjson k "$keys" \
    --arg d "$description" '
    reduce $k[] as $key ({};
        if ($s | has($key)) and ($s[$key] != $t[$key]) then . + {($key): $s[$key]} else . end)
    | if $d != "" and $d != $t.description then . + {description: $d} else . end')"
# Secret scanning and push protection, as oxiguitar has them (Dependabot's
# security updates are set below, through their own endpoint).
security="$(jq -n --argjson s "$source_repo" --argjson t "$target" '
    [ "secret_scanning", "secret_scanning_push_protection",
      "secret_scanning_non_provider_patterns", "secret_scanning_validity_checks" ] as $ks
    | reduce $ks[] as $k ({};
        ($s.security_and_analysis[$k].status // null) as $want
        | if $want != null and $want != ($t.security_and_analysis[$k].status // null)
          then . + {($k): {status: $want}} else . end)')"
if [[ "$security" != "{}" ]]; then
    patch="$(jq --argjson sec "$security" '. + {security_and_analysis: $sec}' <<< "$patch")"
fi
if [[ "$patch" != "{}" ]]; then
    call PATCH "$dst" "$patch"
    note "repository settings: $(jq -r 'keys | join(", ")' <<< "$patch")"
else
    echo "  settings already match"
fi

# --- 2. The first commit and the branches -----------------------------------

echo
echo "2. Files and branches"

files="$work/files"
mkdir -p "$files/.github/workflows" "$files/scripts"

ci_source="$(raw .github/workflows/ci.yml "$default_branch")"
pin() {
    # The ref oxiguitar's CI uses for an action, e.g. pin actions/checkout -> v5.
    local ref
    ref="$(grep -oE "uses: $1@[^ ]+" <<< "$ci_source" | head -n 1 | sed 's/.*@//')"
    [[ -n "$ref" ]] || { echo "oxiguitar's ci.yml no longer uses $1; update new-repo.sh" >&2; exit 1; }
    printf '%s' "$ref"
}
msrv="$(grep -oE '^  MSRV: "[0-9.]+"' <<< "$ci_source" | grep -oE '[0-9.]+')"
[[ -n "$msrv" ]] || { echo "no MSRV in oxiguitar's ci.yml" >&2; exit 1; }
checkout="$(pin actions/checkout)"
toolchain_msrv="$(grep -oE 'uses: dtolnay/rust-toolchain@master' <<< "$ci_source" | head -n 1 | sed 's/.*@//')"
toolchain_stable="$(grep -oE 'uses: dtolnay/rust-toolchain@stable' <<< "$ci_source" | head -n 1 | sed 's/.*@//')"
[[ -n "$toolchain_msrv" && -n "$toolchain_stable" ]] || { echo "oxiguitar's ci.yml changed how it picks Rust; update new-repo.sh" >&2; exit 1; }
cache="$(pin Swatinem/rust-cache)"
deny_action="$(pin EmbarkStudios/cargo-deny-action)"

# The licence texts are oxiguitar's own: the core's GPL, and the plugin
# SDK's MIT and Apache files.
sdk=crates/oxiguitar-plugin
case "$license" in
    GPL-3.0-or-later) raw LICENSE "$default_branch" > "$files/LICENSE"; license_files=(LICENSE) ;;
    MIT) raw "$sdk/LICENSE-MIT" "$default_branch" > "$files/LICENSE"; license_files=(LICENSE) ;;
    Apache-2.0) raw "$sdk/LICENSE-APACHE" "$default_branch" > "$files/LICENSE"; license_files=(LICENSE) ;;
    "MIT OR Apache-2.0")
        raw "$sdk/LICENSE-MIT" "$default_branch" > "$files/LICENSE-MIT"
        raw "$sdk/LICENSE-APACHE" "$default_branch" > "$files/LICENSE-APACHE"
        license_files=(LICENSE-MIT LICENSE-APACHE) ;;
esac

about="${description:-$(jq -r '.description // ""' <<< "$target")}"
[[ -n "$about" ]] || about="Part of OxiGuitar, the guitar tablature viewer, editor and player."

cat > "$files/README.md" << EOF
# $name

$about

Part of [OxiGuitar](https://github.com/$OWNER/$SOURCE), a free, open-source
guitar tablature and score viewer, editor and player for Windows, macOS and
Linux. The user manual and the plugin developer guide live in the
[OxiGuitar repository](https://github.com/$OWNER/$SOURCE).

## Licence

$license. See $(printf '%s ' "${license_files[@]}" | sed 's/ $//; s/ / and /').
Contributions are welcome under the same terms; see
[CONTRIBUTING.md](CONTRIBUTING.md).
EOF

cat > "$files/CONTRIBUTING.md" << EOF
# Contributing to $name

Thank you for helping. Pull requests are welcome from anyone; the maintainer,
Mateus Rocha de Paula ([@$OWNER](https://github.com/$OWNER)), reviews them.
This repository is part of OxiGuitar and follows
[its contributing guide](https://github.com/$OWNER/$SOURCE/blob/$default_branch/CONTRIBUTING.md)
(code standards, file headers, test files, design principles). What follows
is the part every pull request meets first.

## Branches and pull requests

- \`develop\` is where work lands. Branch from it and open your pull request
  against it.
- \`main\` holds released work. Only the maintainer merges into it and tags
  releases.
- Feature branches are squash-merged into \`develop\`; \`develop\` is merged
  into \`main\` with a merge commit.
- CI must be green. The \`ci-success\` check sums up every other job.

## Sign-off (Developer Certificate of Origin)

We use the [Developer Certificate of Origin](https://developercertificate.org/)
instead of a contributor license agreement. By signing off a commit you state
that you wrote it, or otherwise have the right to submit it under the
project's license. Sign off with \`-s\`:

\`\`\`sh
git commit -s -m "Describe the change"
\`\`\`

This adds a line such as \`Signed-off-by: Your Name <you@example.com>\`, which
must match the commit's author email. To fix a branch afterwards:
\`git rebase --signoff develop\`, then force-push your branch.

Commits made by the Claude GitHub app, which writes much of the code with the
maintainer, carry a \`Co-Authored-By: Claude\` trailer instead. The DCO is a
statement by a person, so the check (\`scripts/check-dco.sh\`) skips them, and
also skips GitHub's squash merges of those pull requests into \`develop\` and
Dependabot's commits. The maintainer's review and merge of the pull request
into \`main\` is the human sign-off for that work.

You keep the copyright on what you contribute.
EOF

cat > "$files/CLAUDE.md" << EOF
# Working on $name with Claude

This repository is part of OxiGuitar. Before anything else, every session
reads \`claude/START-HERE.md\` in the OxiGuitar project on claude.ai (the
project's files, not this repository), then the brief and roadmap it links
to, then this repository's \`develop\` branch and the OxiGuitar repository's
(github.com/$OWNER/$SOURCE). START-HERE.md is the source of truth for
decisions; this file only repeats the standing rules.

## Standing rules

- Libraries exactly as published: no forks, no [patch], no vendored or edited copies; never plan on or wait for upstream changes.
- Never commit the corpus (tabs.tar.gz, scores.tar.gz) or publish renders of it.
- No telemetry, no uploads, no downloads by the app.
- No emojis anywhere, including PR attribution lines.
- Keep README.md up to date with screenshots and example sounds.
- I merge PRs and create v* tags. Whenever I need to do something on git, give me step-by-step instructions with the exact commands.
- Stack PRs to save CI runs; I merge stacks with my merge-stack.sh.
- Open PRs with the REST API (gh pr create fails in your sessions).
- Send test-build links, with their expiry dates, for each PR.
- Locally you only have rustc 1.97: lower rust-version to 1.97 for nested plugin builds, always restore $msrv, and delete target/lib after build-plugins.sh.
- When checking a local test run, look at the exit code and every FAILED line.
- Never rewrite a pushed branch without asking me; never rebase a commit of mine into a branch (the DCO check fails on it).
- Update claude/START-HERE.md at the end of the session.

## This repository

- Branches: \`develop\` (default; pull requests, squash merges, \`ci-success\`
  required) and \`main\` (merge commits from \`develop\` only); \`v*\` tags are
  the maintainer's.
- Nothing from this repository goes on crates.io, ever. OxiGuitar's plugin
  SDK is a git dependency on a tag of github.com/$OWNER/$SOURCE.
- Rust $msrv from rustup (\`rust-version\` in Cargo.toml, \`MSRV\` in ci.yml).
EOF

raw scripts/check-dco.sh "$default_branch" > "$files/scripts/check-dco.sh"
chmod +x "$files/scripts/check-dco.sh"
raw .github/pull_request_template.md "$default_branch" > "$files/.github/pull_request_template.md"
raw rustfmt.toml "$default_branch" > "$files/rustfmt.toml"
raw clippy.toml "$default_branch" > "$files/clippy.toml"
# oxiguitar's licence policy and sources, without its advisory exceptions
# (each names a crate oxiguitar depends on; a new repository earns its own).
raw deny.toml "$default_branch" | awk '
    /^ignore = \[/ { print "ignore = []"; skip = 1; next }
    skip && /^\]/ { skip = 0; next }
    !skip
    /^allow-registry = / {
        print "# OxiGuitar'"'"'s SDK, by git tag (nothing of ours is on crates.io)."
        print "allow-git = [\"https://github.com/'"$OWNER/$SOURCE"'\"]"
    }' > "$files/deny.toml"

cat > "$files/.gitignore" << 'EOF'
/target
**/*.rs.bk

# The private test corpus is never committed.
*.tar.gz

# Editors and systems
.idea/
.vscode/
*.swp
.DS_Store
Thumbs.db

/dist/
EOF

cat > "$files/.github/dependabot.yml" << 'EOF'
# Weekly version updates for the Rust crates and the workflows' actions, as
# pull requests into develop. scripts/check-dco.sh skips Dependabot's commits.
version: 2
updates:
  - package-ecosystem: cargo
    directory: /
    target-branch: develop
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
  - package-ecosystem: github-actions
    directory: /
    target-branch: develop
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
EOF

cat > "$files/.github/workflows/ci.yml" << EOF
# Formatting, lint, tests, licence check and sign-off check. The final job,
# ci-success, passes only if every other job passed; branch rules require it
# before anything merges into develop or main.
#
# Made by OxiGuitar's scripts/new-repo.sh with oxiguitar's action pins and
# Rust. The Rust jobs wait for a Cargo.toml: until the repository has one
# they are skipped, which ci-success counts as passing.
name: CI

on:
  pull_request:
  push:
    branches: [develop, main]
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: ci-\${{ github.ref }}
  cancel-in-progress: \${{ github.event_name == 'pull_request' }}

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
  # Must match rust-version in Cargo.toml: the Rust we build with, from
  # rustup, raised only on purpose.
  MSRV: "$msrv"
  # Debian/Ubuntu packages the build needs on Linux, space-separated
  # (e.g. libasound2-dev). Empty: none.
  APT_PACKAGES: ""

jobs:
  rust:
    name: Is there Rust yet
    runs-on: ubuntu-24.04
    outputs:
      present: \${{ steps.look.outputs.present }}
    steps:
      - uses: actions/checkout@$checkout
      - id: look
        run: |
          if [ -f Cargo.toml ]; then echo "present=true"; else echo "present=false"; fi >> "\$GITHUB_OUTPUT"

  fmt:
    name: Formatting
    needs: rust
    if: needs.rust.outputs.present == 'true'
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@$checkout
      - uses: dtolnay/rust-toolchain@$toolchain_stable
        with:
          components: rustfmt
      - run: cargo fmt --all --check

  lint:
    name: Lint and docs
    needs: rust
    if: needs.rust.outputs.present == 'true'
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@$checkout
      # Lints run on the minimum supported Rust, so CI and contributors'
      # machines agree and a new upstream lint cannot turn CI red overnight.
      - uses: dtolnay/rust-toolchain@$toolchain_msrv
        with:
          toolchain: \${{ env.MSRV }}
          components: clippy
      - uses: Swatinem/rust-cache@$cache
      - if: env.APT_PACKAGES != ''
        run: sudo apt-get update && sudo apt-get install -y --no-install-recommends \$APT_PACKAGES
        timeout-minutes: 15
      - name: Clippy
        run: |
          set -o pipefail
          cargo clippy --workspace --all-targets --all-features --locked \\
            --message-format=short -- -D warnings 2>&1 \\
            | sed -E 's/^([^ :]+):([0-9]+):([0-9]+): (error|warning)(\[[^]]*\])?: (.*)\$/::error file=\1,line=\2,col=\3::\6\n&/'
      - name: Docs
        run: |
          set -o pipefail
          cargo doc --workspace --no-deps --locked 2>&1 \\
            | sed -E 's/^(error|warning)(\[[^]]*\])?: (.*)\$/::error::\3\n&/'
        env:
          RUSTDOCFLAGS: -D warnings

  test:
    name: Test (\${{ matrix.name }})
    needs: rust
    if: needs.rust.outputs.present == 'true'
    runs-on: \${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - { name: Linux x86_64, os: ubuntu-24.04, toolchain: stable }
          - { name: Linux x86_64 MSRV, os: ubuntu-24.04, toolchain: msrv }
          - { name: Linux ARM64, os: ubuntu-24.04-arm, toolchain: stable }
          - { name: Windows x86_64, os: windows-2025, toolchain: stable }
          - { name: Windows ARM64, os: windows-11-arm, toolchain: stable }
          - { name: macOS, os: macos-15, toolchain: stable }
    steps:
      - uses: actions/checkout@$checkout
      - uses: dtolnay/rust-toolchain@$toolchain_msrv
        with:
          toolchain: \${{ matrix.toolchain == 'msrv' && env.MSRV || matrix.toolchain }}
      - uses: Swatinem/rust-cache@$cache
        with:
          key: \${{ matrix.toolchain }}
      - if: runner.os == 'Linux' && env.APT_PACKAGES != ''
        run: sudo apt-get update && sudo apt-get install -y --no-install-recommends \$APT_PACKAGES
        timeout-minutes: 15
      # Failing tests also become an annotation, so they can be read without
      # downloading the job log. Bash on every runner, Windows included.
      - name: cargo test
        shell: bash
        env:
          CARGO_TERM_COLOR: never
        run: |
          set +e
          cargo test --workspace --locked 2>&1 | tee test.log
          status=\${PIPESTATUS[0]}
          if [ "\$status" -ne 0 ]; then
            report=\$( (grep -E -A6 'panicked at|^error: test failed|signal: ' test.log; sed -n '/^failures:\$/,/^test result:/p' test.log) | head -n 80)
            [ -n "\$report" ] || report=\$(tail -n 40 test.log)
            report=\${report//'%'/'%25'}
            report=\${report//\$'\r'/'%0D'}
            report=\${report//\$'\n'/'%0A'}
            echo "::error title=cargo test (\${{ matrix.name }})::\$report"
          fi
          exit "\$status"

  deny:
    name: Licences and advisories
    needs: rust
    if: needs.rust.outputs.present == 'true'
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@$checkout
      - uses: EmbarkStudios/cargo-deny-action@$deny_action
        with:
          command: check
          arguments: --all-features

  dco:
    name: Sign-off (DCO)
    if: github.event_name == 'pull_request'
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@$checkout
        with:
          fetch-depth: 0
      - run: >-
          scripts/check-dco.sh
          "\${{ github.event.pull_request.base.sha }}"
          "\${{ github.event.pull_request.head.sha }}"

  ci-success:
    name: ci-success
    if: always()
    needs: [rust, fmt, lint, test, deny, dco]
    runs-on: ubuntu-24.04
    steps:
      - name: Every job passed
        env:
          RESULTS: \${{ toJSON(needs.*.result) }}
        run: |
          echo "\$RESULTS"
          # 'skipped' is fine: the DCO job runs on pull requests only, and
          # the Rust jobs wait for a Cargo.toml.
          if echo "\$RESULTS" | grep -Eq '"(failure|cancelled)"'; then
            exit 1
          fi
EOF

starting_files=()
while IFS= read -r f; do starting_files+=("${f#"$files"/}"); done \
    < <(find "$files" -type f | LC_ALL=C sort)

# Pushing as the signed-in user, signed off: the DCO check reads these.
git_identity() {
    git -C "$1" config user.name "$(gh api user --jq '.name // .login')"
    local email
    email="$(git config --global user.email || true)"
    [[ -n "$email" ]] || email="$(gh api user --jq '.email // empty')"
    [[ -n "$email" ]] || { echo "set git config --global user.email first" >&2; exit 1; }
    git -C "$1" config user.email "$email"
}
remote="https://github.com/$OWNER/$name.git"
# git with gh's login for github.com, for this command only.
gitgh() { git -c credential.helper= -c 'credential.helper=!gh auth git-credential' "$@"; }

has_branch() { get "$dst/branches/$1" > /dev/null; }

if ((created)) || ! has_branch main; then
    repo="$work/repo"
    if ((history)); then
        # oxiguitar's history of what moves here; the starting files on top,
        # each only where the history has no file of that name. Made in a
        # dry run too (it changes nothing on GitHub), to show what it holds.
        echo "  no main yet: $SOURCE's history of what moves here, the starting files on top"
        "$here/split-history.sh" "$name" "$repo" --ref "$ref" | sed 's/^/  /'
        for f in "${starting_files[@]}"; do
            if [[ ! -e "$repo/$f" ]]; then
                mkdir -p "$repo/$(dirname "$f")"
                cp -p "$files/$f" "$repo/$f"
            fi
        done
    else
        echo "  no main yet: the first commit, on main, and develop from it"
        cp -R "$files" "$repo"
        git -C "$repo" init -q -b main
    fi
    if ((dry)); then
        echo "  would: commit the starting files (signed off) and push main and develop to $remote"
        printf '    %s\n' "${starting_files[@]}"
    else
        git_identity "$repo"
        git -C "$repo" add -A
        git -C "$repo" commit -q -s -m "Start $name, set up like $SOURCE"
        gitgh -C "$repo" push -q "$remote" main main:develop
    fi
    if ((history)); then
        note "$SOURCE's history, the starting files on top, on main; develop from it"
    else
        note "first commit on main, develop from it"
    fi
else
    if ((history)); then
        echo "  $OWNER/$name already has main: --history is only for a new repository, so it is left out"
    fi
    if ! has_branch develop; then
        sha="$(get "$dst/git/ref/heads/main" | jq -r .object.sha)"
        call POST "$dst/git/refs" "$(jq -n --arg s "$sha" '{ref: "refs/heads/develop", sha: $s}')"
        note "develop, from main"
    fi
    # Starting files the repository lacks, as a pull request; existing
    # files are never touched.
    missing=()
    for f in "${starting_files[@]}"; do
        get "$dst/contents/$f?ref=develop" > /dev/null || missing+=("$f")
    done
    if ((${#missing[@]})); then
        echo "  develop lacks: ${missing[*]}"
        if ((dry)); then
            echo "  would: push branch new-repo/starting-files with them and open a pull request into develop"
        else
            repo="$work/repo"
            gitgh clone -q --branch develop "$remote" "$repo"
            git_identity "$repo"
            git -C "$repo" checkout -q -b new-repo/starting-files
            for f in "${missing[@]}"; do
                mkdir -p "$repo/$(dirname "$f")"
                cp -p "$files/$f" "$repo/$f"
            done
            git -C "$repo" add -- "${missing[@]}"
            git -C "$repo" commit -q -s -m "Add the starting files $name lacks"
            gitgh -C "$repo" push -q -u origin new-repo/starting-files
            pr="$(gh pr create --repo "$OWNER/$name" --base develop --head new-repo/starting-files \
                --title "Add the starting files $name lacks" \
                --body "Made by $SOURCE's scripts/new-repo.sh: ${missing[*]}")"
            todo "review and merge $pr"
        fi
        note "starting files offered: ${missing[*]}"
    else
        echo "  develop has every starting file"
    fi
fi

if [[ "$(jq -r '.default_branch // ""' <<< "$target")" != "$default_branch" ]]; then
    call PATCH "$dst" "$(jq -n --arg b "$default_branch" '{default_branch: $b}')"
    note "default branch $default_branch"
fi

# --- 3. Actions --------------------------------------------------------------

section "3. Actions permissions"
want="$(get "$src/actions/permissions" | jq '{enabled, allowed_actions}')"
have="$(get "$dst/actions/permissions" 2> /dev/null | jq '{enabled, allowed_actions}' || echo '{}')"
if ! same "$want" "$have"; then
    call PUT "$dst/actions/permissions" "$want"
    note "Actions: $(jq -c . <<< "$want")"
fi
if [[ "$(jq -r .allowed_actions <<< "$want")" == "selected" ]]; then
    want="$(get "$src/actions/permissions/selected-actions")"
    have="$(get "$dst/actions/permissions/selected-actions" || echo '{}')"
    if ! same "$want" "$have"; then
        call PUT "$dst/actions/permissions/selected-actions" "$want"
        note "Actions: the allowed actions"
    fi
fi
want="$(get "$src/actions/permissions/workflow")"
have="$(get "$dst/actions/permissions/workflow" || echo '{}')"
if ! same "$want" "$have"; then
    call PUT "$dst/actions/permissions/workflow" "$want"
    note "Actions: workflow token $(jq -c . <<< "$want")"
fi
# Who needs approval to run workflows from forks (newer endpoint; skipped
# where the API doesn't have it).
if want="$(get "$src/actions/permissions/fork-pr-contributor-approval")"; then
    have="$(get "$dst/actions/permissions/fork-pr-contributor-approval" || echo '{}')"
    if ! same "$want" "$have"; then
        call PUT "$dst/actions/permissions/fork-pr-contributor-approval" "$want"
        note "Actions: approval for fork pull requests"
    fi
fi

end_section

# --- 4. Dependabot and security ----------------------------------------------

section "4. Dependabot and security"
toggle() {
    # toggle LABEL ENDPOINT: on in the target exactly when on in oxiguitar.
    local want have
    want="$(status "$src/$2")"
    if new_in_dry_run; then have=404; else have="$(status "$dst/$2")"; fi
    if [[ "$want" != "$have" ]]; then
        if [[ "$want" == 204 ]]; then call PUT "$dst/$2"; else call DELETE "$dst/$2"; fi
        note "$1 $([[ "$want" == 204 ]] && echo on || echo off)"
    fi
}
toggle "Dependabot alerts" vulnerability-alerts
want="$(get "$src/automated-security-fixes" | jq -r .enabled || echo false)"
have="$(get "$dst/automated-security-fixes" | jq -r .enabled || echo false)"
if [[ "$want" != "$have" ]]; then
    if [[ "$want" == true ]]; then call PUT "$dst/automated-security-fixes"; else call DELETE "$dst/automated-security-fixes"; fi
    note "Dependabot security updates $([[ "$want" == true ]] && echo on || echo off)"
fi
want="$(get "$src/private-vulnerability-reporting" | jq -r .enabled || echo false)"
have="$(get "$dst/private-vulnerability-reporting" | jq -r .enabled || echo false)"
if [[ "$want" != "$have" ]]; then
    if [[ "$want" == true ]]; then call PUT "$dst/private-vulnerability-reporting"; else call DELETE "$dst/private-vulnerability-reporting"; fi
    note "private vulnerability reporting $([[ "$want" == true ]] && echo on || echo off)"
fi

end_section

# --- 5. Labels -----------------------------------------------------------------

section "5. Labels"
want="$(get_all "$src/labels?per_page=100" | jq 'map({name, color, description})')"
if new_in_dry_run; then
    have='[]'
else
    have="$(get_all "$dst/labels?per_page=100" | jq 'map({name, color, description})')"
fi
while IFS= read -r label; do
    lname="$(jq -r .name <<< "$label")"
    current="$(jq --arg n "$lname" 'map(select(.name == $n)) | first // empty' <<< "$have")"
    if [[ -z "$current" ]]; then
        call POST "$dst/labels" "$label"
        note "label $lname"
    elif ! same "$label" "$current"; then
        enc="$(jq -rn --arg n "$lname" '$n | @uri')"
        call PATCH "$dst/labels/$enc" "$(jq '{color, description}' <<< "$label")"
        note "label $lname updated"
    fi
done < <(jq -c '.[]' <<< "$want")
extra="$(jq -r --argjson w "$want" '[.[].name] - [$w[].name] | join(", ")' <<< "$have")"
[[ -z "$extra" ]] || echo "  left alone (not in $SOURCE): $extra"

end_section

# --- 6. Pages ------------------------------------------------------------------

section "6. Pages"
if pages="$(get "$src/pages")"; then
    want="$(jq '{build_type} + (if .build_type == "legacy" then {source} else {} end)' <<< "$pages")"
    if have="$(get "$dst/pages")"; then
        have="$(jq '{build_type} + (if .build_type == "legacy" then {source} else {} end)' <<< "$have")"
        if ! same "$want" "$have"; then
            call PUT "$dst/pages" "$want"
            note "Pages: $(jq -c . <<< "$want")"
        fi
    else
        call POST "$dst/pages" "$want"
        note "Pages: $(jq -c . <<< "$want")"
    fi
    # The github-pages environment: which branches may publish (oxiguitar
    # lists main by name; rulesets don't count as protected branches there).
    if env="$(get "$src/environments/github-pages")"; then
        want="$(jq '{deployment_branch_policy}' <<< "$env")"
        have="$(get "$dst/environments/github-pages" | jq '{deployment_branch_policy}' || echo '{}')"
        if ! same "$want" "$have"; then
            call PUT "$dst/environments/github-pages" "$want"
            note "github-pages environment: $(jq -c . <<< "$want")"
        fi
        if [[ "$(jq -r '.deployment_branch_policy.custom_branch_policies // false' <<< "$want")" == true ]]; then
            want="$(get "$src/environments/github-pages/deployment-branch-policies" | jq '[.branch_policies[] | {name, type: (.type // "branch")}]')"
            have="$(get "$dst/environments/github-pages/deployment-branch-policies" | jq '[.branch_policies[] | {name, type: (.type // "branch")}]' || echo '[]')"
            while IFS= read -r policy; do
                if ! jq -e --argjson p "$policy" 'any(.[]; . == $p)' <<< "$have" > /dev/null; then
                    call POST "$dst/environments/github-pages/deployment-branch-policies" "$policy"
                    note "github-pages may publish from $(jq -r .name <<< "$policy")"
                fi
            done < <(jq -c '.[]' <<< "$want")
        fi
    fi
else
    echo "  $SOURCE has no Pages"
fi

end_section

# --- 7. Rulesets ---------------------------------------------------------------

section "7. Rulesets"
# Only what defines a ruleset; ids, links and dates are the repository's own.
shape='{name, target, enforcement, conditions, bypass_actors: (.bypass_actors // [] | sort_by(.actor_type, .actor_id)), rules: (.rules // [] | sort_by(.type))}'
if new_in_dry_run; then
    theirs='[]'
else
    theirs="$(get_all "$dst/rulesets?includes_parents=false&per_page=100")"
fi
while IFS= read -r id; do
    want="$(get "$src/rulesets/$id" | jq "$shape")"
    rname="$(jq -r .name <<< "$want")"
    match="$(jq -r --arg n "$rname" 'map(select(.name == $n)) | first | .id // empty' <<< "$theirs")"
    if [[ -z "$match" ]]; then
        call POST "$dst/rulesets" "$want"
        note "ruleset $rname"
    else
        have="$(get "$dst/rulesets/$match" | jq "$shape")"
        if ! same "$want" "$have"; then
            call PUT "$dst/rulesets/$match" "$want"
            note "ruleset $rname updated"
        fi
    fi
done < <(get_all "$src/rulesets?includes_parents=false&per_page=100" | jq -r '.[].id')
extra="$(jq -r --argjson w "$(get_all "$src/rulesets?includes_parents=false&per_page=100")" \
    '[.[].name] - [$w[].name] | join(", ")' <<< "$theirs")"
[[ -z "$extra" ]] || echo "  left alone (not in $SOURCE): $extra"

end_section

# --- 8. The GitHub App ---------------------------------------------------------

echo
echo "8. The $app GitHub App"
# gh's own login (an OAuth token) is not allowed to list GitHub App
# installations (HTTP 403): the app is then added by hand, which is no
# failure of the run.
installation=""
hidden=0
if listed_all="$(gh api --paginate "user/installations?per_page=100" --jq '.installations[]' 2> "$work/err")"; then
    installation="$(jq -s --arg a "$app" 'map(select(.app_slug == $a)) | first // empty' <<< "$listed_all")"
elif grep -q 'HTTP 403' "$work/err"; then
    hidden=1
else
    fail "GET user/installations" "$work/err"
fi
if ((hidden)); then
    todo "add $OWNER/$name to the $app GitHub App by hand (this login may not list installations): github.com/settings/installations, Configure beside $app, Repository access, select $name, Save"
elif [[ -z "$installation" ]]; then
    todo "install the $app GitHub App on $OWNER/$name (github.com/apps/$app), or check its slug with: gh api user/installations --jq '.installations[].app_slug'"
else
    inst_id="$(jq -r .id <<< "$installation")"
    if [[ "$(jq -r .repository_selection <<< "$installation")" == "all" ]]; then
        echo "  installed on all of $OWNER's repositories: nothing to do"
    else
        if new_in_dry_run; then repo_id=NEW_REPO_ID; else repo_id="$(get "$dst" | jq -r .id)"; fi
        listed="$(get_all "user/installations/$inst_id/repositories?per_page=100" repositories \
            | jq --arg n "$name" 'any(.[]; .name == $n)')"
        if [[ "$listed" == true ]]; then
            echo "  $OWNER/$name already has the app"
        elif ((dry)); then
            call PUT "user/installations/$inst_id/repositories/$repo_id"
        elif gh api -X PUT "user/installations/$inst_id/repositories/$repo_id" > /dev/null 2> "$work/err"; then
            echo "  gh api -X PUT user/installations/$inst_id/repositories/$repo_id"
            note "the $app app can reach $OWNER/$name"
        else
            todo "add $OWNER/$name to the $app app by hand: https://github.com/settings/installations/$inst_id (the API said: $(tr '\n' ' ' < "$work/err"))"
        fi
    fi
fi

# --- What happened ---------------------------------------------------------------

stop_if_failed

echo
if ((dry)); then echo "Would change:"; else echo "Changed:"; fi
if ((${#changed[@]})); then printf '  - %s\n' "${changed[@]}"; else echo "  nothing: $OWNER/$name matches $OWNER/$SOURCE"; fi
todo "in the OxiGuitar Claude session, attach $OWNER/$name to the session so Claude can push and open pull requests there"
echo "By hand:"
printf '  - %s\n' "${manual[@]}"
((dry)) && echo "The starting files are in $files"
exit 0
