# Factory conventions

Shared by the main orchestrator (`SKILL.md`) and the task orchestrator
(`.claude/agents/task-orchestrator.md`). "You" means whichever of the two is
reading.

## Paths and names

| Thing | Value |
|---|---|
| State dir | `factory/state/` (git-ignored, including everything below) |
| Spec copy | `factory/state/SPEC.md`, linked docs in `factory/state/spec/` |
| Backlog | `factory/state/backlog.md` |
| Board | `factory/state/board.json` |
| Task dir | `factory/state/tasks/<task-id>/` (`T` below) |
| Round dir | `T/<round-id>/`, round ids `round-1`, `round-2`, … (`R` below) |
| Task branch | `factory/<task-id>` |

Agent types (the `subagent_type` you pass): `task-orchestrator`, `planner`,
`builder`, `code-reviewer`, `feature-reviewer`, `ui-reviewer`, `approver`.

## Ground rules

- **Never push.** Never run `git push`, `git reset --hard`, `git clean`,
  `git checkout -- <path>`, `git stash`, or delete anything under
  `factory/state/`. Never rewrite history.
- Only one task is ever `In progress` or `Escalated` at a time.
- **The state is not in git.** There is one copy of `factory/state/`, the same
  on every branch, so read and edit its files directly. Never commit anything
  under it, and never use `git add -f`. Git can't restore these files, so treat
  every overwrite as permanent.
- **Agent prompts contain only the task-id, the round-id and the paths to
  read.** Never paste reviews, specs or advice into a prompt; all context
  comes from files. The agent definitions in `.claude/agents/` carry the
  instructions.
- Wait for agents through their completion notifications. Never poll, never
  sleep, and never resume the builder while reviewers of its round are still
  running.
- **`board.json` has one writer at a time.** The main orchestrator writes it
  between tasks and never while a task orchestrator is running. A task
  orchestrator writes it from its spawn until it stops.
- Update `board.json` every time you take back control from a subagent and on
  every stage change (see "board.json").

## backlog.md

A Markdown table, one row per task, in implementation order:

```
| Task | Status |
|------|--------|
| 001-scaffold-crate | Complete |
| 002-apply-selected-theme | In progress |
```

Statuses: `Planned`, `In progress`, `Escalated`, `Complete`, `Abandoned`.
Change a status by editing only that row's status cell (use Edit).

## Checking changes to the state

`git status` can't see changes under `factory/state/`, so compare checksums
instead. Before spawning an agent, write a snapshot of the state, leaving out
the board and earlier snapshots:

```
find factory/state -type f ! -name .snapshot ! -name 'board.json*' -exec cksum {} + | LC_ALL=C sort -k3 > <snapshot>
```

After the agent finishes, run the same listing and compare:

```
find factory/state -type f ! -name .snapshot ! -name 'board.json*' -exec cksum {} + | LC_ALL=C sort -k3 | diff <snapshot> -
```

No output means nothing changed. Otherwise the last field of each `<` or `>`
line names a file that was changed, added or removed. `<snapshot>` is
`T/.snapshot` while a task is running, and `factory/state/.snapshot` for the
Planner. Don't print the snapshot itself.

## Rounds

The round **kind** follows from the files in its directory:
- `approver-review.md` present: `approver-rework` (started by a REWORK
  verdict).
- `human-review.md` present: `rework` (started by `/factory rework`).
- otherwise: `auto`.

The latest round is the one with the highest number. The next round id is the
latest number + 1. Create a round directory (`mkdir -p`) at the start of each
round, before any agent runs in it.

**Earlier REWORK verdicts** for a task: the number of `verdict.md` files in
its rounds (excluding the current one) whose first line is `VERDICT: REWORK`.

First lines are authoritative and must be exactly:
- `build-notes.md`: `STATUS: DONE` or `STATUS: BLOCKED`
- reviews: `VERDICT: PASS` or `VERDICT: FAIL`
- `verdict.md`: `VERDICT: APPROVE`, `VERDICT: REWORK` or `VERDICT: ESCALATE`

A file whose first line is none of these is an error: halt and say which file.

## board.json

Never leave the file half-edited, and don't print it. The board of a large
spec is tens of kilobytes, so don't rewrite it through the Write tool on every
update. Change only `updated`, `loop`, `agents` and the current task's entry,
with a short one-shot `python3 -I -c` command that loads the JSON, changes
those fields, writes `board.json.tmp` next to it and moves it into place with
`os.replace`. Use Write only to create the file or rebuild it. Don't leave
helper scripts behind.

```json
{
  "updated": "2026-10-27T10:51:00Z",
  "loop": { "state": "running", "message": "Building 002-apply-selected-theme",
            "current": "002-apply-selected-theme" },
  "agents": { "builder-002-apply-selected-theme": "<agent id from the Agent tool>" },
  "tasks": [
    { "id": "002-apply-selected-theme", "title": "Apply selected theme",
      "branch": "factory/002-apply-selected-theme",
      "status": "In progress", "stage": "review",
      "rounds": [
        { "id": "round-1", "kind": "auto", "build": "DONE",
          "reviews": { "code": "PASS", "feature": "FAIL", "ui": "PASS" },
          "verdict": null }
      ],
      "files": [ "task.md", "round-1/build-notes.md", "round-1/code-review.md" ] }
  ]
}
```

- `updated`: current UTC time (`date -u +%Y-%m-%dT%H:%M:%SZ`).
- `loop.state`: `planning`, `awaiting-plan-review`, `running`, `escalated`,
  `halted` (merge conflict, write violation, error; reason in `message`), or
  `complete`. `loop.current`: the task being worked on, or `null`.
- `agents`: builder agent ids, keyed by name. An id is only usable by the task
  orchestrator that spawned it. A later one spawns a fresh builder and replaces
  the id.
- One entry per backlog row, in backlog order. `title`: the `# ` heading of
  its `task.md`. `branch`: `factory/<task-id>`. `status`: the backlog status.
- `stage`: `planned` (Planned), `building` (builder running), `review`
  (reviewers running or done), `approval` (Approver running), `escalated`
  (Escalated, or waiting for a human), `done` (Complete), `abandoned`
  (Abandoned). Between steps, use the stage of the step about to run.
- `rounds`: one entry per round directory. `kind` as above. `build`:
  `pending` until `build-notes.md` exists, then `DONE` or `BLOCKED`.
  `reviews.code|feature|ui`: absent key or `null` before the reviewers are
  spawned, `pending` while running, then `PASS` or `FAIL`. `verdict`:
  `null`, `APPROVE`, `REWORK` or `ESCALATE`.
- `files`: every `.md` file in the task directory plus every file under any
  `round-*/ui/`, relative to the task directory, `task.md` first, then
  rounds in order.

## Merge

On main:
1. `git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>`.
2. If it conflicts: don't resolve or abort anything. Set `loop.state`
   `halted`, message `Merge conflict in <task-id>: <conflicting files>`, and
   halt. The user resolves the conflicts, completes the merge (`git commit`,
   keeping the message `factory: merge <task-id>`), and runs
   `/factory continue`.

## Halting

To halt: update `board.json` with the loop state and message, then stop. The
main orchestrator reports to the user: what happened, the current task, round
and branch, and the command(s) they can run next. States:
`awaiting-plan-review` (after planning), `escalated`, `halted`, `complete`.
