#!/usr/bin/env python3
"""The factory script: the mechanics of the /factory loop.

Run from the repository root as

    python3 -I .claude/skills/factory/factory.py <command> ...

Every command exits 0 when it succeeds and prints its answer, even when the
answer is "no", "none" or an empty list. A non-zero exit always means an
error, with the reason on stderr.

Commands:
  board rebuild [--state S] [--message M]
  board set [--state S] [--message M] [--current ID | --clear-current]
            [--agent NAME=ID ...] [--task ID]
  board show [--task ID]
  spec-copy [SPEC]
  plan-check
  max-serial
  round latest TASK | round previous TASK ROUND | round next TASK
  round kind TASK ROUND | round state TASK [ROUND]
  rework-count TASK ROUND
  rework-note TASK ROUND [--out PATH]
  reviewers TASK
  dependents TASK
  merged TASK
  tasks
  backlog-set TASK STATUS
  first-line PATH
  check-firstlines [TASK]

It uses only the Python standard library and works everything out from the
files in factory/state/ and from git.
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
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
STATE = ROOT / "factory" / "state"
TASKS = STATE / "tasks"
BOARD = STATE / "board.json"
BACKLOG = STATE / "backlog.md"
PREFLIGHT = STATE / "preflight.md"
SPEC = STATE / "SPEC.md"
SPEC_DIR = STATE / "spec"
SPEC_SOURCE = STATE / "spec-source"
MAIN = "main"

# The reviewer roster, in roster order. Rebuild this file when a row is added.
ROSTER = ["code", "feature", "security", "privacy", "api", "ui"]
ALWAYS = ["code", "feature"]

STATUSES = ["Planned", "In progress", "Escalated", "Complete", "Abandoned"]
LOOP_STATES = ["planning", "awaiting-preflight", "awaiting-plan-review",
               "running", "escalated", "halted", "complete"]

FIRST_LINES = {
    "review": ["VERDICT: PASS", "VERDICT: FAIL"],
    "verdict": ["VERDICT: APPROVE", "VERDICT: REWORK", "VERDICT: ESCALATE"],
    "build-notes": ["STATUS: DONE", "STATUS: BLOCKED"],
    "preflight": ["PREFLIGHT: READY", "PREFLIGHT: TOOLS", "PREFLIGHT: BLOCKED"],
}

ID_RE = re.compile(r"^(\d{3,})-([a-z0-9]+(?:-[a-z0-9]+){1,2})$")
LOOSE_ID_RE = re.compile(r"(?<![\w-])(\d+-[a-z0-9]+(?:-[a-z0-9]+)*)(?![\w-])")
ROUND_RE = re.compile(r"^round-([1-9]\d*)$")
FENCE_OPEN_RE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
HEADING_RE = re.compile(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?[ \t]*$")
LIST_ITEM_RE = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s+(.*)$")


class FactoryError(Exception):
    pass


# ---------------------------------------------------------------- text ----

def read_text(path):
    """Read a file as text, dropping a byte order mark."""
    return Path(path).read_bytes().decode("utf-8-sig", errors="replace")


def read_lines(path):
    """Lines without line endings or trailing spaces (CRLF safe)."""
    return [line.rstrip() for line in read_text(path).splitlines()]


def write_atomic(path, text):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    os.replace(tmp, path)


def fence_mask(lines):
    """For each line, True when it is inside a fenced code block (fences included)."""
    mask = []
    fence = None  # (char, length)
    for line in lines:
        if fence is None:
            m = FENCE_OPEN_RE.match(line)
            if m and not (m.group(1)[0] == "`" and "`" in m.group(2)):
                fence = (m.group(1)[0], len(m.group(1)))
                mask.append(True)
            else:
                mask.append(False)
        else:
            mask.append(True)
            s = line.strip()
            if (len(line) - len(line.lstrip(" ")) <= 3 and s
                    and set(s) == {fence[0]} and len(s) >= fence[1]):
                fence = None
    return mask


def clean_token(text):
    """Strip Markdown decoration around a cell or token: backticks, emphasis, links."""
    t = text.strip()
    m = re.fullmatch(r"\[(.*)\]\([^)]*\)", t)
    if m:
        t = m.group(1).strip()
    return t.strip("`*_ ").strip()


def norm_heading(text):
    t = re.sub(r"[`*_]", "", text or "").strip().lower()
    t = re.sub(r"\s+", " ", t)
    return t.rstrip(":").strip()


def headings(lines):
    """(index, level, text) of every ATX heading outside code fences."""
    out = []
    for i, (line, fenced) in enumerate(zip(lines, fence_mask(lines))):
        if fenced:
            continue
        m = HEADING_RE.match(line)
        if not m:
            continue
        text = m.group(2) or ""
        text = re.sub(r"(^|[ \t]+)#+$", "", text).strip()
        out.append((i, len(m.group(1)), text))
    return out


def section_body(lines, index, level):
    """Lines after the heading at `index`, up to the next heading of level <= `level`."""
    end = len(lines)
    for i, lvl, _ in headings(lines):
        if i > index and lvl <= level:
            end = i
            break
    return list(range(index + 1, end))


def first_line_value(path, kind):
    """None if the file is missing, the result word if the first line is valid, else 'invalid'."""
    path = Path(path)
    if not path.is_file():
        return None
    lines = read_lines(path)
    first = lines[0] if lines else ""
    if first in FIRST_LINES[kind]:
        return first.split(": ", 1)[1]
    return "invalid"


def first_line_raw(path):
    lines = read_lines(path)
    return lines[0] if lines else ""


def kind_of_file(path):
    name = Path(path).name
    if name == "build-notes.md":
        return "build-notes"
    if name == "verdict.md":
        return "verdict"
    if name == "preflight.md":
        return "preflight"
    for key in ROSTER:
        if name == f"{key}-review.md":
            return "review"
    return None


# ----------------------------------------------------------------- git ----

def git(*args, check=True):
    r = subprocess.run(["git", "-c", "core.quotepath=off", *args], cwd=ROOT,
                       capture_output=True, text=True)
    if check and r.returncode != 0:
        raise FactoryError(f"git {' '.join(args)} failed: {r.stderr.strip()}")
    return r


def has_ref(ref):
    return git("rev-parse", "--verify", "--quiet", ref, check=False).returncode == 0


def main_subjects():
    if not has_ref(f"refs/heads/{MAIN}"):
        return []
    return [s.rstrip() for s in git("log", MAIN, "--format=%s").stdout.splitlines()]


def factory_branches():
    out = git("for-each-ref", "--format=%(refname)", "refs/heads/factory/").stdout
    return [line[len("refs/heads/factory/"):] for line in out.splitlines() if line.strip()]


def is_merged(tid):
    if f"factory: merge {tid}" in main_subjects():
        return True
    branch = f"refs/heads/factory/{tid}"
    if not has_ref(branch) or not has_ref(f"refs/heads/{MAIN}"):
        return False
    r = git("merge-base", "--is-ancestor", branch, f"refs/heads/{MAIN}", check=False)
    if r.returncode == 1:
        return False
    if r.returncode != 0:
        raise FactoryError(f"git merge-base failed: {r.stderr.strip()}")
    subjects = git("log", branch, "--format=%s").stdout.splitlines()
    return any(s.startswith(f"{tid}: ") for s in subjects)


def used_serials():
    serials = []
    for s in main_subjects():
        m = re.match(r"^factory: merge (\d+)-", s)
        if m:
            serials.append(int(m.group(1)))
    for name in factory_branches():
        m = re.match(r"^(\d+)-", name)
        if m:
            serials.append(int(m.group(1)))
    return serials


# ------------------------------------------------------------- backlog ----

def norm_status(text):
    t = clean_token(text)
    for s in STATUSES:
        if t.lower() == s.lower():
            return s
    return t


def backlog_rows():
    """[(line index, task id, status)] in backlog order, or None without backlog.md."""
    if not BACKLOG.is_file():
        return None
    lines = read_lines(BACKLOG)
    rows = []
    for i, (line, fenced) in enumerate(zip(lines, fence_mask(lines))):
        s = line.strip()
        if fenced or not s.startswith("|"):
            continue
        cells = [c.strip() for c in s.strip("|").split("|")]
        if len(cells) < 2:
            continue
        tid = clean_token(cells[0])
        if not re.fullmatch(r"\d+-[a-z0-9-]+", tid):
            continue  # header or separator
        rows.append((i, tid, norm_status(cells[1])))
    return rows


def backlog():
    rows = backlog_rows()
    return None if rows is None else [(tid, status) for _, tid, status in rows]


def status_of(tid):
    for t, s in backlog() or []:
        if t == tid:
            return s
    raise FactoryError(f"task {tid} is not in backlog.md")


def require_task(tid):
    status_of(tid)
    return TASKS / tid


def preflight_state():
    return PREFLIGHT.is_file() and not BACKLOG.is_file()


# ------------------------------------------------------------- task.md ----

class TaskFile:
    SECTIONS = {
        "goal": lambda t: t.startswith("goal"),
        "spec excerpts": lambda t: "spec" in t or "excerpt" in t,
        "acceptance criteria": lambda t: "acceptance" in t,
        "ui": lambda t: re.match(r"^ui\b", t) is not None,
        "reviews": lambda t: t.startswith("review"),
        "dependencies": lambda t: t.startswith("depend"),
    }

    def __init__(self, tid):
        self.tid = tid
        self.path = TASKS / tid / "task.md"
        self.exists = self.path.is_file()
        self.lines = read_lines(self.path) if self.exists else []
        self.heads = headings(self.lines)
        self.fenced = fence_mask(self.lines)

    def title(self):
        for _, level, text in self.heads:
            if level == 1 and text:
                return text
        return None

    def find(self, name):
        pred = self.SECTIONS[name]
        for i, level, text in self.heads:
            if level >= 2 and pred(norm_heading(text)):
                return i, level, text
        return None

    def body(self, name):
        h = self.find(name)
        if not h:
            return None
        return [self.lines[i] for i in section_body(self.lines, h[0], h[1])
                if not self.fenced[i]]

    def ui_none(self):
        h = self.find("ui")
        if h:
            rest = norm_heading(h[2])[2:].strip(" :")
            if rest == "none":
                return True
            for line in self.body("ui") or []:
                t = norm_heading(line).rstrip(".")
                if t:
                    return t in ("ui: none", "none")
            return False
        for line, fenced in zip(self.lines, self.fenced):
            if not fenced and norm_heading(line).rstrip(".") == "ui: none":
                return True
        return False

    def has_ui_section(self):
        return self.find("ui") is not None or self.ui_none()

    def missing_sections(self):
        missing = [] if self.title() else ["title"]
        for name in self.SECTIONS:
            if name == "ui":
                if not self.has_ui_section():
                    missing.append("UI")
            elif not self.find(name):
                missing.append(name.capitalize() if name != "spec excerpts" else "Spec excerpts")
        return missing

    def listed_reviewers(self):
        """Keys listed in the Reviews section (any word), or None without the section."""
        body = self.body("reviews")
        if body is None:
            return None
        keys = []
        for line in body:
            s = line.strip()
            item = None
            if s.startswith("|"):
                cells = [c.strip() for c in s.strip("|").split("|")]
                if cells and not re.fullmatch(r":?-+:?", cells[0]):
                    item = cells[0]
            else:
                m = LIST_ITEM_RE.match(line)
                if m:
                    item = m.group(1)
            if not item:
                continue
            m = re.search(r"`([^`]+)`", item)
            word = m.group(1) if m else re.split(r"[\s:,(—–]+", item.strip("*_ "), maxsplit=1)[0]
            word = word.strip("*_:. ").lower()
            if word.endswith("-reviewer"):
                word = word[:-len("-reviewer")]
            if word and word not in ("key", "reviewer") and word not in keys:
                keys.append(word)
        return keys

    def reviewers(self):
        listed = self.listed_reviewers()
        if listed is None:
            keys = set(ALWAYS)
            if not self.ui_none():
                keys.add("ui")
        else:
            keys = set(listed) | set(ALWAYS)
        return [k for k in ROSTER if k in keys]

    def dependencies(self):
        body = self.body("dependencies") or []
        deps = []
        for line in body:
            for m in LOOSE_ID_RE.finditer(line.replace("`", " ")):
                if m.group(1) not in deps:
                    deps.append(m.group(1))
        return deps


def task_reviewers(tid):
    tf = TaskFile(tid)
    if not tf.exists:
        return list(ALWAYS)
    return tf.reviewers()


# -------------------------------------------------------------- rounds ----

def round_dirs(tid):
    """[(number, path)] sorted by round number."""
    tdir = TASKS / tid
    if not tdir.is_dir():
        return []
    out = []
    for d in tdir.iterdir():
        m = ROUND_RE.match(d.name)
        if m and d.is_dir():
            out.append((int(m.group(1)), d))
    return sorted(out)


def round_number(rid):
    m = ROUND_RE.match(rid)
    if not m:
        raise FactoryError(f"not a round-id: {rid}")
    return int(m.group(1))


def has_base(rdir):
    f = Path(rdir) / "base-commit"
    return f.is_file() and read_text(f).strip() != ""


def round_kind(rdir):
    rdir = Path(rdir)
    if (rdir / "human-review.md").is_file():
        return "rework"
    if (rdir / "approver-review.md").is_file():
        return "approver-rework"
    return "auto"


def latest_round(tid):
    rounds = round_dirs(tid)
    return f"round-{rounds[-1][0]}" if rounds else None


def previous_round(tid, rid):
    n = round_number(rid)
    prev = None
    for num, d in round_dirs(tid):
        if num < n and has_base(d):
            prev = f"round-{num}"
    return prev


def next_round(tid):
    rounds = round_dirs(tid)
    return f"round-{rounds[-1][0] + 1 if rounds else 1}"


def round_info(tid, rdir, reviewers):
    rdir = Path(rdir)
    reviews = {k: first_line_value(rdir / f"{k}-review.md", "review") for k in reviewers}
    return {
        "id": rdir.name,
        "kind": round_kind(rdir),
        "base": has_base(rdir),
        "build": first_line_value(rdir / "build-notes.md", "build-notes"),
        "reviews": reviews,
        "verdict": first_line_value(rdir / "verdict.md", "verdict"),
    }


def board_round(info):
    build = info["build"] if info["build"] in ("DONE", "BLOCKED") else "pending"
    if build == "BLOCKED":
        reviews = {}
    else:
        reviews = {k: (v if v in ("PASS", "FAIL") else "pending")
                   for k, v in info["reviews"].items()}
    verdict = info["verdict"] if info["verdict"] in ("APPROVE", "REWORK", "ESCALATE") else None
    return {"id": info["id"], "kind": info["kind"], "build": build,
            "reviews": reviews, "verdict": verdict}


def rework_section(path):
    """Lines of the '## Rework note' section (heading included), or None."""
    path = Path(path)
    if not path.is_file():
        return None
    lines = read_lines(path)
    for i, level, text in headings(lines):
        if norm_heading(text) == "rework note":
            idx = section_body(lines, i, level)
            return [lines[i]] + [lines[j] for j in idx]
    return None


def rework_note_text(path):
    section = rework_section(path)
    if not section or not any(line.strip() for line in section[1:]):
        return None
    body = section[1:]
    while body and not body[-1].strip():
        body.pop()
    return "\n".join([section[0]] + body) + "\n"


def count_reworks(tid, rid):
    n = round_number(rid)
    return sum(1 for num, d in round_dirs(tid)
               if num < n and first_line_value(d / "verdict.md", "verdict") == "REWORK")


# --------------------------------------------------------------- tasks ----

def derive_stage(tid, status, infos):
    if status == "Planned":
        return "planned"
    if status == "Complete":
        return "done"
    if status == "Abandoned":
        return "abandoned"
    if status == "Escalated":
        return "escalated"
    if status != "In progress":
        return "planned"
    if is_merged(tid):
        return "done"
    if not infos:
        return "building"
    r = infos[-1]
    if not r["base"] or r["build"] != "DONE":
        return "building"
    results = list(r["reviews"].values())
    if any(v not in ("PASS", "FAIL") for v in results):
        return "review"
    if r["verdict"] is None and r["id"] == "round-1" and r["kind"] == "auto" \
            and "FAIL" in results:
        return "building"
    return "approval"


REVIEW_FILE_ORDER = (["human-review.md", "approver-review.md", "build-notes.md"]
                     + [f"{k}-review.md" for k in ROSTER] + ["verdict.md"])


def task_files(tid):
    tdir = TASKS / tid
    files = ["task.md"] if (tdir / "task.md").is_file() else []
    for _, rdir in round_dirs(tid):
        mds = [p.name for p in rdir.iterdir() if p.is_file() and p.suffix.lower() == ".md"]
        known = [n for n in REVIEW_FILE_ORDER if n in mds]
        files += [f"{rdir.name}/{n}" for n in known + sorted(set(mds) - set(known))]
        for key in ROSTER:
            ev = rdir / key
            if ev.is_dir():
                files += sorted(p.relative_to(tdir).as_posix() for p in ev.rglob("*") if p.is_file())
    return files


def task_entry(tid, status):
    tf = TaskFile(tid)
    reviewers = tf.reviewers() if tf.exists else list(ALWAYS)
    infos = [round_info(tid, d, reviewers) for _, d in round_dirs(tid)]
    return {
        "id": tid,
        "title": tf.title() or tid,
        "branch": f"factory/{tid}",
        "status": status,
        "stage": derive_stage(tid, status, infos),
        "rounds": [board_round(i) for i in infos],
        "files": task_files(tid),
    }


# --------------------------------------------------------------- board ----

def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load_board():
    if not BOARD.is_file():
        return None
    try:
        board = json.loads(read_text(BOARD))
    except ValueError:
        return None
    return board if isinstance(board, dict) else None


def save_board(board):
    board["updated"] = now()
    ordered = {k: board[k] for k in ("updated", "loop", "agents", "tasks")}
    write_atomic(BOARD, json.dumps(ordered, indent=2, ensure_ascii=False) + "\n")


def derived_loop_state(rows):
    if preflight_state():
        return "awaiting-preflight", "Preflight stopped: see preflight.md"
    if rows is None:
        return "planning", "Planning"
    statuses = [s for _, s in rows]
    if "Escalated" in statuses:
        return "escalated", "A task is escalated"
    if statuses and all(s in ("Complete", "Abandoned") for s in statuses):
        return "complete", "Construction complete"
    if all(s == "Planned" for s in statuses) and not any(round_dirs(t) for t, _ in rows):
        return "awaiting-plan-review", "Plan ready for review"
    return "halted", "Board rebuilt from the files; /factory continue resumes"


def active_task(rows):
    for tid, status in rows or []:
        if status in ("In progress", "Escalated"):
            return tid
    return None


def rebuild_board(state=None, message=None):
    rows = backlog()
    old = load_board() or {}
    loop = dict(old.get("loop") or {})
    if state is None and not loop.get("state"):
        state, derived_message = derived_loop_state(rows)
        if message is None:
            message = derived_message
    if state is not None:
        loop["state"] = state
    if message is not None:
        loop["message"] = message
    loop.setdefault("message", "")
    ids = [t for t, _ in rows or []]
    if loop.get("current") not in ids:
        loop["current"] = active_task(rows)
    agents = old.get("agents") if isinstance(old.get("agents"), dict) else {}
    tasks = [task_entry(t, s) for t, s in rows or []]
    board = {"loop": {"state": loop["state"], "message": loop["message"],
                      "current": loop["current"]},
             "agents": agents, "tasks": tasks}
    save_board(board)
    return board


def cmd_board(args):
    if args.action == "rebuild":
        check_state(args.state)
        board = rebuild_board(args.state, args.message)
        loop = board["loop"]
        print(f"board: rebuilt ({len(board['tasks'])} tasks), loop {loop['state']}: {loop['message']}")
    elif args.action == "set":
        check_state(args.state)
        board = load_board() or rebuild_board()
        loop = board["loop"]
        if args.state is not None:
            loop["state"] = args.state
        if args.message is not None:
            loop["message"] = args.message
        if args.clear_current:
            loop["current"] = None
        if args.current is not None:
            require_task(args.current)
            loop["current"] = args.current
        for pair in args.agent or []:
            name, sep, value = pair.partition("=")
            if not sep or not name or not value:
                raise FactoryError(f"--agent needs NAME=ID, got {pair!r}")
            board.setdefault("agents", {})[name] = value
        if args.task is not None:
            entry = task_entry(args.task, status_of(args.task))
            order = [t for t, _ in backlog()]
            tasks = [t for t in board.get("tasks", []) if t.get("id") != args.task]
            tasks.append(entry)
            tasks.sort(key=lambda t: order.index(t["id"]) if t.get("id") in order else len(order))
            board["tasks"] = tasks
        board.setdefault("agents", {})
        save_board(board)
        print(f"board: updated, loop {loop['state']}: {loop.get('message', '')}")
    else:  # show
        board = load_board()
        out = {}
        if board is None:
            out["board"] = "missing" if not BOARD.exists() else "unreadable"
        else:
            out["updated"] = board.get("updated")
            out["loop"] = board.get("loop")
            out["agents"] = board.get("agents", {})
        if args.task is not None:
            entry = None
            for t in (board or {}).get("tasks", []):
                if t.get("id") == args.task:
                    entry = t
            if entry is None:
                out["task"] = {"id": args.task, "on-board": False}
            else:
                rounds = entry.get("rounds") or []
                out["task"] = {"id": entry["id"], "status": entry.get("status"),
                               "stage": entry.get("stage"),
                               "latest-round": rounds[-1] if rounds else None}
        print(json.dumps(out, indent=1, ensure_ascii=False))


def check_state(state):
    if state is not None and state not in LOOP_STATES:
        raise FactoryError(f"unknown loop state {state!r}; use one of {', '.join(LOOP_STATES)}")


# ----------------------------------------------------------- spec copy ----

INLINE_LINK_RE = re.compile(
    r"(!?)\[(?:[^\[\]\\]|\\.)*\]\([ \t]*(<[^<>\n]*>|[^\s()<>]+)"
    r"(?:[ \t]+(?:\"[^\"]*\"|'[^']*'|\([^)]*\)))?[ \t]*\)")
REF_DEF_RE = re.compile(r"^ {0,3}\[(?:[^\[\]\\]|\\.)+\]:[ \t]*(<[^<>\n]*>|\S+)")


def mask_code_spans(line):
    """The line with every inline code span replaced by NULs of the same length."""
    out = list(line)
    i = 0
    while i < len(line):
        if line[i] != "`":
            i += 1
            continue
        j = i
        while j < len(line) and line[j] == "`":
            j += 1
        run = j - i
        k = j
        close = -1
        while k < len(line):
            if line[k] == "`":
                m = k
                while m < len(line) and line[m] == "`":
                    m += 1
                if m - k == run:
                    close = m
                    break
                k = m
            else:
                k += 1
        if close < 0:
            i = j
            continue
        for p in range(i, close):
            out[p] = "\0"
        i = close
    return "".join(out)


def link_spans(line):
    """(start, end) of every link destination in a line, outside code spans (images excluded)."""
    masked = mask_code_spans(line)
    spans = []
    m = REF_DEF_RE.match(masked)
    if m:
        spans.append(m.span(1))
    for m in INLINE_LINK_RE.finditer(masked):
        if m.group(1) == "!":
            continue
        spans.append(m.span(2))
    return spans


def split_destination(dest):
    """(angle, path, suffix) where suffix is '?query#anchor' as written, or None if not relative Markdown."""
    angle = dest.startswith("<") and dest.endswith(">")
    raw = dest[1:-1] if angle else dest
    if not raw or raw.startswith(("#", "/", "\\")) or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", raw):
        return None
    cut = len(raw)
    for ch in "?#":
        pos = raw.find(ch)
        if pos >= 0:
            cut = min(cut, pos)
    path, suffix = raw[:cut], raw[cut:]
    if not urllib.parse.unquote(path).lower().endswith((".md", ".markdown")):
        return None
    return angle, path, suffix


def doc_links(doc_path, text):
    """[(line index, start, end, target abs path, angle, suffix)] for relative Markdown links."""
    lines = text.splitlines(keepends=True)
    mask = fence_mask([l.rstrip("\r\n") for l in lines])
    out = []
    for i, line in enumerate(lines):
        if mask[i]:
            continue
        body = line.rstrip("\r\n")
        for start, end in link_spans(body):
            parts = split_destination(body[start:end])
            if not parts:
                continue
            angle, path, suffix = parts
            target = os.path.normpath(os.path.join(os.path.dirname(doc_path),
                                                   urllib.parse.unquote(path)))
            if os.path.exists(target):
                target = os.path.realpath(target)
            out.append((i, start, end, target, angle, suffix))
    return out


def cmd_spec_copy(args):
    if args.spec is not None:
        source = os.path.abspath(os.path.expanduser(args.spec))
    else:
        if not SPEC_SOURCE.is_file():
            raise FactoryError("no spec given and factory/state/spec-source is missing")
        source = read_text(SPEC_SOURCE).strip()
        if not source:
            raise FactoryError("factory/state/spec-source is empty")
    if not os.path.isfile(source):
        raise FactoryError(f"spec not found: {source}")
    spec_real = os.path.realpath(source)

    # Pass 1: find every linked document, transitively.
    names = {spec_real: None}  # abs path -> copy name in spec/ (None for the spec)
    used = set()
    texts = {}
    links = {}
    missing = []
    queue = [spec_real]
    while queue:
        doc = queue.pop(0)
        texts[doc] = read_text(doc)
        links[doc] = doc_links(doc, texts[doc])
        for *_, target, _angle, _suffix in links[doc]:
            if target in names:
                continue
            if not os.path.isfile(target):
                entry = (target, doc)
                if entry not in missing:
                    missing.append(entry)
                continue
            base = os.path.basename(target)
            stem, ext = os.path.splitext(base)
            name, n = base, 1
            while name.lower() in used:
                n += 1
                name = f"{stem}-{n}{ext}"
            used.add(name.lower())
            names[target] = name
            queue.append(target)

    # Pass 2: write the copies with their links rewritten.
    def rewritten(doc):
        lines = texts[doc].splitlines(keepends=True)
        by_line = {}
        for i, start, end, target, angle, suffix in links[doc]:
            if target not in names:
                continue
            name = names[target]
            if name is None:
                new = "SPEC.md" if doc == spec_real else "../SPEC.md"
            else:
                new = name if doc != spec_real else f"spec/{name}"
            new = new if angle else urllib.parse.quote(new)
            new = new + suffix
            by_line.setdefault(i, []).append((start, end, f"<{new}>" if angle else new))
        for i, edits in by_line.items():
            line = lines[i]
            for start, end, new in sorted(edits, reverse=True):
                line = line[:start] + new + line[end:]
            lines[i] = line
        return "".join(lines)

    STATE.mkdir(parents=True, exist_ok=True)
    if SPEC_DIR.exists():
        shutil.rmtree(SPEC_DIR)
    SPEC_DIR.mkdir()
    write_atomic(SPEC, rewritten(spec_real))
    for doc, name in names.items():
        if name is not None:
            write_atomic(SPEC_DIR / name, rewritten(doc))
    write_atomic(SPEC_SOURCE, source + "\n")

    print(f"spec-source: {source}")
    print(f"copied: SPEC.md <- {source}")
    for doc, name in names.items():
        if name is not None:
            print(f"copied: spec/{name} <- {doc}")
    if missing:
        for target, doc in missing:
            print(f"missing: {target} (linked from {doc})")
    else:
        print("missing: none")


# ---------------------------------------------------------------- plan ----

def valid_id(tid):
    m = ID_RE.match(tid)
    if not m:
        return False
    digits = m.group(1)
    if int(digits) < 1:
        return False
    return len(digits) == 3 or not digits.startswith("0")


def cmd_plan_check(args):
    problems = []
    rows = backlog()
    if rows is None:
        print("plan: 1 problem")
        print("- factory/state/backlog.md is missing")
        return
    ids = [t for t, _ in rows]
    if not ids:
        problems.append("backlog.md lists no tasks")
    seen = set()
    for tid in ids:
        if tid in seen:
            problems.append(f"{tid} appears in backlog.md more than once")
        seen.add(tid)
    dirs = sorted(d.name for d in TASKS.iterdir() if d.is_dir()) if TASKS.is_dir() else []
    for d in dirs:
        if d not in seen:
            problems.append(f"task directory tasks/{d}/ isn't in backlog.md "
                            "(remove the directory too when removing a task)")
    highest = max(used_serials(), default=0)
    serials = {}
    for pos, (tid, status) in enumerate(rows):
        if status != "Planned":
            problems.append(f"{tid} has status {status!r}, not Planned")
        if not valid_id(tid):
            problems.append(f"{tid} isn't a valid task-id (serial of at least three digits, "
                            "then two or three lowercase words)")
        else:
            serial = int(ID_RE.match(tid).group(1))
            if serial <= highest:
                problems.append(f"{tid} reuses serial {serial:03d}; serials must be above "
                                f"{highest:03d}, the highest used in git")
            if serial in serials and serials[serial] != tid:
                problems.append(f"{tid} has the same serial as {serials[serial]}")
            serials.setdefault(serial, tid)
        tf = TaskFile(tid)
        if not tf.exists:
            problems.append(f"{tid} has no tasks/{tid}/task.md")
            continue
        for name in tf.missing_sections():
            problems.append(f"{tid}: task.md has no {name} section")
        listed = tf.listed_reviewers()
        if listed is not None:
            for key in ALWAYS:
                if key not in listed:
                    problems.append(f"{tid}: Reviews section doesn't list {key}")
            for key in listed:
                if key not in ROSTER:
                    problems.append(f"{tid}: Reviews section lists {key!r}, which isn't a "
                                    f"roster key ({', '.join(ROSTER)})")
        for dep in tf.dependencies():
            if dep == tid:
                problems.append(f"{tid} depends on itself")
            elif dep in ids:
                if ids.index(dep) > pos:
                    problems.append(f"{tid} depends on {dep}, which comes later in backlog.md")
            elif not is_merged(dep):
                problems.append(f"{tid} depends on {dep}, which is neither earlier in "
                                "backlog.md nor a merged task from an earlier run")
    if problems:
        print(f"plan: {len(problems)} problem{'s' if len(problems) != 1 else ''}")
        for p in problems:
            print(f"- {p}")
    else:
        print(f"plan: ok ({len(ids)} tasks)")


def cmd_max_serial(args):
    highest = max(used_serials(), default=0)
    print(f"highest-used: {highest:03d}" if highest else "highest-used: none")
    print(f"next: {highest + 1:03d}")


# -------------------------------------------------------------- rounds ----

def cmd_round(args):
    tid = args.task
    require_task(tid)
    if args.action == "latest":
        print(latest_round(tid) or "none")
    elif args.action == "next":
        print(next_round(tid))
    elif args.action == "previous":
        round_number(args.round)
        print(previous_round(tid, args.round) or "none")
    elif args.action == "kind":
        rdir = existing_round(tid, args.round)
        print(round_kind(rdir))
    else:  # state
        rid = args.round or latest_round(tid)
        if rid is None:
            print("round: none")
            return
        rdir = existing_round(tid, rid)
        info = round_info(tid, rdir, task_reviewers(tid))
        print(f"round: {rid}")
        print(f"kind: {info['kind']}")
        print(f"base-commit: {'yes' if info['base'] else 'no'}")
        print(f"build: {info['build'] or 'missing'}")
        reviews = " ".join(f"{k}={v or 'missing'}" for k, v in info["reviews"].items())
        print(f"reviews: {reviews}")
        print(f"verdict: {info['verdict'] or 'missing'}")
        print(f"previous: {previous_round(tid, rid) or 'none'}")


def existing_round(tid, rid):
    round_number(rid)
    rdir = TASKS / tid / rid
    if not rdir.is_dir():
        raise FactoryError(f"no round directory tasks/{tid}/{rid}/")
    return rdir


def cmd_rework_count(args):
    require_task(args.task)
    print(count_reworks(args.task, args.round))


def cmd_rework_note(args):
    require_task(args.task)
    rdir = existing_round(args.task, args.round)
    verdict = rdir / "verdict.md"
    if not verdict.is_file():
        raise FactoryError(f"no verdict.md in tasks/{args.task}/{args.round}/")
    note = rework_note_text(verdict)
    if note is None:
        print("rework-note: none")
        return
    if args.out:
        out = Path(args.out)
        if not out.is_absolute():
            out = Path.cwd() / out
        write_atomic(out, note)
        print(f"rework-note: yes, written to {args.out}")
    else:
        print("rework-note: yes")
        print(note, end="")


# --------------------------------------------------------------- tasks ----

def cmd_reviewers(args):
    require_task(args.task)
    if not TaskFile(args.task).exists:
        raise FactoryError(f"tasks/{args.task}/task.md is missing")
    print(" ".join(task_reviewers(args.task)))


def cmd_dependents(args):
    rows = backlog()
    if rows is None:
        raise FactoryError("factory/state/backlog.md is missing")
    found = [(t, s) for t, s in rows if t != args.task and args.task in TaskFile(t).dependencies()]
    if not found:
        print("none")
    for t, s in found:
        print(f"{t} {s}")


def cmd_merged(args):
    print("yes" if is_merged(args.task) else "no")


def cmd_tasks(args):
    rows = backlog()
    if rows is None:
        print("phase: preflight" if preflight_state() else "phase: none")
        return
    print("phase: backlog")
    for tid, status in rows:
        tf = TaskFile(tid)
        reviewers = tf.reviewers() if tf.exists else list(ALWAYS)
        infos = [round_info(tid, d, reviewers) for _, d in round_dirs(tid)]
        stage = derive_stage(tid, status, infos)
        print(f"task: {tid} | {status} | {stage} | {latest_round(tid) or '-'}")
    active = [(t, s) for t, s in rows if s in ("In progress", "Escalated")]
    print("active: " + (" ".join(f"{t} ({s})" for t, s in active) if active else "none"))
    nxt = next((t for t, s in rows if s == "Planned"), None)
    print(f"next: {nxt or 'none'}")


def cmd_backlog_set(args):
    status = norm_status(args.status)
    if status not in STATUSES:
        raise FactoryError(f"unknown status {args.status!r}; use one of {', '.join(STATUSES)}")
    rows = backlog_rows()
    if rows is None:
        raise FactoryError("factory/state/backlog.md is missing")
    index = next((i for i, t, _ in rows if t == args.task), None)
    if index is None:
        raise FactoryError(f"task {args.task} is not in backlog.md")
    raw = read_text(BACKLOG).splitlines(keepends=True)
    line = raw[index]
    body = line.rstrip("\r\n")
    ending = line[len(body):]
    parts = body.split("|")
    # parts[0] is the text before the first pipe; parts[1] the id cell, parts[2] the status.
    cell = parts[2]
    pad_left = cell[:len(cell) - len(cell.lstrip())] or " "
    pad_right = cell[len(cell.rstrip()):] or " "
    parts[2] = f"{pad_left}{status}{pad_right}"
    raw[index] = "|".join(parts) + ending
    write_atomic(BACKLOG, "".join(raw))
    msg = f"backlog: {args.task} is {status}"
    if BOARD.is_file():
        board = load_board() or rebuild_board()
        tasks = [t for t in board.get("tasks", []) if t.get("id") != args.task]
        tasks.append(task_entry(args.task, status))
        order = [t for t, _ in backlog()]
        tasks.sort(key=lambda t: order.index(t["id"]) if t.get("id") in order else len(order))
        board["tasks"] = tasks
        board.setdefault("agents", {})
        save_board(board)
        msg += " (board entry updated)"
    print(msg)


# --------------------------------------------------------- first lines ----

def cmd_first_line(args):
    path = Path(args.path)
    if not path.is_absolute():
        path = Path.cwd() / path
    kind = kind_of_file(path)
    if kind is None:
        raise FactoryError(f"{args.path} has no fixed first line")
    value = first_line_value(path, kind)
    if value is None:
        raise FactoryError(f"{args.path} is missing")
    if value == "invalid":
        raise FactoryError(f"{args.path} has an invalid first line {first_line_raw(path)!r}; "
                           f"allowed: {' | '.join(FIRST_LINES[kind])}")
    print(value)


def cmd_check_firstlines(args):
    files = []
    if args.task is None:
        if PREFLIGHT.is_file():
            files.append(PREFLIGHT)
        tids = [d.name for d in sorted(TASKS.iterdir()) if d.is_dir()] if TASKS.is_dir() else []
    else:
        require_task(args.task)
        tids = [args.task]
    for tid in tids:
        for _, rdir in round_dirs(tid):
            for name in ["build-notes.md"] + [f"{k}-review.md" for k in ROSTER] + ["verdict.md"]:
                if (rdir / name).is_file():
                    files.append(rdir / name)
    bad = [p for p in files if first_line_value(p, kind_of_file(p)) == "invalid"]
    if not bad:
        print(f"first-lines: ok ({len(files)} files)")
    for p in bad:
        print(f"invalid: {p.relative_to(ROOT).as_posix()}: {first_line_raw(p)!r}")


# ---------------------------------------------------------------- main ----

def parser():
    p = argparse.ArgumentParser(prog="factory.py", description="The factory script.")
    sub = p.add_subparsers(dest="command", required=True)

    b = sub.add_parser("board", help="read or change board.json")
    bsub = b.add_subparsers(dest="action", required=True)
    br = bsub.add_parser("rebuild", help="create or rebuild the board from the files")
    br.add_argument("--state")
    br.add_argument("--message")
    bs = bsub.add_parser("set", help="update the loop, agents and one task's entry")
    bs.add_argument("--state")
    bs.add_argument("--message")
    g = bs.add_mutually_exclusive_group()
    g.add_argument("--current")
    g.add_argument("--clear-current", action="store_true")
    bs.add_argument("--agent", action="append", metavar="NAME=ID")
    bs.add_argument("--task")
    bsh = bsub.add_parser("show", help="print the loop, the agents and one task")
    bsh.add_argument("--task")
    b.set_defaults(func=cmd_board)

    s = sub.add_parser("spec-copy", help="copy the spec and its linked documents")
    s.add_argument("spec", nargs="?")
    s.set_defaults(func=cmd_spec_copy)

    sub.add_parser("plan-check", help="mechanical plan checks").set_defaults(func=cmd_plan_check)
    sub.add_parser("max-serial", help="highest serial used in git").set_defaults(func=cmd_max_serial)

    r = sub.add_parser("round", help="round ids, kinds and state")
    rsub = r.add_subparsers(dest="action", required=True)
    for name in ("latest", "next"):
        rsub.add_parser(name).add_argument("task")
    for name in ("previous", "kind"):
        rp = rsub.add_parser(name)
        rp.add_argument("task")
        rp.add_argument("round")
    rs = rsub.add_parser("state")
    rs.add_argument("task")
    rs.add_argument("round", nargs="?")
    r.set_defaults(func=cmd_round)

    rc = sub.add_parser("rework-count", help="REWORK verdicts in rounds before ROUND")
    rc.add_argument("task")
    rc.add_argument("round")
    rc.set_defaults(func=cmd_rework_count)

    rn = sub.add_parser("rework-note", help="a verdict's '## Rework note' section")
    rn.add_argument("task")
    rn.add_argument("round")
    rn.add_argument("--out")
    rn.set_defaults(func=cmd_rework_note)

    for name, func, helptext in (("reviewers", cmd_reviewers, "a task's reviewer keys"),
                                 ("dependents", cmd_dependents, "tasks that depend on a task"),
                                 ("merged", cmd_merged, "whether a task is merged")):
        sp = sub.add_parser(name, help=helptext)
        sp.add_argument("task")
        sp.set_defaults(func=func)

    sub.add_parser("tasks", help="phase, tasks, active and next task").set_defaults(func=cmd_tasks)

    bl = sub.add_parser("backlog-set", help="change a task's status in backlog.md")
    bl.add_argument("task")
    bl.add_argument("status")
    bl.set_defaults(func=cmd_backlog_set)

    fl = sub.add_parser("first-line", help="the result word of a file's fixed first line")
    fl.add_argument("path")
    fl.set_defaults(func=cmd_first_line)

    cf = sub.add_parser("check-firstlines", help="check every fixed first line")
    cf.add_argument("task", nargs="?")
    cf.set_defaults(func=cmd_check_firstlines)
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        args.func(args)
    except FactoryError as e:
        print(f"factory.py: error: {e}", file=sys.stderr)
        return 1
    except OSError as e:
        print(f"factory.py: error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
