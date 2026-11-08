# Factory conventions

Both orchestrator levels read this file before anything else: the main
orchestrator (the session running `/factory`, see `SKILL.md`) and the task
orchestrator (`.claude/agents/task-orchestrator.md`). It is read as it is, so
the shell snippets below are exact.

## Names and paths

- `S` is `factory/state/`, the run's state. It is git-ignored, so there is one
  copy of it, the same on every branch. Nothing under it is ever committed.
- `T` is a task directory, `factory/state/tasks/<task-id>/`.
- `R` is a round directory, `T/<round-id>/`, such as
  `factory/state/tasks/002-apply-selected-theme/round-3/`.
- `FP` is the factory script, always run from the repository root as
  `python3 -I .claude/skills/factory/factory.py <command> …`.
- Task-ids are a zero-padded serial of at least three digits, a dash, and two
  or three lowercase words separated by dashes (`002-apply-selected-theme`).
  Round-ids are `round-<n>`, numbered from 1. Branches are
  `factory/<task-id>`.
- State files: `S/board.json`, `S/backlog.md`, `S/SPEC.md`, `S/spec-source`,
  `S/project/`, `S/spec/`, `S/preflight.md`, `S/stack.md`, `S/.snapshot`
  (Planner write check), `T/task.md`, `T/.snapshot`, and in `R`:
  `base-commit`, `human-review.md`, `approver-review.md`, `build-notes.md`,
  `reviews-started`, `<key>-review.md`, `<key>/` (evidence), `verdict.md`.
- Agent types (the `subagent_type` names): `planner`, `task-orchestrator`,
  `builder`, `code-reviewer`, `feature-reviewer`, `security-reviewer`,
  `privacy-reviewer`, `api-reviewer`, `ui-reviewer`, `approver`. Only the
  main orchestrator spawns `planner` and `task-orchestrator`, and never two
  task orchestrators at once. Only the task orchestrator spawns the builder,
  the reviewers and the Approver.

### Agent prompts

Agent prompts contain only ids, paths and these fixed messages, nothing else.
All other context comes from files and the agent's definition.

| Agent | Prompt |
|---|---|
| Planner | `Spec: factory/state/SPEC.md.` |
| Task orchestrator | `Task-id: <task-id>.` |
| Builder, every reviewer, Approver | `Task-id: <task-id>. Round-id: <round-id>.` |
| Builder, resumed after leaving a dirty tree | `Task-id: <task-id>. Round-id: <round-id>. Uncommitted changes: commit them or fix their cause, then update the build notes.` |

## Ground rules

- Nobody pushes, forces anything, rewrites history, or discards commits or
  uncommitted changes. Never run `git push`, `reset`, `rebase`,
  `commit --amend`, `cherry-pick`, `revert`, `stash`, `clean`, `restore`,
  `checkout -- <path>`, `branch -D`, `branch -f`, `tag -d`, `update-ref`,
  `reflog expire`, `gc --prune` or `merge --abort`. Only the Merge step
  merges. Branches are deleted only with `git branch -d`; if that refuses,
  halt instead of forcing. Only the human pushes.
- Nobody changes the factory's own files while the factory runs: the skill,
  this file, the factory script, the agent definitions, the dashboard,
  `factory/factory.md` and `factory/floorplan.md`. In `.gitignore` only the
  two factory entries (`/factory/state`, `/factory/archive`) are off limits.
- The orchestrators never install anything. Install commands go to the human,
  who runs them (in Claude Code, for example, with `! <command>`).
- Only one task is ever `In progress` or `Escalated` at a time.
- The state never goes into git: never `git add -f` anything under `S`.
- The orchestrators never do an agent's work themselves, and never edit an
  agent's result files.
- Wait for completion notifications instead of polling. Spawn agents with
  `run_in_background: true` (the Planner may run in the foreground), then end
  the turn with the single word `WAITING`. Never sleep, poll, read an agent's
  output file, or message a running agent to ask how it is doing.
- `board.json` has one writer at a time: the task orchestrator while it runs,
  the main orchestrator otherwise. Every change to it goes through
  `FP board-update` or `FP board-rebuild`; never write, edit or print the
  whole file.
- Leave no helper scripts behind. The commands below are all that is needed.
- A command that finds, lists or counts things exits 0 when it succeeds, even
  when it finds nothing. The exceptions are plain git checks whose exit status
  is the answer, labelled **(exit status is the answer)** where they appear.
  Every `FP` command exits 0 on success and prints its answer; a non-zero
  exit is an error, and the orchestrator halts with the reason from stderr.

## Preconditions

`FP preconditions` prints `branch: <name>`, `state-ignored: yes|no` (from
`git check-ignore -q factory/state`, **exit status is the answer**),
`tree: clean|dirty`, and a `dirty: <porcelain line>` per change. It uses
`git status --porcelain --untracked-files=all`, so untracked files count
whatever `status.showUntrackedFiles` says. Files git ignores (such as
`target/`) never count.

## The backlog

`S/backlog.md` is written by the Planner and has this shape:

```markdown
# Backlog

| Task | Status |
|---|---|
| 001-scaffold-project | Planned |
| 002-list-sessions | Planned |
```

One row per task, in implementation order, two cells: the task-id and its
status. Statuses are `Planned`, `In progress`, `Escalated`, `Complete` and
`Abandoned`. The orchestrators change a status only with
`FP set-status <task-id> "<status>"`, and then run `FP board-update`.
`FP backlog` lists the rows, and `FP next-task` prints the first `Planned`
task or `none`.

## Write verification

After each subagent finishes, check two things.

**Outside the state** (git): run `FP preconditions`. Except for a builder
that is still in its round, the tree must be clean. After the Builder, also
check that nothing under `S` is in its commits:
`git diff --name-only "$(cat <R>/base-commit)"..HEAD -- factory/state` prints
nothing. If the Builder left uncommitted changes, resume it once with the
uncommitted-changes prompt above, without taking a new snapshot (the Build
snapshot covers both passes). If the tree still
isn't clean afterwards, halt. Every other agent changes nothing outside `S`;
any change is a violation.

**Inside the state** (checksums): just before spawning an agent, run
`FP snapshot --task <task-id>` (writes a `cksum` listing of every file under
`S`, leaving out `board.json*` and `.snapshot` files, to `T/.snapshot`), or
`FP snapshot --planner` for the Planner (to `S/.snapshot`). Afterwards run
`FP check-writes --task <task-id> --allow <glob> …` (or `--planner`). It
lists every file added, changed or removed since the snapshot, marks each
one that no `--allow` glob covers `(not allowed)`, and ends with
`writes: ok` or `writes: violation`. Globs are relative to `S`; `*` stays
within a directory and `**` crosses directories. Quote globs in the shell.
Between the snapshot and the check, the orchestrator itself writes nothing
under `S` except through `FP board-update`.

| After | Allowed (`--allow`) |
|---|---|
| Planner | `preflight.md`, `stack.md`, `backlog.md`, `'tasks/*/task.md'` |
| Builder | `tasks/<task-id>/<round-id>/build-notes.md` |
| Each reviewer, as it finishes | `'tasks/<task-id>/<round-id>/**'` |
| All the round's reviewers, once all are done | for each listed key: `tasks/<task-id>/<round-id>/<key>-review.md` and `'tasks/<task-id>/<round-id>/<key>/**'` |
| Approver | `tasks/<task-id>/<round-id>/verdict.md` |

The `reviews-started` marker: once the Builder's round has passed these
checks, and just before the first reviewer of the round is spawned, create
the empty file `R/reviews-started` (`touch`), and only then take the
snapshot. Uncommitted changes found before the marker exists are the
Builder's own; changes found after it came from a reviewer or the Approver.
When a resumed round re-runs only its missing reviewers, the marker is
already there.

On a violation, halt (see "Halting") without reverting anything. Tell the
user what changed, and that they must inspect those changes and commit or
remove them before `/factory continue`.

## Rounds

- A round directory is created at the start of each round. Write the round's
  note first (`human-review.md` for a human rework round, `approver-review.md`
  for an Approver rework round), and `base-commit` last, with
  `FP write-base-commit <task-id> <round-id>`. It writes main's commit for
  `round-1` and `HEAD` (the task branch) for later rounds. A round directory
  with a `base-commit` is complete.
- A round's kind follows from its files: `rework` if it has
  `human-review.md`, `approver-review.md` makes it `approver-rework`,
  otherwise `auto`.
- `FP round-info <task-id> [<round-id>]` prints the task's status and
  reviewers, its rounds, the `next-round`, and for one round (default: the
  latest): `kind`, `base-commit`, `human-review`, `approver-review`, `build`
  (`pending|DONE|BLOCKED|invalid`), `reviews-started`, each `review <key>`
  (`PASS|FAIL|invalid|missing`), `reviews-missing`, `verdict`, `rework-note`
  (when the verdict is REWORK), `earlier-reworks` and `previous-round`. Rounds
  sort by number, so `round-10` comes after `round-9`.
- A round's previous round is the latest earlier round directory that has a
  `base-commit`: `FP previous-round <task-id> <round-id>` (or `none`).
- The number of earlier REWORK verdicts: `FP rework-count <task-id> <round-id>`
  counts the `verdict.md` files starting `VERDICT: REWORK` in rounds before
  `<round-id>`. Human rework rounds don't reset it.
- A verdict's rework note: `FP rework-note <task-id> <round-id>` prints
  `present` only if `verdict.md` has a `## Rework note` section with text.
  `FP copy-rework-note <task-id> <from-round> <to-round>` copies that section
  to `<to-round>/approver-review.md`.
- On re-entry, a round directory without `base-commit` was interrupted while
  it was being created. If its previous round's verdict is REWORK, it is an
  Approver rework round: run `FP copy-rework-note` again from that verdict,
  then `FP write-base-commit`. Any other round just gets its `base-commit`.
- Automatic rounds: at most two per task (round 1 and one fix round, which
  happens only when round 1 has a FAIL review). The Approver adds at most two
  Approver rework rounds. Any further rounds come from `/factory rework`.
  Neither kind of rework round triggers an automatic fix round.

### Fixed first lines

These files have a fixed first line. Any other first line, including an
empty one, is an error that halts the loop. `FP first-lines [--task <id>]`
checks them all and prints `first-lines: ok` or `first-lines: invalid` with
one `invalid:` line per bad file.

| File | First line |
|---|---|
| `<key>-review.md` | `VERDICT: PASS` or `VERDICT: FAIL` |
| `verdict.md` | `VERDICT: APPROVE`, `VERDICT: REWORK` or `VERDICT: ESCALATE` |
| `build-notes.md` | `STATUS: DONE` or `STATUS: BLOCKED` |
| `preflight.md` | `PREFLIGHT: READY`, `PREFLIGHT: TOOLS` or `PREFLIGHT: BLOCKED` |
| `task.md` | `# <task-id>: <title>` |

## The preflight state

The run is in the preflight state when `S/preflight.md` exists and
`S/backlog.md` doesn't (`FP run-info` prints `preflight-state: yes`). To
re-copy the spec, run

```sh
python3 -I .claude/skills/factory/factory.py spec-copy "$(cat factory/state/spec-source)"
```

It replaces `S/SPEC.md`, `S/project/` and `S/spec/` the same way
`/factory start` does, and lists any linked document that is `missing:`.

## The reviewer roster

Keys, in roster order: `code`, `feature`, `security`, `privacy`, `api`, `ui`.
Each key's agent is `<key>-reviewer`, and its review is `R/<key>-review.md`
with evidence in `R/<key>/`. `code` and `feature` run on every task. A task's
reviewers come from the top-level bullets of its `## Reviews` section, each
by its leading key; with no such section they are `code`, `feature`, and `ui`
unless the `UI` section is `## UI: none`. Always read them with
`FP reviewers <task-id>`, which prints them space-separated in roster order.

## board.json

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
  the reason is in `message`) or `complete`. Only the plan-review halt sets
  `awaiting-plan-review`, by passing it explicitly; a rebuild never derives
  it.
- `agents`: builder agent ids under `builder-<task-id>`. An id is only usable
  by the task orchestrator that spawned it.
- `stage`: `planned`, `building`, `review`, `approval`, `escalated`, `done`
  or `abandoned`. `kind`: `auto`, `approver-rework` or `rework`. `build`:
  `pending`, `DONE` or `BLOCKED`. Review values: `pending`, `PASS` or `FAIL`,
  one key per listed reviewer, in roster order. `verdict`: `null`, `APPROVE`,
  `REWORK` or `ESCALATE`. `files`: `task.md`, the Markdown files in the round
  directories and the reviewers' evidence, relative to `T`.

The script works out every task entry (status, stage, rounds, reviews,
files) from `backlog.md` and the task files each time it writes the board,
and writes `board.json.tmp` and moves it into place, so the file is never
half-written.

- `FP board-update [--loop-state <state> --message "<text>"] [--message "<text>"] [--current <task-id>|none] [--agent builder-<task-id>=<agent-id>]`
  sets `updated`, the given loop fields and agents, and recomputes the tasks.
  `--loop-state` always comes with `--message`. `--agent NAME=` removes an
  id. If the board is missing it is rebuilt first.
- `FP board-rebuild [same options]` rebuilds the board from the files, with
  no agents and the loop derived from them unless given: `awaiting-preflight`
  in the preflight state, `planning` with no backlog, `escalated` or
  `running` for the active task, `complete` when every task is `Complete` or
  `Abandoned`, and otherwise `halted`.
- `FP board-show [--task <task-id>]` prints `loop-state`, `loop-message`,
  `loop-current`, the agents, and one task's status, stage and latest round
  (default: `loop.current`).

Update the board every time the orchestrator takes back control from a
subagent, and on every stage change.

## The factory script

All commands run from the repository root as
`python3 -I .claude/skills/factory/factory.py <command> …`; `--help` on any
command describes it.

| Command | Use |
|---|---|
| `preconditions` | Branch, `state-ignored`, clean tree (see "Preconditions"). |
| `spec-copy <spec>` | `/factory start` and the preflight restart: writes `spec-source`, copies the spec to `SPEC.md`, the project documents next to it to `project/`, and every Markdown file they link to (transitively) to `spec/`, rewriting the links; prints `copied:` and `missing:` lines. |
| `run-info` | The run at a glance: `preflight`, `backlog`, `preflight-state`, `spec-source`, `board`, `loop-state`, task counts, `active: <id> <status>`, `next-planned`, `all-planned-no-rounds`. |
| `backlog` | Each task-id and status. |
| `set-status <task-id> "<status>"` | Change a status in `backlog.md`. |
| `next-task` | First `Planned` task, or `none`. |
| `highest-serial` | Highest serial used before this run, and the next one. |
| `archived-stack` | Newest `factory/archive/*/stack.md`, or `none`. |
| `plan-check` | The mechanical plan check: `plan: sound` or `plan: problems`, with `problem:` and `warning:` lines. Warnings don't make a plan unsound. |
| `reviewers <task-id>` | The task's reviewers, in roster order. |
| `dependents <task-id>` | Tasks that depend on it, with status, or `none`. |
| `merged <task-id>` | `yes` or `no` (see below). |
| `first-lines [--task <task-id>]` | Fixed first lines (see "Fixed first lines"). |
| `round-info <task-id> [<round-id>]` | Rounds and the state of one round (see "Rounds"). |
| `previous-round <task-id> <round-id>` | The previous round, or `none`. |
| `rework-count <task-id> <round-id>` | REWORK verdicts before that round. |
| `rework-note <task-id> <round-id>` | `present` or `absent`. |
| `copy-rework-note <task-id> <from> <to>` | Writes `<to>/approver-review.md`. |
| `write-base-commit <task-id> <round-id>` | Writes `R/base-commit`. |
| `snapshot --task <task-id>` / `--planner` | Checksum listing before an agent runs. |
| `check-writes --task <task-id>` / `--planner` `--allow <glob> …` | What the agent changed under `S`. |
| `board-update`, `board-rebuild`, `board-show` | The board (see "board.json"). |

A task is merged (`FP merged`) if `git log main --format=%s --` has the
subject `factory: merge <task-id>`, or if its branch exists, is an ancestor
of main (`git merge-base --is-ancestor`, **exit status is the answer**) and
has at least one builder commit (a subject starting `<task-id>: `).

## Shared steps

### Merge

Used after an APPROVE verdict (task orchestrator) and by `/factory approve`
(main orchestrator).

1. `FP preconditions` must show a clean tree. If not, halt.
2. `git switch main`.
3. `git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>`.
4. If the merge fails with conflicts: do not abort it and touch nothing.
   `FP set-status <task-id> "In progress"` if it isn't already, then halt with
   the message `Merge conflict merging <task-id>: resolve the conflicts,
   complete the merge with git commit, then run /factory continue`. Tell the
   user which files conflict (`git diff --name-only --diff-filter=U`). If the
   merge fails for any other reason, halt with git's message.
5. If it succeeds, the task is merged. The task orchestrator stops here, on
   main, leaving the task `In progress`. The main orchestrator runs
   "Complete a task".

### Complete a task (main orchestrator only)

1. `FP set-status <task-id> Complete`.
2. If the branch exists (`git rev-parse --verify --quiet refs/heads/factory/<task-id>`,
   **exit status is the answer**), delete it with
   `git branch -d factory/<task-id>`. If that refuses, halt with its message;
   never force.
3. `FP board-update --loop-state running --message "Merged <task-id>" --current none`.
4. Carry on with the next task (see `SKILL.md`, "Running tasks"). If no
   `Planned` task remains, run
   `FP board-update --loop-state complete --message "Construction is complete" --current none`
   and tell the user that construction is complete.

### Halting

1. If agents this orchestrator spawned are still running, wait for them
   first (end the turn with `WAITING`).
2. `FP board-update --loop-state halted --message "<reason>"`.
3. The task orchestrator ends with the one-line message `halted <task-id>`.
   The main orchestrator tells the user the reason and what to do, and that
   `/factory continue` resumes from the files.

Nothing is reverted when halting.
