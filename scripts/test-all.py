#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
"""Run every test of this repository, on your own test corpus.

    python3 scripts/test-all.py --corpus /path/to/your/files

Every OxiGuitar repository has this same script. It builds and runs all of
the repository's tests (`cargo test` over the whole workspace, plus the extra
stages `tests/suite.json` names), points the corpus tests at your files,
checks that every feature in the manual's features page has a test, and
writes three things to `target/test-all/<time>/`:

- `summary.md`: the result, short enough to paste into a GitHub issue;
- `test-all.log`: everything that happened, for whoever diagnoses a failure
  (a person or a language model);
- `corpus-*.json`: what each corpus test checked, file by file.

The corpus is any folder of files (searched recursively); each corpus test
takes the files it understands and skips the rest, so a different corpus
gives a different, still complete, run.

In a plugin's repository it also tests the plugin against the newest release
of OxiGuitar itself: it fetches that release's source from GitHub (git, into a cache
folder shared by every OxiGuitar repository on the machine, once per
release; OXIGUITAR_TEST_CACHE moves it), builds its `oxiguitar-cli` and
`oxiguitar-plugin-test`, builds this repository's plugin files, runs the
plugin contract on them, and (format plugins) opens and converts every corpus
file through the real program. `--core DIR` uses a checkout of the core you
already have instead. The only network use is reading GitHub (the release's
tag and source, and the crates cargo fetches); nothing is uploaded.

Exit status: 0 when everything passed, 1 when something failed, 2 when the
run could not start (a missing tool, a bad option).

Python 3.8 or newer, standard library only. `--help` lists the options.
"""

import argparse
import datetime as _dt
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import threading
import time
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

RUNNER_VERSION = "1.0.0"
ROOT = Path(__file__).resolve().parent.parent
SUITE_FILE = ROOT / "tests" / "suite.json"
MAX_HASH_BYTES = 64 * 1024 * 1024  # corpus files larger than this are not hashed
CORE_REPO = "mateusrdp/oxiguitar"
CORE_URL = "https://github.com/%s" % CORE_REPO
COMMAND_TIMEOUT = 600  # seconds one oxiguitar-cli command may take on one corpus file


# ---------------------------------------------------------------- output --


class Out:
    """The terminal (with a progress bar when it is one) and the log file."""

    def __init__(self, log_path):
        self.log = open(log_path, "w", encoding="utf-8", errors="replace")
        self.tty = sys.stdout.isatty() and os.environ.get("TERM") != "dumb"
        self.colour = self.tty and os.environ.get("NO_COLOR") is None
        self.bar_shown = False
        self.lock = threading.Lock()
        self.last_plain = 0.0
        if os.name == "nt":
            os.system("")  # turns on ANSI sequences in Windows consoles

    def stamp(self):
        return _dt.datetime.now().strftime("%H:%M:%S")

    def logline(self, text):
        with self.lock:
            self.log.write(text.rstrip("\n") + "\n")

    def section(self, title):
        self.logline("")
        self.logline("=" * 78)
        self.logline("== %s  [%s]" % (title, self.stamp()))
        self.logline("=" * 78)

    def say(self, text="", style=None):
        """A line for the terminal and the log."""
        self.logline(text)
        with self.lock:
            self._clear_bar()
            if self.colour and style:
                codes = {"ok": "32", "bad": "31;1", "warn": "33", "head": "1", "dim": "2"}
                text = "\033[%sm%s\033[0m" % (codes[style], text)
            print(text, flush=True)

    def _clear_bar(self):
        if self.bar_shown:
            sys.stdout.write("\r\033[K")
            self.bar_shown = False

    def progress(self, label, done, total, note=""):
        """Draws the bar (a terminal) or a line now and then (anything else)."""
        width = 30
        if total:
            frac = min(done / total, 1.0)
            filled = int(frac * width)
            bar = "#" * filled + "-" * (width - filled)
            text = "%s [%s] %3d%% %d/%d %s" % (label, bar, frac * 100, done, total, note)
        else:
            spin = "|/-\\"[int(time.time() * 4) % 4]
            text = "%s [%s] %d %s" % (label, spin, done, note)
        with self.lock:
            if self.tty:
                cols = shutil.get_terminal_size((100, 20)).columns
                sys.stdout.write("\r\033[K" + text[: cols - 1])
                sys.stdout.flush()
                self.bar_shown = True
            elif time.time() - self.last_plain > 30 or (total and done >= total):
                print(text, flush=True)
                self.last_plain = time.time()

    def end_progress(self):
        with self.lock:
            self._clear_bar()

    def close(self):
        self.log.close()


# ------------------------------------------------------------- commands --


def resolved(cmd, env=None):
    """The command with its program found along PATH as a shell finds it.
    On Windows a bare "bash" would otherwise start System32's (WSL's)
    before Git's, since CreateProcess looks in System32 before PATH."""
    path = (env or os.environ).get("PATH")
    found = shutil.which(cmd[0], path=path) if cmd else None
    return [found] + list(cmd[1:]) if found else list(cmd)


def run_capture(cmd, cwd=ROOT, env=None, timeout=120):
    """Runs a short command; returns (exit status, output). Never raises."""
    try:
        p = subprocess.run(
            resolved(cmd, env), cwd=str(cwd), env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            timeout=timeout, universal_newlines=True, encoding="utf-8", errors="replace",
        )
        return p.returncode, p.stdout
    except FileNotFoundError:
        return 127, "not found: %s" % cmd[0]
    except subprocess.TimeoutExpired:
        return 124, "timed out after %d s: %s" % (timeout, " ".join(cmd))
    except OSError as e:
        return 126, str(e)


def stream(cmd, out, env, on_line, cwd=ROOT):
    """Runs a long command, sends every output line to the log and to
    `on_line`, and returns its exit status."""
    out.logline("$ " + " ".join(cmd))
    try:
        p = subprocess.Popen(
            resolved(cmd, env), cwd=str(cwd), env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            universal_newlines=True, encoding="utf-8", errors="replace", bufsize=1,
        )
    except FileNotFoundError:
        out.logline("not found: %s" % cmd[0])
        return 127
    for line in p.stdout:
        line = line.rstrip("\r\n")
        out.logline(line)
        on_line(line)
    return p.wait()


# ------------------------------------------------------------ the suite --


def load_suite():
    if not SUITE_FILE.exists():
        return {}
    with open(SUITE_FILE, encoding="utf-8") as f:
        return json.load(f)


_HOST = []


def host_triple():
    """The Rust host triple (x86_64-unknown-linux-gnu, aarch64-apple-darwin,
    ...): the system and processor the tests are built for and run on."""
    if not _HOST:
        code, text = run_capture(["rustc", "-vV"])
        m = re.search(r"^host: (\S+)", text, re.M) if code == 0 else None
        _HOST.append(m.group(1) if m else "")
    return _HOST[0]


def this_os():
    t = host_triple()
    if "linux" in t:
        return "linux"
    if "apple" in t:
        return "macos"
    if "windows" in t:
        return "windows"
    s = platform.system().lower()
    return {"darwin": "macos"}.get(s, s)


def this_arch():
    t = host_triple()
    arch = t.split("-")[0] if t else platform.machine().lower()
    return {"amd64": "x86_64", "x64": "x86_64", "arm64": "aarch64"}.get(arch, arch)


def applies_here(item):
    """Whether a suite entry (the suite, a stage, a plugin) is for this system
    and processor: its "os" and "arch" lists, when it has them."""
    systems, arches = item.get("os"), item.get("arch")
    return (not systems or this_os() in systems) and (not arches or this_arch() in arches)


def why_not_here(item):
    parts = []
    if item.get("os") and this_os() not in item["os"]:
        parts.append(", ".join(item["os"]))
    if item.get("arch") and this_arch() not in item["arch"]:
        parts.append(", ".join(item["arch"]))
    return "only on " + " / ".join(parts)


def repository(suite):
    if suite.get("repository"):
        return suite["repository"]
    code, url = run_capture(["git", "remote", "get-url", "origin"])
    m = re.search(r"github\.com[:/]([^/]+/[^/.\s]+)", url) if code == 0 else None
    return m.group(1) if m else ROOT.name


# --------------------------------------------------------------- corpus --


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def describe_corpus(corpus, out):
    """Logs every corpus file (path, size, SHA-256) and returns totals."""
    files = sorted(p for p in corpus.rglob("*") if p.is_file())
    by_ext = {}
    hashes = {}
    total = 0
    out.section("Corpus: %s" % corpus)
    for i, p in enumerate(files):
        size = p.stat().st_size
        total += size
        ext = p.suffix.lower().lstrip(".") or "(none)"
        by_ext[ext] = by_ext.get(ext, 0) + 1
        digest = sha256_of(p) if size <= MAX_HASH_BYTES else "(not hashed: %d bytes)" % size
        hashes[p.relative_to(corpus).as_posix()] = digest
        out.logline("%s  %10d  %s" % (digest, size, p.relative_to(corpus).as_posix()))
        out.progress("Reading the corpus", i + 1, len(files))
    out.end_progress()
    return {"files": len(files), "bytes": total, "by_extension": dict(sorted(by_ext.items())),
            "hashes": hashes}


# ---------------------------------------------------------------- cargo --


RUNNING = re.compile(r"^\s*Running (?:unittests )?(\S+) \((.+?)\)\s*$")
DOCTESTS = re.compile(r"^\s*Doc-tests (\S+)\s*$")
RESULT = re.compile(r"^test (.+?) \.\.\. (ok|FAILED|ignored)(?:,.*)?$")
LISTED = re.compile(r"^(.+): (test|benchmark)$")
FAIL_HEAD = re.compile(r"^---- (.+?) stdout ----$")
CRASH = re.compile(r"process didn't exit successfully: `([^`]+)`(.*)$")


def exe_key(path):
    """A test binary's file name without its hash and extension."""
    name = Path(path.replace("\\", "/")).name
    name = re.sub(r"\.exe$", "", name)
    return name


def package_name(package_id):
    # "path+file:///x/y#name@1.0.0", "path+file:///x/name#1.0.0" or "name 1.0.0 (...)"
    m = re.search(r"#([^@#]+)@", package_id)
    if m:
        return m.group(1)
    m = re.search(r"/([^/#]+)#[\d.]+", package_id)
    if m:
        return m.group(1)
    return package_id.split(" ")[0]


class Cargo:
    def __init__(self, args, suite, out, env):
        self.out = out
        self.env = env
        c = suite.get("cargo", {})
        self.enabled = c.get("enabled", True) and (ROOT / "Cargo.toml").exists()
        base = list(c.get("args", ["--workspace"]))
        if c.get("locked", True):
            base.append("--locked")
        if args.release:
            base.append("--release")
        if args.ignore_rust_version:
            base.append("--ignore-rust-version")
        for p in c.get("exclude", []):
            base += ["--exclude", p]
        if c.get("features"):
            base += ["--features", ",".join(c["features"])]
        self.base = base
        self.test_args = list(c.get("test_args", []))
        self.binaries = {}  # exe key -> {"package", "target", "kind"}
        self.lock_packages = self._count_lock_packages()

    def _count_lock_packages(self):
        lock = ROOT / "Cargo.lock"
        if not lock.exists():
            return 0
        return lock.read_text(encoding="utf-8", errors="replace").count("[[package]]")

    def build(self):
        """`cargo test --no-run`: builds every test binary and learns which
        package and target each one is."""
        self.out.section("Build: cargo test --no-run")
        units = [0]
        estimate = max(self.lock_packages, 1)

        def on_line(line):
            if not line.startswith("{"):
                return
            try:
                msg = json.loads(line)
            except ValueError:
                return
            if msg.get("reason") != "compiler-artifact":
                return
            units[0] += 1
            exe = msg.get("executable")
            if exe and msg.get("profile", {}).get("test"):
                t = msg["target"]
                self.binaries[exe_key(exe)] = {
                    "package": package_name(msg.get("package_id", "")),
                    "target": t["name"],
                    "kind": t["kind"][0],
                }
            self.out.progress("Building", min(units[0], estimate - 1), estimate,
                              "(estimate) " + msg["target"]["name"])

        cmd = ["cargo", "test"] + self.base + ["--no-run", "--message-format=json-render-diagnostics"]
        status = stream(cmd, self.out, self.env, on_line)
        self.out.end_progress()
        return status

    def ident(self, exe, test):
        info = self.binaries.get(exe_key(exe)) if exe else None
        if info is None:
            return "%s::%s" % (exe_key(exe) if exe else "?", test)
        target = "lib" if info["kind"] in ("lib", "rlib", "cdylib", "proc-macro") else info["target"]
        return "%s/%s::%s" % (info["package"], target, test)

    def list_tests(self):
        """Every test cargo would run, by id, and the ignored ones."""
        found = {}
        for ignored in (False, True):
            self.out.section("List: cargo test -- --list%s" % (" --ignored" if ignored else ""))
            current = [None]

            def on_line(line):
                m = RUNNING.match(line)
                if m:
                    current[0] = m.group(2)
                    return
                m = DOCTESTS.match(line)
                if m:
                    current[0] = "doctest:" + m.group(1)
                    return
                m = LISTED.match(line)
                if m and m.group(2) == "test" and current[0]:
                    tid = self.test_id(current[0], m.group(1))
                    if ignored:
                        found[tid] = "ignored"
                    else:
                        found.setdefault(tid, "run")

            cmd = ["cargo", "test"] + self.base + ["--", "--list"] + (["--ignored"] if ignored else [])
            status = stream(cmd, self.out, self.env, on_line)
            if status != 0:
                return None, status
        return found, 0

    def test_id(self, current, name):
        if current.startswith("doctest:"):
            # A doc test runs as "NAME - compile" (or "- compile fail",
            # "- should panic"), but is listed as NAME.
            name = re.sub(r" - (compile( fail)?|should panic)$", "", name)
            return "%s/doc::%s" % (current[len("doctest:"):].replace("_", "-"), name)
        return self.ident(current, name)

    def run(self, expected):
        """`cargo test --no-fail-fast`, read as it goes."""
        self.out.section("Run: cargo test --no-fail-fast")
        state = {"current": None, "failing": None, "done": 0}
        results = {}
        failures = {}
        crashes = []

        def on_line(line):
            m = RUNNING.match(line)
            if m:
                state["current"] = m.group(2)
                state["failing"] = None
                return
            m = DOCTESTS.match(line)
            if m:
                state["current"] = "doctest:" + m.group(1)
                state["failing"] = None
                return
            cur = state["current"]
            m = RESULT.match(line)
            if m and cur:
                tid = self.test_id(cur, m.group(1))
                results[tid] = {"ok": "ok", "FAILED": "failed", "ignored": "ignored"}[m.group(2)]
                if results[tid] != "ignored":
                    state["done"] += 1
                self.out.progress("Testing", state["done"], expected, Path(cur).name[:40])
                if results[tid] == "failed":
                    self.out.say("  FAILED  " + tid, "bad")
                return
            m = FAIL_HEAD.match(line)
            if m and cur:
                tid = self.test_id(cur, m.group(1))
                state["failing"] = tid
                failures[tid] = []
                return
            if state["failing"]:
                if line.startswith("failures:") or line.startswith("test result:") or line.startswith("---- "):
                    state["failing"] = None
                else:
                    failures[state["failing"]].append(line)
                    return
            m = CRASH.search(line)
            if m:
                crashes.append({"binary": self.ident(m.group(1).split(" ")[0], "*").rstrip(":*"),
                                "detail": line.strip()})
                self.out.say("  CRASHED " + line.strip(), "bad")

        cmd = ["cargo", "test"] + self.base + ["--no-fail-fast", "--"] + self.test_args
        status = stream(cmd, self.out, self.env, on_line)
        self.out.end_progress()
        return status, results, failures, crashes


# ---------------------------------------------------------- extra stages --


def run_stage(stage, out, env, args=None):
    """A command from suite.json. Returns ("passed"|"failed"|"skipped"|
    "advisory", note): an "advisory" stage (a speed budget, which depends on
    the machine) that fails is said, but doesn't fail the run. A `cargo`
    command takes the runner's --ignore-rust-version."""
    name = stage["name"]
    if not applies_here(stage):
        return "skipped", why_not_here(stage)
    for tool in stage.get("needs", []):
        if shutil.which(tool) is None:
            return "skipped", "needs %s, not found" % tool
    if stage.get("when") == "corpus" and not env.get("OXIGUITAR_CORPUS"):
        return "skipped", "needs a corpus"
    # "ready": a quick command that says whether what the stage needs is
    # running, not only installed (a docker daemon, say); "why" says what
    # to do when it isn't.
    ready = stage.get("ready")
    if ready:
        try:
            ok = subprocess.run(resolved(ready["run"], env), cwd=str(ROOT), env=env, stdin=subprocess.DEVNULL,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                timeout=60).returncode == 0
        except (OSError, subprocess.TimeoutExpired):
            ok = False
        if not ok:
            out.logline("Stage %s: %s failed" % (name, " ".join(ready["run"])))
            return "skipped", ready.get("why", "%s failed" % " ".join(ready["run"]))
    out.section("Stage: %s" % name)
    stage_env = dict(env)
    stage_env.update(stage.get("env", {}))
    lines = [0]

    def on_line(_line):
        lines[0] += 1
        out.progress(name, lines[0], 0, "output lines")

    cmd = list(stage["run"])
    if args is not None and args.ignore_rust_version and cmd[:1] == ["cargo"] and len(cmd) > 1:
        cmd.insert(2, "--ignore-rust-version")
    status = stream(cmd, out, stage_env, on_line)
    out.end_progress()
    if status == 0:
        return "passed", "exit status 0"
    if stage.get("advisory"):
        return "advisory", "exit status %d (advisory: depends on this machine)" % status
    return "failed", "exit status %d" % status


def run_setup(step, out, env):
    """A setup step from suite.json, run before the build: fetches or builds
    something the tests need. With "env_from_stdout", the last line it
    prints becomes that environment variable for everything after it.
    Returns ("passed"|"failed"|"skipped", note)."""
    if not applies_here(step):
        return "skipped", why_not_here(step)
    var = step.get("env_from_stdout")
    if var and env.get(var):
        out.logline("Setup %s: %s is already set, to %s" % (step["name"], var, env[var]))
        return "skipped", "%s already set" % var
    for tool in step.get("needs", []):
        if shutil.which(tool) is None:
            return "failed", "needs %s, not found" % tool
    out.section("Setup: %s" % step["name"])
    lines = []
    status = stream(step["run"], out, env, lambda l: lines.append(l))
    if status != 0:
        return "failed", "exit status %d" % status
    if var:
        value = next((l.strip() for l in reversed(lines) if l.strip()), "")
        if not value:
            return "failed", "printed nothing for %s" % var
        env[var] = value
        out.logline("%s=%s" % (var, value))
        return "passed", "%s set" % var
    return "passed", "done"


# -------------------------------------------------------------- features --


TOKEN = re.compile(r"`([^`]+)`")


def parse_features(path):
    """Rows of the features page's tables: (feature, [test ids])."""
    rows = []
    in_table = False
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line.startswith("|"):
            in_table = False
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if not in_table:
            in_table = True  # the header row
            header = [c.lower() for c in cells]
            col = header.index("tests") if "tests" in header else len(cells) - 1
            continue
        if set(line.replace("|", "").strip()) <= set("-: "):
            continue  # the separator row
        if col >= len(cells):
            rows.append((cells[0], []))
            continue
        rows.append((cells[0], TOKEN.findall(cells[col])))
    return rows


def matches(token, tests):
    if token.endswith("*"):
        stem = token[:-1]
        return any(t.startswith(stem) or ("/" + stem) in t or ("::" + stem) in t for t in tests)
    return any(t == token or t.endswith("/" + token) or t.endswith("::" + token) for t in tests)


def check_features(path, tests, stages):
    """Every feature names at least one test, and every test it names exists."""
    if not path.exists():
        return {"status": "failed", "problems": ["no features page at %s" % path.relative_to(ROOT)],
                "features": 0, "not_here": []}
    problems = []
    not_here = []
    rows = parse_features(path)
    if not rows:
        problems.append("the features page lists no feature")
    here = this_os()
    for feature, tokens in rows:
        if not tokens:
            problems.append("%s: names no test" % feature)
        for token in tokens:
            systems = None
            m = re.match(r"^((?:linux|windows|macos|x86_64|aarch64)(?:,(?:linux|windows|macos|x86_64|aarch64))*):(.+)$", token)
            if m:
                systems = m.group(1).split(",")
                token = m.group(2)
            if token.startswith("stage:"):
                if token[len("stage:"):] not in stages:
                    problems.append("%s: no stage named %r in tests/suite.json" % (feature, token))
                continue
            if systems and not set(systems) & {here, this_arch()}:
                not_here.append("%s: %s (on %s)" % (feature, token, ", ".join(systems)))
                continue
            if not matches(token, tests):
                problems.append("%s: no test %r" % (feature, token))
    return {"status": "failed" if problems else "passed", "problems": problems,
            "features": len(rows), "not_here": not_here}


# ---------------------------------------------------------- latest core --


def version_key(tag):
    return tuple(int(n) for n in re.findall(r"\d+", tag)[:3])


def latest_core_tag(out):
    """The tag of the newest published release of the core (pre-releases
    count: every release is one until v1.0)."""
    req = urllib.request.Request(
        "https://api.github.com/repos/%s/releases?per_page=20" % CORE_REPO,
        headers={"Accept": "application/vnd.github+json", "User-Agent": "oxiguitar-test-all"})
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            releases = json.load(r)
        tags = [x["tag_name"] for x in releases if not x.get("draft")]
        if tags:
            return max(tags, key=version_key)
        out.logline("GitHub lists no published release of the core")
    except (urllib.error.URLError, OSError, ValueError) as e:
        out.logline("GitHub's API did not answer (%s); asking git for the tags instead" % e)
    code, text = run_capture(["git", "ls-remote", "--tags", "--refs", CORE_URL + ".git", "v*"], timeout=120)
    tags = [l.split("refs/tags/")[1] for l in text.splitlines() if "refs/tags/" in l] if code == 0 else []
    if not tags:
        return None
    return max(tags, key=version_key)


def exe(name):
    return name + (".exe" if os.name == "nt" else "")


def cargo_target_dir(cwd, env):
    code, text = run_capture(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=cwd, env=env)
    if code == 0:
        try:
            return Path(json.loads(text)["target_directory"])
        except (ValueError, KeyError):
            pass
    return Path(cwd) / "target"


def cargo_with_progress(cmd, cwd, out, env, label):
    count = [0]

    def on_line(line):
        if line.startswith("{"):
            try:
                msg = json.loads(line)
            except ValueError:
                return
            if msg.get("reason") == "compiler-artifact":
                count[0] += 1
                out.progress(label, count[0], 0, msg["target"]["name"])

    status = stream(cmd + ["--message-format=json-render-diagnostics"], out, env, on_line, cwd=cwd)
    out.end_progress()
    return status


def cache_dir():
    """Where fetched releases of the core are kept between runs, shared by
    every OxiGuitar repository on this machine (OXIGUITAR_TEST_CACHE)."""
    if os.environ.get("OXIGUITAR_TEST_CACHE"):
        return Path(os.environ["OXIGUITAR_TEST_CACHE"]).expanduser()
    if this_os() == "windows":
        base = Path(os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local")))
    elif this_os() == "macos":
        base = Path.home() / "Library" / "Caches"
    else:
        base = Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
    return base / "oxiguitar-test-all"


def prepare_core(args, out, env):
    """A checkout of the core (the newest release unless --core says) with
    its oxiguitar-cli and oxiguitar-plugin-test built. Returns (info, note):
    info is None when it can't be had."""
    out.section("The core")
    if args.core:
        path = Path(args.core).expanduser().resolve()
        code, desc = run_capture(["git", "describe", "--tags", "--always", "--dirty"], cwd=path)
        tag = "local %s (%s)" % (path, desc.strip() if code == 0 else "not a git checkout")
    else:
        tag = args.core_version or latest_core_tag(out)
        if not tag:
            return None, "could not find the core's newest release (no network?); use --core DIR"
        path = cache_dir() / "core" / tag
        if not (path / "Cargo.toml").exists():
            if path.exists():
                shutil.rmtree(str(path))
            path.parent.mkdir(parents=True, exist_ok=True)
            out.say("Fetching OxiGuitar %s (the core's newest release)..." % tag)
            status = stream(["git", "clone", "--quiet", "--depth", "1", "--branch", tag,
                             CORE_URL + ".git", str(path)], out, env, lambda l: None)
            if status != 0:
                shutil.rmtree(str(path), ignore_errors=True)
                return None, "git could not fetch %s %s (exit status %d)" % (CORE_URL, tag, status)
    out.logline("core: %s at %s" % (tag, path))
    out.say("Building oxiguitar-cli and oxiguitar-plugin-test of OxiGuitar %s..." % tag)
    cmd = ["cargo", "build", "--release", "--locked", "-p", "oxiguitar-cli", "-p", "oxiguitar-plugin-test"]
    if args.ignore_rust_version:
        cmd.append("--ignore-rust-version")
    status = cargo_with_progress(cmd, path, out, env, "Building the core")
    if status != 0:
        return None, ("the core %s does not build here (exit status %d; its CONTRIBUTING.md lists what "
                      "a build needs)" % (tag, status))
    bins = cargo_target_dir(path, env) / "release"
    return {"tag": tag, "path": path, "cli": bins / exe("oxiguitar-cli"),
            "checker": bins / exe("oxiguitar-plugin-test")}, "OxiGuitar %s" % tag


def build_plugin_files(suite, args, out, env, dest):
    """This repository's plugins as .oxp files for this system, in `dest`.
    Returns (paths, note)."""
    out.section("Plugin files")
    dest.mkdir(parents=True, exist_ok=True)
    target = cargo_target_dir(ROOT, env)
    made = []
    for p in suite.get("plugins", []):
        cmd = ["cargo", "rustc", "--locked", "--release", "-p", p["package"],
               "--features", p.get("features", "oxp"), "--crate-type", "cdylib"]
        if args.ignore_rust_version:
            cmd.append("--ignore-rust-version")
        status = cargo_with_progress(cmd, ROOT, out, env, "Building %s.oxp" % p["name"])
        lib = p["library"]
        built = target / "release" / {"windows": lib + ".dll", "macos": "lib%s.dylib" % lib}.get(
            this_os(), "lib%s.so" % lib)
        if status != 0 or not built.exists():
            return made, "%s.oxp did not build (exit status %d)" % (p["name"], status)
        file = dest / ("%s.oxp" % p["name"])
        shutil.copyfile(str(built), str(file))
        made.append(file)
    return made, "%d plugin file(s)" % len(made)


def run_contract(core, files, out, env, needed=()):
    """The latest core's plugin checker on each plugin file, with the
    plugins they need (`needed`, .oxp paths) beside them."""
    out.section("Contract: oxiguitar-plugin-test (%s)" % core["tag"])
    fails = []
    extra = []
    if needed:
        _, usage = run_capture([str(core["checker"]), "--help"], env=env)
        if "--with" not in usage:
            return ("skipped", "%s's oxiguitar-plugin-test can't load the plugins this one needs "
                    "(its --with option comes in a later release)" % core["tag"], [])
        for n in needed:
            extra += ["--with", str(n)]

    def on_line(line):
        if line.strip().startswith("FAIL"):
            fails.append(line.strip())
            out.say("  " + line.strip(), "bad")
        else:
            out.progress("Plugin contract", 0, 0, line.strip()[:50])

    status = stream([str(core["checker"])] + extra + [str(f) for f in files], out, env, on_line)
    out.end_progress()
    if status == 0 and not fails:
        return "passed", "%d plugin file(s) on %s" % (len(files), core["tag"]), []
    return "failed", "%d check(s) failed on %s" % (max(len(fails), 1), core["tag"]), fails


def info_text(cli, file, env, names):
    """`oxiguitar-cli info --bars FILE`: the score's structure, bar by bar,
    the file's name taken out, and the import log left out (what a reader
    notes about a file differs between a first and a second reading, when
    the first trip lost something, while the score stays the same)."""
    code, text = run_capture([str(cli), "info", "--bars", str(file)], env=env, timeout=COMMAND_TIMEOUT)
    for n in names:
        text = text.replace(str(n), "FILE").replace(Path(n).name, "FILE")
    kept, in_log = [], False
    for line in text.splitlines():
        if line.startswith("Import log:"):
            in_log = True
            continue
        if in_log:
            if line.strip() == "":
                in_log = False
            continue
        kept.append(line)
    return code, "\n".join(kept)


def plugin_names(suite):
    """The names our plugins' manifests give (what `detect` prints)."""
    names = set()
    for p in suite.get("plugins", []):
        try:
            with open(ROOT / p.get("manifest", "manifest.json"), encoding="utf-8") as f:
                names.add(json.load(f)["name"])
        except (OSError, ValueError, KeyError):
            pass
    return names


def fetch_readers(core, names, out, env, out_dir):
    """The newest releases of other format plugins (`names`), through the
    core release's own scripts/released-plugins.sh, so that files of their
    formats are written in ours. Returns (folder or None, note)."""
    script = core["path"] / "scripts" / "released-plugins.sh"
    if not script.exists() or shutil.which("bash") is None or shutil.which("curl") is None:
        return None, "can't fetch %s here (needs bash and curl)" % ", ".join(names)
    out.section("Readers: %s" % ", ".join(names))
    lines = []
    status = stream(["bash", str(script), str(out_dir / "readers")] + list(names), out, env,
                    lambda l: lines.append(l))
    folder = next((l.strip() for l in reversed(lines) if l.strip()), "")
    if status != 0 or not Path(folder).is_dir():
        return None, "couldn't fetch %s (exit status %d)" % (", ".join(names), status)
    return Path(folder), "with the released %s" % ", ".join(names)


def corpus_through_core(core, plugin_dir, corpus, cfg, out, env, out_dir, ours, readers=None):
    """Every corpus file our plugins recognise, through the newest core's
    oxiguitar-cli: recognised, read, and written in each of our formats and
    read back three times, the third trip changing nothing the second wrote. With `readers` (a
    folder of other format plugins), their files are written in our formats
    too."""
    out.section("Corpus through %s" % core["tag"])
    home = Path(tempfile.mkdtemp(prefix="home-", dir=str(out_dir)))
    folders = [str(plugin_dir)] + ([str(readers)] if readers else [])
    (home / "settings.json").write_text(json.dumps({"plugin_folders": folders}), encoding="utf-8")
    cenv = dict(env)
    cenv["OXIGUITAR_HOME"] = str(home)
    work = Path(tempfile.mkdtemp(prefix="convert-", dir=str(out_dir)))
    ours = {e.lower() for e in cfg.get("extensions", [])}
    write_as = cfg.get("write_as", [])
    files = sorted(p for p in corpus.rglob("*") if p.is_file())
    report = {"test": "through OxiGuitar %s (oxiguitar-cli)" % core["tag"], "corpus": str(corpus),
              "checked": 0, "passed": 0, "not_taken": 0, "failures": []}
    cli = core["cli"]

    def fail(path, step, text):
        size = path.stat().st_size
        report["failures"].append({
            "file": path.relative_to(corpus).as_posix(), "size": size,
            "sha256": sha256_of(path) if size <= MAX_HASH_BYTES else "",
            "error": "%s: %s" % (step, " ".join(text.strip().splitlines()[-6:])[:1500])})
        out.say("  FAILED  %s: %s" % (path.relative_to(corpus).as_posix(), step), "bad")

    for i, path in enumerate(files):
        out.progress("Corpus through the core", i + 1, len(files), path.name[:40])
        rel = path.relative_to(corpus).as_posix()
        code, text = run_capture([str(cli), "detect", str(path)], env=cenv, timeout=COMMAND_TIMEOUT)
        out.logline("detect %s: %d %s" % (rel, code, text.strip()))
        if code != 0:
            if path.suffix.lower().lstrip(".") in ours:
                report["checked"] += 1
                fail(path, "not recognised, though its extension is ours", text)
            else:
                report["not_taken"] += 1
            continue
        theirs = not any(("read by the %s plugin" % n) in text for n in ours)
        if theirs and not write_as:
            report["not_taken"] += 1
            continue
        report["checked"] += 1
        code, text = info_text(cli, path, cenv, [path])
        out.logline("info %s: exit %d, %d lines" % (rel, code, len(text.splitlines())))
        if code != 0:
            fail(path, "info (reading it)", text)
            continue
        ok = True
        for ext in write_as:
            # Written once from the file, then twice more from what was
            # read back: a lossy format settles after its first trip, so
            # the second and third must be alike (the plugins' own corpus
            # tests hold them to the same).
            trips = [work / ("trip%d.%s" % (n, ext)) for n in (1, 2, 3)]
            source, problem = path, None
            for n, target in enumerate(trips, 1):
                code, text = run_capture([str(cli), "convert", str(source), str(target)], env=cenv,
                                         timeout=COMMAND_TIMEOUT)
                out.logline("convert %s trip %d -> .%s: %d %s" % (rel, n, ext, code, text.strip()[-300:]))
                if code != 0:
                    problem = ("convert to .%s" % ext if n == 1 else
                               "reading back the .%s written (trip %d)" % (ext, n)), text
                    break
                source = target
            if problem:
                fail(path, *problem)
                ok = False
                break
            code, a = info_text(cli, trips[1], cenv, [trips[1]])
            code2, b = info_text(cli, trips[2], cenv, [trips[2]])
            if code != 0 or code2 != 0:
                fail(path, "reading back the .%s written" % ext, a if code else b)
                ok = False
                break
            if a != b:
                diff = [x for x in zip(a.splitlines(), b.splitlines()) if x[0] != x[1]][:3]
                fail(path, "a third trip through .%s changed what the second wrote" % ext,
                     "; ".join("%r became %r" % d for d in diff) or "line count changed")
                ok = False
                break
        if ok:
            report["passed"] += 1
    out.end_progress()
    shutil.rmtree(str(work), ignore_errors=True)
    shutil.rmtree(str(home), ignore_errors=True)
    with open(out_dir / "corpus-through-core.json", "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1)
    status = "failed" if report["failures"] else "passed"
    return status, "%d files checked, %d failed" % (report["checked"], len(report["failures"]))


# --------------------------------------------------------------- report --


def env_info(args, suite, corpus_info):
    info = {
        "runner": RUNNER_VERSION,
        "repository": repository(suite),
        "date (UTC)": _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%d %H:%M"),
        "system": "%s (%s)" % (platform.platform(), platform.machine()),
        "tested for": "%s %s (%s)" % (this_os(), this_arch(), host_triple() or "no rustc"),
        "python": platform.python_version(),
    }
    code, v = run_capture(["rustc", "-V"])
    info["rustc"] = v.strip() if code == 0 else "not found"
    code, v = run_capture(["cargo", "-V"])
    info["cargo"] = v.strip() if code == 0 else "not found"
    code, head = run_capture(["git", "rev-parse", "HEAD"])
    info["commit"] = head.strip() if code == 0 else "unknown"
    code, branch = run_capture(["git", "rev-parse", "--abbrev-ref", "HEAD"])
    info["branch"] = branch.strip() if code == 0 else "unknown"
    code, dirty = run_capture(["git", "status", "--porcelain"])
    if code == 0:
        changed = [l for l in dirty.splitlines() if l.strip()]
        info["local changes"] = "%d file(s)" % len(changed) if changed else "none"
    opts = []
    if args.release:
        opts.append("--release")
    if args.ignore_rust_version:
        opts.append("--ignore-rust-version")
    info["options"] = " ".join(opts) or "none"
    for k in sorted(os.environ):
        if k.startswith("OXIGUITAR_") and k not in ("OXIGUITAR_CORPUS", "OXIGUITAR_TEST_REPORT_DIR"):
            info[k] = "set"
    if corpus_info:
        info["corpus"] = "%d files, %.1f MB: %s" % (
            corpus_info["files"], corpus_info["bytes"] / 1e6,
            ", ".join("%s %d" % kv for kv in corpus_info["by_extension"].items()))
    else:
        info["corpus"] = "none"
    return info


def read_corpus_reports(report_dir):
    reports = []
    for p in sorted(report_dir.glob("corpus-*.json")):
        try:
            with open(p, encoding="utf-8") as f:
                reports.append(json.load(f))
        except (OSError, ValueError):
            pass
    return reports


def tail(lines, n=25):
    """The lines worth reading of a failure, a backtrace left out (it is in
    the log)."""
    kept, skipping = [], False
    for l in lines:
        if l.startswith("stack backtrace:"):
            skipping = True
            kept.append("(backtrace in the log)")
            continue
        if skipping:
            if l.startswith("note: Some details are omitted"):
                skipping = False
            continue
        kept.append(l)
    lines = [l for l in kept if l.strip()]
    if len(lines) <= n:
        return lines
    return lines[:8] + ["... (%d lines; all of them are in the log)" % (len(lines) - n + 8)] + lines[-(n - 9):]


def write_summary(path, info, stages, results, failures, crashes, corpus_reports, features, log_name,
                  contract_fails, hashes=None):
    repo = info["repository"]
    failed_tests = sorted(t for t, r in results.items() if r == "failed")
    passed = sum(1 for r in results.values() if r == "ok")
    ignored = sum(1 for r in results.values() if r == "ignored")
    overall = all(s["status"] != "failed" for s in stages.values())
    L = []
    L.append("# Test run: %s" % repo)
    L.append("")
    L.append("**Result: %s.** %d passed, %d failed, %d ignored on purpose."
             % ("PASSED" if overall else "FAILED", passed, len(failed_tests), ignored))
    L.append("")
    L.append("| | |")
    L.append("| --- | --- |")
    for k, v in info.items():
        L.append("| %s | %s |" % (k, str(v).replace("|", "/")))
    L.append("")
    L.append("## Stages")
    L.append("")
    L.append("| Stage | Result | Note |")
    L.append("| --- | --- | --- |")
    for name, s in stages.items():
        L.append("| %s | %s | %s |" % (name, s["status"], s.get("note", "")))
    skipped = [n for n, st in stages.items() if st["status"] == "skipped"]
    if skipped:
        L.append("")
        L.append("Skipped stages are for other systems or processors, or need something this "
                 "machine lacks; they are not failures.")
    if any(st["status"] == "advisory" for st in stages.values()):
        L.append("")
        L.append("An advisory stage (a speed budget) missed its mark on this machine: worth "
                 "mentioning if the machine isn't slow, but not a failure.")
    if failed_tests or crashes:
        L.append("")
        L.append("## Failures")
        for c in crashes:
            L.append("")
            L.append("- A test binary crashed: `%s`" % c["detail"])
        for t in failed_tests:
            L.append("")
            L.append("### `%s`" % t)
            L.append("")
            L.append("```")
            L.extend(tail(failures.get(t, ["(no output; see the log)"])))
            L.append("```")
    if contract_fails:
        L.append("")
        L.append("## Plugin contract on the newest core")
        L.append("")
        L.extend("- `%s`" % f for f in contract_fails)
    if corpus_reports:
        L.append("")
        L.append("## Corpus")
        L.append("")
        L.append("| Corpus test | Files checked | Passed | Failed | Not for this test |")
        L.append("| --- | --- | --- | --- | --- |")
        for r in corpus_reports:
            L.append("| `%s` | %d | %d | %d | %d |" % (
                r.get("test", "?"), r.get("checked", 0), r.get("passed", 0),
                len(r.get("failures", [])), r.get("not_taken", 0)))
        bad = [(r.get("test", "?"), f) for r in corpus_reports for f in r.get("failures", [])]
        if bad:
            L.append("")
            L.append("Files that failed (the SHA-256 tells us whether we have the same file; "
                     "please don't attach files you have no right to share):")
            L.append("")
            for test, f in bad[:200]:
                L.append("- `%s`: `%s` (%s bytes, sha256 `%s`): %s" % (
                    test, f.get("file"), f.get("size"),
                    (f.get("sha256") or (hashes or {}).get(f.get("file"), ""))[:16],
                    f.get("error", "").splitlines()[0][:300] if f.get("error") else ""))
            if len(bad) > 200:
                L.append("- ... and %d more (in the corpus-*.json files and the log)" % (len(bad) - 200))
    if features:
        L.append("")
        L.append("## Features page")
        L.append("")
        L.append("%d features; %s." % (features["features"], features["status"]))
        for p in features["problems"]:
            L.append("- %s" % p)
        if features["not_here"]:
            L.append("")
            L.append("Tested only on other systems (not checked here): %d." % len(features["not_here"]))
    L.append("")
    L.append("## Reporting a problem")
    L.append("")
    L.append("If anything above failed, please tell us, so we can fix it:")
    L.append("")
    L.append("1. Update to the newest `develop` (`git checkout develop && git pull`) and run the "
             "script again: it may be fixed already.")
    L.append("2. Look for an existing issue about the same test: "
             "https://github.com/%s/issues?q=is%%3Aissue+%s" % (
                 repo, urllib.parse.quote_plus((failed_tests[0].split("::")[-1]) if failed_tests else "test-all")))
    L.append("3. Otherwise open a new issue: https://github.com/%s/issues/new" % repo)
    L.append("   - Title: `test-all: <the first failing test or stage>`.")
    L.append("   - Body: paste this whole `summary.md`.")
    L.append("   - Attach `%s` (drag it onto the issue's text box). Over 25 MB, zip it first. "
             "It lists your corpus's file names and your folder paths: look it over, and "
             "replace anything private before you attach it." % log_name)
    L.append("   - Don't attach corpus files unless they are yours to share. If a failing file is "
             "yours, attaching it helps a lot.")
    L.append("4. Or hand `summary.md` and `%s` to a language model together with this repository "
             "and ask it to find the cause." % log_name)
    L.append("")
    path.write_text("\n".join(L) + "\n", encoding="utf-8")
    return overall


# ----------------------------------------------------------------- main --


def main():
    ap = argparse.ArgumentParser(
        description="Run every test of this repository on your own test corpus, "
                    "for this system and processor.",
        epilog="The corpus can also be given as OXIGUITAR_CORPUS. The manual's Testing page says more.")
    ap.add_argument("--corpus", metavar="DIR", help="your folder of test files (searched recursively)")
    ap.add_argument("--no-corpus", action="store_true", help="run without a corpus (its tests are skipped)")
    ap.add_argument("--release", action="store_true", help="optimised test build (faster corpus tests, slower build)")
    ap.add_argument("--ignore-rust-version", action="store_true",
                    help="pass --ignore-rust-version to cargo (a Rust older than rust-version)")
    ap.add_argument("--core", metavar="DIR",
                    help="plugins: test against this checkout of the core instead of its newest release")
    ap.add_argument("--core-version", metavar="TAG", help="plugins: test against this release of the core (v0.21.5)")
    ap.add_argument("--features-only", action="store_true",
                    help="only check the features page against the tests (what CI runs)")
    ap.add_argument("--out", metavar="DIR", help="where to write the results (default target/test-all/<time>)")
    ap.add_argument("--version", action="version", version=RUNNER_VERSION)
    args = ap.parse_args()

    suite = load_suite()
    uses_corpus = suite.get("corpus", {}).get("uses", False)
    stamp = _dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out_dir = Path(args.out).resolve() if args.out else ROOT / "target" / "test-all" / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    log_path = out_dir / "test-all.log"
    out = Out(log_path)

    env = dict(os.environ)
    env["OXIGUITAR_TEST_REPORT_DIR"] = str(out_dir)
    if args.ignore_rust_version:
        env["OXIGUITAR_IGNORE_RUST_VERSION"] = "1"  # for setup scripts that build
    env.setdefault("CARGO_TERM_COLOR", "never")
    corpus = None
    corpus_info = None
    if args.features_only:
        args.no_corpus = True
    if uses_corpus and not args.no_corpus:
        given = args.corpus or os.environ.get("OXIGUITAR_CORPUS")
        if not given:
            out.say("This repository's tests use a corpus of test files. Give yours with "
                    "--corpus DIR (or OXIGUITAR_CORPUS=DIR), or run with --no-corpus.", "bad")
            out.close()
            return 2
        corpus = Path(given).expanduser().resolve()
        if not corpus.is_dir():
            out.say("The corpus %s is not a folder." % corpus, "bad")
            out.close()
            return 2
        env["OXIGUITAR_CORPUS"] = str(corpus)
        env["OXIGUITAR_CORPUS_REQUIRED"] = "1"
    else:
        env.pop("OXIGUITAR_CORPUS", None)
        env.pop("OXIGUITAR_CORPUS_REQUIRED", None)

    started = time.time()
    out.say("OxiGuitar test run (test-all %s): %s" % (RUNNER_VERSION, repository(suite)), "head")
    out.say("For %s %s. Results go to %s" % (this_os(), this_arch(), out_dir), "dim")
    stages = {}
    results, failures, crashes = {}, {}, []
    listed = {}
    features = None
    contract_fails = []

    if not applies_here(suite):
        # A plugin for another system (WASAPI on Linux, say): nothing here to test.
        note = "this repository is %s; this is %s %s" % (why_not_here(suite), this_os(), this_arch())
        stages["everything"] = {"status": "skipped", "note": note}
        out.say("Nothing to test here: " + note, "warn")
        info = env_info(args, suite, None)
        summary_path = out_dir / "summary.md"
        write_summary(summary_path, info, stages, {}, {}, [], [], None, log_path.name, [])
        out.say("Summary: %s" % summary_path)
        out.close()
        return 0

    if corpus:
        corpus_info = describe_corpus(corpus, out)
        out.say("Corpus: %d files in %s" % (corpus_info["files"], corpus))
    info = env_info(args, suite, corpus_info)
    out.section("Environment")
    for k, v in info.items():
        out.logline("%s: %s" % (k, v))
    out.logline("suite.json: " + json.dumps(suite, indent=2))

    for step in suite.get("setup", []) if not args.features_only else []:
        status, note = run_setup(step, out, env)
        stages["setup: " + step["name"]] = {"status": status, "note": note}
        out.say("  setup %s: %s (%s)" % (step["name"], status, note),
                {"passed": "ok", "failed": "bad", "skipped": "warn"}[status])

    cargo = Cargo(args, suite, out, env)
    if cargo.enabled:
        if shutil.which("cargo") is None:
            out.say("cargo is not installed: see https://rustup.rs", "bad")
            out.close()
            return 2
        t = time.time()
        out.say("Building the tests (the first build takes a while)...")
        status = cargo.build()
        stages["build"] = {"status": "passed" if status == 0 else "failed",
                           "note": "%d test binaries, %.0f s" % (len(cargo.binaries), time.time() - t)}
        out.say("  build: %s (%s)" % (stages["build"]["status"], stages["build"]["note"]),
                "ok" if status == 0 else "bad")
        if status != 0:
            out.say("The build failed: the errors are in the log (search for 'error').", "bad")
        else:
            listed, status = cargo.list_tests()
            if listed is None:
                stages["list"] = {"status": "failed", "note": "exit status %d" % status}
                listed = {}
            expected = sum(1 for v in listed.values() if v == "run")
        if status == 0 and not args.features_only:
            t = time.time()
            out.say("Running %d tests (%d ignored on purpose)..." % (
                expected, sum(1 for v in listed.values() if v == "ignored")))
            status, results, failures, crashes = cargo.run(expected)
            failed = sum(1 for r in results.values() if r == "failed")
            missing = [tid for tid, v in listed.items() if v == "run" and tid not in results]
            note = "%d passed, %d failed, %.0f s" % (
                sum(1 for r in results.values() if r == "ok"), failed, time.time() - t)
            if missing:
                note += ", %d never reported (a crash?)" % len(missing)
                out.logline("Listed but never reported: " + ", ".join(sorted(missing)))
            bad = status != 0 or failed or missing or crashes
            stages["tests"] = {"status": "failed" if bad else "passed", "note": note}
            out.say("  tests: %s (%s)" % (stages["tests"]["status"], note), "bad" if bad else "ok")

    # A plugin repository: its plugins against the newest release of the core.
    plugins = [] if args.features_only else [p for p in suite.get("plugins", []) if applies_here(p)]
    if plugins:
        core, note = prepare_core(args, out, env)
        stages["latest core"] = {"status": "passed" if core else "failed", "note": note}
        out.say("  latest core: %s (%s)" % (stages["latest core"]["status"], note), "ok" if core else "bad")
        if core:
            info["core"] = core["tag"]
            plugin_dir = out_dir / "plugins"
            made, note = build_plugin_files({"plugins": plugins}, args, out, env, plugin_dir)
            ok = len(made) == len(plugins)
            stages["plugin files"] = {"status": "passed" if ok else "failed", "note": note}
            out.say("  plugin files: %s (%s)" % (stages["plugin files"]["status"], note), "ok" if ok else "bad")
            if made:
                needed = [env[v] for p in plugins for v in p.get("with", []) if env.get(v)]
                missing = [v for p in plugins for v in p.get("with", []) if not env.get(v)]
                if missing:
                    status, note = "failed", "no %s (its setup step failed?)" % ", ".join(missing)
                else:
                    status, note, contract_fails = run_contract(core, made, out, env, needed)
                stages["contract (latest core)"] = {"status": status, "note": note}
                out.say("  contract (latest core): %s (%s)" % (status, note),
                        {"passed": "ok", "failed": "bad", "skipped": "warn"}[status])
            cfg = suite.get("corpus", {}).get("through_core")
            if made and ok and cfg:
                if corpus:
                    readers, rnote = (None, "")
                    if cfg.get("readers"):
                        readers, rnote = fetch_readers(core, cfg["readers"], out, env, out_dir)
                        out.say("  readers: " + rnote, "ok" if readers else "warn")
                    status, note = corpus_through_core(core, plugin_dir, corpus, cfg, out, env, out_dir,
                                                       plugin_names(suite), readers)
                    if rnote:
                        note += "; " + rnote
                else:
                    status, note = "skipped", "no corpus"
                stages["corpus through the core"] = {"status": status, "note": note}
                out.say("  corpus through the core: %s (%s)" % (status, note),
                        {"passed": "ok", "failed": "bad", "skipped": "warn"}[status])
    elif suite.get("plugins") and not args.features_only:
        stages["latest core"] = {"status": "skipped", "note": "no plugin of this repository is for %s %s"
                                 % (this_os(), this_arch())}

    for stage in [] if args.features_only else suite.get("stages", []):
        status, note = run_stage(stage, out, env, args)
        stages[stage["name"]] = {"status": status, "note": note}
        out.say("  %s: %s (%s)" % (stage["name"], status, note),
                {"passed": "ok", "failed": "bad", "skipped": "warn", "advisory": "warn"}[status])

    page = suite.get("features", "docs/manual/src/features.md")
    if page and (results or listed or not cargo.enabled):
        all_tests = set(listed) | set(results)
        known = {s["name"] for s in suite.get("stages", [])} | set(stages)
        if suite.get("plugins"):
            known |= {"latest core", "plugin files", "contract (latest core)"}
            if suite.get("corpus", {}).get("through_core"):
                known.add("corpus through the core")
        features = check_features(ROOT / page, all_tests, known)
        stages["features page"] = {"status": features["status"],
                                   "note": "%d features, %d problems" % (features["features"],
                                                                         len(features["problems"]))}
        out.section("Features page: %s" % page)
        for p in features["problems"]:
            out.logline("problem: " + p)
        for p in features["not_here"]:
            out.logline("other systems: " + p)
        out.say("  features page: %s (%s)" % (features["status"], stages["features page"]["note"]),
                "ok" if features["status"] == "passed" else "bad")

    corpus_reports = read_corpus_reports(out_dir)
    if corpus:
        if not corpus_reports and stages.get("tests"):
            stages["corpus"] = {"status": "failed",
                                "note": "no corpus test reported anything (were they all skipped?)"}
        for r in corpus_reports:
            out.say("  corpus test %s: %d checked, %d failed" % (
                r.get("test"), r.get("checked", 0), len(r.get("failures", []))),
                "bad" if r.get("failures") else "ok")

    summary_path = out_dir / "summary.md"
    overall = write_summary(summary_path, info, stages, results, failures, crashes, corpus_reports,
                            features, log_path.name, contract_fails,
                            corpus_info["hashes"] if corpus_info else None)
    out.section("Summary")
    out.logline(summary_path.read_text(encoding="utf-8"))
    elapsed = time.time() - started
    out.say("")
    if overall:
        out.say("PASSED in %.0f s. Summary: %s" % (elapsed, summary_path), "ok")
    else:
        out.say("FAILED in %.0f s." % elapsed, "bad")
        out.say("Summary: %s" % summary_path)
        out.say("Log:     %s" % log_path)
        out.say("")
        out.say("Please report it: the 'Reporting a problem' steps at the end of summary.md say how")
        out.say("(open an issue at https://github.com/%s/issues/new, paste summary.md, attach the log)."
                % info["repository"])
    out.close()
    return 0 if overall else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\nStopped.", file=sys.stderr)
        sys.exit(130)
