#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Prepares a release in the working tree: every file that names the version
# takes the new one, and the CHANGELOG's [Unreleased] section becomes the
# section of the release. Nothing is committed. It is run by the Release
# workflow of oxiguitar-packages (which commits, tags and pushes), and by
# CI with a made-up version, to see that it still finds every file.
#
# Usage: scripts/release/bump.sh VERSION [YYYY-MM-DD]
#
# Fails, changing nothing, when [Unreleased] is empty, when VERSION is not
# higher than the current one, or when a file does not hold the version it
# should (a new place that names the version must be added here).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
exec python3 -I - "$@" <<'PY'
import datetime, html, pathlib, re, sys

args = sys.argv[1:]
if not 1 <= len(args) <= 2:
    sys.exit("usage: scripts/release/bump.sh VERSION [YYYY-MM-DD]")
new = args[0].removeprefix("v")
date = args[1] if len(args) == 2 else datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
if not re.fullmatch(r"\d+\.\d+\.\d+", new):
    sys.exit(f"{new!r} is not a version like 0.12.1")

root = pathlib.Path(".")
changes = {}  # path -> new text, written only when every file is ready


def read(path):
    return changes.get(path) or pathlib.Path(path).read_text(encoding="utf-8")


def put(path, text):
    changes[path] = text


m = re.search(r'^\[workspace\.package\][^\[]*?^version = "([^"]+)"', read("Cargo.toml"), re.M | re.S)
if not m:
    sys.exit("Cargo.toml: no [workspace.package] version")
old = m.group(1)
key = lambda v: tuple(int(x) for x in v.split("."))
if key(new) <= key(old):
    sys.exit(f"{new} is not higher than the current version {old}")


def replace_once(path, pattern, repl, what, flags=0):
    text = read(path)
    out, n = re.subn(pattern, repl, text, count=1, flags=flags)
    if n != 1:
        sys.exit(f"{path}: {what} not found (looking for version {old})")
    put(path, out)


# Cargo.toml and the lock files (only our own crates, by name).
replace_once("Cargo.toml", r'(\[workspace\.package\][^\[]*?^version = )"' + re.escape(old) + '"',
             r'\1"' + new + '"', "the workspace version", re.M | re.S)
for lock in ("Cargo.lock", "fuzz/Cargo.lock", "spikes/gui/Cargo.lock"):
    text = read(lock)
    out, n = re.subn(r'(name = "oxiguitar[^"]*"\nversion = )"' + re.escape(old) + '"', r'\1"' + new + '"', text)
    if n == 0 and lock == "Cargo.lock":
        sys.exit(f"{lock}: none of our crates is at {old}")
    if n:  # the fuzz and spikes locks are bumped only when they were at the old version
        put(lock, out)

# The test plugins' manifests.
for manifest in ("plugins/test/manifest.json", "plugins/test-second/manifest.json"):
    replace_once(manifest, r'("version": )"' + re.escape(old) + '"', r'\1"' + new + '"', "the version")

# The Debian changelog's first entry.
replace_once("debian/changelog", r'^(oxiguitar \()' + re.escape(old) + r'(\+ds-\d+\))', r'\g<1>' + new + r'\2',
             "the first entry", re.M)

# CHANGELOG.md: [Unreleased] becomes the release.
changelog = read("CHANGELOG.md")
m = re.search(r"^## \[Unreleased\]\n(.*?)(?=^## \[)", changelog, re.M | re.S)
if not m or not m.group(1).strip():
    sys.exit("CHANGELOG.md: nothing under [Unreleased]; write what changed first")
body = m.group(1).strip("\n")
put("CHANGELOG.md", changelog[:m.start()] + f"## [Unreleased]\n\n## [{new}] - {date}\n\n{body}\n\n" + changelog[m.end():])


def plain(line):
    line = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", line)
    line = re.sub(r"[*`_]{1,2}", "", line)
    return re.sub(r"\s+", " ", line.removeprefix("- ")).strip()


# The AppStream metainfo gets a release entry made from the first line of
# the section (the release workflow's draft job needs the entry).
first = next((plain(l) for l in body.splitlines() if l.strip() and not l.startswith("#")), "")
if len(first) > 300:
    first = first[:300].rsplit(" ", 1)[0] + " ..."
if not first:
    sys.exit("CHANGELOG.md: the section has no text to describe the release with")
meta = "assets/linux/io.github.mateusrdp.OxiGuitar.metainfo.xml"
entry = (f'    <release version="{new}" date="{date}" type="development">\n'
         f"      <description>\n        <p>{html.escape(first, quote=False)}</p>\n      </description>\n"
         f"      <url>https://github.com/mateusrdp/oxiguitar/releases/tag/v{new}</url>\n    </release>\n")
replace_once(meta, r"(<releases>\n)", lambda m: m.group(1) + entry, "<releases>")

# Last look: no file we changed may still say the old version where it must
# not (the changelogs, the metainfo and Cargo.lock's third-party crates may).
for path in ("Cargo.toml", "plugins/test/manifest.json", "plugins/test-second/manifest.json"):
    if f'"{old}"' in read(path):
        sys.exit(f"{path} still says {old}")
if not read("debian/changelog").startswith(f"oxiguitar ({new}+ds-"):
    sys.exit("debian/changelog was not updated")

for path, text in changes.items():
    pathlib.Path(path).write_text(text, encoding="utf-8")
print(f"{old} -> {new} ({date}): " + ", ".join(sorted(changes)))
PY
