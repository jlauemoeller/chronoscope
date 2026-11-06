# Factory conventions

Both orchestrator levels read this file before anything else: the main
orchestrator (the session running `/factory`) and the task orchestrator (the
`task-orchestrator` subagent). It is read as it is, so the shell snippets here
can use `$`.

Run every command from the repository root. Shell state doesn't persist
between commands, so always write paths and commands in full.

## Names and paths

| Name | Path or meaning |
|---|---|
| state | `factory/state/`, never in git, the same on every branch |
| board | `factory/state/board.json` (the factory script is its only writer) |
| backlog | `factory/state/backlog.md` |
| spec | `factory/state/SPEC.md`, linked documents in `factory/state/spec/`, original path in `factory/state/spec-source` |
| preflight | `factory/state/preflight.md` |
| stack profile | `factory/state/stack.md` |
| `T` | the task directory `factory/state/tasks/<task-id>/` |
| `R` | the round directory `T/<round-id>/` |
| task branch | `factory/<task-id>` |
| main branch | `main` |

Files in `T`: `task.md` (the Planner's), `.snapshot` (write verification).
Files in `R`: `base-commit`, `build-notes.md`, one `<key>-review.md` and
optional evidence directory `<key>/` per listed reviewer, `verdict.md`,
`approver-review.md` (Approver rework rounds), `human-review.md` (human rework
rounds).

Identifiers:
- A task-id is a serial of at least three digits (zero-padded), a dash, and a
  two-to-three word fingerprint in lowercase letters and digits, separated by
  dashes: `002-apply-selected-theme`. Serials are never reused in a
  repository.
- A round-id is `round-<n>`, with `n` counting up from 1: `round-4`. Rounds
  sort by number, so `round-10` comes after `round-9`.

Agent types, used as the Agent tool's `subagent_type`:
- `planner`, `task-orchestrator`, `builder`, `approver`;
- one `<key>-reviewer` per roster key: `code-reviewer`, `feature-reviewer`,
  `security-reviewer`, `privacy-reviewer`, `api-reviewer`, `ui-reviewer`.

`factory.py <command>` below is short for
`python3 -I .claude/skills/factory/factory.py <command>`. Always run it in
full.

## Ground rules

- **Never push, force or rewrite history**, and never discard commits or
  uncommitted changes. These are never allowed: `git push`, `git reset`,
  `git rebase`, `git commit --amend`, `git cherry-pick`, `git revert`,
  `git stash`, `git clean`, `git restore`, `git checkout -- <path>`,
  `git branch -D`, `git branch -f`, `git tag -d`, `git update-ref`,
  `git reflog expire`, `git gc --prune`, and any `--force` option. Only the
  Merge step merges. Branches are deleted only with `git branch -d`; if that
  refuses, halt instead of forcing.
- **Never change the factory's own files** while the factory runs: the skill,
  this file, the factory script, the agent definitions, the dashboard,
  `factory/factory.md` and `factory/floorplan.md`. In `.gitignore`, only the
  entries `/factory/state` and `/factory/archive` are off limits.
- **One active task.** Only one task is ever `In progress` or `Escalated` at
  a time, and the main orchestrator never runs two task orchestrators at
  once.
- **The state never goes into git.** Nothing under `factory/state/` is ever
  staged or committed (never `git add -f` it). Orchestrators and agents read
  and edit its files directly, on any branch.
- **Agent prompts contain only ids, paths and the fixed messages** listed
  under "Spawn prompts". All other context comes from files.
- **Wait for completion notifications instead of polling.** To wait for
  agents, end the turn with the single word `WAITING`. Never poll, sleep,
  read an agent's output file, or send a running agent messages to check on
  it.
- **One writer of `board.json` at a time:** the task orchestrator while it
  runs, the main orchestrator otherwise. Every change goes through
  `factory.py board ...` or `factory.py backlog-set ...`. Never write the
  board with the Write or Edit tools, never print the whole board, and leave
  no helper scripts behind.
- **Orchestrators never do an agent's work**: no code, tests, `task.md`,
  reviews or verdicts. They never install anything.
- **Exit codes.** A command that finds, lists or counts things exits 0 when
  it succeeds, including when it finds nothing, so a non-zero exit always
  means an error. The only exceptions are plain git checks whose exit status
  is the answer; each one is labelled "(plain git check)" where it appears.
  Every `factory.py` command exits 0 with its answer, and non-zero only on an
  error, with the reason on stderr.
- **Halts never revert anything.** They leave every change in place for the
  human to inspect.

## Spawn prompts

These are the only prompts. Fill in the ids; add nothing.

| Agent | When | Prompt |
|---|---|---|
| `planner` | spawn | `Spec: factory/state/SPEC.md. Linked documents: factory/state/spec/.` |
| `task-orchestrator` | spawn | `Task-id: <task-id>.` |
| `builder` | spawn, or resume for a new round | `Task-id: <task-id>. Round-id: <round-id>.` |
| `builder` | it left uncommitted changes | `Task-id: <task-id>. Round-id: <round-id>. Uncommitted changes: commit them, then update the build notes.` |
| `<key>-reviewer` | spawn | `Task-id: <task-id>. Round-id: <round-id>.` |
| `approver` | spawn | `Task-id: <task-id>. Round-id: <round-id>.` |

## Git checks

- Clean tree: `git status --porcelain=v1 --untracked-files=all` prints
  nothing. Files git ignores (such as build outputs) never show, so they
  don't count. The option makes the check independent of
  `status.showUntrackedFiles`.
- Current branch: `git branch --show-current`.
- Current commit: `git rev-parse HEAD`.
- State ignored: `git check-ignore -q factory/state` (plain git check: exit 0
  means ignored, 1 means not ignored).
- Branch exists: `git rev-parse --verify --quiet refs/heads/factory/<task-id>`
  (plain git check: exit 0 means it exists, 1 means it doesn't).
- Merge in progress: `git ls-files -u` lists unmerged paths (empty when
  there are none).

## The backlog

`backlog.md` is the execution plan: a heading and a Markdown table with one
row per task, in implementation order from top to bottom.

```markdown
# Backlog

| Task | Status |
|---|---|
| 001-scaffold-project | Complete |
| 002-apply-selected-theme | In progress |
| 003-save-user-settings | Planned |
```

Statuses: `Planned`, `In progress`, `Escalated`, `Complete`, `Abandoned`.
The Planner writes the file; after that, the orchestrators are its only
writers (the human may edit it during the plan review). Change a status only
with

```
python3 -I .claude/skills/factory/factory.py backlog-set <task-id> "<status>"
```

which rewrites that row's status cell and nothing else, and refreshes the
task's entry on the board when the board exists. Read the backlog with
`factory.py tasks`.

## Write verification

After each subagent finishes, check what it changed, both outside and inside
the state.

### Inside the state: the snapshot

`git status` can't see the state, so compare checksums. The snapshot file is
`T/.snapshot` for agents working on a task, and `factory/state/.snapshot` for
the Planner. Just before spawning (or resuming) an agent, write it:

```
find factory/state -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | LC_ALL=C sort -z | xargs -0 -r cksum > factory/state/tasks/<task-id>/.snapshot
```

After the agent finishes, list the changed paths (added, removed or
modified), one per line; empty output means nothing changed:

```
test -s factory/state/tasks/<task-id>/.snapshot && { find factory/state -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | LC_ALL=C sort -z | xargs -0 -r cksum | diff factory/state/tasks/<task-id>/.snapshot - | sed -n 's/^[<>] [0-9]* [0-9]* //p' | LC_ALL=C sort -u; }
```

A non-zero exit means the snapshot is missing: halt. For the Planner, use
`factory/state/.snapshot` in both commands. The `board.json*` files and
`.snapshot` files are always left out.

### Outside the state: git

Record `git branch --show-current` and `git rev-parse HEAD` just before
spawning the agent, then check afterwards with the git checks above.

### What each agent may change

| Agent | Outside the state | Changed paths inside the state |
|---|---|---|
| `planner` | nothing: clean tree, still on `main`, `HEAD` unchanged | only `factory/state/preflight.md`, `factory/state/stack.md`, `factory/state/backlog.md` and `factory/state/tasks/*/task.md` |
| `builder` | commits on the task branch only: clean tree, on `factory/<task-id>`, and `git diff --name-only <base> HEAD -- factory .claude/skills/factory .claude/agents` prints nothing (`<base>` from `R/base-commit`), and `git check-ignore -q factory/state` and `git check-ignore -q factory/archive` both still exit 0 (plain git checks) | only `R/build-notes.md` |
| each `<key>-reviewer` | nothing: clean tree, branch and `HEAD` unchanged | after each one finishes: only paths under `R/` |
| all reviewers of a round | | once all have finished: only `R/<key>-review.md` and `R/<key>/**` for the task's listed keys, and every file in `R/` is one of `build-notes.md`, `base-commit`, `human-review.md`, `approver-review.md`, or a listed key's `<key>-review.md` or `<key>/**` (check with `find factory/state/tasks/<task-id>/<round-id> -type f`) |
| `approver` | nothing: clean tree, branch and `HEAD` unchanged | only `R/verdict.md` |

The builder's dirty tree has its own remedy: if the tree isn't clean after
the builder finishes, resume it once with the uncommitted-changes prompt (see
"Spawn prompts"), then check again. If it still isn't clean, halt.

Any other difference is a violation. On a violation, halt (see "Halting")
with a reason that names the agent and lists what changed, revert nothing,
and tell the human to inspect those changes and commit or remove them before
`/factory continue`.

## Rounds

- **Directories.** A round is `R = T/round-<n>/`. The orchestrator creates a
  new round directory at the beginning of each round.
- **`base-commit`** holds the commit the round started from, on one line.
  Round 1: `git rev-parse main`. Later rounds: `git rev-parse HEAD` on the
  task branch. Write the round's note (`human-review.md` or
  `approver-review.md`) first and `base-commit` last, before spawning any
  agent for the round, so a round directory with a `base-commit` is complete.
  Write it with `git rev-parse <rev> > <R>/base-commit`.
- **Kind**, from the round's files (`factory.py round kind <task-id> <round-id>`):
  `rework` if it has `human-review.md`, `approver-rework` if it has
  `approver-review.md`, otherwise `auto`.
- **Latest and next.** The latest round is the highest-numbered round
  directory (`factory.py round latest <task-id>`, which prints `none` when
  there is none). The next round-id is one above it
  (`factory.py round next <task-id>`).
- **Previous round:** the latest earlier round directory that has a
  `base-commit`; directories without one are never a previous round
  (`factory.py round previous <task-id> <round-id>`).
- **A round's state** (kind, `base-commit`, build result, each listed
  reviewer's result, verdict, previous round):
  `factory.py round state <task-id> [<round-id>]`. A missing file shows as
  `missing`, a file with a bad first line as `invalid`.
- **Earlier REWORK verdicts:** `factory.py rework-count <task-id> <round-id>`
  counts the `VERDICT: REWORK` verdicts in the rounds before `<round-id>`.
- **Rework note:** `factory.py rework-note <task-id> <round-id>` prints
  `rework-note: none` when `R/verdict.md` has no `## Rework note` section
  with text, or `rework-note: yes` and the section. With
  `--out <path>` it writes the section to `<path>` instead.
- **Limits.** At most two automatic rounds per task: round 1 and one fix
  round. At most two Approver rework rounds (REWORK verdicts) per task.
  Human rework rounds (`/factory rework`) are unlimited and count against
  neither limit. Neither kind of rework round triggers an automatic fix
  round: after the builder and the reviewers, the Approver runs straight
  away.
- **Interrupted round creation.** On re-entry, a round directory without
  `base-commit` was interrupted while it was being created. If the previous
  round's verdict is `VERDICT: REWORK`, it is an Approver rework round: copy
  the note again with `factory.py rework-note <task-id> <previous> --out
  <R>/approver-review.md`, then write `base-commit`. Any other round just gets
  its `base-commit`. Then the work goes on. (A human rework round is never
  found like this while the task is `In progress`, because `/factory rework`
  marks the task only after the round is complete.)

### Fixed first lines

| File | Allowed first lines |
|---|---|
| `<key>-review.md` | `VERDICT: PASS`, `VERDICT: FAIL` |
| `verdict.md` | `VERDICT: APPROVE`, `VERDICT: REWORK`, `VERDICT: ESCALATE` |
| `build-notes.md` | `STATUS: DONE`, `STATUS: BLOCKED` |
| `preflight.md` | `PREFLIGHT: READY`, `PREFLIGHT: TOOLS`, `PREFLIGHT: BLOCKED` |

Read a result with `factory.py first-line <path>`, which prints the result
word (`PASS`, `APPROVE`, `DONE`, `READY` and so on). It exits non-zero when
the file is missing or its first line is anything else, including empty.
That is an error that halts the loop. `factory.py check-firstlines
[<task-id>]` checks every such file at once and prints `first-lines: ok` or
one `invalid:` line per bad file.

## The preflight state

The run is in the preflight state when `preflight.md` exists and
`backlog.md` doesn't (`factory.py tasks` prints `phase: preflight`). The
preflight stopped, and the human is fixing the spec or installing tools. To
restart it, re-copy the spec with `factory.py spec-copy` (no argument): it
reads the path in `spec-source`, replaces `SPEC.md` and `spec/` the same way
`/factory start` created them, and lists any linked document that is
missing. `phase: none` means there is no run at all.

## The reviewer roster

| Key | Agent type | Review file | Runs on |
|---|---|---|---|
| `code` | `code-reviewer` | `code-review.md` | every task |
| `feature` | `feature-reviewer` | `feature-review.md` | every task |
| `security` | `security-reviewer` | `security-review.md` | tasks that list it |
| `privacy` | `privacy-reviewer` | `privacy-review.md` | tasks that list it |
| `api` | `api-reviewer` | `api-review.md` | tasks that list it |
| `ui` | `ui-reviewer` | `ui-review.md` | tasks that list it |

The order of the rows is roster order: the order of the reviewer chips on
the dashboard and of each round's `reviews` on the board.

A task's reviewers come from the `Reviews` section of its `task.md`, and
`code` and `feature` always run, whatever it says. A `task.md` without a
`Reviews` section runs `code`, `feature`, and `ui` unless its `UI` section is
`UI: none`. Read them with `factory.py reviewers <task-id>`, which prints the
keys in roster order, separated by spaces. Never work them out by hand.

## `board.json`

The board reflects the loop for the dashboard. It can always be rebuilt from
`backlog.md` and the task files. The factory script works every task entry
out from the files, so update the board after every agent finishes and on
every stage change, and it is right.

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

- `loop.state`: `planning`, `awaiting-preflight` (the preflight stopped;
  when rebuilding, this is the state whenever `preflight.md` exists and
  `backlog.md` doesn't), `awaiting-plan-review`, `running`, `escalated`,
  `halted` (merge conflict, write violation, error; the reason goes in
  `message`), or `complete`. `loop.current` is the task being worked on, or
  `null`.
- `agents`: builder agent ids keyed by name, `builder-<task-id>`. An id is
  only usable by the task orchestrator that spawned it; a later one spawns a
  fresh builder and replaces the id.
- `stage`: `planned`, `building`, `review`, `approval`, `escalated`, `done`
  or `abandoned`.
- `kind`: `auto`, `approver-rework` or `rework`.
- `build`: `pending`, `DONE` or `BLOCKED`.
- `reviews`: one key per reviewer the task lists, in roster order, each
  `pending`, `PASS` or `FAIL`. A review that hasn't landed (or has a bad
  first line) is `pending`. A round whose build is `BLOCKED` has
  `"reviews": {}`, because its reviewers never run.
- `verdict`: `null`, `APPROVE`, `REWORK` or `ESCALATE`.
- `files`: `task.md`, every Markdown file in the round directories, and every
  file in the reviewers' evidence directories (`round-*/<key>/`), relative to
  the task directory.

How the script works out a task's stage: `Planned` is `planned`,
`Escalated` is `escalated`, `Complete` is `done`, `Abandoned` is
`abandoned`. An `In progress` task is `done` once merged; otherwise, from
its latest round, `building` until the build notes say `STATUS: DONE`,
`review` until every listed review has landed, then `building` again if a
fix round follows (round 1 with a FAIL), and `approval` otherwise.

## The factory script

`.claude/skills/factory/factory.py` does the factory's mechanics, so the
same files always give the same answer. The orchestrators keep the judgement:
what to do next and what to tell the human. Every command exits 0 with its
answer when it succeeds; non-zero is an error, with the reason on stderr.

| Command | Prints or does | Used by |
|---|---|---|
| `factory.py board rebuild [--state <s>] [--message <m>]` | Creates the board, or rebuilds every task entry from `backlog.md` and the task files, keeping the agents and (unless given) the loop. Without a loop and without `--state`, it derives the state from the files. | main: after the Planner, at the plan review, when the board is missing |
| `factory.py board set [--state <s>] [--message <m>] [--current <task-id> \| --clear-current] [--agent builder-<task-id>=<agent-id>] [--task <task-id>]` | Updates `updated`, the loop, the agents, and one task's entry, worked out from that task's files. Creates the board from the files first if it is missing. | both levels, on every update |
| `factory.py board show [--task <task-id>]` | The loop and agents, and one task's status, stage and latest round, as JSON (`"board": "missing"` without a board). | both levels, to read the board |
| `factory.py spec-copy [<spec>]` | Writes the spec's absolute path (with `~` expanded) to `spec-source`, copies the spec to `SPEC.md` and every Markdown file it links to with a relative link, transitively, to `spec/`, with links rewritten to resolve from the copies, replacing earlier copies. Lists each copy and each `missing:` linked document (or `missing: none`). Without `<spec>` it re-reads `spec-source`. | main: `start`, preflight restart |
| `factory.py plan-check` | `plan: ok (<n> tasks)`, or `plan: <n> problems` and one `- ` line per problem (the mechanical part of "Checking the plan"). | Planner, main |
| `factory.py max-serial` | `highest-used: <serial>` (or `none`) and `next: <serial>`, from `factory: merge <id>` subjects on main and local `factory/<id>` branches. | Planner |
| `factory.py round latest <task-id>` | The latest round-id, or `none`. | both |
| `factory.py round next <task-id>` | The next round-id. | both |
| `factory.py round previous <task-id> <round-id>` | The previous round-id, or `none`. | task orchestrator, Builder, reviewers |
| `factory.py round kind <task-id> <round-id>` | `auto`, `approver-rework` or `rework`. | task orchestrator, agents |
| `factory.py round state <task-id> [<round-id>]` | The round's kind, `base-commit`, build, reviews, verdict and previous round (latest round by default). | both |
| `factory.py rework-count <task-id> <round-id>` | The number of REWORK verdicts before `<round-id>`. | task orchestrator, Approver |
| `factory.py rework-note <task-id> <round-id> [--out <path>]` | Whether `R/verdict.md` has a rework note with text, and the section (or writes it to `<path>`). | task orchestrator |
| `factory.py reviewers <task-id>` | The task's reviewer keys in roster order. | task orchestrator, agents |
| `factory.py dependents <task-id>` | Each backlog task that depends on the task, with its status, or `none`. | main: `abandon` |
| `factory.py merged <task-id>` | `yes` or `no` (see "Merged"). | both |
| `factory.py tasks` | `phase: none`, `phase: preflight`, or `phase: backlog` followed by one `task: <id> \| <status> \| <stage> \| <latest round>` line per task, `active:` (the `In progress` or `Escalated` task) and `next:` (the first `Planned` task), each `none` when there is none. | main |
| `factory.py backlog-set <task-id> <status>` | Changes the task's status in `backlog.md`, and refreshes its board entry if the board exists. | both |
| `factory.py first-line <path>` | The result word of a fixed first line; non-zero exit if missing or invalid. | both |
| `factory.py check-firstlines [<task-id>]` | `first-lines: ok (<n> files)` or one `invalid:` line per bad file. | both |

### Merged

A task is merged if `git log main --format=%s` contains the subject
`factory: merge <task-id>`, or if its branch exists, is an ancestor of main
(`git merge-base --is-ancestor`), and has at least one builder commit (a
subject starting with `<task-id>: `). A branch with no builder commits is not
merged, even though it is an ancestor of main. Always ask
`factory.py merged <task-id>`.

## Shared steps

### Merge

Used by the task orchestrator after APPROVE and by the main orchestrator for
`/factory approve`. The tree must be clean.

1. Switch to main: `git switch main`.
2. Merge: `git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>`.
3. If it exits 0, the merge succeeded. Stay on `main`.
4. If it stopped with conflicts (`git ls-files -u` lists paths), leave the
   merge exactly as it is: never abort it, resolve it or commit it. The
   result is a merge conflict. The task is (or stays) `In progress`. Halt
   (see "Halting") with the message `Merge conflict merging
   factory/<task-id> into main` and tell the human to resolve the conflicts,
   complete the merge with `git commit`, and run `/factory continue`, which
   finds the task merged and marks it `Complete`.
5. If any step fails in another way, halt with git's error as the reason.

After a successful merge, the main orchestrator completes the task:
`factory.py backlog-set <task-id> Complete`, then
`git branch -d factory/<task-id>` (if that refuses, halt instead of forcing),
then `factory.py board set --task <task-id>`.

### Halting

1. The task orchestrator first waits for any of its agents that are still
   running (end the turn with `WAITING`, spawn nothing new), and only then
   halts.
2. Set the state:
   `factory.py board set --state halted --message "<reason>"`, adding
   `--task <task-id>` when a task is involved.
3. Revert nothing.
4. Report. The task orchestrator ends with the one-line final message
   `halted <task-id>`. The main orchestrator tells the human the reason, what
   to inspect or fix, and that `/factory continue` resumes.
