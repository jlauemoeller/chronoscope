#!/usr/bin/env python3
"""The factory script: the mechanics of the /factory software factory.

Run it from the repository root as

    python3 -I .claude/skills/factory/factory.py <command> ...

It finds the repository from the working directory (git rev-parse
--show-toplevel), never from its own location, and works out everything it
reports or writes from the files in factory/state/ and from git. It uses only
the Python standard library.

Every command exits 0 when it succeeds and prints its answer, even when the
answer is "no", "none" or an empty list. A non-zero exit always means an
error, with the reason on stderr.
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

ROSTER = ["code", "feature", "security", "privacy", "api", "ui"]
ALWAYS_REVIEWERS = ["code", "feature"]
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
PROJECT_DOCS = ["product.md", "architecture.md", "coding.md", "testing.md"]
TASK_SECTIONS = ["Goal", "Spec excerpts", "Acceptance criteria", "UI", "Reviews", "Depends on"]

FIRST_LINE_PATTERNS = {
    "review": re.compile(r"VERDICT: (PASS|FAIL)"),
    "verdict": re.compile(r"VERDICT: (APPROVE|REWORK|ESCALATE)"),
    "build-notes": re.compile(r"STATUS: (DONE|BLOCKED)"),
    "preflight": re.compile(r"PREFLIGHT: (READY|TOOLS|BLOCKED)"),
}

TASK_ID_RE = re.compile(r"^(\d{3,})-[a-z0-9]+(?:-[a-z0-9]+){1,2}$")
LOOSE_ID_RE = re.compile(r"^\d+-[^\s|]+$")
ROUND_RE = re.compile(r"^round-(\d+)$")
FENCE_OPEN_RE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
BULLET_RE = re.compile(r"^[-*+][ \t]+(.*)$")
LIST_ITEM_RE = re.compile(r"^ {0,3}(?:[-*+]|\d{1,9}[.)])(?:[ \t]|$)")


class FactoryError(Exception):
    """An error that the orchestrator halts on."""


# --------------------------------------------------------------------------
# Repository, git and file helpers
# --------------------------------------------------------------------------


def run_git(args, check=True):
    proc = subprocess.run(
        ["git"] + args, capture_output=True, text=True, cwd=ROOT
    )
    if check and proc.returncode != 0:
        raise FactoryError("git %s failed: %s" % (" ".join(args), proc.stderr.strip()))
    return proc


def find_root():
    proc = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True
    )
    if proc.returncode != 0:
        raise FactoryError("not inside a git repository: %s" % proc.stderr.strip())
    return proc.stdout.strip()


ROOT = None
STATE = None


def state_path(*parts):
    return os.path.join(STATE, *parts)


def read_text(path):
    """Read a text file, dropping a byte order mark and normalising line ends."""
    with open(path, "rb") as fh:
        data = fh.read()
    text = data.decode("utf-8", errors="replace")
    if text.startswith("﻿"):
        text = text[1:]
    return text.replace("\r\n", "\n").replace("\r", "\n")


def text_lines(text):
    return [line.rstrip() for line in text.split("\n")]


def first_line(path):
    if not os.path.isfile(path):
        return None
    lines = text_lines(read_text(path))
    return lines[0].strip() if lines else ""


def write_atomic(path, content):
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(content)
    os.replace(tmp, path)


def now_utc():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def file_value(path, kind):
    """The value of a file's fixed first line: None if missing, 'invalid' if bad."""
    line = first_line(path)
    if line is None:
        return None
    m = FIRST_LINE_PATTERNS[kind].fullmatch(line)
    return m.group(1) if m else "invalid"


def clean_token(text):
    """Strip backticks, emphasis and surrounding punctuation from a cell or key."""
    return text.strip().strip("`*_ ").strip()


# --------------------------------------------------------------------------
# Markdown structure
# --------------------------------------------------------------------------


def fence_flags(lines):
    """For each line, whether it is part of a fenced code block (fences included)."""
    flags = []
    fence = None
    for line in lines:
        if fence is not None:
            flags.append(True)
            m = re.match(r"^ {0,3}(`{3,}|~{3,})\s*$", line)
            if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= fence[1]:
                fence = None
            continue
        m = FENCE_OPEN_RE.match(line)
        if m and not (m.group(1)[0] == "`" and "`" in m.group(2)):
            fence = (m.group(1)[0], len(m.group(1)))
            flags.append(True)
            continue
        flags.append(False)
    return flags


def heading_of(line):
    """(level, text) for an ATX heading line, else None."""
    m = re.match(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?[ \t]*$", line)
    if not m:
        return None
    text = m.group(2) or ""
    text = re.sub(r"(?:^|[ \t]+)#+$", "", text).strip()
    return len(m.group(1)), text


def level2_sections(text):
    """Ordered list of (heading text, body lines) for the ## headings outside code.

    A section ends at the next heading of level 1 or 2 outside code fences.
    """
    lines = text_lines(text)
    flags = fence_flags(lines)
    sections = []
    current = None
    for line, in_code in zip(lines, flags):
        h = None if in_code else heading_of(line)
        if h and h[0] <= 2:
            if h[0] == 2:
                current = (h[1], [])
                sections.append(current)
            else:
                current = None
            continue
        if current is not None:
            current[1].append((line, in_code))
    return [(name, body) for name, body in sections]


def top_level_bullets(body):
    """The text of each top-level bullet in a section body (outside code)."""
    items = []
    for line, in_code in body:
        if in_code:
            continue
        m = BULLET_RE.match(line)
        if m:
            items.append(m.group(1))
    return items


def bullet_key(item):
    """The leading key of a bullet, such as `security` or 003-some-task."""
    m = re.match(r"^[`*_\s]*([A-Za-z0-9][A-Za-z0-9_-]*)", item)
    return m.group(1) if m else ""


def has_text(body):
    return any(line.strip() for line, _ in body)


# --------------------------------------------------------------------------
# task.md
# --------------------------------------------------------------------------


def norm_section(name):
    return re.sub(r"\s+", " ", name.strip().lower())


def parse_task(task_id):
    """Parse factory/state/tasks/<id>/task.md by its fixed shape."""
    path = state_path("tasks", task_id, "task.md")
    info = {
        "id": task_id,
        "exists": os.path.isfile(path),
        "first_line": None,
        "title": task_id,
        "sections": [],
        "ui_none": False,
        "has_ui": False,
        "reviews": None,
        "deps": None,
    }
    if not info["exists"]:
        return info
    text = read_text(path)
    lines = text_lines(text)
    info["first_line"] = lines[0].strip() if lines else ""
    m = re.fullmatch(r"#[ \t]+(.+?):[ \t]+(.+)", info["first_line"])
    if m and clean_token(m.group(1)) == task_id:
        info["title"] = m.group(2).strip()
    allowed = {norm_section(s): s for s in TASK_SECTIONS}
    for name, body in level2_sections(text):
        key = norm_section(name)
        if re.fullmatch(r"ui:\s*none\.?", key):
            info["sections"].append("UI")
            info["has_ui"] = True
            info["ui_none"] = True
            continue
        if key not in allowed:
            continue
        canonical = allowed[key]
        info["sections"].append(canonical)
        if canonical == "UI":
            info["has_ui"] = True
        elif canonical == "Reviews":
            keys = [bullet_key(b).lower() for b in top_level_bullets(body)]
            info["reviews"] = [k for k in keys if k]
        elif canonical == "Depends on":
            deps = [bullet_key(b) for b in top_level_bullets(body)]
            info["deps"] = [d for d in deps if d]
    return info


def task_reviewers(task):
    """The reviewers that run on a task, in roster order."""
    if task["reviews"] is None:
        keys = set(ALWAYS_REVIEWERS)
        if not task["ui_none"]:
            keys.add("ui")
    else:
        keys = set(k for k in task["reviews"] if k in ROSTER) | set(ALWAYS_REVIEWERS)
    return [k for k in ROSTER if k in keys]


def task_deps(task):
    if not task["deps"]:
        return []
    return [d for d in task["deps"] if d.lower() != "none"]


# --------------------------------------------------------------------------
# backlog.md
# --------------------------------------------------------------------------


def normalise_status(raw):
    for s in STATUSES:
        if raw.strip().lower() == s.lower():
            return s
    return raw.strip()


def split_row(line):
    m = re.match(r"^\s*\|(.*)\|\s*$", line)
    if not m:
        return None
    return m.group(1).split("|")


def parse_backlog():
    """List of dicts {id, status, line} in backlog order, or None if missing."""
    path = state_path("backlog.md")
    if not os.path.isfile(path):
        return None
    lines = text_lines(read_text(path))
    flags = fence_flags(lines)
    rows = []
    for idx, (line, in_code) in enumerate(zip(lines, flags)):
        if in_code:
            continue
        cells = split_row(line)
        if not cells or len(cells) < 2:
            continue
        task_id = clean_token(cells[0])
        if not LOOSE_ID_RE.match(task_id):
            continue
        rows.append({"id": task_id, "status": normalise_status(clean_token(cells[1])), "line": idx})
    return rows


def require_backlog():
    rows = parse_backlog()
    if rows is None:
        raise FactoryError("factory/state/backlog.md does not exist")
    return rows


def require_task(task_id, rows=None):
    rows = rows if rows is not None else require_backlog()
    for row in rows:
        if row["id"] == task_id:
            return row
    raise FactoryError("task %s is not in backlog.md" % task_id)


def task_dirs():
    base = state_path("tasks")
    if not os.path.isdir(base):
        return []
    return sorted(d for d in os.listdir(base) if os.path.isdir(os.path.join(base, d)))


# --------------------------------------------------------------------------
# Rounds
# --------------------------------------------------------------------------


def round_number(name):
    m = ROUND_RE.match(name)
    if not m:
        raise FactoryError("not a round-id: %s" % name)
    return int(m.group(1))


def rounds_of(task_id):
    """Round directory names of a task, sorted by number."""
    base = state_path("tasks", task_id)
    if not os.path.isdir(base):
        return []
    found = []
    for d in os.listdir(base):
        m = ROUND_RE.match(d)
        if m and os.path.isdir(os.path.join(base, d)):
            found.append((int(m.group(1)), d))
    return [d for _, d in sorted(found)]


def round_dir(task_id, round_id):
    return state_path("tasks", task_id, round_id)


def previous_round(task_id, round_id):
    """The latest earlier round directory that has a base-commit."""
    n = round_number(round_id)
    prev = None
    for r in rounds_of(task_id):
        if round_number(r) < n and os.path.isfile(os.path.join(round_dir(task_id, r), "base-commit")):
            prev = r
    return prev


def next_round(task_id):
    rounds = rounds_of(task_id)
    if not rounds:
        return "round-1"
    return "round-%d" % (round_number(rounds[-1]) + 1)


def round_kind(task_id, round_id):
    rdir = round_dir(task_id, round_id)
    if os.path.isfile(os.path.join(rdir, "human-review.md")):
        return "rework"
    if os.path.isfile(os.path.join(rdir, "approver-review.md")):
        return "approver-rework"
    return "auto"


def rework_note_body(task_id, round_id):
    """Lines of the verdict's ## Rework note section, or None if there is none."""
    path = os.path.join(round_dir(task_id, round_id), "verdict.md")
    if not os.path.isfile(path):
        return None
    for name, body in level2_sections(read_text(path)):
        if norm_section(name) == "rework note":
            return body if has_text(body) else None
    return None


def rework_count(task_id, round_id):
    n = round_number(round_id)
    count = 0
    for r in rounds_of(task_id):
        if round_number(r) < n:
            if file_value(os.path.join(round_dir(task_id, r), "verdict.md"), "verdict") == "REWORK":
                count += 1
    return count


def round_info(task_id, round_id, reviewers):
    rdir = round_dir(task_id, round_id)
    build = file_value(os.path.join(rdir, "build-notes.md"), "build-notes")
    reviews = {}
    for key in reviewers:
        reviews[key] = file_value(os.path.join(rdir, "%s-review.md" % key), "review")
    verdict = file_value(os.path.join(rdir, "verdict.md"), "verdict")
    return {
        "id": round_id,
        "kind": round_kind(task_id, round_id),
        "base_commit": os.path.isfile(os.path.join(rdir, "base-commit")),
        "reviews_started": os.path.isfile(os.path.join(rdir, "reviews-started")),
        "build": build,
        "reviews": reviews,
        "verdict": verdict,
    }


# --------------------------------------------------------------------------
# Git questions
# --------------------------------------------------------------------------


def main_subjects():
    return run_git(["log", "main", "--format=%s", "--"]).stdout.splitlines()


def branch_exists(branch):
    # Plain git check: its exit status is the answer.
    return run_git(["rev-parse", "--verify", "--quiet", "refs/heads/" + branch], check=False).returncode == 0


def is_merged(task_id, subjects=None):
    subjects = subjects if subjects is not None else main_subjects()
    if ("factory: merge %s" % task_id) in (s.strip() for s in subjects):
        return True
    branch = "factory/" + task_id
    if not branch_exists(branch):
        return False
    # Plain git check: its exit status is the answer.
    if run_git(["merge-base", "--is-ancestor", branch, "main"], check=False).returncode != 0:
        return False
    branch_subjects = run_git(["log", branch, "--format=%s", "--"]).stdout.splitlines()
    return any(s.startswith(task_id + ": ") for s in branch_subjects)


def used_ids():
    """Task-ids the factory has used, from main's merge subjects and local branches."""
    ids = set()
    for s in main_subjects():
        m = re.match(r"^factory: merge (\S+)$", s.strip())
        if m:
            ids.add(m.group(1))
    refs = run_git(["for-each-ref", "--format=%(refname:short)", "refs/heads/factory/"]).stdout.splitlines()
    for ref in refs:
        if ref.startswith("factory/"):
            ids.add(ref[len("factory/"):])
    return ids


def run_ids():
    ids = set(task_dirs())
    rows = parse_backlog()
    if rows:
        ids |= set(r["id"] for r in rows)
    return ids


def highest_serial_before_run():
    this_run = run_ids()
    serials = []
    for task_id in used_ids():
        if task_id in this_run:
            continue
        m = re.match(r"^(\d+)-", task_id)
        if m:
            serials.append(int(m.group(1)))
    return max(serials) if serials else None


# --------------------------------------------------------------------------
# The board
# --------------------------------------------------------------------------


def task_files(task_id):
    base = state_path("tasks", task_id)
    files = []
    if os.path.isfile(os.path.join(base, "task.md")):
        files.append("task.md")
    for r in rounds_of(task_id):
        rdir = os.path.join(base, r)
        names = sorted(os.listdir(rdir))
        for name in names:
            if name.endswith(".md") and os.path.isfile(os.path.join(rdir, name)):
                files.append("%s/%s" % (r, name))
        for key in ROSTER:
            edir = os.path.join(rdir, key)
            if not os.path.isdir(edir):
                continue
            for dirpath, dirnames, filenames in os.walk(edir):
                dirnames.sort()
                for name in sorted(filenames):
                    rel = os.path.relpath(os.path.join(dirpath, name), base)
                    files.append(rel.replace(os.sep, "/"))
    return files


def stage_of(status, rounds):
    if status == "Planned":
        return "planned"
    if status == "Complete":
        return "done"
    if status == "Abandoned":
        return "abandoned"
    if status == "Escalated":
        return "escalated"
    if not rounds:
        return "building"
    last = rounds[-1]
    if last["build"] == "BLOCKED":
        return "escalated"
    if last["build"] != "DONE":
        return "building"
    if last["verdict"] == "APPROVE":
        return "approval"
    if last["verdict"] == "REWORK":
        return "building"
    if last["verdict"] == "ESCALATE":
        return "escalated"
    values = list(last["reviews"].values())
    if any(v not in ("PASS", "FAIL") for v in values):
        return "review"
    if last["id"] == "round-1" and last["kind"] == "auto" and "FAIL" in values:
        return "building"
    return "approval"


def task_entry(row):
    task_id = row["id"]
    task = parse_task(task_id)
    reviewers = task_reviewers(task)
    rounds = []
    for r in rounds_of(task_id):
        info = round_info(task_id, r, reviewers)
        build = info["build"] if info["build"] in ("DONE", "BLOCKED") else "pending"
        reviews = {
            k: (v if v in ("PASS", "FAIL") else "pending") for k, v in info["reviews"].items()
        }
        verdict = info["verdict"] if info["verdict"] in ("APPROVE", "REWORK", "ESCALATE") else None
        rounds.append({"id": r, "kind": info["kind"], "build": build, "reviews": reviews, "verdict": verdict})
    return {
        "id": task_id,
        "title": task["title"],
        "branch": "factory/" + task_id,
        "status": row["status"],
        "stage": stage_of(row["status"], rounds),
        "rounds": rounds,
        "files": task_files(task_id),
    }


def derive_loop(rows):
    preflight = state_path("preflight.md")
    if rows is None:
        if os.path.isfile(preflight):
            return {"state": "awaiting-preflight", "message": "Preflight stopped: %s" % (first_line(preflight) or "(empty)"), "current": None}
        return {"state": "planning", "message": "Planning", "current": None}
    for row in rows:
        if row["status"] == "Escalated":
            return {"state": "escalated", "message": "%s is escalated" % row["id"], "current": row["id"]}
    for row in rows:
        if row["status"] == "In progress":
            return {"state": "running", "message": "Working on %s" % row["id"], "current": row["id"]}
    if rows and all(r["status"] in ("Complete", "Abandoned") for r in rows):
        return {"state": "complete", "message": "Construction is complete", "current": None}
    return {"state": "halted", "message": "Board rebuilt from the files; /factory continue resumes", "current": None}


def load_board():
    path = state_path("board.json")
    if not os.path.isfile(path):
        return None
    try:
        with open(path, encoding="utf-8") as fh:
            board = json.load(fh)
    except (OSError, ValueError):
        return None
    if not isinstance(board, dict) or not isinstance(board.get("loop"), dict):
        return None
    return board


def write_board(loop, agents):
    rows = parse_backlog()
    board = {
        "updated": now_utc(),
        "loop": {"state": loop.get("state"), "message": loop.get("message", ""), "current": loop.get("current")},
        "agents": agents,
        "tasks": [task_entry(r) for r in (rows or [])],
    }
    os.makedirs(STATE, exist_ok=True)
    write_atomic(state_path("board.json"), json.dumps(board, indent=2) + "\n")
    return board


def apply_board_args(loop, agents, args):
    if args.loop_state is not None:
        if args.loop_state not in LOOP_STATES:
            raise FactoryError("unknown loop state %s; allowed: %s" % (args.loop_state, ", ".join(LOOP_STATES)))
        if args.message is None:
            raise FactoryError("--loop-state needs --message")
        loop["state"] = args.loop_state
    if args.message is not None:
        loop["message"] = args.message
    if args.current is not None:
        loop["current"] = None if args.current == "none" else args.current
    for item in args.agent or []:
        if "=" not in item:
            raise FactoryError("--agent takes NAME=ID (empty ID removes it): %s" % item)
        name, value = item.split("=", 1)
        if value:
            agents[name] = value
        else:
            agents.pop(name, None)


def print_board_summary(board, task_id=None):
    loop = board["loop"]
    print("updated: %s" % board.get("updated"))
    print("loop-state: %s" % loop.get("state"))
    print("loop-message: %s" % (loop.get("message") or ""))
    print("loop-current: %s" % (loop.get("current") or "none"))
    agents = board.get("agents") or {}
    if agents:
        for name in sorted(agents):
            print("agent %s: %s" % (name, agents[name]))
    else:
        print("agents: none")
    task_id = task_id or loop.get("current")
    if not task_id:
        print("task: none")
        return
    for t in board.get("tasks", []):
        if t["id"] == task_id:
            print("task: %s" % t["id"])
            print("task-status: %s" % t["status"])
            print("task-stage: %s" % t["stage"])
            if t["rounds"]:
                r = t["rounds"][-1]
                reviews = " ".join("%s=%s" % kv for kv in r["reviews"].items()) or "none"
                print("latest-round: %s kind=%s build=%s reviews: %s verdict=%s"
                      % (r["id"], r["kind"], r["build"], reviews, r["verdict"] or "none"))
            else:
                print("latest-round: none")
            return
    print("task: %s (not on the board)" % task_id)


# --------------------------------------------------------------------------
# Write verification
# --------------------------------------------------------------------------


def cksum_listing():
    """A cksum listing of every file under factory/state/, minus board.json* and .snapshot."""
    paths = []
    for dirpath, dirnames, filenames in os.walk(STATE):
        dirnames.sort()
        for name in filenames:
            if name.startswith("board.json") or name == ".snapshot":
                continue
            rel = os.path.relpath(os.path.join(dirpath, name), STATE).replace(os.sep, "/")
            if "\n" in rel:
                raise FactoryError("file name with a newline under factory/state: %r" % rel)
            paths.append(rel)
    paths.sort()
    lines = []
    for i in range(0, len(paths), 200):
        chunk = paths[i:i + 200]
        proc = subprocess.run(["cksum", "--"] + chunk, capture_output=True, text=True, cwd=STATE)
        if proc.returncode != 0:
            raise FactoryError("cksum failed: %s" % proc.stderr.strip())
        lines.extend(proc.stdout.splitlines())
    return lines


def listing_map(lines):
    result = {}
    for line in lines:
        parts = line.split(" ", 2)
        if len(parts) == 3:
            result[parts[2]] = parts[0] + " " + parts[1]
    return result


def snapshot_file(args):
    if args.planner:
        return state_path(".snapshot")
    if not args.task:
        raise FactoryError("give --task <task-id> or --planner")
    tdir = state_path("tasks", args.task)
    if not os.path.isdir(tdir):
        raise FactoryError("no task directory for %s" % args.task)
    return os.path.join(tdir, ".snapshot")


def glob_to_re(pattern):
    out = ""
    i = 0
    while i < len(pattern):
        if pattern.startswith("**", i):
            out += ".*"
            i += 2
        elif pattern[i] == "*":
            out += "[^/]*"
            i += 1
        elif pattern[i] == "?":
            out += "[^/]"
            i += 1
        else:
            out += re.escape(pattern[i])
            i += 1
    return re.compile(out + r"\Z")


# --------------------------------------------------------------------------
# The spec copy
# --------------------------------------------------------------------------


def expand_tabs(line):
    return line.expandtabs(4)


def mask_code(text):
    """Text of the same shape with code (fenced, indented, inline) blanked out."""
    lines = text.split("\n")
    flags = fence_flags(lines)
    out = []
    prev_blank = True
    in_indented = False
    list_ctx = False
    for line, in_fence in zip(lines, flags):
        if in_fence:
            out.append(re.sub(r"[^\n]", " ", line))
            prev_blank = False
            in_indented = False
            continue
        blank = line.strip() == ""
        expanded = expand_tabs(line)
        indent = len(expanded) - len(expanded.lstrip(" "))
        if blank:
            out.append(line)
            prev_blank = True
            continue
        if indent >= 4 and (in_indented or (prev_blank and not list_ctx)):
            in_indented = True
            out.append(" " * len(line))
            prev_blank = False
            continue
        in_indented = False
        if LIST_ITEM_RE.match(line):
            list_ctx = True
        elif indent == 0 and prev_blank:
            list_ctx = False
        out.append(line)
        prev_blank = False
    masked = "\n".join(out)
    return mask_code_spans(masked)


def mask_code_spans(text):
    chars = list(text)
    i = 0
    n = len(text)
    while i < n:
        if text[i] != "`":
            i += 1
            continue
        j = i
        while j < n and text[j] == "`":
            j += 1
        run = j - i
        para_end = re.search(r"\n[ \t]*\n", text[j:])
        limit = j + para_end.start() if para_end else n
        k = j
        close = None
        while k < limit:
            if text[k] == "`":
                m = k
                while m < n and text[m] == "`":
                    m += 1
                if m - k == run:
                    close = m
                    break
                k = m
            else:
                k += 1
        if close is None:
            i = j
            continue
        for p in range(i, close):
            if chars[p] != "\n":
                chars[p] = " "
        i = close
    return "".join(chars)


def find_link_destinations(text):
    """(start, end) spans of link destinations outside code, in the original text."""
    masked = mask_code(text)
    spans = []
    for m in re.finditer(r"\]\(", masked):
        i = m.end()
        n = len(masked)
        while i < n and masked[i] in " \t\n":
            i += 1
        if i >= n:
            continue
        start = i
        if masked[i] == "<":
            close = masked.find(">", i)
            if close == -1 or "\n" in masked[i:close]:
                continue
            spans.append((start, close + 1))
            continue
        depth = 0
        while i < n:
            c = masked[i]
            if c in " \t\n":
                break
            if c == "(":
                depth += 1
            elif c == ")":
                if depth == 0:
                    break
                depth -= 1
            elif c == "\\" and i + 1 < n:
                i += 1
            i += 1
        if i > start:
            spans.append((start, i))
    for m in re.finditer(r"(?m)^ {0,3}\[(?!\^)[^\]\n]+\]:[ \t]*\n?[ \t]*(<[^>\n]*>|\S+)", masked):
        spans.append((m.start(1), m.end(1)))
    spans.sort()
    return spans


def split_destination(dest):
    """(path, suffix, angle) for a relative Markdown link destination, else None."""
    angle = dest.startswith("<") and dest.endswith(">")
    raw = dest[1:-1] if angle else dest
    if not raw or raw.startswith("#") or raw.startswith("/"):
        return None
    if re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", raw):
        return None
    cut = len(raw)
    for sep in ("#", "?"):
        pos = raw.find(sep)
        if pos != -1:
            cut = min(cut, pos)
    path, suffix = raw[:cut], raw[cut:]
    path = urllib.parse.unquote(path)
    if not path.lower().endswith(".md"):
        return None
    return path, suffix, angle


def format_destination(path, suffix, angle):
    dest = path + suffix
    if angle or re.search(r"[\s()<>]", dest):
        return "<%s>" % dest
    return dest


def spec_copy(spec_arg):
    spec = os.path.realpath(os.path.expanduser(spec_arg))
    if not os.path.isfile(spec):
        raise FactoryError("spec not found: %s" % spec)
    spec_dir = os.path.dirname(spec)
    os.makedirs(STATE, exist_ok=True)
    write_atomic(state_path("spec-source"), spec + "\n")

    for name in ("SPEC.md",):
        if os.path.isfile(state_path(name)):
            os.remove(state_path(name))
    for name in ("project", "spec"):
        if os.path.isdir(state_path(name)):
            shutil.rmtree(state_path(name))

    copies = {spec: "SPEC.md"}  # original realpath -> copy path relative to state
    queue = [spec]
    project = []
    # Each project document sits next to the spec or, failing that, in its
    # parent directory (specs may live in a subdirectory such as doc/cycles/).
    for doc in PROJECT_DOCS:
        for doc_dir in (spec_dir, os.path.dirname(spec_dir)):
            path = os.path.realpath(os.path.join(doc_dir, doc))
            if os.path.isfile(path):
                break
        if os.path.isfile(path) and path not in copies:
            copies[path] = "project/" + doc
            project.append(path)
            queue.append(path)
    used_names = set()
    missing = []
    order = list(queue)

    def assign(path):
        base = os.path.basename(path)
        stem, ext = os.path.splitext(base)
        name = base
        k = 2
        while name.lower() in used_names:
            name = "%s-%d%s" % (stem, k, ext)
            k += 1
        used_names.add(name.lower())
        copies[path] = "spec/" + name
        order.append(path)
        return copies[path]

    i = 0
    outputs = {}
    while i < len(queue):
        src = queue[i]
        i += 1
        text = read_text(src)
        src_copy = copies[src]
        edits = []
        for start, end in find_link_destinations(text):
            parsed = split_destination(text[start:end])
            if not parsed:
                continue
            path, suffix, angle = parsed
            target = os.path.realpath(os.path.join(os.path.dirname(src), path))
            if not os.path.isfile(target):
                missing.append((target, src_copy))
                continue
            if target not in copies:
                assign(target)
                queue.append(target)
            rel = os.path.relpath(copies[target], os.path.dirname(src_copy) or ".")
            edits.append((start, end, format_destination(rel.replace(os.sep, "/"), suffix, angle)))
        for start, end, new in reversed(edits):
            text = text[:start] + new + text[end:]
        outputs[src_copy] = text

    for src in order:
        dest = state_path(copies[src])
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        with open(dest, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(outputs[copies[src]])

    print("spec-source: %s" % spec)
    for src in order:
        print("copied: %s <- %s" % (copies[src], src))
    if missing:
        seen = set()
        for target, origin in missing:
            if (target, origin) in seen:
                continue
            seen.add((target, origin))
            print("missing: %s (linked from %s)" % (target, origin))
    else:
        print("missing: none")


# --------------------------------------------------------------------------
# The plan check
# --------------------------------------------------------------------------


def plan_check():
    problems = []
    warnings = []
    rows = parse_backlog()
    if rows is None:
        problems.append("factory/state/backlog.md does not exist")
        rows = []
    seen = {}
    for row in rows:
        seen[row["id"]] = seen.get(row["id"], 0) + 1
    for task_id, count in seen.items():
        if count > 1:
            problems.append("%s appears %d times in backlog.md" % (task_id, count))
    dirs = task_dirs()
    for d in dirs:
        if d not in seen:
            problems.append("task directory tasks/%s is not in backlog.md (removing a task from backlog.md also means deleting its task directory)" % d)
    highest = highest_serial_before_run()
    # The plan is checked before any task is built, so no id of this run can
    # be in git yet: an id that is (a merge subject or a branch) was used by
    # an earlier run.
    in_git = used_ids()
    serials = {}
    position = {row["id"]: idx for idx, row in reversed(list(enumerate(rows)))}
    subjects = None
    for idx, row in enumerate(rows):
        task_id = row["id"]
        if task_id in in_git:
            problems.append("%s was already used by an earlier run (a 'factory: merge %s' commit on main or a factory/%s branch); give the task a new serial" % (task_id, task_id, task_id))
        if row["status"] != "Planned":
            problems.append("%s has status %s, not Planned" % (task_id, row["status"]))
        m = TASK_ID_RE.match(task_id)
        if not m:
            problems.append("%s is not a valid task-id (zero-padded serial of at least three digits, then two or three lowercase words)" % task_id)
        else:
            serial = int(m.group(1))
            serials.setdefault(serial, []).append(task_id)
            if highest is not None and serial <= highest:
                problems.append("%s reuses serial %03d; serials must continue above %03d, the highest already used" % (task_id, serial, highest))
        task = parse_task(task_id)
        if not task["exists"]:
            problems.append("%s has no tasks/%s/task.md" % (task_id, task_id))
            continue
        if task["first_line"] is None or not re.fullmatch(r"#[ \t]+`?%s`?:[ \t]+\S.*" % re.escape(task_id), task["first_line"]):
            problems.append("%s: task.md first line must be '# %s: <title>'" % (task_id, task_id))
        found = [s for s in task["sections"] if s in TASK_SECTIONS]
        for s in TASK_SECTIONS:
            if s not in found:
                problems.append("%s: task.md has no '## %s' section" % (task_id, "UI' (or '## UI: none" if s == "UI" else s))
            elif found.count(s) > 1:
                problems.append("%s: task.md has more than one '## %s' section" % (task_id, s))
        if all(s in found for s in TASK_SECTIONS) and [s for s in TASK_SECTIONS if s in found] != sorted(dict.fromkeys(found), key=found.index):
            problems.append("%s: task.md sections are not in the order %s" % (task_id, ", ".join(TASK_SECTIONS)))
        if task["reviews"] is not None:
            for key in ALWAYS_REVIEWERS:
                if key not in task["reviews"]:
                    problems.append("%s: Reviews does not list `%s`" % (task_id, key))
            for key in task["reviews"]:
                if key not in ROSTER:
                    warnings.append("%s: Reviews lists `%s`, which is not a roster key (ignored)" % (task_id, key))
            if task["has_ui"] and task["ui_none"] and "ui" in task["reviews"]:
                warnings.append("%s: Reviews lists `ui` but the UI section is UI: none" % task_id)
            if task["has_ui"] and not task["ui_none"] and "ui" not in task["reviews"]:
                warnings.append("%s: the task has UI changes but Reviews does not list `ui`" % task_id)
        if task["deps"] is not None:
            deps = task["deps"]
            if not deps:
                problems.append("%s: Depends on has no bullets (use the single bullet '- none')" % task_id)
            elif any(d.lower() == "none" for d in deps) and len(deps) > 1:
                problems.append("%s: Depends on mixes '- none' with task-ids" % task_id)
            for dep in task_deps(task):
                if dep == task_id:
                    problems.append("%s depends on itself" % task_id)
                elif dep in position:
                    if position[dep] > idx:
                        problems.append("%s depends on %s, which comes later in backlog.md" % (task_id, dep))
                else:
                    if subjects is None:
                        subjects = main_subjects()
                    if not is_merged(dep, subjects):
                        problems.append("%s depends on %s, which is neither earlier in backlog.md nor merged from an earlier run" % (task_id, dep))
    for serial, ids in serials.items():
        if len(ids) > 1 and len(set(ids)) > 1:
            problems.append("serial %03d is used by more than one task: %s" % (serial, ", ".join(sorted(set(ids)))))
    print("plan: %s" % ("sound" if not problems else "problems"))
    for p in problems:
        print("problem: %s" % p)
    for w in warnings:
        print("warning: %s" % w)


# --------------------------------------------------------------------------
# First lines
# --------------------------------------------------------------------------


def fixed_files_of_task(task_id):
    """(path relative to state, kind) for each file in a task dir with a fixed first line."""
    items = []
    if os.path.isfile(state_path("tasks", task_id, "task.md")):
        items.append(("tasks/%s/task.md" % task_id, "task"))
    for r in rounds_of(task_id):
        rdir = round_dir(task_id, r)
        if os.path.isfile(os.path.join(rdir, "build-notes.md")):
            items.append(("tasks/%s/%s/build-notes.md" % (task_id, r), "build-notes"))
        for key in ROSTER:
            if os.path.isfile(os.path.join(rdir, "%s-review.md" % key)):
                items.append(("tasks/%s/%s/%s-review.md" % (task_id, r, key), "review"))
        if os.path.isfile(os.path.join(rdir, "verdict.md")):
            items.append(("tasks/%s/%s/verdict.md" % (task_id, r), "verdict"))
    return items


def check_first_line(rel, kind):
    line = first_line(state_path(rel))
    if kind == "task":
        task_id = rel.split("/")[1]
        ok = bool(re.fullmatch(r"#[ \t]+`?%s`?:[ \t]+\S.*" % re.escape(task_id), line or ""))
    else:
        ok = bool(FIRST_LINE_PATTERNS[kind].fullmatch(line or ""))
    return ok, line


# --------------------------------------------------------------------------
# Commands
# --------------------------------------------------------------------------


def cmd_preconditions(args):
    branch = run_git(["branch", "--show-current"]).stdout.strip() or "(detached)"
    # Plain git check: its exit status is the answer.
    ignored = run_git(["check-ignore", "-q", "factory/state"], check=False).returncode == 0
    dirty = run_git(["status", "--porcelain", "--untracked-files=all"]).stdout.splitlines()
    print("branch: %s" % branch)
    print("state-ignored: %s" % ("yes" if ignored else "no"))
    print("tree: %s" % ("clean" if not dirty else "dirty"))
    for line in dirty:
        print("dirty: %s" % line)


def cmd_spec_copy(args):
    spec_copy(args.spec)


def cmd_run_info(args):
    rows = parse_backlog()
    preflight = state_path("preflight.md")
    pf_exists = os.path.isfile(preflight)
    print("preflight: %s" % ((file_value(preflight, "preflight") or "invalid") if pf_exists else "absent"))
    print("backlog: %s" % ("present" if rows is not None else "absent"))
    print("preflight-state: %s" % ("yes" if pf_exists and rows is None else "no"))
    print("spec-source: %s" % (read_text(state_path("spec-source")).strip() if os.path.isfile(state_path("spec-source")) else "absent"))
    print("task-dirs: %d" % len(task_dirs()))
    board = load_board()
    print("board: %s" % ("present" if board else "absent"))
    print("loop-state: %s" % (board["loop"].get("state") if board else "none"))
    if rows is None:
        print("tasks: none")
        return
    print("tasks: %d" % len(rows))
    counts = {}
    for r in rows:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    for status in sorted(counts):
        print("count %s: %d" % (status, counts[status]))
    active = [r for r in rows if r["status"] in ("In progress", "Escalated")]
    if active:
        for r in active:
            print("active: %s %s" % (r["id"], r["status"]))
    else:
        print("active: none")
    nxt = next((r["id"] for r in rows if r["status"] == "Planned"), None)
    print("next-planned: %s" % (nxt or "none"))
    all_planned = bool(rows) and all(r["status"] == "Planned" for r in rows)
    no_rounds = all(not rounds_of(r["id"]) for r in rows)
    print("all-planned-no-rounds: %s" % ("yes" if all_planned and no_rounds else "no"))


def cmd_backlog(args):
    rows = require_backlog()
    if not rows:
        print("tasks: none")
    for r in rows:
        print("%s %s" % (r["id"], r["status"]))


def cmd_set_status(args):
    status = normalise_status(args.status)
    if status not in STATUSES:
        raise FactoryError("unknown status %s; allowed: %s" % (args.status, ", ".join(STATUSES)))
    rows = require_backlog()
    row = require_task(args.task, rows)
    path = state_path("backlog.md")
    raw = read_text(path)
    lines = raw.split("\n")
    cells = split_row(lines[row["line"]])
    cells[1] = " %s " % status
    indent = re.match(r"^\s*", lines[row["line"]]).group(0)
    lines[row["line"]] = indent + "|" + "|".join(cells) + "|"
    write_atomic(path, "\n".join(lines))
    print("%s %s" % (args.task, status))


def cmd_next_task(args):
    rows = require_backlog()
    nxt = next((r["id"] for r in rows if r["status"] == "Planned"), None)
    print(nxt or "none")


def cmd_highest_serial(args):
    highest = highest_serial_before_run()
    print("highest: %s" % ("%03d" % highest if highest is not None else "none"))
    print("next: %03d" % ((highest or 0) + 1))


def cmd_archived_stack(args):
    base = os.path.join(ROOT, "factory", "archive")
    best = None
    if os.path.isdir(base):
        for d in os.listdir(base):
            path = os.path.join(base, d, "stack.md")
            if os.path.isfile(path):
                mtime = os.path.getmtime(path)
                if best is None or mtime > best[0]:
                    best = (mtime, path)
    print(os.path.relpath(best[1], ROOT) if best else "none")


def cmd_plan_check(args):
    plan_check()


def cmd_reviewers(args):
    task = parse_task(args.task)
    if not task["exists"]:
        raise FactoryError("no task.md for %s" % args.task)
    print(" ".join(task_reviewers(task)))


def cmd_dependents(args):
    rows = require_backlog()
    found = []
    for r in rows:
        if args.task in task_deps(parse_task(r["id"])):
            found.append("%s %s" % (r["id"], r["status"]))
    print("\n".join(found) if found else "none")


def cmd_merged(args):
    print("yes" if is_merged(args.task) else "no")


def cmd_first_lines(args):
    items = []
    if args.task:
        if not os.path.isdir(state_path("tasks", args.task)):
            raise FactoryError("no task directory for %s" % args.task)
        items = fixed_files_of_task(args.task)
    else:
        if os.path.isfile(state_path("preflight.md")):
            items.append(("preflight.md", "preflight"))
        for d in task_dirs():
            items.extend(fixed_files_of_task(d))
    bad = []
    for rel, kind in items:
        ok, line = check_first_line(rel, kind)
        if not ok:
            bad.append((rel, line))
    print("first-lines: %s" % ("ok" if not bad else "invalid"))
    for rel, line in bad:
        print("invalid: factory/state/%s: %r" % (rel, line))


def cmd_round_info(args):
    rows = parse_backlog()
    status = "unknown"
    if rows:
        for r in rows:
            if r["id"] == args.task:
                status = r["status"]
    task = parse_task(args.task)
    reviewers = task_reviewers(task)
    print("task: %s" % args.task)
    print("status: %s" % status)
    print("reviewers: %s" % " ".join(reviewers))
    rounds = rounds_of(args.task)
    print("rounds: %s" % (" ".join(rounds) if rounds else "none"))
    print("next-round: %s" % next_round(args.task))
    round_id = args.round or (rounds[-1] if rounds else None)
    if round_id is None:
        print("round: none")
        return
    round_number(round_id)
    if not os.path.isdir(round_dir(args.task, round_id)):
        raise FactoryError("no round directory %s for %s" % (round_id, args.task))
    info = round_info(args.task, round_id, reviewers)
    rdir = round_dir(args.task, round_id)
    print("round: %s" % round_id)
    print("kind: %s" % info["kind"])
    print("base-commit: %s" % ("present" if info["base_commit"] else "absent"))
    print("human-review: %s" % ("present" if os.path.isfile(os.path.join(rdir, "human-review.md")) else "absent"))
    print("approver-review: %s" % ("present" if os.path.isfile(os.path.join(rdir, "approver-review.md")) else "absent"))
    print("build: %s" % (info["build"] or "pending"))
    print("reviews-started: %s" % ("yes" if info["reviews_started"] else "no"))
    missing = []
    for key, value in info["reviews"].items():
        print("review %s: %s" % (key, value or "missing"))
        if value is None:
            missing.append(key)
    print("reviews-missing: %s" % (" ".join(missing) if missing else "none"))
    print("verdict: %s" % (info["verdict"] or "none"))
    if info["verdict"] == "REWORK":
        print("rework-note: %s" % ("present" if rework_note_body(args.task, round_id) is not None else "absent"))
    print("earlier-reworks: %d" % rework_count(args.task, round_id))
    print("previous-round: %s" % (previous_round(args.task, round_id) or "none"))


def cmd_previous_round(args):
    print(previous_round(args.task, args.round) or "none")


def cmd_rework_count(args):
    print(rework_count(args.task, args.round))


def cmd_rework_note(args):
    print("present" if rework_note_body(args.task, args.round) is not None else "absent")


def cmd_copy_rework_note(args):
    body = rework_note_body(args.task, args.source)
    if body is None:
        raise FactoryError("%s/%s/verdict.md has no rework note with text" % (args.task, args.source))
    dest_dir = round_dir(args.task, args.dest)
    if not os.path.isdir(dest_dir):
        raise FactoryError("round directory %s does not exist" % args.dest)
    lines = [line for line, _ in body]
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    dest = os.path.join(dest_dir, "approver-review.md")
    write_atomic(dest, "## Rework note\n\n" + "\n".join(lines) + "\n")
    print("wrote: %s" % os.path.relpath(dest, ROOT))


def cmd_write_base_commit(args):
    rdir = round_dir(args.task, args.round)
    if not os.path.isdir(rdir):
        raise FactoryError("round directory %s does not exist" % rdir)
    ref = "main" if round_number(args.round) == 1 else "HEAD"
    sha = run_git(["rev-parse", "--verify", ref + "^{commit}"]).stdout.strip()
    write_atomic(os.path.join(rdir, "base-commit"), sha + "\n")
    print("base-commit: %s (%s)" % (sha, ref))


def cmd_snapshot(args):
    path = snapshot_file(args)
    lines = cksum_listing()
    write_atomic(path, "".join(line + "\n" for line in lines))
    print("snapshot: %s (%d files)" % (os.path.relpath(path, ROOT), len(lines)))


def cmd_check_writes(args):
    path = snapshot_file(args)
    if not os.path.isfile(path):
        raise FactoryError("no snapshot at %s" % os.path.relpath(path, ROOT))
    before = listing_map(text_lines(read_text(path)))
    after = listing_map(cksum_listing())
    allows = [glob_to_re(g) for g in (args.allow or [])]
    changed = []
    for p in sorted(set(before) | set(after)):
        if p not in after:
            changed.append(("removed", p))
        elif p not in before:
            changed.append(("added", p))
        elif before[p] != after[p]:
            changed.append(("changed", p))
    violations = []
    for how, p in changed:
        allowed = any(rx.match(p) for rx in allows)
        print("%s: factory/state/%s%s" % (how, p, "" if allowed else "  (not allowed)"))
        if not allowed:
            violations.append(p)
    print("writes: %s" % ("ok" if not violations else "violation"))


def cmd_board_rebuild(args):
    rows = parse_backlog()
    loop = derive_loop(rows)
    agents = {}
    apply_board_args(loop, agents, args)
    board = write_board(loop, agents)
    print_board_summary(board)


def cmd_board_update(args):
    board = load_board()
    if board is None:
        loop = derive_loop(parse_backlog())
        agents = {}
    else:
        loop = dict(board["loop"])
        agents = dict(board.get("agents") or {})
    apply_board_args(loop, agents, args)
    board = write_board(loop, agents)
    print_board_summary(board)


def cmd_board_show(args):
    board = load_board()
    if board is None:
        print("board: absent")
        return
    print_board_summary(board, args.task)


def add_board_args(p):
    p.add_argument("--loop-state", help="new loop.state (needs --message): " + ", ".join(LOOP_STATES))
    p.add_argument("--message", help="new loop.message")
    p.add_argument("--current", help="task-id for loop.current, or 'none'")
    p.add_argument("--agent", action="append", metavar="NAME=ID", help="record an agent id (NAME= removes it); repeatable")


def build_parser():
    parser = argparse.ArgumentParser(
        prog="factory.py",
        description="The factory script. Run from the repository root: python3 -I .claude/skills/factory/factory.py <command> ...",
    )
    sub = parser.add_subparsers(dest="command", metavar="<command>")
    sub.required = True

    def add(name, func, help_text):
        p = sub.add_parser(name, help=help_text, description=help_text)
        p.set_defaults(func=func)
        return p

    add("preconditions", cmd_preconditions, "Current branch, whether factory/state is git-ignored, and whether the tree is clean (untracked files included).")
    p = add("spec-copy", cmd_spec_copy, "Copy a spec, the project documents next to it (or in its parent directory) and every Markdown file they link to into factory/state/, rewriting links; write spec-source.")
    p.add_argument("spec")
    add("run-info", cmd_run_info, "The state of the run: preflight result, backlog, preflight state, active task, next Planned task, loop state.")
    add("backlog", cmd_backlog, "Each task in backlog.md with its status, in backlog order.")
    p = add("set-status", cmd_set_status, "Set a task's status in backlog.md: " + ", ".join(STATUSES) + ".")
    p.add_argument("task")
    p.add_argument("status")
    add("next-task", cmd_next_task, "The first Planned task in backlog.md, or none.")
    add("highest-serial", cmd_highest_serial, "The highest task serial used before this run (from main's merge subjects and factory/* branches) and the next one.")
    add("archived-stack", cmd_archived_stack, "The most recently modified factory/archive/*/stack.md, or none.")
    add("plan-check", cmd_plan_check, "The mechanical part of 'Checking the plan': prints 'plan: sound' or 'plan: problems' with problem: and warning: lines.")
    p = add("reviewers", cmd_reviewers, "The reviewers that run on a task, in roster order (code and feature always; default when there is no Reviews section).")
    p.add_argument("task")
    p = add("dependents", cmd_dependents, "The tasks whose Depends on lists this task, with their status, or none.")
    p.add_argument("task")
    p = add("merged", cmd_merged, "Whether a task is merged into main: yes or no.")
    p.add_argument("task")
    p = add("first-lines", cmd_first_lines, "Check the first line of every file that has a fixed one (all of factory/state, or one task's directory).")
    p.add_argument("--task")
    p = add("round-info", cmd_round_info, "A task's rounds and the state of one round (default: the latest): kind, base-commit, notes, build, reviews, verdict, rework note, earlier reworks, previous and next round.")
    p.add_argument("task")
    p.add_argument("round", nargs="?")
    p = add("previous-round", cmd_previous_round, "The previous round of a round: the latest earlier round directory with a base-commit, or none.")
    p.add_argument("task")
    p.add_argument("round")
    p = add("rework-count", cmd_rework_count, "The number of REWORK verdicts in the rounds before this round.")
    p.add_argument("task")
    p.add_argument("round")
    p = add("rework-note", cmd_rework_note, "Whether a round's verdict.md has a '## Rework note' section with text: present or absent.")
    p.add_argument("task")
    p.add_argument("round")
    p = add("copy-rework-note", cmd_copy_rework_note, "Copy the '## Rework note' section of <source> round's verdict.md to <dest> round's approver-review.md.")
    p.add_argument("task")
    p.add_argument("source")
    p.add_argument("dest")
    p = add("write-base-commit", cmd_write_base_commit, "Write a round's base-commit: main's commit for round-1, HEAD for later rounds.")
    p.add_argument("task")
    p.add_argument("round")
    p = add("snapshot", cmd_snapshot, "Write a cksum listing of factory/state/ (minus board.json* and .snapshot files) to tasks/<task>/.snapshot or, with --planner, to factory/state/.snapshot.")
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--task")
    g.add_argument("--planner", action="store_true")
    p = add("check-writes", cmd_check_writes, "Compare factory/state/ with the snapshot; list each added, changed or removed file and whether an --allow glob (relative to factory/state, * within a directory, ** across) covers it; ends with 'writes: ok' or 'writes: violation'.")
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--task")
    g.add_argument("--planner", action="store_true")
    p.add_argument("--allow", action="append", metavar="GLOB")
    p = add("board-rebuild", cmd_board_rebuild, "Rebuild board.json from backlog.md and the task files; the loop is derived from the files unless given (never awaiting-plan-review unless passed).")
    add_board_args(p)
    p = add("board-update", cmd_board_update, "Update board.json: set updated, loop fields and agents as given, and recompute the task entries from their files.")
    add_board_args(p)
    p = add("board-show", cmd_board_show, "Print the loop, the agents, and one task's status, stage and latest round (default: loop.current).")
    p.add_argument("--task")
    return parser


def main(argv=None):
    global ROOT, STATE
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        ROOT = find_root()
        STATE = os.path.join(ROOT, "factory", "state")
        args.func(args)
    except FactoryError as exc:
        print("factory.py: error: %s" % exc, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
