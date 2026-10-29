#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Runs the AppImage catalog's own test (github.com/AppImage/appimage.github.io,
# MIT; .github/workflows/test.yml and code/worker.sh there) on an AppImage we
# just built, so we see its verdict and its screenshot before a release.
# Their test fetches the AppImage from the URL in a file of data/; here that
# URL is a web server on this machine. Everything else is theirs, at the
# commit below: the same packages, Firejail without network, an 800x600
# Xvfb with icewm, the lint, the C library check and the screenshot checks.
#
# Runs on GitHub's ubuntu-22.04 runner, as theirs does, and changes the
# machine (packages, Firejail in /usr, a dummy sound card): CI only.
# .github/workflows/release.yml, job "AppImage catalog".
#
# Usage: scripts/package/appimage-catalog-test.sh OxiGuitar-VERSION-x86_64.AppImage
# Writes catalog-result/: log.txt, the screenshot, the labels and hints
# their bot would post on the pull request.

set -euo pipefail

# Their master on 25 Oct 2026; move it on when their test changes.
commit=6fe0b4eb79bac5676ed98fef07dbb35790f0ae65
appimage="$(readlink -f "${1:?usage: appimage-catalog-test.sh APPIMAGE}")"
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/catalog-result"
work="${RUNNER_TEMP:-/tmp}/appimage-catalog"
rm -rf "$out" "$work"
mkdir -p "$out"

git init -q "$work"
git -C "$work" fetch -q --depth 1 https://github.com/AppImage/appimage.github.io "$commit"
git -C "$work" checkout -q FETCH_HEAD
cd "$work"

# Their workflow's packages (test.yml, "Configure APT Packages").
sudo apt-get update -qq
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    sudo imagemagick libasound2-dev pulseaudio-utils alsa-utils alsa-oss libjack0 \
    desktop-file-utils xmlstarlet xsel xterm xvfb icewm x11-utils x11-apps netpbm \
    xdotool libgl1-mesa-dri libgl1-mesa-dev mesa-utils libosmesa6 libsdl1.2-dev \
    libsdl2-2.0-0 fonts-wqy-microhei libfile-mimeinfo-perl tesseract-ocr \
    tesseract-ocr-eng tesseract-ocr-chi-sim tesseract-ocr-ara tesseract-ocr-rus \
    tesseract-ocr-ell libfuse2 stalonetray > /dev/null
bash code/fetch-deps.sh > /dev/null
cp -a deps/. .
sudo bash code/prep-dummy-soundcard.sh
sudo gem install dupervisor -v 1.0.5 > /dev/null

mkdir -p "$HOME/.icewm"
cat > "$HOME/.icewm/preferences" << 'EOF'
ShowTaskBar = 0
TaskBarAutoHide = 1
TaskBarShowWorkspaces = 0
TaskBarShowAllWindows = 0
TaskBarShowClock = 0
TaskBarShowMailboxStatus = 0
TaskBarShowCPUStatus = 0
TaskBarShowWindowListMenu = 0
EOF
export DISPLAY=:99
Xvfb :99 -screen 0 800x600x24 > /dev/null 2>&1 &
xvfb=$!

# The entry, as their bot's pull request would add it, pointing at a web
# server on this machine instead of our GitHub releases.
mkdir -p "$work/serve"
cp "$appimage" "$work/serve/"
python3 -m http.server 8642 --bind 127.0.0.1 --directory "$work/serve" > /dev/null 2>&1 &
server=$!
sleep 1
name="$(basename "$appimage")"
echo "http://127.0.0.1:8642/$name" > data/OxiGuitar

set +e
IS_PULLREQUEST=true STRICT=true timeout --kill-after=30 600 \
    bash -e code/worker.sh "$(readlink -f data/OxiGuitar)" > log.txt 2>&1
rc=$?
set -e
kill "$server" "$xvfb" 2> /dev/null || true

cp log.txt "$out/"
cp database/OxiGuitar/screenshot.png "$out/" 2> /dev/null || true
cp failure-screens/*.png "$out/" 2> /dev/null || true
cp database/OxiGuitar/*.desktop "$out/" 2> /dev/null || true
bash code/diagnose.sh --labels log.txt > "$out/labels.txt"
bash code/diagnose.sh log.txt > "$out/hints.md"
grep -hE '^(ERROR|WARNING|FATAL|X-AppImage-[A-Za-z-]+=|Screenshot:)' log.txt | sort -u > "$out/remarks.txt" || true
# Their bot posts its hints on a passing test too, and a glibc newer than
# its system's anywhere in the log is the one this test exists for.
if grep -qx error-glibc-too-new "$out/labels.txt"; then rc=1; fi

{
    echo "## AppImage catalog test"
    echo
    if [ "$rc" -eq 0 ]; then echo "Passed (worker.sh exited 0)."; else echo "**Failed** (worker.sh exited $rc)."; fi
    echo
    echo '```'
    cat "$out/remarks.txt"
    if [ -s "$out/labels.txt" ]; then echo "Labels: $(tr '\n' ' ' < "$out/labels.txt")"; fi
    echo '```'
    if [ -s "$out/hints.md" ]; then echo; cat "$out/hints.md"; fi
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"

if [ "$rc" -ne 0 ]; then
    echo "::error::the AppImage catalog's test failed: see the job's summary and the appimage-catalog artifact" >&2
    tail -n 60 log.txt >&2
fi
exit "$rc"
