#!/usr/bin/env python3
"""The factory's board script: the only writer of factory/state/board.json.

Run it from the repository root, with only the standard library:

    python3 -I .claude/skills/factory/board.py <command> ...

Commands:
    rebuild [--state S] [--message M] [--current ID | --no-current]
        Create the board, or rebuild it, from backlog.md and the task files.
        Keeps `agents`, and `loop` unless overridden. In the preflight state
        (preflight.md exists, backlog.md doesn't) loop.state is always
        `awaiting-preflight` and there are no tasks.
    loop [--state S] [--message M] [--current ID | --no-current]
        Update `loop`. Creates the board first if it is missing.
    agent NAME [AGENT_ID] [--remove]
        Record (or remove) an agent id under `agents`, e.g. builder-<task-id>.
    task ID
        Work out one task's entry from its files, write it, and print it as
        `show ID` does.
    show [ID]
        Print `updated`, the loop and the agents, plus, with an ID, that
        task's status, stage, round count, latest round and reviewers.
        Read-only.

Every write goes to board.json.tmp and is moved into place with os.replace.
Exit status: 0 on success. 2 for a usage error or an unknown task. 3 when a
state file is invalid (a first line that isn't one of the allowed ones, or a
backlog row with an unknown status); the board is still written, and stderr
names every invalid file, so the orchestrator halts with that message.
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone

STATE = os.path.join("factory", "state")
BOARD = os.path.join(STATE, "board.json")
BACKLOG = os.path.join(STATE, "backlog.md")
PREFLIGHT = os.path.join(STATE, "preflight.md")
TASKS = os.path.join(STATE, "tasks")
RUN_FILES = ["SPEC.md", "preflight.md", "stack.md", "backlog.md"]

# The reviewer roster, in roster order. Keep in step with factory.md.
ROSTER = ["code", "feature", "security", "privacy", "api", "ui"]

LOOP_STATES = [
    "planning", "awaiting-preflight", "awaiting-plan-review", "running",
    "escalated", "halted", "complete",
]
STATUSES = ["Planned", "In progress", "Escalated", "Complete", "Abandoned"]
STAGE_BY_STATUS = {
    "Planned": "planned", "Escalated": "escalated",
    "Complete": "done", "Abandoned": "abandoned",
}

BUILD_LINES = {"STATUS: DONE": "DONE", "STATUS: BLOCKED": "BLOCKED"}
REVIEW_LINES = {"VERDICT: PASS": "PASS", "VERDICT: FAIL": "FAIL"}
VERDICT_LINES = {
    "VERDICT: APPROVE": "APPROVE", "VERDICT: REWORK": "REWORK",
    "VERDICT: ESCALATE": "ESCALATE",
}
REWORK_LIMIT = 2

ROUND_RE = re.compile(r"^round-([1-9][0-9]*)$")
KEY_LINE_RE = re.compile(r"^\s*[-*+]\s+`?([a-z][a-z0-9-]*)`?(?=[\s:,.;)—-]|$)")
TASK_ID_RE = re.compile(r"\b([0-9]{3,}-[a-z0-9]+(?:-[a-z0-9]+)*)\b")

errors = []


def fail_usage(message):
    print(f"board.py: {message}", file=sys.stderr)
    sys.exit(2)


def now():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def first_line(path):
    """The file's first line, stripped; None if the file doesn't exist."""
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            line = handle.readline()
    except FileNotFoundError:
        return None
    return line.lstrip("﻿").strip()


def classify(path, allowed):
    """Map a file's first line through `allowed`.

    A missing or still-empty file is None (not there yet). Any other line
    that isn't allowed is recorded as an error and also returns None.
    """
    line = first_line(path)
    if line is None or line == "":
        return None
    if line in allowed:
        return allowed[line]
    errors.append(f"invalid first line in {path}: {line!r}")
    return None


def read_text(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            return handle.read()
    except FileNotFoundError:
        return None


def sections(text):
    """Split Markdown into {lowercased level-1/2 heading: body lines}."""
    found = {}
    current = None
    in_fence = False
    for line in text.splitlines():
        if line.lstrip().startswith(("```", "~~~")):
            in_fence = not in_fence
        heading = None if in_fence else re.match(r"^(#{1,2})\s+(.*?)\s*#*\s*$", line)
        if heading:
            current = heading.group(2).strip().lower()
            found.setdefault(current, [])
            continue
        if current is not None:
            found[current].append(line)
    return found


def task_title(text, task_id):
    for line in text.splitlines():
        match = re.match(r"^#\s+(.*?)\s*$", line)
        if match:
            title = match.group(1)
            title = re.sub(r"^`?" + re.escape(task_id) + r"`?\s*[:—-]?\s*", "", title)
            return title or task_id
    return task_id


def ui_is_none(found):
    if "ui: none" in found:
        return True
    body = found.get("ui")
    if body is None:
        return False
    for line in body:
        stripped = line.strip().strip("`*_").strip()
        if stripped:
            return stripped.lower() in ("ui: none", "none")
    return False


def reviewers_from(text):
    """The task's reviewer keys, in roster order (see "The reviewer roster")."""
    if text is None:
        return ["code", "feature", "ui"]
    found = sections(text)
    body = found.get("reviews")
    if body is None:
        keys = {"code", "feature"}
        if not ui_is_none(found):
            keys.add("ui")
    else:
        keys = set()
        for line in body:
            match = KEY_LINE_RE.match(line)
            if match and match.group(1) in ROSTER:
                keys.add(match.group(1))
    return [key for key in ROSTER if key in keys]


def read_backlog():
    """[(task-id, status)] in backlog order, or None without a backlog."""
    text = read_text(BACKLOG)
    if text is None:
        return None
    rows = []
    canonical = {status.lower(): status for status in STATUSES}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) < 2:
            continue
        task_id = cells[0].strip("`").strip()
        status = cells[1].strip("`*_").strip()
        if not re.match(r"^[0-9]{3,}-", task_id):
            continue  # the header row and the separator row
        if status.lower() not in canonical:
            errors.append(f"unknown status in {BACKLOG} for {task_id}: {status!r}")
            rows.append((task_id, status))
            continue
        rows.append((task_id, canonical[status.lower()]))
    return rows


def round_dirs(task_dir):
    """[(number, name)] of the task's round directories, in number order."""
    try:
        names = os.listdir(task_dir)
    except FileNotFoundError:
        return []
    rounds = []
    for name in names:
        match = ROUND_RE.match(name)
        if match and os.path.isdir(os.path.join(task_dir, name)):
            rounds.append((int(match.group(1)), name))
    return sorted(rounds)


def has_rework_note(path):
    text = read_text(path) or ""
    return re.search(r"^##\s+Rework note\s*$", text, re.MULTILINE | re.IGNORECASE) is not None


def round_entries(task_dir, reviewers):
    """The `rounds` list, plus the facts the stage needs about each round."""
    entries = []
    reworks_before = 0  # REWORK verdicts in earlier rounds
    previous = None  # facts about the previous round
    for number, name in round_dirs(task_dir):
        path = os.path.join(task_dir, name)
        build = classify(os.path.join(path, "build-notes.md"), BUILD_LINES)
        reviews = {}
        for key in reviewers:
            value = classify(os.path.join(path, f"{key}-review.md"), REVIEW_LINES)
            reviews[key] = value or "pending"
        verdict_path = os.path.join(path, "verdict.md")
        verdict = classify(verdict_path, VERDICT_LINES)

        if os.path.exists(os.path.join(path, "human-review.md")):
            kind = "rework"
        elif os.path.exists(os.path.join(path, "approver-review.md")):
            kind = "approver-rework"
        elif number == 1 or previous is None:
            kind = "auto"
        elif previous["usable_rework"]:
            kind = "approver-rework"
        elif previous["escalated"]:
            kind = "rework"
        else:
            kind = "auto"

        usable_rework = (
            verdict == "REWORK"
            and reworks_before < REWORK_LIMIT
            and has_rework_note(verdict_path)
        )
        escalated = build == "BLOCKED" or verdict == "ESCALATE" or (
            verdict == "REWORK" and not usable_rework)
        previous = {"usable_rework": usable_rework, "escalated": escalated}
        if verdict == "REWORK":
            reworks_before += 1

        entries.append({
            "id": name, "kind": kind, "build": build or "pending",
            "reviews": reviews, "verdict": verdict,
        })
    return entries


def stage_for(status, rounds):
    if status in STAGE_BY_STATUS:
        return STAGE_BY_STATUS[status]
    if not rounds:
        return "building"
    latest = rounds[-1]
    if latest["build"] != "DONE":
        return "building"  # still building, or BLOCKED and about to escalate
    values = latest["reviews"].values()
    if "pending" in values:
        return "review"
    if latest["verdict"] is None and latest["id"] == "round-1" and "FAIL" in values:
        return "review"  # the fix round is next, not the Approver
    return "approval"


def task_files(task_dir, rounds, reviewers):
    files = []
    if os.path.exists(os.path.join(task_dir, "task.md")):
        files.append("task.md")
    order = (["human-review.md", "approver-review.md", "build-notes.md"]
             + [f"{key}-review.md" for key in ROSTER] + ["verdict.md"])
    for entry in rounds:
        path = os.path.join(task_dir, entry["id"])
        names = sorted(os.listdir(path))
        markdown = [n for n in names if n.endswith(".md") and os.path.isfile(os.path.join(path, n))]
        markdown.sort(key=lambda n: (order.index(n) if n in order else len(order), n))
        files += [f"{entry['id']}/{n}" for n in markdown]
        for key in ROSTER:
            evidence = os.path.join(path, key)
            if not os.path.isdir(evidence):
                continue
            for root, dirs, filenames in os.walk(evidence):
                dirs.sort()
                for filename in sorted(filenames):
                    full = os.path.join(root, filename)
                    files.append(os.path.relpath(full, task_dir).replace(os.sep, "/"))
    return files


def task_entry(task_id, status):
    task_dir = os.path.join(TASKS, task_id)
    text = read_text(os.path.join(task_dir, "task.md"))
    reviewers = reviewers_from(text)
    rounds = round_entries(task_dir, reviewers)
    return {
        "id": task_id,
        "title": task_title(text, task_id) if text else task_id,
        "branch": f"factory/{task_id}",
        "status": status,
        "stage": stage_for(status, rounds),
        "rounds": rounds,
        "files": task_files(task_dir, rounds, reviewers),
    }


def in_preflight_state():
    return os.path.exists(PREFLIGHT) and not os.path.exists(BACKLOG)


def run_files():
    return [name for name in RUN_FILES if os.path.isfile(os.path.join(STATE, name))]


def load_board():
    """The board as a dict, or None if it is missing or unreadable."""
    try:
        with open(BOARD, encoding="utf-8") as handle:
            board = json.load(handle)
    except (FileNotFoundError, json.JSONDecodeError, UnicodeDecodeError):
        return None
    if not isinstance(board, dict):
        return None
    return board


def save_board(board):
    board["updated"] = now()
    board["files"] = run_files()
    ordered = {key: board[key] for key in ("updated", "loop", "agents", "files", "tasks")}
    tmp = BOARD + ".tmp"
    with open(tmp, "w", encoding="utf-8") as handle:
        json.dump(ordered, handle, indent=2)
        handle.write("\n")
    os.replace(tmp, BOARD)


def derive_loop(tasks, old_loop):
    """loop.state when there is no board to keep it from and no --state."""
    statuses = [task["status"] for task in tasks]
    if in_preflight_state():
        return {"state": "awaiting-preflight", "message": preflight_message()}
    if not os.path.exists(BACKLOG):
        return {"state": "planning", "message": "Planning"}
    if "Escalated" in statuses:
        return {"state": "escalated", "message": "A task is escalated"}
    if tasks and all(status in ("Complete", "Abandoned") for status in statuses):
        return {"state": "complete", "message": "Construction is complete"}
    if tasks and all(status == "Planned" for status in statuses):
        return {"state": "awaiting-plan-review", "message": "Awaiting plan review"}
    if old_loop:
        return old_loop
    return {"state": "halted", "message": "Board rebuilt from the state files; run /factory continue"}


def preflight_message():
    line = first_line(PREFLIGHT) or "PREFLIGHT"
    return f"{line}: see preflight.md"


def current_task(tasks):
    for task in tasks:
        if task["status"] in ("In progress", "Escalated"):
            return task["id"]
    return None


def build_board(old):
    rows = [] if in_preflight_state() else (read_backlog() or [])
    tasks = [task_entry(task_id, status) for task_id, status in rows]
    old_loop = old.get("loop") if old and isinstance(old.get("loop"), dict) else None
    if in_preflight_state():
        kept = old_loop and old_loop.get("state") == "awaiting-preflight" and old_loop.get("message")
        loop = {"state": "awaiting-preflight", "message": kept or preflight_message()}
    elif old_loop and old_loop.get("state") in LOOP_STATES:
        loop = {"state": old_loop["state"], "message": old_loop.get("message", "")}
    else:
        loop = derive_loop(tasks, None)
    current = (old_loop or {}).get("current")
    ids = [task["id"] for task in tasks]
    loop["current"] = current if current in ids else current_task(tasks)
    agents = old.get("agents") if old and isinstance(old.get("agents"), dict) else {}
    return {"loop": loop, "agents": agents, "tasks": tasks}


def apply_loop_args(board, args):
    loop = board["loop"]
    if getattr(args, "state", None):
        if in_preflight_state() and args.command == "rebuild":
            pass  # rebuilding in the preflight state is always awaiting-preflight
        else:
            loop["state"] = args.state
    if getattr(args, "message", None) is not None:
        loop["message"] = args.message
    if getattr(args, "no_current", False):
        loop["current"] = None
    elif getattr(args, "current", None):
        if args.current not in [task["id"] for task in board["tasks"]]:
            fail_usage(f"unknown task {args.current!r}")
        loop["current"] = args.current
    board["loop"] = {"state": loop["state"], "message": loop.get("message", ""),
                     "current": loop.get("current")}


def board_or_new():
    old = load_board()
    if old is None or not isinstance(old.get("tasks"), list):
        return build_board(old)
    old.setdefault("agents", {})
    loop = old.get("loop") if isinstance(old.get("loop"), dict) else None
    if loop is None or loop.get("state") not in LOOP_STATES:
        old["loop"] = build_board(old)["loop"]
    return old


def show(board, task_id=None):
    out = {"updated": board.get("updated"), "loop": board["loop"], "agents": board["agents"]}
    if task_id is not None:
        task = next((t for t in board["tasks"] if t["id"] == task_id), None)
        if task is None:
            fail_usage(f"task {task_id!r} is not on the board")
        text = read_text(os.path.join(TASKS, task_id, "task.md"))
        out["task"] = {
            "id": task["id"], "title": task["title"], "branch": task["branch"],
            "status": task["status"], "stage": task["stage"],
            "rounds": len(task["rounds"]),
            "latest": task["rounds"][-1] if task["rounds"] else None,
            "reviewers": reviewers_from(text),
        }
    print(json.dumps(out, indent=2))


def finish():
    for message in errors:
        print(f"board.py: {message}", file=sys.stderr)
    sys.exit(3 if errors else 0)


def main():
    parser = argparse.ArgumentParser(prog="board.py", description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    def loop_options(sub):
        sub.add_argument("--state", choices=LOOP_STATES)
        sub.add_argument("--message")
        group = sub.add_mutually_exclusive_group()
        group.add_argument("--current", metavar="ID")
        group.add_argument("--no-current", action="store_true")

    loop_options(commands.add_parser("rebuild", help="create or rebuild the board"))
    loop_options(commands.add_parser("loop", help="update the loop"))
    agent = commands.add_parser("agent", help="record an agent id")
    agent.add_argument("name")
    agent.add_argument("agent_id", nargs="?")
    agent.add_argument("--remove", action="store_true")
    task = commands.add_parser("task", help="update one task's entry and print it")
    task.add_argument("id")
    show_parser = commands.add_parser("show", help="print the loop, agents and one task")
    show_parser.add_argument("id", nargs="?")
    args = parser.parse_args()

    if not os.path.isdir("factory"):
        fail_usage("run from the repository root (no factory/ directory here)")

    if args.command == "show":
        board = load_board()
        if board is None:
            fail_usage(f"{BOARD} is missing or unreadable; rebuild it")
        board.setdefault("agents", {})
        board.setdefault("tasks", [])
        show(board, args.id)
        return

    os.makedirs(STATE, exist_ok=True)

    if args.command == "rebuild":
        board = build_board(load_board())
        apply_loop_args(board, args)
        save_board(board)
    elif args.command == "loop":
        board = board_or_new()
        apply_loop_args(board, args)
        save_board(board)
    elif args.command == "agent":
        board = board_or_new()
        if args.remove:
            board["agents"].pop(args.name, None)
        elif not args.agent_id:
            fail_usage("agent needs an AGENT_ID, or --remove")
        else:
            board["agents"][args.name] = args.agent_id
        save_board(board)
    elif args.command == "task":
        rows = read_backlog() or []
        statuses = dict(rows)
        if args.id not in statuses:
            fail_usage(f"task {args.id!r} is not in {BACKLOG}")
        board = board_or_new()
        entry = task_entry(args.id, statuses[args.id])
        tasks = [t for t in board["tasks"] if t.get("id") != args.id]
        position = [task_id for task_id, _ in rows].index(args.id)
        before = {task_id for task_id, _ in rows[:position]}
        index = sum(1 for t in tasks if t.get("id") in before)
        tasks.insert(index, entry)
        board["tasks"] = tasks
        save_board(board)
        show(board, args.id)
    finish()


if __name__ == "__main__":
    main()
