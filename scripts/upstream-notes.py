#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
"""Lists the code's notes on what our libraries do or lack, and fails when
Cargo.lock no longer has a version a note names.

A note reads `Upstream (CRATE VERSION): ...` in a comment (CONTRIBUTING.md,
"Comments in the code"). VERSION is as precise as it was checked: `1.19`
holds for any 1.19.x in Cargo.lock, `4.0.4` only for 4.0.4. When a library
moves, the notes that named the old version are listed: read each, change
the code if the library now does what we worked around, and write the
version you checked.

    python3 scripts/upstream-notes.py          # check; exit 1 on stale notes
    python3 scripts/upstream-notes.py --list   # every note, by crate

Python 3.8 or newer, standard library only, as scripts/test-all.py. Reads
the files git tracks, apart from spikes/ (which has lock files of its own).
"""

import pathlib
import re
import subprocess
import sys

NOTE = re.compile(r"Upstream\s*\(([A-Za-z0-9_-]+)\s+([0-9]+(?:\.[0-9]+)*)\)")
KINDS = (".rs", ".slint", ".toml", ".py", ".sh", ".yml")
PACKAGE = re.compile(r'^\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', re.M)


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    # Cargo.lock's packages: `name = "..."` then `version = "..."`.
    lock = (root / "Cargo.lock").read_text(encoding="utf-8")
    versions = {}
    for name, version in PACKAGE.findall(lock):
        versions.setdefault(name, set()).add(version)

    tracked = subprocess.run(
        ["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True
    ).stdout.split("\n")
    notes = []
    for rel in tracked:
        if not rel.endswith(KINDS) or rel.startswith("spikes/") or rel == "scripts/upstream-notes.py":
            continue
        try:
            text = (root / rel).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for number, line in enumerate(text.split("\n"), 1):
            for m in NOTE.finditer(line):
                notes.append((m.group(1), m.group(2), rel, number))

    def holds(crate, version):
        parts = version.split(".")
        return any(v.split(".")[: len(parts)] == parts for v in versions.get(crate, ()))

    if "--list" in sys.argv[1:]:
        for crate, version, rel, number in sorted(notes):
            mark = "" if holds(crate, version) else "   <- Cargo.lock has " + (
                ", ".join(sorted(versions.get(crate, ()))) or "no such crate"
            )
            print(f"{crate} {version}  {rel}:{number}{mark}")
        return 0

    stale = [n for n in notes if not holds(n[0], n[1])]
    for crate, version, rel, number in stale:
        now = ", ".join(sorted(versions.get(crate, ()))) or "no such crate"
        print(f"{rel}:{number}: Upstream ({crate} {version}), but Cargo.lock has {now}")
    if stale:
        print(
            f"\n{len(stale)} note(s) name a version Cargo.lock no longer has: a library moved. "
            "Read each note, change the code if the library now does what we worked around, "
            "and write the version you checked (CONTRIBUTING.md, \"Comments in the code\")."
        )
        return 1
    print(f"{len(notes)} upstream notes, all on the versions in Cargo.lock.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
