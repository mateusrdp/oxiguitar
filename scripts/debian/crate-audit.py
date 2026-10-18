#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
"""Audit every crate the Linux packages are built from against Debian.

Debian's archive (main) builds Rust programs only from crates it packages
itself, as librust-NAME-dev, one source package per crate. This script
says, for each crate in Cargo.lock that the shipped Linux programs use,
whether Debian has it and at which version.

    crate-audit.py list > crates.tsv
        On a machine with Cargo: the crates (name, version, builds) of
        oxiguitar (both Linux builds), oxiguitar-cli and the basic plugins,
        for x86_64 and aarch64 Linux, normal and build dependencies (not
        dev-dependencies: Debian's tests may skip those).

    crate-audit.py check crates.tsv > report.md
        On Debian (sid, after apt-get update): a Markdown report, one row
        per crate, with what Debian has. Reads apt's package lists only
        (apt-cache dumpavail).

    crate-audit.py annotate report.md
        In GitHub Actions: the report as notices on the check run, so it
        can be read on the pull request and through the API as well as in
        the run's summary.

Status, per crate:
    same        Debian has this exact version.
    newer       Debian has a newer version of the same semver series
                (cargo accepts it unless our Cargo.toml asks for more).
    older       Debian has an older version of the same series: it needs
                updating, or our requirement relaxing.
    other       Debian has the crate, in another semver series only.
    missing     Debian has no librust package of that crate.
"""

import glob
import os
import re
import subprocess
import sys

PACKAGES = [
    "oxiguitar-cli",
    "oxiguitar-gp-plugin",
    "oxiguitar-gp3-plugin",
    "oxiguitar-gp4-plugin",
    "oxiguitar-gp5-plugin",
    "oxiguitar-gpx-plugin",
]
TARGETS = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
# The Wayland build is the default: the app, the CLI and the plugins. The
# X11 build is the app alone with other features.
BUILDS = {
    "wayland": ["-p", "oxiguitar-app"] + [a for p in PACKAGES for a in ("-p", p)],
    "x11": ["-p", "oxiguitar-app", "--no-default-features", "--features", "x11"],
}


def tree(extra):
    out = set()
    for target in TARGETS:
        args = ["cargo", "tree", "--locked", "-e", "normal,build",
                "--target", target, "--prefix", "none", "--format", "{p}"]
        text = subprocess.run(args + extra, check=True, capture_output=True,
                              text=True).stdout
        for line in text.splitlines():
            line = line.removesuffix(" (*)").strip()
            # Our own crates come from a path; registry crates don't.
            if not line or "(/" in line:
                continue
            name, version = line.split()[:2]
            out.add((name, version.removeprefix("v")))
    return out


def list_crates():
    seen = {}
    for build, extra in BUILDS.items():
        for crate in tree(extra):
            seen.setdefault(crate, []).append(build)
    for (name, version), builds in sorted(seen.items()):
        print(f"{name}\t{version}\t{','.join(builds)}")


def series(version):
    """The semver series cargo treats as compatible: 1.2.3 → 1, 0.2.3 →
    0.2, 0.0.3 → 0.0.3."""
    parts = version.split("+")[0].split("-")[0].split(".")
    if parts[0] != "0":
        return parts[0]
    if len(parts) > 1 and parts[1] != "0":
        return "0." + parts[1]
    return ".".join(parts[:3])


def numbers(version):
    """1.2.3 (or Debian's 1.2.3-4, 1:1.2.3-4, 1.2.3+dfsg-1) as a tuple."""
    version = version.split(":")[-1].rsplit("-", 1)[0]
    version = re.split(r"[+~]", version)[0]
    return tuple(int(x) if x.isdigit() else 0 for x in version.split("."))


def debian_crates():
    """librust-NAME-dev → {Debian versions}, from apt's lists (through
    apt-cache, which reads them however they are stored)."""
    found = {}
    text = subprocess.run(["apt-cache", "dumpavail"], check=True,
                          capture_output=True, text=True, errors="replace").stdout
    for record in text.split("\n\n"):
        fields = dict(re.findall(r"^(Package|Version): (.+)$", record, re.M))
        package, version = fields.get("Package"), fields.get("Version")
        if package and version and re.fullmatch(r"librust-.+-dev", package):
            found.setdefault(package, set()).add(version)
    return found


def debian_name(crate):
    return "librust-" + crate.lower().replace("_", "-") + "-dev"


def check(path):
    archive = debian_crates()
    if not archive:
        sys.exit("no librust packages in apt's lists: run apt-get update first")
    # A semver-incompatible series is packaged apart, as
    # librust-NAME-SERIES-dev (rust-rand-0.8 → librust-rand-0.8-dev).
    rows = []
    for line in open(path, encoding="utf-8"):
        name, version, builds = line.rstrip("\n").split("\t")
        ours = series(version)
        base = debian_name(name)
        versioned = base.removesuffix("-dev") + "-" + ours + "-dev"
        candidates = sorted(archive.get(base, set()) | archive.get(versioned, set()),
                            key=numbers)
        in_series = [v for v in candidates if series(".".join(map(str, numbers(v)))) == ours]
        if not candidates:
            status, theirs = "missing", ""
        elif not in_series:
            status, theirs = "other", ", ".join(candidates)
        else:
            best = in_series[-1]
            have = numbers(best)
            want = numbers(version)
            status = "same" if have == want else ("newer" if have > want else "older")
            theirs = best
        rows.append((name, version, builds, status, theirs))

    order = ["missing", "other", "older", "newer", "same"]
    counts = {s: sum(1 for r in rows if r[3] == s) for s in order}
    release = "unknown"
    for path in glob.glob("/etc/os-release"):
        for line in open(path, encoding="utf-8"):
            if line.startswith("PRETTY_NAME="):
                release = line.split("=", 1)[1].strip().strip('"')
    print("# Crates against Debian\n")
    print(f"Archive: {release}, {len(archive)} librust packages. "
          f"Crates: {len(rows)} (the Linux programs and the basic plugins, "
          "both builds, x86_64 and aarch64).\n")
    print("| Status | Crates |")
    print("| --- | --- |")
    for s in order:
        print(f"| {s} | {counts[s]} |")
    print()
    print("| Crate | Ours | Builds | Status | Debian |")
    print("| --- | --- | --- | --- | --- |")
    for r in sorted(rows, key=lambda r: (order.index(r[3]), r[0])):
        print("| " + " | ".join(r) + " |")


def annotate(path):
    # GitHub keeps at most 10 notices per step, each of a few thousand
    # characters: the report in that many pieces, cut at row ends.
    lines = open(path, encoding="utf-8").read().splitlines()
    pieces, piece = [], ""
    for line in lines:
        if len(piece) + len(line) > 3500:
            pieces.append(piece)
            piece = ""
        piece += line + "\n"
    pieces.append(piece)
    for i, text in enumerate(pieces[:10], 1):
        text = text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        print(f"::notice title=Crates against Debian ({i} of {len(pieces)})::{text}")


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "list":
        os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
        list_crates()
    elif len(sys.argv) == 3 and sys.argv[1] == "check":
        check(sys.argv[2])
    elif len(sys.argv) == 3 and sys.argv[1] == "annotate":
        annotate(sys.argv[2])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
