# Factory conventions

Shared by both orchestrator levels: the main orchestrator (the session running
`/factory`, see `SKILL.md`) and the task orchestrator
(`.claude/agents/task-orchestrator.md`). Read this file before anything else.
Where this file gives a command, use it as written.

## Names and paths

Run every command from the repository root, with paths relative to it.

| Name | Path |
|---|---|
| state | `factory/state/` |
| spec copy | `factory/state/SPEC.md`, linked documents in `factory/state/spec/` |
| spec source | `factory/state/spec-source` (one line: the absolute path of the original spec) |
| preflight | `factory/state/preflight.md` |
| stack profile | `factory/state/stack.md` |
| backlog | `factory/state/backlog.md` |
| board | `factory/state/board.json` |
| Planner snapshot | `factory/state/.snapshot` |
| `T` | the task directory, `factory/state/tasks/<task-id>` |
| task snapshot | `T/.snapshot` |
| `R` | the round directory, `T/<round-id>` |
| round files | `R/base-commit`, `R/build-notes.md`, `R/<key>-review.md`, `R/<key>/` (evidence), `R/verdict.md`, `R/approver-review.md`, `R/human-review.md` |
| task branch | `factory/<task-id>` |
| `B` | `python3 -I .claude/skills/factory/board.py` (the board script) |

Agent types, used as the Agent tool's `subagent_type`:

| Agent | Spawned by |
|---|---|
| `planner` | main orchestrator |
| `task-orchestrator` | main orchestrator |
| `builder` | task orchestrator |
| `code-reviewer`, `feature-reviewer`, `security-reviewer`, `privacy-reviewer`, `api-reviewer`, `ui-reviewer` (`<key>-reviewer`) | task orchestrator |
| `approver` | task orchestrator |

Task ids are `<serial>-<word>-<word>[-<word>]`: a serial of at least three
digits, then two or three words of lowercase letters and digits, joined by
dashes (`^[0-9]{3,}-[a-z0-9]+(-[a-z0-9]+){1,2}$`). Round ids are
`round-<n>`, with `n` counting up from 1.

## Ground rules

- **Never push, never rewrite history.** The factory never runs `git push` in
  any form. Never run `git reset --hard`, `git rebase`, `git commit --amend`,
  `git filter-branch`, `git clean`, `git stash`, `git checkout -- <path>`,
  `git restore`, `git merge --abort`, any `--force` variant, or anything else
  that discards commits or uncommitted changes. `git branch -D` is allowed
  only as the Merge step's last resort for a merged task. Never delete,
  revert or "tidy" files an agent changed, even after a violation.
- **One active task.** Only one task is ever `In progress` or `Escalated` at
  a time. The main orchestrator never runs two task orchestrators at once.
- **The state never goes into git.** Nothing under `factory/state/` is ever
  staged or committed. Never `git add -f` anything.
- **Agent prompts contain only ids and paths,** in the exact templates the
  orchestrator files give, plus the fixed messages defined there. All other
  context comes from files.
- **Wait, don't poll.** Spawn every agent with `run_in_background: true`,
  then end the turn with the single word `WAITING`. You are woken when an
  agent reports. Never sleep, poll, read an agent's output file, or check on
  a running agent.
- **One writer of `board.json`.** The task orchestrator while it runs, the
  main orchestrator otherwise. Every change goes through `B`. Never print
  the whole board, edit it with another tool, or write helper scripts.
- **No factory edits, no installs.** Never create, edit or regenerate the
  factory files (`.claude/skills/factory/`, `.claude/agents/`,
  `factory/dashboard/`, `factory/factory.md`, `factory/floorplan.md`), and
  never install anything.
- **Don't do an agent's work.** The orchestrators never write code, plans,
  reviews or verdicts themselves.
- **Exit statuses.** A command that finds, lists or counts things exits 0
  when it succeeds, including when it finds nothing, so a non-zero exit
  always means an error. The few checks whose exit status is their answer
  are labelled *(exit status is the answer)*.

## The backlog

`backlog.md` is the execution plan: one row per task, in implementation
order, exactly in this format:

```markdown
# Backlog

| Task | Status |
|---|---|
| 015-scaffold-project | Planned |
| 016-list-sessions | Planned |
```

The task cell is the bare task-id. Statuses are `Planned`, `In progress`,
`Escalated`, `Complete` and `Abandoned`. Only the orchestrators and the
Planner write this file. It isn't in git, so it can be changed on any branch.

List the rows (prints `<task-id><TAB><status>` per task):

```bash
awk -F'|' '/^\| *[0-9][0-9][0-9]/ { gsub(/^[ `]+|[ `]+$/, "", $2); gsub(/^ +| +$/, "", $3); print $2 "\t" $3 }' factory/state/backlog.md
```

Change a task's status, then confirm it changed:

```bash
sed -i -E 's/^\| *`?<task-id>`? *\|[^|]*\|$/| <task-id> | <Status> |/' factory/state/backlog.md
grep -Fxq '| <task-id> | <Status> |' factory/state/backlog.md   # (exit status is the answer)
```

If the `grep` fails, halt (see "Halting"). After every status change, run
`B task <task-id>`.

### task.md

The Planner writes `T/task.md` in exactly this shape. The plan check, `B` and
the task orchestrator read these headings:

```markdown
# <task-id>: <Title>

## Goal
## Spec excerpts
## Acceptance criteria
## UI
## Reviews
## Depends on
```

- `## UI` holds the single line `UI: none` when the task changes no UI.
- `## Reviews` has one list item per reviewer, the key in backticks first:
  `` - `security`: adds a dependency ``.
- `## Depends on` lists task-ids in backticks, one per item, or the single
  line `none`.

## The reviewer roster

In roster order (the dashboard's chip order, and the Approver's priority
order after security and data protection):

| Key | Agent | Review | Runs on |
|---|---|---|---|
| `code` | `code-reviewer` | `code-review.md` | every task |
| `feature` | `feature-reviewer` | `feature-review.md` | every task |
| `security` | `security-reviewer` | `security-review.md` | tasks that list it |
| `privacy` | `privacy-reviewer` | `privacy-review.md` | tasks that list it |
| `api` | `api-reviewer` | `api-review.md` | tasks that list it |
| `ui` | `ui-reviewer` | `ui-review.md` | tasks whose `UI` section isn't `UI: none` |

Each reviewer writes `R/<key>-review.md` and may put evidence in `R/<key>/`.

A task's reviewers are the roster keys in its `## Reviews` list, in roster
order whatever order the list uses. A `task.md` without a `## Reviews`
section runs `code`, `feature`, and `ui` unless its `## UI` section is
`UI: none`. Don't parse this yourself: `B show <task-id>` and
`B task <task-id>` print the list as `task.reviewers`.

## Rounds

- **Latest and next round.** The latest round is the round directory with the
  highest number (compare numerically: `round-10` comes after `round-9`). The
  next round is that number plus one, or `round-1` when there are none.

  ```bash
  find factory/state/tasks/<task-id> -mindepth 1 -maxdepth 1 -type d -name 'round-*' | sed 's/.*round-//' | sort -n | tail -n 1   # empty if no rounds
  ```

- **Kind,** from the round's files (the first rule that applies):
  1. `human-review.md` exists: `rework` (a human rework round).
  2. `approver-review.md` exists: `approver-rework`.
  3. It is `round-1`: `auto`.
  4. The previous round's verdict is a *usable REWORK* (below):
     `approver-rework`.
  5. The previous round ended in an escalation (its build was BLOCKED, its
     verdict is ESCALATE, or its verdict is a REWORK that isn't usable):
     `rework`.
  6. Otherwise: `auto` (the fix round after round 1).

  Rules 4 and 5 only matter for a round directory whose note isn't written
  yet.

- **Creating a round.** In this order, so that a round directory with a
  `base-commit` is complete:
  1. `mkdir -p R`
  2. Write the round's note, if its kind has one: `approver-review.md`
     (copied from the previous round's verdict, below) or `human-review.md`.
  3. Write `base-commit`, a single line with the full commit hash the round
     starts from: for `round-1` the commit `main` points at
     (`git rev-parse main > R/base-commit`), for later rounds the `HEAD` of
     the task branch (`git rev-parse HEAD > R/base-commit`, run on the task
     branch).

  No agent is spawned for a round before its `base-commit` exists.

- **Interrupted round creation.** On re-entry, a round directory without
  `base-commit` was interrupted while it was being created. By its kind:
  - `approver-rework`: copy the note again from the previous round's
    verdict, then write `base-commit`;
  - `rework` without `human-review.md`: halt, with the message
    `<task-id>: <round-id> was interrupted before its rework note was saved; run /factory rework <task-id> <note> again`;
  - anything else: write `base-commit`.

  Then the work goes on.

- **REWORK verdicts.** Count the task's REWORK verdicts, including the
  latest round's:

  ```bash
  find factory/state/tasks/<task-id> -path '*/round-*/verdict.md' -exec head -q -n 1 {} + | awk '$0 == "VERDICT: REWORK" { n++ } END { print n + 0 }'
  ```

  A REWORK verdict is *usable* when the count, including it, is at most 2,
  and the verdict has a rework note. A REWORK that isn't usable is handled
  like an ESCALATE. Human rework rounds don't reset the count.

- **The rework note** is the verdict's `## Rework note` section, from that
  heading to the next `## ` heading or the end of the file. Extract it (with
  its heading) into the new round:

  ```bash
  awk '/^## Rework note[[:space:]]*$/ { f = 1; print; next } f && /^## / { exit } f' <previous R>/verdict.md > R/approver-review.md
  ```

  The verdict has a rework note when `approver-review.md` then has a
  non-blank line after the heading:
  `tail -n +2 R/approver-review.md | grep -q '[^[:space:]]'`
  *(exit status is the answer)*. If it hasn't, the REWORK isn't usable:
  remove nothing, and handle it like an ESCALATE (the stray, empty
  `approver-review.md` in the unfinished round directory is harmless,
  because that round has no `base-commit` and is never started).

  Check for the note *before* creating the round directory when you can:
  `awk '/^## Rework note[[:space:]]*$/ { f = 1; next } f && /^## / { exit } f && /[^[:space:]]/ { n++ } END { print n + 0 }' <previous R>/verdict.md`
  prints the number of non-blank note lines (0 means no note).

- **First lines.** Read a file's first line with `head -n 1 <file>`. These
  are the only allowed first lines:

  | File | First line |
  |---|---|
  | `<key>-review.md` | `VERDICT: PASS` or `VERDICT: FAIL` |
  | `verdict.md` | `VERDICT: APPROVE`, `VERDICT: REWORK` or `VERDICT: ESCALATE` |
  | `build-notes.md` | `STATUS: DONE` or `STATUS: BLOCKED` |
  | `preflight.md` | `PREFLIGHT: READY`, `PREFLIGHT: TOOLS` or `PREFLIGHT: BLOCKED` |

  Any other first line is an error that halts the loop, with the message
  `Invalid first line in <path>: <line>`. So is an agent that finishes
  without writing the file it owns.

## The preflight state

The run is in the preflight state when `preflight.md` exists and
`backlog.md` doesn't:

```bash
test -f factory/state/preflight.md && test ! -f factory/state/backlog.md   # (exit status is the answer)
```

### Copying the spec

`/factory start` copies the spec, and `/factory continue` re-copies it in the
preflight state, the same way. Linked documents are copied flat into
`factory/state/spec/`.

1. Find the source. For `start` it is the command's `<spec>` argument; expand
   a leading `~` to `$HOME`, make it absolute
   (`realpath -- "<path>"`), check it is a file (`test -f`), and write that
   path to `factory/state/spec-source`
   (`printf '%s\n' "<absolute path>" > factory/state/spec-source`). For a
   re-copy, read it from `spec-source`. If it is not a file, stop and tell
   the user (`start` refuses; `continue` halts and asks the user to restore
   the spec or fix `spec-source`).
2. For a re-copy only, remove the previous copies:
   `rm -rf factory/state/spec factory/state/SPEC.md`. These are the factory's
   own copies; nothing else under the state is ever deleted.
3. `cp -- "<source>" factory/state/SPEC.md` and
   `mkdir -p factory/state/spec`.
4. Find the source's relative links to Markdown files. List candidate lines
   with `grep -nE '\]\(|^ {0,3}\[[^]]+\]:' "<file>"` (it prints nothing when
   there are none) and read them. A link counts when it is an inline link
   `[text](target)` or a reference definition `[label]: target` whose target
   (without a `#fragment`, an optional `"title"`, or `<…>` brackets) ends in
   `.md` (any case), has no URL scheme (such as `https:` or `mailto:`), and
   doesn't start with `/` or `#`. Ignore links inside fenced code blocks.
   Resolve each target relative to the directory of the *original* file
   that contains it.
5. Copy each linked file that exists to `factory/state/spec/<basename>`. If
   that name is already taken by a different source file, use
   `<parent directory name>-<basename>`, then add `-2`, `-3`, … before
   `.md` until the name is free. Keep a list of `original path → copy name`,
   and copy each original only once.
6. Repeat steps 4 and 5 for every copied file, reading the original, until
   no new files turn up (links are followed transitively).
7. Rewrite each counted link target, keeping any `#fragment` and title:
   in `SPEC.md` to `spec/<copy name>`, and in the copies under `spec/` to
   `<copy name>`. Change only the link targets, with the Edit tool.
8. Leave a link to a missing file unchanged. The Planner reports it as a
   spec issue.

## board.json

`factory/state/board.json` reflects the loop for the dashboard
(`factory/dashboard/`). It isn't in git and can always be rebuilt from
`backlog.md` and the task files. Its format:

```json
{
  "updated": "2026-10-27T10:51:00Z",
  "loop": { "state": "running", "message": "Building 016-list-sessions round-2",
            "current": "016-list-sessions" },
  "agents": { "builder-016-list-sessions": "<agent id from the Agent tool>" },
  "files": [ "SPEC.md", "preflight.md", "stack.md", "backlog.md" ],
  "tasks": [
    { "id": "016-list-sessions", "title": "List sessions",
      "branch": "factory/016-list-sessions",
      "status": "In progress", "stage": "review",
      "rounds": [
        { "id": "round-1", "kind": "auto", "build": "DONE",
          "reviews": { "code": "PASS", "feature": "FAIL", "ui": "PASS" },
          "verdict": null },
        { "id": "round-2", "kind": "auto", "build": "DONE",
          "reviews": { "code": "PASS", "feature": "pending", "ui": "pending" },
          "verdict": null }
      ],
      "files": [ "task.md", "round-1/build-notes.md", "round-1/code-review.md",
                 "round-1/feature-review.md", "round-1/ui-review.md",
                 "round-2/build-notes.md", "round-2/code-review.md" ] }
  ]
}
```

- `loop.state`: `planning`, `awaiting-preflight`, `awaiting-plan-review`,
  `running`, `escalated`, `halted` (the reason goes in `message`) or
  `complete`. When rebuilding, the state is always `awaiting-preflight` in
  the preflight state.
- `loop.current`: the task being worked on, or `null`.
- `agents`: builder agent ids, keyed `builder-<task-id>`. An id is usable
  only by the task orchestrator that spawned it.
- `files`: the run's own files that exist (`SPEC.md`, `preflight.md`,
  `stack.md`, `backlog.md`), so the dashboard can open them even before
  there are tasks. This is the one field beyond the board in `factory.md`;
  the dashboard copes without it.
- `stage`: `planned`, `building`, `review`, `approval`, `escalated`, `done`
  or `abandoned`, worked out from the status and the latest round.
- `kind`: `auto`, `approver-rework` or `rework` (see "Rounds").
- `build`: `pending`, `DONE` or `BLOCKED`.
- `reviews`: one key per reviewer of the task, in roster order, each
  `pending`, `PASS` or `FAIL`.
- `verdict`: `null`, `APPROVE`, `REWORK` or `ESCALATE`.
- `files` (per task): `task.md`, every Markdown file in the round
  directories, and every file in the reviewers' evidence directories, relative
  to `T`.

### The board script

`B` works out everything it writes from `backlog.md` and the task files. It
writes `board.json.tmp` and moves it into place, so the board is never
half-written.

| Command | Does |
|---|---|
| `B rebuild [--state S] [--message M]` | Create the board, or rebuild every task from the files. Keeps `agents`, and `loop` unless overridden. |
| `B loop [--state S] [--message M] [--current <task-id> \| --no-current]` | Update the loop. |
| `B agent builder-<task-id> <agent-id>` | Record a builder's agent id (`--remove` deletes it). |
| `B task <task-id>` | Work out one task's entry from its files, write it, and print it like `show`. |
| `B show [<task-id>]` | Print `updated`, the loop and the agents, and for a task its status, stage, round count, latest round and reviewers. Read-only. |

Exit status 0 is success. 2 is a usage error or an unknown task. 3 means a
state file is invalid (a bad first line, or a backlog row with an unknown
status); the board is still written and stderr names each file. Treat any
non-zero exit as an error and halt with the script's message.

Update the board every time you take back control from an agent and on
every stage change: `B task <task-id>` for the task, and
`B loop --message "<what is happening>"` for the header. Messages are short:
`Building <task-id> <round-id>`, `Reviewing <task-id> <round-id>`,
`Approving <task-id> <round-id>`, `Merged <task-id>`.

## Write verification

After each agent finishes, check what it changed, outside the state with git
and inside it with checksums.

**Snapshot,** just before spawning the agent (`<snapshot>` is `T/.snapshot`,
or `factory/state/.snapshot` for the Planner):

```bash
find factory/state -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | sort -z | xargs -0 -r cksum > <snapshot>
```

Also note `git rev-parse HEAD` and `git branch --show-current`.

**Changed state files** since the snapshot (one path per created, changed or
deleted file; prints nothing when nothing changed):

```bash
diff <snapshot> <(find factory/state -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | sort -z | xargs -0 -r cksum) | sed -n 's/^[<>] [0-9]* [0-9]* //p' | sort -u
```

**Outside the state,** the tree is clean when this prints nothing (files git
ignores, such as build outputs, never show):

```bash
git status --porcelain --untracked-files=all
```

What each agent may change:

| Agent | In the state | In git |
|---|---|---|
| Planner | only `preflight.md`, `stack.md`, `backlog.md` and `tasks/*/task.md` | nothing: clean tree, same `HEAD` and branch |
| Builder | only `R/build-notes.md` | its commits on `factory/<task-id>`, a clean tree, and nothing under `factory/state/` in the branch (`git diff --name-only main...HEAD -- factory/state` prints nothing) |
| Each reviewer, as it finishes | nothing outside `R/` | nothing: clean tree, same `HEAD` and branch |
| All reviewers, once all are done | only `R/<key>-review.md` and files under `R/<key>/`, for keys the task lists | as above |
| Approver | only `R/verdict.md` | nothing: clean tree, same `HEAD` and branch |

Once all the reviewers are done, also list the round directory
(`find R -type f | sort`). Every file in it must be `build-notes.md`,
`base-commit`, `human-review.md`, `approver-review.md`, or, for a key the
task lists, `<key>-review.md` or a file under `<key>/`.

On any violation, halt (see "Halting") with the message
`Write violation by <agent> on <task-id> <round-id>: <paths>`, revert
nothing, and tell the user what changed. The one exception is a Builder that
leaves uncommitted changes: the task orchestrator resumes it once (see
`task-orchestrator.md`) before it halts.

## Shared steps

### The merged check

A task is merged when either holds *(exit status is the answer: 0 means
merged)*:

```bash
git log main --format=%s | grep -Fxq "factory: merge <task-id>" || { git rev-parse -q --verify refs/heads/factory/<task-id> > /dev/null && git merge-base --is-ancestor refs/heads/factory/<task-id> main && git log refs/heads/factory/<task-id> --format=%s | grep -Fq "<task-id>: "; }
```

The second half needs the branch to have a builder commit (a subject
starting with `<task-id>: `). A branch with none isn't merged, even though it
is an ancestor of main. Ids are never reused, so always match the full
task-id, never a bare serial.

### Merge

Used by the task orchestrator after APPROVE, and by the main orchestrator for
`/factory approve`. The tree must be clean.

```bash
git switch main
git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>
```

- If the merge succeeds, you are on `main` with a clean tree. The main
  orchestrator then marks the task `Complete` (backlog, then
  `B task <task-id>`) and deletes the branch with
  `git branch -d factory/<task-id>`. If git refuses because the branch isn't
  an ancestor of main but the merged check passes,
  `git branch -D factory/<task-id>` is allowed.
- If it conflicts (`git diff --name-only --diff-filter=U` lists files), leave
  the merge as it is, unfinished, on `main`. Halt with the message
  `Merge conflict merging <task-id>: <files>. Resolve the conflicts on main, commit the merge, then run /factory continue.`
  The user completes the merge; `/factory continue` then finds the task
  merged and marks it `Complete`.
- If it fails for any other reason, halt with git's message.

### Halting

1. If an agent you spawned is still running, wait for it first: end your
   turn with `WAITING` until every running agent has reported, and start no
   new ones.
2. `B loop --state halted --message "<reason>"`, then `B task <task-id>` if a
   task is current.
3. Revert nothing.
4. The task orchestrator ends with the one line `halted <task-id>`. The main
   orchestrator tells the user the reason and what to do; unless the reason
   says otherwise, `/factory continue` resumes from the files.

A halt with a plan problem is the exception: it keeps `loop.state`
`awaiting-plan-review` so that `/factory continue` checks the plan again
(see `SKILL.md`).
