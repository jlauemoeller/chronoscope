# Factory conventions

Shared by both orchestrator levels: the main orchestrator (the session that
ran `/factory`, see `SKILL.md`) and the task orchestrator (the subagent in
`.claude/agents/task-orchestrator.md`). Read all of this before doing
anything else. Every command below runs from the repository root.

## Names and paths

- `factory/state/` is the run's state. It is ignored by git, so there is one
  copy of it, the same on every branch. Nothing under it is ever committed.
  - `board.json`: the loop status the dashboard shows. Written only through
    the factory script.
  - `backlog.md`: the task queue and each task's status.
  - `SPEC.md`: the copy of the spec. `spec/`: copies of the documents it
    links to. `spec-source`: the absolute path of the original spec.
  - `preflight.md`, `stack.md`: written by the Planner.
  - `tasks/<task-id>/`: one directory per task, with `task.md`, `.snapshot`
    and one `round-<n>/` directory per round.
  - `.snapshot`: the Planner's checksum listing (see "Write verification").
- `T` is shorthand for the task directory `factory/state/tasks/<task-id>`.
- `R` is shorthand for a round directory `T/<round-id>`, such as
  `factory/state/tasks/002-apply-selected-theme/round-2`.
- Task-ids look like `002-apply-selected-theme`: a serial of at least three
  digits and two or three lowercase words. Round-ids look like `round-4`.
  Branches are `factory/<task-id>`.
- `factory/archive/` holds finished runs, moved there by hand. It is ignored
  by git too.
- The agent types (the Agent tool's `subagent_type`, which is the `name` in
  each definition's frontmatter):
  - `task-orchestrator`: runs one task. Only the main orchestrator spawns it.
  - `planner`: preflight and plan. Only the main orchestrator spawns it.
  - `builder`: implements a task.
  - `code-reviewer`, `feature-reviewer`, `security-reviewer`,
    `privacy-reviewer`, `api-reviewer`, `ui-reviewer`: one per roster key.
  - `approver`: decides APPROVE, REWORK or ESCALATE.

  Only the task orchestrator spawns the Builder, the reviewers and the
  Approver. Never pass `model` or `effort` when spawning: each definition's
  frontmatter sets them.

## Ground rules

- **Never push or rewrite history.** The factory never pushes to an origin;
  only the human pushes. Never force anything, and never discard commits or
  uncommitted changes. These commands are never allowed: `git push`,
  `git reset`, `git rebase`, `git commit --amend`, `git cherry-pick`,
  `git revert`, `git stash`, `git clean`, `git restore`,
  `git checkout -- <path>`, `git branch -D`, `git branch -f`, `git tag -d`,
  `git update-ref`, `git reflog expire`, `git gc --prune`, and
  `git merge --abort`. Only the Merge step below merges. Branches are
  deleted only with `git branch -d`; if that refuses, halt instead of
  forcing.
- **Never change the factory's own files** while the factory runs: the
  skill, this file, the factory script, the agent definitions, the
  dashboard, `factory/factory.md` and `factory/floorplan.md`. In
  `.gitignore`, only the factory's two entries (`/factory/state` and
  `/factory/archive`) are off limits.
- **Never install anything.** The human installs tools; the orchestrators
  only tell them the command.
- **Only one active task.** At most one task is ever `In progress` or
  `Escalated`. The main orchestrator never runs two task orchestrators at
  once.
- **The state never goes into git.** Never `git add -f` anything under
  `factory/state/`.
- **Agent prompts contain only ids, paths and the fixed messages below.**
  All other context comes from files. Never add explanations, summaries or
  instructions to a prompt.
- **Wait, don't poll.** To wait for agents, end the turn with the single
  word `WAITING`. A completion notification (a `<task-notification>`) wakes
  you. Never sleep, never poll, and never read an agent's output file. If an
  Agent call returns the agent's final result directly, that agent has
  finished.
- **One writer of `board.json`.** The task orchestrator writes it while it
  runs, and the main orchestrator otherwise. Every change goes through the
  factory script. Never print the whole board, and never edit it any other
  way.
- **Exit statuses.** A command that finds, lists or counts things exits 0
  when it succeeds, including when it finds nothing, so a non-zero exit
  always means an error: halt with the reason. The commands below are
  written that way. The only exceptions are plain git checks whose exit
  status is the answer, each labelled **(plain git check)** where it
  appears.
- **No helper scripts.** Don't leave scripts or scratch files behind in the
  repository or in `factory/state/`.

## Fixed agent prompts

| Agent | Prompt |
|---|---|
| `planner` | `Spec: factory/state/SPEC.md.` |
| `task-orchestrator` | `Task-id: <task-id>.` |
| `builder`, every reviewer, `approver` | `Task-id: <task-id>. Round-id: <round-id>.` |
| `builder`, after uncommitted changes | `Task-id: <task-id>. Round-id: <round-id>. Uncommitted changes: commit them or fix their cause, then update the build notes.` |

A builder resumed for a new round (with SendMessage) gets the same
`Task-id: <task-id>. Round-id: <round-id>.` message.

## Git checks

- State ignored: `git check-ignore -q factory/state` **(plain git check)**:
  exit 0 means ignored, exit 1 means not ignored (refuse), anything else is
  an error.
- Clean tree: `git status --porcelain --untracked-files=all` prints
  nothing. `--untracked-files=all` overrides any `status.showUntrackedFiles`
  setting. Files that git ignores (build outputs, `factory/state/`) never
  show here.
- Current branch: `git branch --show-current`.
- Merged: `python3 -I .claude/skills/factory/factory.py task merged <task-id>`
  prints `yes` or `no`. A task is merged if `git log main --format=%s --`
  has the subject `factory: merge <task-id>`, or if its branch exists, is an
  ancestor of main, and has at least one builder commit (subject starting
  with `<task-id>: `).

## The backlog

`factory/state/backlog.md` is a Markdown table with one row per task, in
implementation order, and two cells per row: the task-id and its status.

```markdown
| Task | Status |
|---|---|
| 001-scaffold-project | Complete |
| 002-apply-selected-theme | In progress |
| 003-export-session-file | Planned |
```

Statuses: `Planned`, `In progress`, `Escalated`, `Complete`, `Abandoned`.
Only the orchestrators and the Planner write this file. Change a status
only with the script, which also refreshes that task's entry on the board:

```
python3 -I .claude/skills/factory/factory.py backlog set <task-id> "In progress"
```

The next available task is the first `Planned` row:
`python3 -I .claude/skills/factory/factory.py backlog next` prints its id,
or `none`.

## Rounds

- Round directories are `T/round-<n>`, numbered from 1. Order them by
  number, never as text: `round-10` comes after `round-9`.
  `round list <task-id>` prints the latest round, whether it has a
  `base-commit`, build notes and the `reviews-started` marker, its previous
  round, and the next round-id.
- **Kind** (`round kind <task-id> <round-id>`), from the round's files:
  - `rework`: it has `human-review.md` (started by `/factory rework`);
  - `approver-rework`: it has `approver-review.md` (started by a REWORK
    verdict), or it has no `base-commit` yet and its previous round's
    verdict is REWORK;
  - `auto`: anything else (round 1 and the fix round).
- **Creating a round:** create the directory, write its note first
  (`human-review.md` or `approver-review.md`, if it has one), and write
  `base-commit` last, so a round directory with a `base-commit` is
  complete. The base commit is the commit `main` points at for round 1
  (`git rev-parse main > R/base-commit`) and the task branch's `HEAD` for
  later rounds (`git rev-parse HEAD > R/base-commit`). Before creating an
  Approver rework round, check that the verdict has a rework note, so a
  REWORK without one never leaves a round directory behind.
- **A round directory without `base-commit`** was interrupted while it was
  being created. An Approver rework round gets its note copied again from
  the previous round's verdict and then its `base-commit`. Any other round
  just gets its `base-commit`.
- **The previous round** of a round is the latest earlier round directory
  that has a `base-commit` (`round previous <task-id> <round-id>`).
  Directories without one are never a previous round.
- **`reviews-started`** is an empty marker file in `R`. The task
  orchestrator writes it (`: > R/reviews-started`) after the Builder's work
  has passed write verification and just before it spawns the round's first
  reviewer. From then on the Builder is done with the round: uncommitted
  changes found before the marker are the Builder's own, and changes found
  after it came from a reviewer or the Approver.
- **REWORK verdicts:** `round rework-count <task-id> <round-id>` counts the
  `verdict.md` files of rounds before `<round-id>` whose first line is
  `VERDICT: REWORK`. The Approver may issue at most two per task. Human
  rework rounds don't reset the count.
- **Rework note:** `round rework-note <task-id> <round-id>` prints
  `rework-note: yes` and the verdict's `## Rework note` section, or
  `rework-note: no` when the section is missing or has no text under its
  heading. With `--write R/approver-review.md` it writes the section there
  instead of printing it.
- **Automatic rounds:** at most two per task: round 1 and one fix round
  (when a round-1 review is FAIL). The Approver adds at most two Approver
  rework rounds. Any further rounds come only from `/factory rework`. After
  a fix round or either kind of rework round, the Approver runs straight
  away; neither kind of rework round triggers an automatic fix round.
- **Result files are final once they exist.** The build notes, the reviews
  and the verdict are each written once, complete. A resumed round treats an
  existing one as done.
- **Fixed first lines.** Any other first line, including an empty one, is
  an error that halts the loop. `first-lines [<task-id>]` checks them all.

  | File | First line |
  |---|---|
  | `preflight.md` | `PREFLIGHT: READY`, `PREFLIGHT: TOOLS` or `PREFLIGHT: BLOCKED` |
  | `T/task.md` | `# <task-id>: <title>` |
  | `R/build-notes.md` | `STATUS: DONE` or `STATUS: BLOCKED` |
  | `R/<key>-review.md`, for each roster key | `VERDICT: PASS` or `VERDICT: FAIL` |
  | `R/verdict.md` | `VERDICT: APPROVE`, `VERDICT: REWORK` or `VERDICT: ESCALATE` |

  `human-review.md` and `approver-review.md` are notes, not reviews, and
  have no fixed first line.

## The preflight state

The run is in the preflight state when `factory/state/preflight.md` exists
and `factory/state/backlog.md` doesn't: the Planner's preflight stopped with
`PREFLIGHT: TOOLS` or `PREFLIGHT: BLOCKED` (or it never got as far as
planning). A rebuilt board then has `loop.state` `awaiting-preflight`.

To re-copy the spec, run `spec recopy`. It reads the path in
`factory/state/spec-source` and replaces `SPEC.md` and `spec/` exactly as
`spec copy` did at `/factory start`.

## The reviewer roster

In roster order, which is the order of the dashboard's chips and, after
security and data protection, the Approver's priority order:

| Key | Agent | Runs on |
|---|---|---|
| `code` | `code-reviewer` | every task |
| `feature` | `feature-reviewer` | every task |
| `security` | `security-reviewer` | tasks that list it |
| `privacy` | `privacy-reviewer` | tasks that list it |
| `api` | `api-reviewer` | tasks that list it |
| `ui` | `ui-reviewer` | tasks that list it |

A task's reviewers come from the top-level bullets of the `## Reviews`
section of its `task.md`, each by its leading key (backticks don't matter).
`code` and `feature` always run, whatever the section says. A `task.md`
without a `## Reviews` section runs `code`, `feature`, and `ui` unless its
UI section is `## UI: none`. Always get the list from the script, in roster
order: `task reviewers <task-id>` prints the keys separated by spaces, such
as `code feature ui`.

## board.json

Written only by the factory script, atomically (`board.json.tmp`, then a
rename), so the dashboard never reads half a file.

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

- `loop.state`: `planning`, `awaiting-preflight`, `awaiting-plan-review`,
  `running`, `escalated`, `halted` (merge conflict, write violation, error;
  the reason goes in `message`), or `complete`. Only the plan-review halt
  sets `awaiting-plan-review`, by passing it explicitly. A rebuild never
  derives it, so a rebuilt board can't make `/factory continue` skip the
  plan review.
- `loop.current`: the task being worked on, or `null`.
- `agents`: builder agent ids, keyed `builder-<task-id>`. An id is only
  usable by the task orchestrator that spawned it.
- Each task entry is worked out from `backlog.md` and the task's files:
  - `stage`: `planned`, `building`, `review`, `approval`, `escalated`,
    `done` or `abandoned`;
  - each round's `kind` (`auto`, `approver-rework`, `rework`), `build`
    (`pending`, `DONE`, `BLOCKED`), `reviews` (one key per listed reviewer,
    in roster order, each `pending`, `PASS` or `FAIL`, shown once the
    reviewers have started) and `verdict` (`null`, `APPROVE`, `REWORK`,
    `ESCALATE`);
  - `files`: `task.md`, every Markdown file in the round directories, and
    every file in the reviewers' evidence directories (`round-*/<key>/`),
    relative to `T`.

Update the board every time you take back control from a subagent and on
every stage change: `board task <task-id>` after anything in `T` changes,
`board loop ...` when the loop changes, and `board agent ...` when you
spawn a builder.

## The factory script

`.claude/skills/factory/factory.py`, always run from the repository root as
`python3 -I .claude/skills/factory/factory.py <command> ...` (below,
`factory.py` is short for that whole command). It finds the repository with
`git rev-parse --show-toplevel`. Every command exits 0 and prints its answer
when it succeeds, even when the answer is `no`, `none` or an empty list. A
non-zero exit is always an error with the reason on stderr: halt with that
reason.

| Command | Prints | Used by |
|---|---|---|
| `board build [--state S] [--message M]` | Creates or rebuilds `board.json` from `backlog.md` and the task files, keeping recorded agent ids. Without `--state` it derives the loop state from the files (never `awaiting-plan-review`). | main |
| `board loop S [--message M] [--current ID \| --no-current]` | Sets the loop state and message (an omitted message becomes empty) and, if given, `current`. | both |
| `board agent NAME AGENT-ID` | Records an agent id, such as `builder-<task-id>`. | task orchestrator |
| `board task ID` | Works out one task's entry from its files and writes it. Prints its status, stage and latest round. | both |
| `board show [ID]` | The loop, the agents and, with an id, that task's status, stage and latest round. | both |
| `spec copy PATH` | Writes `spec-source`, copies the spec to `SPEC.md` and the Markdown files it links to (transitively) into `spec/`, rewrites the links, and lists every missing linked document (`missing: none` when there is none). | main |
| `spec recopy` | The same, from the path in `spec-source`. | main |
| `plan check` | `PLAN: OK` or `PLAN: PROBLEMS`, then one `- ` line per problem, and `- warning: ` lines. The mechanical part of "Checking the plan". | main, Planner |
| `plan highest-serial` | `highest: <serial>` (or `none`) and `next: <serial>`: the highest serial used before this run, from git. | Planner |
| `plan archived-stack` | The newest `factory/archive/*/stack.md`, or `none`. | Planner |
| `round list ID` | `latest`, `latest-kind`, `latest-base-commit`, `latest-build-notes`, `latest-reviews-started`, `previous`, `next` (each `yes`/`no`, a round-id or `none`). | both |
| `round previous ID ROUND` | The previous round of `ROUND`, or `none`. | Builder, reviewers |
| `round kind ID ROUND` | `auto`, `approver-rework` or `rework`. | both |
| `round rework-count ID ROUND` | The number of REWORK verdicts before `ROUND`. | Approver, task orchestrator |
| `round rework-note ID ROUND [--write PATH]` | `rework-note: yes` and the note (or writes it to `PATH`), or `rework-note: no`. | task orchestrator |
| `task reviewers ID` | The task's reviewer keys in roster order. | both |
| `task dependents ID` | One `<task-id> <status>` line per task that depends on it, or `none`. | main |
| `task merged ID` | `yes` or `no`. | both |
| `task next-step ID` | Where an `In progress` task resumes (see `task-orchestrator.md`): `step:` and, where they apply, `round:`, `kind:`, `missing:`, `reason:` and `after:` lines. | both |
| `backlog next` | The first `Planned` task, or `none`. | main |
| `backlog set ID STATUS` | Changes a status in `backlog.md` and refreshes that task's board entry. | both |
| `first-lines [ID]` | `FIRST-LINES: OK`, or `FIRST-LINES: INVALID` and one line per bad file. Without an id it checks `preflight.md` and every task. | both |
| `summary` | Everything `/factory status` prints, including what `/factory continue` would do next. Read-only. | main |

## Write verification

After each subagent finishes, check what it changed, in two places.

**Outside the state:** `git status --porcelain --untracked-files=all` must
print nothing. Changes to files that git ignores (such as `target/`) don't
show and don't count. Only the Builder may change anything here, and it
must commit it all and leave a clean tree. Its commits must contain nothing
under `factory/state/`:
`git diff --name-only "$(cat R/base-commit)" HEAD -- factory/state` must
print nothing.

**Inside the state** `git status` can't see changes, so compare checksums.
Just before spawning an agent, write a checksum listing of every file under
`factory/state/`, leaving out `board.json*` and `.snapshot` files, to the
snapshot file `S`: `T/.snapshot` for task agents, `factory/state/.snapshot`
for the Planner.

```
(cd factory/state && find . -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | LC_ALL=C sort -z | xargs -0r cksum) > S
```

After the agent finishes, run the same listing and compare it with the
snapshot:

```
(cd factory/state && find . -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | LC_ALL=C sort -z | xargs -0r cksum) | diff S - || test $? -eq 1
```

`diff` exits 1 when the listings differ; the `|| test $? -eq 1` turns that
into success, so only a real error (exit 2) fails. Each output line starting
with `<` or `>` names a file (the path after the checksum and size,
relative to `factory/state/`) that was changed, created or deleted. Those
are the files the agent changed. Allowed:

- **Planner:** only `./preflight.md`, `./stack.md`, `./backlog.md` and
  `./tasks/*/task.md`.
- **Builder:** only `./tasks/<task-id>/<round-id>/build-notes.md`.
- **Each reviewer, as it finishes:** nothing outside
  `./tasks/<task-id>/<round-id>/`. Reviewers run in parallel, so the
  listing also shows the files of the others that have finished.
- **All reviewers, once they are all done:** list the round directory
  (`(cd R && find . -type f | LC_ALL=C sort)`). Every file in it must be one
  of `build-notes.md`, `base-commit`, `reviews-started`, `human-review.md`,
  `approver-review.md` and, for each reviewer the task lists,
  `<key>-review.md` and anything under `<key>/`.
- **Approver:** only `./tasks/<task-id>/<round-id>/verdict.md`.

Every agent except the Builder must also leave the branch as it found it
(`git branch --show-current`).

**On a violation**, use the Halting step with a reason that names the agent
and every file it changed that it may not, and tell the human to inspect
those changes and commit or remove them before `/factory continue`. Revert
nothing yourself.

## Shared steps

### Merge

Used by the task orchestrator after APPROVE and by the main orchestrator
for `/factory approve`. The tree must be clean.

1. `git switch main`.
2. `git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>`.
3. If the merge succeeded, it is done. Leave the task's status to the
   caller.
4. If it failed, list the conflicted files with
   `git diff --name-only --diff-filter=U`. Never abort or undo the merge.
   - With conflicts: the task must be `In progress` (the main orchestrator
     sets that for `/factory approve`; it already is for the task
     orchestrator). Then use the Halting step with the reason
     `Merge conflict merging factory/<task-id> into main: <files>`. Tell
     the human to resolve the conflicts and complete the merge with
     `git commit` (git keeps the message `factory: merge <task-id>`), then
     run `/factory continue`, which finds the task merged and marks it
     `Complete`.
   - Without conflicts (for example, untracked files would be
     overwritten): use the Halting step with git's error as the reason.

### Halting

1. The task orchestrator first waits for any of its agents that are still
   running (end the turn with `WAITING` until they have all reported), and
   checks their writes too.
2. `factory.py board loop halted --message "<reason>"`. Keep the reason to
   one line that says what went wrong and what the human must do.
3. Revert nothing.
4. The main orchestrator reports the reason to the human and says that
   `/factory continue` resumes once it is dealt with. The task orchestrator
   ends with the one-line message `halted <task-id>`.

### Opening the dashboard

Tell the human to run `python3 -m http.server -d factory` from the
repository root and open `http://localhost:8000/dashboard/`.
