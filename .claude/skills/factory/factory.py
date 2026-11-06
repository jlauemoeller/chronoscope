#!/usr/bin/env python3
"""The factory script: the mechanics of the /factory skill.

Run it from the repository root as

    python3 -I .claude/skills/factory/factory.py <command> ...

It finds the repository from the working directory (git rev-parse
--show-toplevel), never from its own location, so it can be tested against a
sample repository. It uses only the Python standard library and works out
everything from the files under factory/state/ and from git.

Every command exits 0 when it succeeds and prints its answer, even when the
answer is "no", "none" or an empty list. A non-zero exit always means an
error, with the reason on stderr.

Commands (see conventions.md for when to use each one):

  board build [--state S] [--message M]   create or rebuild board.json
  board loop S [--message M] [--current ID | --no-current]
  board agent NAME AGENT-ID               record an agent id
  board task ID                           recompute one task's entry
  board show [ID]                         print loop, agents (and one task)

  spec copy PATH                          copy a spec into factory/state/
  spec recopy                             copy it again from spec-source

  plan check                              mechanical plan check
  plan highest-serial                     highest serial used before this run
  plan archived-stack                     newest factory/archive/*/stack.md

  round list ID                           latest, previous and next round
  round previous ID ROUND                 the previous round of ROUND
  round kind ID ROUND                     auto | approver-rework | rework
  round rework-count ID ROUND             REWORK verdicts before ROUND
  round rework-note ID ROUND [--write P]  a verdict's rework note

  task reviewers ID                       the reviewers that run on a task
  task dependents ID                      tasks that depend on a task
  task merged ID                          yes | no
  task next-step ID                       where an In progress task resumes

  backlog next                            first Planned task, or none
  backlog set ID STATUS                   change a task's status

  first-lines [ID]                        check every fixed first line
  summary                                 what /factory status prints
"""

import argparse
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.parse
from collections import deque

ROSTER = ["code", "feature", "security", "privacy", "api", "ui"]
STATUSES = ["Planned", "In progress", "Escalated", "Complete", "Abandoned"]
LOOP_STATES = [
    "planning",
    "awaiting-preflight",
    "awaiting-plan-review",
    "running",
    "escalated",
    "halted",
    "complete",
]
SECTIONS = ["Goal", "Spec excerpts", "Acceptance criteria", "UI", "Reviews", "Depends on"]

FIRST_LINES = {
    "preflight": re.compile(r"PREFLIGHT: (READY|TOOLS|BLOCKED)"),
    "build": re.compile(r"STATUS: (DONE|BLOCKED)"),
    "review": re.compile(r"VERDICT: (PASS|FAIL)"),
    "verdict": re.compile(r"VERDICT: (APPROVE|REWORK|ESCALATE)"),
}
TASK_ID = re.compile(r"(\d{3,})-([a-z0-9]+(?:-[a-z0-9]+){1,2})")
TASK_TITLE = re.compile(r"#[ \t]+`?([^`\s:]+)`?[ \t]*:[ \t]*(.*)")
ROUND_ID = re.compile(r"round-(\d+)")
MERGE_SUBJECT = re.compile(r"factory: merge `?(\d{3,}-[a-z0-9-]+)`?")

ROOT = None  # repository root, set in main()


class FactoryError(Exception):
    pass


def fail(message):
    raise FactoryError(message)


# --------------------------------------------------------------------------
# Paths, files and git


def state_dir():
    return os.path.join(ROOT, "factory", "state")


def state_path(*parts):
    return os.path.join(state_dir(), *parts)


def task_dir(task_id):
    return state_path("tasks", task_id)


def rel(path):
    return os.path.relpath(path, ROOT)


def read_text(path):
    """Read a file as text: BOM removed, every line ending turned into \\n."""
    with open(path, "rb") as f:
        data = f.read()
    text = data.decode("utf-8", errors="replace")
    if text.startswith("﻿"):
        text = text[1:]
    return text.replace("\r\n", "\n").replace("\r", "\n")


def read_lines(path):
    """Lines without trailing whitespace."""
    return [line.rstrip() for line in read_text(path).split("\n")]


def first_line(path):
    return read_text(path).split("\n", 1)[0].rstrip()


def write_atomic(path, text):
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    os.replace(tmp, path)


def git(*args, check=True):
    proc = subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, errors="replace"
    )
    if check and proc.returncode != 0:
        fail("git %s failed: %s" % (" ".join(args), proc.stderr.strip() or proc.returncode))
    return proc


def find_root():
    proc = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True
    )
    if proc.returncode != 0:
        fail("not inside a git repository: " + proc.stderr.strip())
    return proc.stdout.strip()


_main_subjects = None


def main_subjects():
    """Commit subjects on main (the trailing -- stops git reading main as a path)."""
    global _main_subjects
    if _main_subjects is None:
        out = git("log", "main", "--format=%s", "--").stdout
        _main_subjects = [s.strip() for s in out.splitlines()]
    return _main_subjects


def branch_exists(task_id):
    ref = "refs/heads/factory/" + task_id
    return git("rev-parse", "--verify", "--quiet", ref, check=False).returncode == 0


def is_merged(task_id):
    """Merged: main has the subject 'factory: merge <id>', or the branch exists,
    is an ancestor of main, and has at least one builder commit."""
    if ("factory: merge " + task_id) in main_subjects():
        return True
    if not branch_exists(task_id):
        return False
    ref = "refs/heads/factory/" + task_id
    # Plain git check: the exit status is the answer (0 yes, 1 no).
    proc = git("merge-base", "--is-ancestor", ref, "main", check=False)
    if proc.returncode == 1:
        return False
    if proc.returncode != 0:
        fail("git merge-base --is-ancestor failed: " + proc.stderr.strip())
    subjects = git("log", ref, "--format=%s", "--").stdout.splitlines()
    return any(s.startswith(task_id + ": ") for s in subjects)


def current_branch():
    return git("branch", "--show-current").stdout.strip()


def tree_changes():
    """Uncommitted changes and untracked files git doesn't ignore."""
    out = git("status", "--porcelain", "--untracked-files=all").stdout
    return [line for line in out.splitlines() if line.strip()]


def state_ignored():
    # Plain git check: the exit status is the answer (0 ignored, 1 not).
    proc = git("check-ignore", "-q", "factory/state", check=False)
    if proc.returncode not in (0, 1):
        fail("git check-ignore failed: " + proc.stderr.strip())
    return proc.returncode == 0


# --------------------------------------------------------------------------
# Markdown helpers

FENCE_OPEN = re.compile(r"^[ \t]*(`{3,}|~{3,})(.*)$")
FENCE_CLOSE = re.compile(r"^[ \t]*(`{3,}|~{3,})[ \t]*$")
HEADING = re.compile(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$")
BULLET = re.compile(r"^[-*+][ \t]+(.*)$")
BLANK_LINE = re.compile(r"\n[ \t]*\n")


def opens_fence(line):
    m = FENCE_OPEN.match(line)
    if m and not (m.group(1)[0] == "`" and "`" in m.group(2)):
        return m.group(1)
    return None


def closes_fence(line, fence):
    m = FENCE_CLOSE.match(line)
    return bool(m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence))


def walk_lines(lines):
    """Yield (index, line, in_code) where in_code marks fenced code lines.
    Fences count at any indentation, so a fence nested in a list item counts."""
    fence = None
    for i, line in enumerate(lines):
        if fence is not None:
            if closes_fence(line, fence):
                fence = None
            yield i, line, True
            continue
        fence = opens_fence(line)
        yield i, line, fence is not None


def heading_of(line):
    m = HEADING.match(line)
    if not m:
        return None
    return len(m.group(1)), (m.group(2) or "").strip()


def leading_token(text):
    """The leading key or task-id of a bullet, without backticks or emphasis."""
    m = re.match(r"^[`*_\[\]]*([A-Za-z0-9][A-Za-z0-9_.-]*)", text.strip())
    if not m:
        return None
    return m.group(1).rstrip(".")


def md_sections(lines, level=2):
    """Map heading text -> body lines, for headings of `level` outside code."""
    out = {}
    name = None
    for _, line, code in walk_lines(lines):
        h = None if code else heading_of(line)
        if h and h[0] <= level:
            name = h[1].strip().strip("*").rstrip(":").strip() if h[0] == level else None
            if name is not None:
                out.setdefault(name, [])
            continue
        if name is not None:
            out[name].append(line)
    return out


# --------------------------------------------------------------------------
# task.md


class Task:
    def __init__(self, task_id):
        self.id = task_id
        self.path = os.path.join(task_dir(task_id), "task.md")
        self.exists = os.path.isfile(self.path)
        self.first = ""
        self.file_id = None
        self.title = None
        self.sections = []  # fixed section names in the order found
        self.ui_none = False
        self.review_bullets = []  # leading tokens of top-level bullets
        self.depends = []  # leading tokens of top-level bullets
        self.has_reviews = False
        if self.exists:
            self._parse()

    def _parse(self):
        lines = read_lines(self.path)
        self.first = lines[0] if lines else ""
        m = TASK_TITLE.fullmatch(self.first)
        if m:
            self.file_id, self.title = m.group(1), m.group(2).strip()
        current = None
        for i, line, code in walk_lines(lines):
            if i == 0:
                continue
            h = None if code else heading_of(line)
            if h and h[0] == 2 and (h[1] in SECTIONS or h[1] == "UI: none"):
                current = "UI" if h[1] == "UI: none" else h[1]
                if h[1] == "UI: none":
                    self.ui_none = True
                self.sections.append(current)
                continue
            if code or current not in ("Reviews", "Depends on"):
                continue
            b = BULLET.match(line)
            if not b:
                continue
            token = leading_token(b.group(1))
            if current == "Reviews":
                self.review_bullets.append(token.lower() if token else "")
            else:
                self.depends.append(token or "")
        self.has_reviews = "Reviews" in self.sections

    def dependencies(self):
        return [d for d in self.depends if d.lower() != "none"]

    def reviewers(self):
        if self.exists and self.has_reviews:
            keys = set(self.review_bullets)
        elif self.exists and self.ui_none:
            keys = set()
        else:
            keys = {"ui"}
        keys |= {"code", "feature"}
        return [k for k in ROSTER if k in keys]


def task_dirs():
    root = state_path("tasks")
    if not os.path.isdir(root):
        return []
    return sorted(d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)))


# --------------------------------------------------------------------------
# backlog.md


class Backlog:
    def __init__(self, path):
        self.path = path
        self.rows = []  # dicts: id, status, line, raw_id
        self.problems = []
        lines = read_lines(path)
        blocks = []
        block = []
        for i, line, code in walk_lines(lines):
            if not code and line.strip().startswith("|"):
                block.append((i, line))
            else:
                if block:
                    blocks.append(block)
                block = []
        if block:
            blocks.append(block)
        for block in blocks:
            seps = [k for k, (_, line) in enumerate(block) if self._is_separator(line)]
            start = seps[0] + 1 if seps else 0
            for i, line in block[start:]:
                if self._is_separator(line):
                    continue
                cells = self._cells(line)
                if not any(cells):
                    continue
                if len(cells) < 2:
                    self.problems.append("backlog.md line %d: a row needs two cells" % (i + 1))
                    continue
                raw_id = cells[0]
                task_id = raw_id.strip().strip("`*_ ").strip()
                status = cells[1].strip().strip("`*_ ").strip()
                if status not in STATUSES:
                    self.problems.append(
                        "backlog.md line %d: unknown status %r for %s" % (i + 1, status, task_id)
                    )
                self.rows.append({"id": task_id, "status": status, "line": i, "raw_id": raw_id})

    @staticmethod
    def _cells(line):
        s = line.strip()
        if s.startswith("|"):
            s = s[1:]
        if s.endswith("|") and not s.endswith("\\|"):
            s = s[:-1]
        return [c.strip() for c in re.split(r"(?<!\\)\|", s)]

    @classmethod
    def _is_separator(cls, line):
        cells = cls._cells(line)
        return bool(cells) and all(re.fullmatch(r":?-+:?", c.replace(" ", "")) for c in cells)

    def ids(self):
        return [r["id"] for r in self.rows]

    def status_of(self, task_id):
        for r in self.rows:
            if r["id"] == task_id:
                return r["status"]
        return None


def read_backlog(required=False):
    path = state_path("backlog.md")
    if not os.path.isfile(path):
        if required:
            fail("factory/state/backlog.md does not exist")
        return None
    return Backlog(path)


def valid_backlog():
    backlog = read_backlog(required=True)
    if backlog.problems:
        fail("; ".join(backlog.problems))
    return backlog


def set_status(task_id, status):
    if status not in STATUSES:
        fail("unknown status %r (one of: %s)" % (status, ", ".join(STATUSES)))
    backlog = valid_backlog()
    matches = [r for r in backlog.rows if r["id"] == task_id]
    if len(matches) != 1:
        fail("%s appears %d times in backlog.md" % (task_id, len(matches)))
    row = matches[0]
    with open(backlog.path, "rb") as f:
        raw = f.read().decode("utf-8", errors="replace")
    bom = raw.startswith("﻿")
    if bom:
        raw = raw[1:]
    newline = "\r\n" if "\r\n" in raw else "\n"
    lines = raw.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    lines[row["line"]] = "| %s | %s |" % (row["raw_id"], status)
    text = ("﻿" if bom else "") + newline.join(lines)
    tmp = backlog.path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="") as f:
        f.write(text)
    os.replace(tmp, backlog.path)
    return row["status"]


# --------------------------------------------------------------------------
# Rounds


def round_num(round_id):
    m = ROUND_ID.fullmatch(round_id or "")
    if not m:
        fail("not a round-id: %r" % round_id)
    return int(m.group(1))


def round_dirs(task_id):
    t = task_dir(task_id)
    if not os.path.isdir(t):
        return []
    found = []
    for name in os.listdir(t):
        m = ROUND_ID.fullmatch(name)
        if m and os.path.isdir(os.path.join(t, name)):
            found.append((int(m.group(1)), name))
    return [name for _, name in sorted(found)]


def rpath(task_id, round_id, *parts):
    return os.path.join(task_dir(task_id), round_id, *parts)


def has_base(task_id, round_id):
    return os.path.isfile(rpath(task_id, round_id, "base-commit"))


def previous_round(task_id, round_id):
    n = round_num(round_id)
    earlier = [r for r in round_dirs(task_id) if round_num(r) < n and has_base(task_id, r)]
    return earlier[-1] if earlier else None


def next_round_id(task_id):
    rounds = round_dirs(task_id)
    return "round-%d" % (round_num(rounds[-1]) + 1) if rounds else "round-1"


def file_result(path, kind):
    """The value on a fixed first line, None if the file is missing,
    'invalid' if the first line isn't an allowed one."""
    if not os.path.isfile(path):
        return None
    m = FIRST_LINES[kind].fullmatch(first_line(path))
    return m.group(1) if m else "invalid"


def build_result(task_id, round_id):
    return file_result(rpath(task_id, round_id, "build-notes.md"), "build")


def review_result(task_id, round_id, key):
    return file_result(rpath(task_id, round_id, key + "-review.md"), "review")


def verdict_result(task_id, round_id):
    return file_result(rpath(task_id, round_id, "verdict.md"), "verdict")


def round_kind(task_id, round_id):
    if os.path.isfile(rpath(task_id, round_id, "human-review.md")):
        return "rework"
    if os.path.isfile(rpath(task_id, round_id, "approver-review.md")):
        return "approver-rework"
    if not has_base(task_id, round_id):
        # Interrupted while it was being created: an Approver rework round
        # follows a REWORK verdict and gets its note copied again.
        prev = previous_round(task_id, round_id)
        if prev and verdict_result(task_id, prev) == "REWORK":
            return "approver-rework"
    return "auto"


def rework_count(task_id, round_id):
    n = round_num(round_id)
    return sum(
        1
        for r in round_dirs(task_id)
        if round_num(r) < n and verdict_result(task_id, r) == "REWORK"
    )


def rework_note(task_id, round_id):
    """(has_text, section_text) for the verdict's '## Rework note' section."""
    path = rpath(task_id, round_id, "verdict.md")
    if not os.path.isfile(path):
        fail("%s does not exist" % rel(path))
    lines = read_lines(path)
    start = end = None
    for i, line, code in walk_lines(lines):
        h = None if code else heading_of(line)
        if start is None:
            if h and h[0] == 2 and h[1].strip("*").rstrip(":").strip().lower() == "rework note":
                start = i
        elif h and h[0] <= 2:
            end = i
            break
    if start is None:
        return False, ""
    section = lines[start:end]
    while section and not section[-1].strip():
        section.pop()
    has_text = any(line.strip() for line in section[1:])
    return has_text, "\n".join(section) + "\n"


# --------------------------------------------------------------------------
# Next step of an In progress task (the task orchestrator's entry table)


def next_step(task_id):
    """Where an In progress task resumes, worked out from its files."""
    if is_merged(task_id):
        return {"step": "merged"}
    task = Task(task_id)
    if not task.exists:
        fail("factory/state/tasks/%s/task.md does not exist" % task_id)
    rounds = round_dirs(task_id)
    if not rounds:
        return {"step": "new-round", "round": "round-1", "kind": "auto"}
    latest = rounds[-1]
    if not has_base(task_id, latest):
        return {"step": "complete-round", "round": latest, "kind": round_kind(task_id, latest)}
    build = build_result(task_id, latest)
    if build is None:
        return {"step": "build", "round": latest}
    if build == "invalid":
        fail("%s/build-notes.md has an invalid first line" % rel(rpath(task_id, latest)))
    if build == "BLOCKED":
        return {"step": "escalate", "round": latest, "reason": "blocked"}
    keys = task.reviewers()
    results = {k: review_result(task_id, latest, k) for k in keys}
    bad = [k for k, v in results.items() if v == "invalid"]
    if bad:
        fail("invalid first line in %s" % ", ".join("%s-review.md" % k for k in bad))
    started = os.path.isfile(rpath(task_id, latest, "reviews-started"))
    if not started and all(v is None for v in results.values()):
        return {"step": "verify-build", "round": latest}
    missing = [k for k, v in results.items() if v is None]
    if missing:
        return {"step": "review", "round": latest, "missing": " ".join(missing)}
    verdict = verdict_result(task_id, latest)
    if verdict == "invalid":
        fail("%s/verdict.md has an invalid first line" % rel(rpath(task_id, latest)))
    if verdict is None:
        failed = [k for k, v in results.items() if v == "FAIL"]
        if latest == "round-1" and round_kind(task_id, latest) == "auto" and failed:
            return {"step": "fix-round", "round": next_round_id(task_id), "after": latest}
        return {"step": "approve", "round": latest}
    if verdict == "APPROVE":
        return {"step": "merge", "round": latest}
    if verdict == "ESCALATE":
        return {"step": "escalate", "round": latest, "reason": "approver"}
    # REWORK
    if rework_count(task_id, latest) >= 2:
        return {"step": "escalate", "round": latest, "reason": "rework-limit"}
    if not rework_note(task_id, latest)[0]:
        return {"step": "escalate", "round": latest, "reason": "no-rework-note"}
    return {"step": "approver-rework", "round": next_round_id(task_id), "after": latest}


# --------------------------------------------------------------------------
# board.json


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def task_files(task_id, keys):
    t = task_dir(task_id)
    files = []
    if os.path.isfile(os.path.join(t, "task.md")):
        files.append("task.md")
    order = ["human-review.md", "approver-review.md", "build-notes.md"]
    order += ["%s-review.md" % k for k in ROSTER] + ["verdict.md"]
    for r in round_dirs(task_id):
        rdir = os.path.join(t, r)
        names = [n for n in os.listdir(rdir) if os.path.isfile(os.path.join(rdir, n))]
        mds = [n for n in names if n.lower().endswith(".md")]
        mds.sort(key=lambda n: (order.index(n) if n in order else len(order), n))
        files += ["%s/%s" % (r, n) for n in mds]
        for k in ROSTER:
            edir = os.path.join(rdir, k)
            if not os.path.isdir(edir):
                continue
            found = []
            for base, dirs, fnames in os.walk(edir):
                dirs.sort()
                for n in fnames:
                    found.append(os.path.relpath(os.path.join(base, n), t).replace(os.sep, "/"))
            files += sorted(found)
    return files


def round_entry(task_id, round_id, keys):
    build = build_result(task_id, round_id)
    entry = {
        "id": round_id,
        "kind": round_kind(task_id, round_id),
        "build": build if build in ("DONE", "BLOCKED") else "pending",
        "reviews": {},
        "verdict": None,
    }
    started = os.path.isfile(rpath(task_id, round_id, "reviews-started"))
    results = {k: review_result(task_id, round_id, k) for k in keys}
    if started or any(v is not None for v in results.values()):
        entry["reviews"] = {
            k: (v if v in ("PASS", "FAIL") else "pending") for k, v in results.items()
        }
    verdict = verdict_result(task_id, round_id)
    if verdict in ("APPROVE", "REWORK", "ESCALATE"):
        entry["verdict"] = verdict
    return entry


def task_stage(task_id, status, rounds, keys):
    if status == "Planned":
        return "planned"
    if status == "Complete":
        return "done"
    if status == "Abandoned":
        return "abandoned"
    if status == "Escalated":
        return "escalated"
    if is_merged(task_id):
        return "done"
    if not rounds:
        return "building"
    latest = rounds[-1]
    if latest["build"] != "DONE":
        return "building"
    reviews = latest["reviews"]
    if len(reviews) < len(keys) or "pending" in reviews.values():
        return "review"
    if latest["verdict"] is None:
        if latest["id"] == "round-1" and latest["kind"] == "auto" and "FAIL" in reviews.values():
            return "building"
        return "approval"
    if latest["verdict"] == "REWORK":
        return "building"
    return "approval"


def task_entry(task_id, status):
    task = Task(task_id)
    keys = task.reviewers()
    rounds = [round_entry(task_id, r, keys) for r in round_dirs(task_id)]
    return {
        "id": task_id,
        "title": task.title or task_id,
        "branch": "factory/" + task_id,
        "status": status,
        "stage": task_stage(task_id, status, rounds, keys),
        "rounds": rounds,
        "files": task_files(task_id, keys),
    }


def board_path():
    return state_path("board.json")


def load_board():
    path = board_path()
    if not os.path.isfile(path):
        return None
    try:
        with open(path, encoding="utf-8-sig") as f:
            board = json.load(f)
    except (OSError, ValueError) as e:
        fail("factory/state/board.json is not valid JSON (%s); run 'board build'" % e)
    if not isinstance(board, dict):
        fail("factory/state/board.json is not a JSON object; run 'board build'")
    return board


def save_board(board):
    board["updated"] = now()
    os.makedirs(state_dir(), exist_ok=True)
    write_atomic(board_path(), json.dumps(board, indent=2, ensure_ascii=False) + "\n")


def derived_loop(backlog):
    """The loop a rebuild derives from the files (never awaiting-plan-review)."""
    if backlog is None:
        if os.path.isfile(state_path("preflight.md")):
            return "awaiting-preflight", "The preflight stopped: see preflight.md", None
        return "planning", "", None
    active = [r for r in backlog.rows if r["status"] in ("In progress", "Escalated")]
    current = active[0]["id"] if active else None
    if any(r["status"] == "Escalated" for r in backlog.rows):
        esc = [r["id"] for r in backlog.rows if r["status"] == "Escalated"][0]
        return "escalated", "%s is escalated" % esc, current
    if backlog.rows and all(r["status"] in ("Complete", "Abandoned") for r in backlog.rows):
        return "complete", "Construction is complete", None
    return "halted", "Board rebuilt from the files; /factory continue resumes", current


def build_board(state=None, message=None):
    old = None
    try:
        old = load_board()
    except FactoryError:
        old = None
    backlog = read_backlog()
    if backlog is not None and backlog.problems:
        fail("; ".join(backlog.problems))
    d_state, d_message, current = derived_loop(backlog)
    if state is not None:
        d_state, d_message = state, ""
    if message is not None:
        d_message = message
    tasks = [task_entry(r["id"], r["status"]) for r in backlog.rows] if backlog else []
    board = {
        "updated": now(),
        "loop": {"state": d_state, "message": d_message, "current": current},
        "agents": (old or {}).get("agents") or {},
        "tasks": tasks,
    }
    save_board(board)
    return board


def board_or_build():
    board = load_board()
    if board is None:
        board = build_board()
    board.setdefault("loop", {"state": "halted", "message": "", "current": None})
    board.setdefault("agents", {})
    board.setdefault("tasks", [])
    return board


def update_task_entry(board, task_id):
    backlog = valid_backlog()
    status = backlog.status_of(task_id)
    if status is None:
        fail("%s is not in backlog.md" % task_id)
    entry = task_entry(task_id, status)
    tasks = [t for t in board["tasks"] if t.get("id") != task_id]
    order = backlog.ids()
    pos = order.index(task_id)
    insert_at = len(tasks)
    for i, t in enumerate(tasks):
        if t.get("id") in order and order.index(t.get("id")) > pos:
            insert_at = i
            break
    tasks.insert(insert_at, entry)
    board["tasks"] = tasks
    return entry


# --------------------------------------------------------------------------
# Spec copy

SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
LIST_ITEM = re.compile(r"^( *)([-*+]|\d{1,9}[.)])( +|$)")


def indent_width(line):
    width = 0
    for ch in line:
        if ch == " ":
            width += 1
        elif ch == "\t":
            width += 4 - width % 4
        else:
            break
    return width


def mask_code(text):
    """Replace code (fenced, indented and inline) with spaces, keeping offsets."""
    out = list(text)
    lines = text.split("\n")
    offsets = []
    pos = 0
    for line in lines:
        offsets.append(pos)
        pos += len(line) + 1

    def blank(i):
        for k in range(offsets[i], offsets[i] + len(lines[i])):
            out[k] = " "

    fence = None
    prev_blank = True
    prev_code = False
    list_indent = 0  # content column of the list item we are in, or 0
    for i, line in enumerate(lines):
        if fence is not None:
            if closes_fence(line, fence):
                fence = None
            blank(i)
            prev_blank, prev_code = False, False
            continue
        if not line.strip():
            prev_blank = True
            continue
        width = indent_width(line)
        item = LIST_ITEM.match(line)
        if list_indent and width < list_indent and prev_blank and not item:
            list_indent = 0  # a paragraph that isn't indented ends the list
        if (width - list_indent) >= 4 and (prev_blank or prev_code):
            blank(i)  # indented code block
            prev_blank, prev_code = False, True
            continue
        if (width - list_indent) <= 3:
            fence = opens_fence(line)
            if fence is not None:
                blank(i)
                prev_blank, prev_code = False, False
                continue
        if item:
            spaces = len(item.group(3))
            pad = 1 if spaces > 4 or not spaces else spaces
            list_indent = len(item.group(1)) + len(item.group(2)) + pad
        prev_blank, prev_code = False, False

    # Inline code spans: a run of N backticks up to the next run of exactly N,
    # within one paragraph. Masking a span never changes the text after it.
    masked = "".join(out)
    i = 0
    n = len(masked)
    while i < n:
        if masked[i] != "`":
            i += 1
            continue
        if i > 0 and masked[i - 1] == "\\":
            i += 1
            continue
        j = i
        while j < n and masked[j] == "`":
            j += 1
        run = j - i
        k = j
        close = None
        while k < n:
            c = masked[k]
            if c == "\n" and BLANK_LINE.match(masked, k):
                break
            if c == "`":
                m = k
                while m < n and masked[m] == "`":
                    m += 1
                if m - k == run:
                    close = m
                    break
                k = m
                continue
            k += 1
        if close is None:
            i = j
            continue
        for p in range(i, close):
            if out[p] != "\n":
                out[p] = " "
        i = close
    return "".join(out)


def find_link_destinations(masked):
    """(start, end) spans of link destinations in inline links, images and
    reference definitions, found in text whose code is masked."""
    spans = []
    n = len(masked)
    for i, ch in enumerate(masked):
        if ch != "[" or (i > 0 and masked[i - 1] == "\\"):
            continue
        depth = 0
        j = i
        close = None
        while j < n:
            c = masked[j]
            if c == "\\":
                j += 2
                continue
            if c == "\n" and BLANK_LINE.match(masked, j):
                break
            if c == "[":
                depth += 1
            elif c == "]":
                depth -= 1
                if depth == 0:
                    close = j
                    break
            j += 1
        if close is None or close + 1 >= n or masked[close + 1] != "(":
            continue
        k = close + 2
        while k < n and masked[k] in " \t":
            k += 1
        if k < n and masked[k] == "\n":
            k += 1
            while k < n and masked[k] in " \t":
                k += 1
        if k >= n:
            continue
        start = k
        if masked[k] == "<":
            end = masked.find(">", k)
            if end == -1 or "\n" in masked[k:end]:
                continue
            end += 1
        else:
            parens = 0
            end = k
            while end < n:
                c = masked[end]
                if c == "\\" and end + 1 < n:
                    end += 2
                    continue
                if c.isspace():
                    break
                if c == "(":
                    parens += 1
                elif c == ")":
                    if parens == 0:
                        break
                    parens -= 1
                end += 1
        if end > start:
            spans.append((start, end))
    for m in re.finditer(
        r"^ {0,3}\[(?:[^\]\\\n]|\\.)+\]:[ \t]*\n?[ \t]*(<[^>\n]*>|\S+)", masked, re.M
    ):
        spans.append((m.start(1), m.end(1)))
    return sorted(set(spans))


def split_target(raw):
    angle = raw.startswith("<") and raw.endswith(">")
    target = raw[1:-1] if angle else raw
    path, sep, anchor = target.partition("#")
    return angle, path, (sep + anchor)


def is_relative_md(path):
    if not path or SCHEME.match(path) or path.startswith("/") or path.startswith("~"):
        return False
    clean = urllib.parse.unquote(path.split("?", 1)[0])
    return clean.lower().endswith((".md", ".markdown"))


def copy_spec(source):
    source = os.path.abspath(os.path.expanduser(source))
    if not os.path.isfile(source):
        fail("the spec %s does not exist" % source)
    source = os.path.realpath(source)
    state_real = os.path.realpath(state_dir())
    if source == state_real or source.startswith(state_real + os.sep):
        fail("the spec %s is inside factory/state/, which the copy replaces" % source)
    os.makedirs(state_dir(), exist_ok=True)
    with open(state_path("spec-source"), "w", encoding="utf-8") as f:
        f.write(source + "\n")
    spec_out = state_path("SPEC.md")
    spec_dir = state_path("spec")
    if os.path.isfile(spec_out):
        os.remove(spec_out)
    if os.path.isdir(spec_dir):
        shutil.rmtree(spec_dir)

    names = {source: "SPEC.md"}  # source path -> copy path relative to the state dir
    used = {"SPEC.md"}
    queue = deque([source])
    missing = []
    copied = []
    while queue:
        src = queue.popleft()
        out_rel = names[src]
        out_dir = os.path.dirname(out_rel)
        with open(src, "rb") as f:
            raw = f.read().decode("utf-8", errors="replace")
        if raw.startswith("﻿"):
            raw = raw[1:]
        masked = mask_code(raw)
        pieces = []
        last = 0
        for start, end in find_link_destinations(masked):
            angle, path, anchor = split_target(raw[start:end])
            if not is_relative_md(path):
                continue
            plain = urllib.parse.unquote(path.split("?", 1)[0])
            target = os.path.realpath(os.path.join(os.path.dirname(src), plain))
            if target not in names:
                if not os.path.isfile(target):
                    if (out_rel, target) not in [(m[0], m[2]) for m in missing]:
                        missing.append((out_rel, raw[start:end], target))
                    continue
                base = os.path.basename(target)
                stem, ext = os.path.splitext(base)
                candidate = "spec/" + base
                k = 2
                while candidate in used:
                    candidate = "spec/%s-%d%s" % (stem, k, ext)
                    k += 1
                used.add(candidate)
                names[target] = candidate
                queue.append(target)
            new = os.path.relpath(names[target], out_dir or ".").replace(os.sep, "/")
            new = urllib.parse.quote(new, safe="/-_.~") + anchor
            if angle:
                new = "<" + new + ">"
            pieces.append(raw[last:start])
            pieces.append(new)
            last = end
        pieces.append(raw[last:])
        dest = state_path(*out_rel.split("/"))
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        with open(dest, "w", encoding="utf-8", newline="") as f:
            f.write("".join(pieces))
        copied.append((out_rel, src))
    return source, copied, missing


def print_spec_copy(result):
    source, copied, missing = result
    print("spec-source: %s" % source)
    for out_rel, src in copied:
        print("copied: %s <- %s" % (out_rel, src))
    if missing:
        for out_rel, link, target in missing:
            print("missing: %s (linked from %s as %s)" % (target, out_rel, link))
    else:
        print("missing: none")


# --------------------------------------------------------------------------
# The plan


def used_serials(exclude):
    """Serials the factory has used, from git: merge subjects on main and the
    local factory/<id> branches. Ids in `exclude` (this run) are left out."""
    serials = []
    for subject in main_subjects():
        m = MERGE_SUBJECT.fullmatch(subject)
        if m and m.group(1) not in exclude:
            serials.append(int(m.group(1).split("-", 1)[0]))
    out = git("for-each-ref", "--format=%(refname)", "refs/heads/factory/").stdout
    for ref in out.splitlines():
        name = ref.strip()[len("refs/heads/factory/"):]
        m = re.match(r"(\d{3,})-", name)
        if m and name not in exclude:
            serials.append(int(m.group(1)))
    return serials


def this_run_ids():
    ids = set(task_dirs())
    backlog = read_backlog()
    if backlog:
        ids |= set(backlog.ids())
    return ids


def highest_serial():
    serials = used_serials(this_run_ids())
    return max(serials) if serials else None


def serial_text(n):
    return "%03d" % n


def check_plan():
    problems = []
    warnings = []
    backlog = read_backlog()
    if backlog is None:
        return ["factory/state/backlog.md is missing"], warnings
    problems += backlog.problems
    ids = backlog.ids()
    if not ids:
        problems.append("backlog.md lists no tasks")
    for d in task_dirs():
        if d not in ids:
            problems.append("task directory tasks/%s/ is not in backlog.md" % d)
    for task_id in sorted(set(ids)):
        if ids.count(task_id) > 1:
            problems.append("%s appears %d times in backlog.md" % (task_id, ids.count(task_id)))
    highest = highest_serial()
    serials = {}
    for pos, row in enumerate(backlog.rows):
        task_id = row["id"]
        if row["status"] != "Planned":
            problems.append("%s has status %r, not Planned" % (task_id, row["status"]))
        m = TASK_ID.fullmatch(task_id)
        if not m:
            problems.append(
                "%s does not follow the identifier rules (serial, then 2-3 lowercase words)" % task_id
            )
        else:
            digits = m.group(1)
            serial = int(digits)
            if len(digits) > 3 and digits.startswith("0"):
                problems.append("%s: the serial is zero-padded beyond three digits" % task_id)
            if highest is not None and serial <= highest:
                problems.append(
                    "%s: serial %s is not above the highest serial already used (%s)"
                    % (task_id, digits, serial_text(highest))
                )
            if serial in serials and serials[serial] != task_id:
                problems.append("%s and %s share serial %s" % (serials[serial], task_id, digits))
            serials.setdefault(serial, task_id)
        task = Task(task_id)
        if not task.exists:
            problems.append("%s has no tasks/%s/task.md" % (task_id, task_id))
            continue
        if task.file_id is None:
            problems.append("%s: task.md's first line is not '# <task-id>: <title>'" % task_id)
        elif task.file_id != task_id or not task.title:
            problems.append(
                "%s: task.md's first line is %r, expected '# %s: <title>'" % (task_id, task.first, task_id)
            )
        for name in SECTIONS:
            if name not in task.sections:
                problems.append("%s: task.md has no '## %s' section" % (task_id, name))
        for name in set(task.sections):
            if task.sections.count(name) > 1:
                problems.append("%s: task.md has %d '## %s' sections" % (task_id, task.sections.count(name), name))
        if task.sections != [s for s in SECTIONS if s in task.sections]:
            problems.append(
                "%s: task.md's sections are out of order (%s)" % (task_id, ", ".join(task.sections))
            )
        if task.has_reviews:
            for key in ("code", "feature"):
                if key not in task.review_bullets:
                    problems.append("%s: the Reviews section does not list `%s`" % (task_id, key))
            for key in task.review_bullets:
                if key not in ROSTER:
                    problems.append("%s: the Reviews section lists %r, which is not a roster key" % (task_id, key))
            if ("ui" in task.review_bullets) == task.ui_none:
                warnings.append(
                    "%s: `ui` should be listed exactly when the UI section isn't 'UI: none'" % task_id
                )
        if "Depends on" in task.sections and not task.depends:
            problems.append("%s: the Depends on section needs a bullet per task-id, or '- none'" % task_id)
        for dep in task.dependencies():
            if dep == task_id:
                problems.append("%s depends on itself" % task_id)
            elif dep in ids[:pos]:
                continue
            elif dep in ids:
                problems.append("%s depends on %s, which comes later in backlog.md" % (task_id, dep))
            elif not is_merged(dep):
                problems.append(
                    "%s depends on %s, which is neither earlier in this backlog nor a merged task" % (task_id, dep)
                )
    return problems, warnings


def archived_stack():
    root = os.path.join(ROOT, "factory", "archive")
    if not os.path.isdir(root):
        return None
    found = []
    for name in os.listdir(root):
        path = os.path.join(root, name, "stack.md")
        if os.path.isfile(path):
            found.append((os.path.getmtime(path), path))
    if not found:
        return None
    return rel(max(found)[1])


# --------------------------------------------------------------------------
# First lines


def first_line_problems(task_id=None):
    problems = []

    def check(path, kind):
        if os.path.isfile(path) and not FIRST_LINES[kind].fullmatch(first_line(path)):
            problems.append((rel(path), first_line(path)))

    if task_id is None:
        check(state_path("preflight.md"), "preflight")
        ids = task_dirs()
    else:
        ids = [task_id]
    for tid in ids:
        task = Task(tid)
        if task.exists and (task.file_id != tid or not task.title):
            problems.append((rel(task.path), task.first))
        for r in round_dirs(tid):
            check(rpath(tid, r, "build-notes.md"), "build")
            for key in ROSTER:
                check(rpath(tid, r, key + "-review.md"), "review")
            check(rpath(tid, r, "verdict.md"), "verdict")
    return problems


# --------------------------------------------------------------------------
# Summary (for /factory status)


def describe_step(task_id, s):
    step = s["step"]
    if step == "merged":
        return "mark %s Complete and delete its branch, then start the next task" % task_id
    if step == "new-round":
        return "create round-1 and run the builder"
    if step == "complete-round":
        return "finish creating %s (%s), then run the builder" % (s["round"], s["kind"])
    if step == "build":
        return "run a fresh builder for %s" % s["round"]
    if step == "verify-build":
        return "check the builder's work in %s, then start the reviewers" % s["round"]
    if step == "review":
        return "run the missing reviewers in %s: %s" % (s["round"], s["missing"])
    if step == "fix-round":
        return "start the fix round %s" % s["round"]
    if step == "approve":
        return "run the Approver for %s" % s["round"]
    if step == "merge":
        return "merge factory/%s into main (APPROVE in %s)" % (task_id, s["round"])
    if step == "approver-rework":
        return "start the Approver rework round %s" % s["round"]
    if step == "escalate":
        return "escalate %s (%s in %s)" % (task_id, s["reason"], s["round"])
    return step


def continue_action(backlog, board):
    if backlog is None:
        if os.path.isfile(state_path("preflight.md")):
            return "re-copy the spec from spec-source and run the Planner again (preflight)"
        return "nothing to continue: no plan in factory/state/ (use /factory start <spec>)"
    if backlog.problems:
        return "halt: " + "; ".join(backlog.problems)
    rows = backlog.rows
    esc = [r["id"] for r in rows if r["status"] == "Escalated"]
    if esc:
        return "refuse: %s is Escalated; use /factory approve, rework or abandon" % esc[0]
    prefix = "" if board is not None else "rebuild board.json, then "
    loop_state = (board or {}).get("loop", {}).get("state")
    untouched = rows and all(r["status"] == "Planned" for r in rows) and not any(
        round_dirs(r["id"]) for r in rows
    )
    if untouched and loop_state != "awaiting-plan-review":
        return prefix + "check the plan and halt for plan review"
    if loop_state == "awaiting-plan-review":
        prefix += "check the plan again and rebuild board.json, then "
    nxt = [r["id"] for r in rows if r["status"] == "Planned"]
    after = ("start %s" % nxt[0]) if nxt else "halt: construction is complete"
    active = [r["id"] for r in rows if r["status"] == "In progress"]
    if active:
        task_id = active[0]
        if is_merged(task_id):
            return prefix + "mark %s Complete, delete factory/%s, then %s" % (task_id, task_id, after)
        return prefix + "resume %s: %s" % (task_id, describe_step(task_id, next_step(task_id)))
    return prefix + after


def summary():
    backlog = read_backlog()
    try:
        board = load_board()
        board_note = None
    except FactoryError as e:
        board, board_note = None, str(e)
    if board is not None:
        loop = board.get("loop", {})
        print("loop: %s%s" % (loop.get("state"), (" - " + loop["message"]) if loop.get("message") else ""))
    else:
        print("loop: board.json %s" % ("is invalid" if board_note else "is missing"))
    print("branch: %s" % (current_branch() or "(detached)"))
    changes = tree_changes()
    print("tree: %s" % ("clean" if not changes else "%d uncommitted or untracked paths" % len(changes)))
    print("factory/state ignored by git: %s" % ("yes" if state_ignored() else "no"))
    src = state_path("spec-source")
    if os.path.isfile(src):
        print("spec-source: %s" % read_text(src).strip())
    if backlog is None:
        pre = state_path("preflight.md")
        if os.path.isfile(pre):
            lines = read_lines(pre)
            print("preflight: %s" % (lines[0] if lines else ""))
            sections = md_sections(lines)
            for name in ("Spec issues", "Environment issues", "Tools on offer"):
                body = sections.get(name)
                print("")
                print("%s:" % name)
                text = "\n".join(body).strip() if body else ""
                print(text or "(none)")
        else:
            print("plan: none (no backlog.md or preflight.md)")
    else:
        print("")
        print("backlog:")
        for r in backlog.rows:
            print("  %s  %s" % (r["id"], r["status"]))
        for problem in backlog.problems:
            print("  problem: %s" % problem)
        active = [r for r in backlog.rows if r["status"] in ("In progress", "Escalated")]
        if active and not backlog.problems:
            r = active[0]
            entry = task_entry(r["id"], r["status"])
            print("")
            print("current: %s (%s), stage %s" % (r["id"], r["status"], entry["stage"]))
            if entry["rounds"]:
                latest = entry["rounds"][-1]
                print("round: %s (%s), build %s, reviews %s, verdict %s" % (
                    latest["id"], latest["kind"], latest["build"],
                    " ".join("%s %s" % kv for kv in latest["reviews"].items()) or "none",
                    latest["verdict"] or "none"))
        else:
            print("")
            print("current: none")
    print("")
    print("continue would: %s" % continue_action(backlog, board))


# --------------------------------------------------------------------------
# Command line


def cmd_board(args):
    if args.action == "build":
        if args.state is not None and args.state not in LOOP_STATES:
            fail("unknown loop state %r" % args.state)
        board = build_board(args.state, args.message)
        loop = board["loop"]
        print("board.json built: %d tasks, loop.state %s" % (len(board["tasks"]), loop["state"]))
    elif args.action == "loop":
        if args.state not in LOOP_STATES:
            fail("unknown loop state %r" % args.state)
        board = board_or_build()
        loop = board["loop"]
        loop["state"] = args.state
        loop["message"] = args.message or ""
        if args.no_current:
            loop["current"] = None
        elif args.current is not None:
            loop["current"] = args.current
        loop.setdefault("current", None)
        save_board(board)
        print("loop: %s" % json.dumps(loop, ensure_ascii=False))
    elif args.action == "agent":
        board = board_or_build()
        board["agents"][args.name] = args.agent_id
        save_board(board)
        print("agents: %s" % json.dumps(board["agents"], ensure_ascii=False))
    elif args.action == "task":
        board = board_or_build()
        entry = update_task_entry(board, args.id)
        save_board(board)
        latest = entry["rounds"][-1] if entry["rounds"] else None
        print("%s: status %s, stage %s, latest round %s" % (
            entry["id"], entry["status"], entry["stage"],
            json.dumps(latest, ensure_ascii=False) if latest else "none"))
    elif args.action == "show":
        board = load_board()
        if board is None:
            fail("factory/state/board.json does not exist; run 'board build'")
        print("loop: %s" % json.dumps(board.get("loop"), ensure_ascii=False))
        print("agents: %s" % json.dumps(board.get("agents", {}), ensure_ascii=False))
        if args.id:
            entries = [t for t in board.get("tasks", []) if t.get("id") == args.id]
            if not entries:
                print("task %s: not on the board" % args.id)
            else:
                t = entries[0]
                latest = t["rounds"][-1] if t.get("rounds") else None
                print("task %s: status %s, stage %s" % (t["id"], t.get("status"), t.get("stage")))
                print("latest round: %s" % (json.dumps(latest, ensure_ascii=False) if latest else "none"))


def cmd_spec(args):
    if args.action == "copy":
        print_spec_copy(copy_spec(args.path))
    else:
        src = state_path("spec-source")
        if not os.path.isfile(src):
            fail("factory/state/spec-source does not exist")
        print_spec_copy(copy_spec(read_text(src).strip()))


def cmd_plan(args):
    if args.action == "check":
        problems, warnings = check_plan()
        print("PLAN: %s" % ("OK" if not problems else "PROBLEMS"))
        for p in problems:
            print("- %s" % p)
        for w in warnings:
            print("- warning: %s" % w)
    elif args.action == "highest-serial":
        h = highest_serial()
        print("highest: %s" % (serial_text(h) if h is not None else "none"))
        print("next: %s" % serial_text((h or 0) + 1))
    elif args.action == "archived-stack":
        print(archived_stack() or "none")


def cmd_round(args):
    if args.action == "list":
        rounds = round_dirs(args.id)
        latest = rounds[-1] if rounds else None
        print("latest: %s" % (latest or "none"))
        if latest:
            print("latest-kind: %s" % round_kind(args.id, latest))
            print("latest-base-commit: %s" % ("yes" if has_base(args.id, latest) else "no"))
            print("latest-build-notes: %s" % (
                "yes" if os.path.isfile(rpath(args.id, latest, "build-notes.md")) else "no"))
            print("latest-reviews-started: %s" % (
                "yes" if os.path.isfile(rpath(args.id, latest, "reviews-started")) else "no"))
            prev = previous_round(args.id, latest)
            print("previous: %s" % (prev or "none"))
        else:
            print("previous: none")
        print("next: %s" % next_round_id(args.id))
    elif args.action == "previous":
        round_num(args.round)
        print(previous_round(args.id, args.round) or "none")
    elif args.action == "kind":
        round_num(args.round)
        print(round_kind(args.id, args.round))
    elif args.action == "rework-count":
        round_num(args.round)
        print(rework_count(args.id, args.round))
    elif args.action == "rework-note":
        has_text, text = rework_note(args.id, args.round)
        print("rework-note: %s" % ("yes" if has_text else "no"))
        if has_text:
            if args.write:
                path = os.path.join(ROOT, args.write) if not os.path.isabs(args.write) else args.write
                with open(path, "w", encoding="utf-8") as f:
                    f.write(text)
                print("written: %s" % rel(path))
            else:
                sys.stdout.write(text)


def cmd_task(args):
    if args.action == "reviewers":
        task = Task(args.id)
        if not task.exists:
            fail("factory/state/tasks/%s/task.md does not exist" % args.id)
        print(" ".join(task.reviewers()))
    elif args.action == "dependents":
        backlog = valid_backlog()
        found = []
        for r in backlog.rows:
            if args.id in Task(r["id"]).dependencies():
                found.append("%s %s" % (r["id"], r["status"]))
        print("\n".join(found) if found else "none")
    elif args.action == "merged":
        print("yes" if is_merged(args.id) else "no")
    elif args.action == "next-step":
        for key, value in next_step(args.id).items():
            print("%s: %s" % (key, value))


def cmd_backlog(args):
    if args.action == "next":
        backlog = valid_backlog()
        nxt = [r["id"] for r in backlog.rows if r["status"] == "Planned"]
        print(nxt[0] if nxt else "none")
    elif args.action == "set":
        old = set_status(args.id, args.status)
        if os.path.isfile(board_path()):
            board = board_or_build()
            update_task_entry(board, args.id)
            save_board(board)
        print("%s: %s -> %s" % (args.id, old, args.status))


def cmd_first_lines(args):
    problems = first_line_problems(args.id)
    print("FIRST-LINES: %s" % ("INVALID" if problems else "OK"))
    for path, line in problems:
        print("- %s: %s" % (path, json.dumps(line, ensure_ascii=False)))


def parser():
    p = argparse.ArgumentParser(prog="factory.py", description="The factory script.")
    sub = p.add_subparsers(dest="command", required=True)

    board = sub.add_parser("board").add_subparsers(dest="action", required=True)
    b = board.add_parser("build")
    b.add_argument("--state")
    b.add_argument("--message")
    b = board.add_parser("loop")
    b.add_argument("state")
    b.add_argument("--message")
    g = b.add_mutually_exclusive_group()
    g.add_argument("--current")
    g.add_argument("--no-current", action="store_true")
    b = board.add_parser("agent")
    b.add_argument("name")
    b.add_argument("agent_id")
    board.add_parser("task").add_argument("id")
    board.add_parser("show").add_argument("id", nargs="?")

    spec = sub.add_parser("spec").add_subparsers(dest="action", required=True)
    spec.add_parser("copy").add_argument("path")
    spec.add_parser("recopy")

    plan = sub.add_parser("plan").add_subparsers(dest="action", required=True)
    plan.add_parser("check")
    plan.add_parser("highest-serial")
    plan.add_parser("archived-stack")

    rnd = sub.add_parser("round").add_subparsers(dest="action", required=True)
    rnd.add_parser("list").add_argument("id")
    for name in ("previous", "kind", "rework-count"):
        r = rnd.add_parser(name)
        r.add_argument("id")
        r.add_argument("round")
    r = rnd.add_parser("rework-note")
    r.add_argument("id")
    r.add_argument("round")
    r.add_argument("--write")

    task = sub.add_parser("task").add_subparsers(dest="action", required=True)
    for name in ("reviewers", "dependents", "merged", "next-step"):
        task.add_parser(name).add_argument("id")

    backlog = sub.add_parser("backlog").add_subparsers(dest="action", required=True)
    backlog.add_parser("next")
    s = backlog.add_parser("set")
    s.add_argument("id")
    s.add_argument("status")

    sub.add_parser("first-lines").add_argument("id", nargs="?")
    sub.add_parser("summary")
    return p


def main(argv=None):
    global ROOT
    args = parser().parse_args(argv)
    try:
        ROOT = find_root()
        {
            "board": cmd_board,
            "spec": cmd_spec,
            "plan": cmd_plan,
            "round": cmd_round,
            "task": cmd_task,
            "backlog": cmd_backlog,
            "first-lines": cmd_first_lines,
            "summary": lambda a: summary(),
        }[args.command](args)
    except FactoryError as e:
        print("factory.py: error: %s" % e, file=sys.stderr)
        return 1
    except OSError as e:
        print("factory.py: error: %s" % e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
