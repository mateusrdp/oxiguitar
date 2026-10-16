#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# fuzz/run.sh TARGET SECONDS
#
# Fuzzes TARGET for SECONDS on every core (libFuzzer's fork mode), starting
# from fuzz/corpus/TARGET (fill it with fuzz/seeds.sh), then shrinks the
# corpus to the inputs that matter. Exits non-zero on a crash, a hang (an
# input that takes over 30 s) or an input that needs over 2 GB; the input
# is then in fuzz/artifacts/TARGET/. In GitHub Actions the result also
# becomes a notice or an error on the run.
set -uo pipefail
target=$1
seconds=$2
cd "$(dirname "$0")"
extra=()
if [ "$target" = gpif ]; then
    extra+=(-dict=gpif.dict)
fi
limits=(-timeout=30 -rss_limit_mb=2048)

cargo fuzz build "$target" || exit 1
cargo fuzz run "$target" "corpus/$target" -- \
    -max_total_time="$seconds" -fork="$(nproc)" -print_final_stats=1 \
    "${limits[@]}" "${extra[@]}" 2>&1 | tee "fuzz-$target.log"
status=${PIPESTATUS[0]}

stats=$(grep -E '^#[0-9]+.*cov:' "fuzz-$target.log" | tail -n 1)
if [ -n "${GITHUB_ACTIONS:-}" ]; then
    echo "::notice title=Fuzz $target::${seconds} s: ${stats}"
    printf '### %s\n\n%s s on %s cores: `%s`\n\n' "$target" "$seconds" "$(nproc)" "$stats" >> "$GITHUB_STEP_SUMMARY"
fi
if [ "$status" -ne 0 ]; then
    report=$(grep -E -A14 'panicked at|ERROR: libFuzzer|==[0-9]+==ERROR|SUMMARY:' "fuzz-$target.log" | head -n 50)
    [ -n "$report" ] || report=$(tail -n 30 "fuzz-$target.log")
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        echo "::error title=Fuzz $target failed::$(printf '%s' "$report" | sed ':a;N;$!ba;s/%/%25/g;s/\r/%0D/g;s/\n/%0A/g')"
    fi
    exit "$status"
fi

# Keep only the inputs that add coverage, for the next run.
cargo fuzz cmin "$target" "corpus/$target" -- "${limits[@]}" > "cmin-$target.log" 2>&1 \
    || echo "corpus minimisation failed; keeping the whole corpus" >&2
echo "corpus/$target: $(find "corpus/$target" -type f | wc -l) inputs"
