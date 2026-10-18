#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Writes dist/THIRD-PARTY-LICENSES.html: every crate compiled into the
# desktop app, the command-line tool and our plugins on the release
# targets, with its licence text and NOTICE file, plus the bundled fonts.
# Every release package carries it.
#
# Needs cargo-about (cargo install cargo-about --features cli --locked); configuration in
# about.toml, layout in scripts/package/licenses.hbs.
#
# Usage: scripts/package/licenses.sh

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$root/dist"
cd "$root"
# The whole workspace: the programs and every plugin we package. Its test
# crates (oxiguitar-testgen, oxiguitar-plugin-test, plugins/test) bring no
# crate of their own outside dev-dependencies, which about.toml ignores,
# so this is exactly what the packages are built from. (The app alone
# would miss oxiguitar-cli's and the plugins' crates.)
cargo about generate --locked --fail --workspace \
    --output-file dist/THIRD-PARTY-LICENSES.html \
    scripts/package/licenses.hbs

# The Apache licence asks for each crate's NOTICE file to travel with it:
# those of the crates above, before the end of the page.
python3 -I - "$root" dist/THIRD-PARTY-LICENSES.html <<'PY'
import html, json, os, subprocess, sys

root, page = sys.argv[1], sys.argv[2]
targets = [
    "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
    "x86_64-apple-darwin", "aarch64-apple-darwin",
]
used = set()
for target in targets:
    out = subprocess.run(
        ["cargo", "tree", "--locked", "--workspace", "-e", "normal", "--target", target,
         "--prefix", "none", "--format", "{p}"],
        cwd=root, check=True, capture_output=True, text=True).stdout
    for line in out.splitlines():
        if line and "(/" not in line:
            name, version = line.split()[:2]
            used.add((name, version.removeprefix("v")))
meta = json.loads(subprocess.run(
    ["cargo", "metadata", "--locked", "--format-version", "1"],
    cwd=root, check=True, capture_output=True, text=True).stdout)
notices = []
for package in sorted(meta["packages"], key=lambda p: (p["name"], p["version"])):
    if (package["name"], package["version"]) not in used:
        continue
    folder = os.path.dirname(package["manifest_path"])
    for name in ("NOTICE", "NOTICE.txt", "NOTICE.md"):
        path = os.path.join(folder, name)
        if os.path.isfile(path):
            with open(path, encoding="utf-8", errors="replace") as f:
                notices.append((package["name"], package["version"], f.read()))
section = ["<h2>NOTICE files</h2>",
           "<p>The NOTICE files of the crates above, as the Apache License asks.</p>"]
for name, version, text in notices:
    section.append(f"<h3>{html.escape(name)} {html.escape(version)}</h3>")
    section.append(f"<pre>{html.escape(text)}</pre>")
with open(page, encoding="utf-8") as f:
    content = f.read()
if "</body>" not in content:
    sys.exit("licenses.sh: no </body> in the page")
content = content.replace("</body>", "\n".join(section) + "\n</body>", 1)
with open(page, "w", encoding="utf-8") as f:
    f.write(content)
print(f"licenses.sh: {len(notices)} NOTICE file(s)")
PY
echo "Wrote dist/THIRD-PARTY-LICENSES.html"
