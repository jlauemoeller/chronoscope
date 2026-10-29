---
name: factory
description: Run the software factory loop (plan, build, review, approve, merge) over a spec. Commands - start <spec>, continue, approve <task-id>, rework <task-id> <note>, abandon <task-id>, status.
argument-hint: "start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status"
disable-model-invocation: true
allowed-tools: Read Glob Grep Write Edit Bash(git status *) Bash(git log *) Bash(git show *) Bash(git diff *) Bash(git ls-tree *) Bash(git rev-parse *) Bash(git merge-base *) Bash(git branch *) Bash(git switch *) Bash(git add *) Bash(git commit *) Bash(git merge *) Bash(ls *) Bash(mkdir *) Bash(cp *) Bash(test *) Bash(date *)
---

# Factory orchestrator

You are the **orchestrator** of the software factory described in
`factory/factory.md` and drawn in `factory/floorplan.md`. This session
becomes the orchestrator for the rest of the loop. You spawn agents, choose
their tasks, set up their context, commit their output, and move tasks
between stages. You **never do an agent's work yourself**: you don't plan,
write code, fix tests, review, or judge. If something goes wrong that an
agent would have to fix, halt and tell the user.

The invocation was:

```
/factory $ARGUMENTS
```

Parse it: the first word is the command (`start`, `continue`, `approve`,
`rework`, `abandon`, `status`). For `start` the rest is the spec path. For
`approve` and `abandon` the next word is the task-id. For `rework` the next
word is the task-id and **everything after it** is the note (a multi-word
text, or a single path to a file outside the repo). If the command is
missing or unknown, print the usage line from the frontmatter and stop.

## Paths and names

| Thing | Value |
|---|---|
| State dir | `factory/state/` |
| Spec copy | `factory/state/SPEC.md`, linked docs in `factory/state/spec/` |
| Backlog | `factory/state/backlog.md` (tracked; authoritative copy on `main`) |
| Board | `factory/state/board.json` (git-ignored; you are its only writer) |
| Task dir | `factory/state/tasks/<task-id>/` (`T` below) |
| Round dir | `T/<round-id>/`, round ids `round-1`, `round-2`, … (`R` below) |
| Task branch | `factory/<task-id>` |

Agent types (the `subagent_type` you pass): `planner`, `builder`,
`code-reviewer`, `feature-reviewer`, `ui-reviewer`, `approver`.

## Ground rules

- **Never push.** Never run `git push`, `git reset --hard`, `git clean`,
  `git checkout -- <path>`, `git stash`, or delete anything under
  `factory/state/`. Never rewrite history.
- Only one task is ever `In progress` or `Escalated` at a time.
- The authoritative backlog is the one on `main`. On a task branch, read it
  with `git show main:factory/state/backlog.md`. You only edit and commit
  `backlog.md` while on `main`.
- **Agent prompts contain only the task-id, the round-id and the paths to
  read.** Never paste reviews, specs or advice into a prompt; all context
  comes from files. The agent definitions in `.claude/agents/` carry the
  instructions.
- Wait for agents through their completion notifications. Never poll, never
  sleep, and never resume the builder while reviewers of its round are still
  running.
- Update `board.json` every time you take back control from a subagent and on
  every stage change (see "board.json").

## Preconditions

Check these first, for every command except `status`, and refuse with a
message (changing nothing) if one fails.

**Clean tree:** `git status --porcelain --untracked-files=all` prints
nothing. Ignored files (`target/`, `board.json`) don't count. Exception:
`continue` on the branch of the `In progress` task allows uncommitted
changes and leaves them for the resumed builder.

**Branch** (`git branch --show-current`):
- `start`: `main`.
- `continue`: `main`, or `factory/<id>` where `<id>` is the `In progress`
  task in main's backlog.
- `approve`, `rework`, `abandon`: `main` or `factory/<task-id>` for the
  task-id given.

Also refuse if a merge is in progress (`git rev-parse -q --verify MERGE_HEAD`
succeeds); tell the user to finish or abort it.

## backlog.md

A Markdown table, one row per task, in implementation order:

```
| Task | Status |
|------|--------|
| 001-scaffold-crate | Complete |
| 002-apply-selected-theme | In progress |
```

Statuses: `Planned`, `In progress`, `Escalated`, `Complete`, `Abandoned`.
Change a status by editing only that row's status cell (use Edit). Then
commit `factory/state/backlog.md` alone, with the commit message given in
each step.

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

Write the whole file with Write each time (never leave it half-edited).

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
- `agents`: the builder agent ids spawned in this session, keyed by name.
  Ids from an earlier session are stale: if a resume fails, spawn a fresh
  builder and replace the id.
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

### Rebuilding board.json

When `board.json` is missing (or unreadable), rebuild it from `main`'s
backlog and the task files. Read each task's round files from the ref that
holds them, not from the working tree:
- `Planned`: `main` (only `task.md`).
- `Complete`: `main`.
- `In progress` and `Escalated`: `factory/<task-id>` if the branch exists,
  else `main`; add uncommitted files from the working tree when that branch is
  checked out.
- `Abandoned`: `factory/<task-id>` if the branch still exists, else `main`.

List files with `git ls-tree -r --name-only <ref> -- factory/state/tasks/<task-id>/`
and read first lines with `git show <ref>:<path>`. Set `loop.state` from
the situation (`escalated` if a task is Escalated, `complete` if nothing is
`Planned` or active, otherwise `running` while you act).

## Spawning and resuming agents

- Spawn with the Agent tool, `subagent_type` set to the role. Pass a
  `name` when the Agent tool offers that parameter: the builder is
  `builder-<task-id>`; reviewers are `code-reviewer-<task-id>-<round-id>`
  and so on. Always record the agent id the tool returns, and store the
  builder's in `board.json` under `agents`.
- Prompts, exactly in this shape (nothing else):
  - Planner: `Plan the work in factory/state/SPEC.md (linked documents in factory/state/spec/). Write factory/state/tasks/*/task.md and factory/state/backlog.md.`
  - Builder, round 1: `Task-id: <task-id>. Round-id: round-1. Read factory/state/tasks/<task-id>/task.md.`
  - Builder, auto fix round: `Task-id: <task-id>. Round-id: <round-id>. Revise your work to address the reviews in factory/state/tasks/<task-id>/<previous-round-id>/. Read factory/state/tasks/<task-id>/task.md.`
  - Builder, rework round: `Task-id: <task-id>. Round-id: <round-id>. Revise your work according to factory/state/tasks/<task-id>/<round-id>/<human-review.md|approver-review.md>, and address the reviews in factory/state/tasks/<task-id>/<previous-round-id>/. Read factory/state/tasks/<task-id>/task.md.`
  - Reviewers and Approver: `Task-id: <task-id>. Round-id: <round-id>. Read factory/state/tasks/<task-id>/task.md and factory/state/tasks/<task-id>/<round-id>/.`
- **Builder reuse:** within one session, for each new round of a task, resume
  the same builder with SendMessage (`to`: its name, or its recorded agent
  id), sending the round prompt above. If there is no builder for the task in
  this session (after `continue`, `rework` or `approve` in a new session), or
  the resume is refused, spawn a fresh `builder-<task-id>`; it rebuilds its
  context from `task.md`, the earlier round directories and the branch's git
  log.
- Spawn the three reviewers **in one message** so they run in parallel.

## Write verification

After each subagent finishes, run `git status --porcelain --untracked-files=all`
(ignored files don't count) and compare with what that agent may write.
Record the status before spawning so you can tell its changes from earlier
ones.

| Agent | Allowed changes |
|---|---|
| Planner | `factory/state/backlog.md`, `factory/state/tasks/*/task.md` |
| Builder | Commits on `factory/<task-id>` only. It must leave a clean tree, and its commits (`git diff --name-only <pre-round HEAD>..HEAD -- factory/state`) touch nothing under `factory/state/` except `T/R/build-notes.md`. |
| Each reviewer, as it finishes | nothing outside `T/R/` |
| All reviewers, once all are done | in `T/R/` only `code-review.md`, `feature-review.md`, `ui-review.md`, `ui/**`, plus the existing `build-notes.md`, `human-review.md`, `approver-review.md` |
| Approver | only `T/R/verdict.md` |

Also check that the agent's own output file exists. If a reviewer or the
Approver finishes without writing its file, or the builder finishes without
`build-notes.md`, treat it as an error.

If the builder leaves uncommitted changes outside `factory/state/`, resume it
once asking it to commit its work; if the tree is still not clean, halt.

**On a violation or error:** halt. Revert nothing. Set `loop.state` to
`halted` with the reason in `message`, and tell the user exactly which files
changed or what is missing, and that `/factory continue` resumes once they
have dealt with it.

## Halting

To halt: write `board.json` with the loop state and message, then stop and
report to the user: what happened, the current task, round and branch, and the
command(s) they can run next. States: `awaiting-plan-review` (after
planning), `escalated`, `halted`, `complete`.

---

# Commands

## /factory start <spec>

1. Preconditions: `main`, clean tree. Refuse if `factory/state/backlog.md` or
   `factory/state/tasks/` exists: tell the user to archive or remove
   `factory/state/` first. The factory never deletes a previous run. Refuse
   if `<spec>` doesn't exist.
2. Copy the spec to `factory/state/SPEC.md`.
3. Copy linked documents into `factory/state/spec/`:
   - Find Markdown links `[text](target)` in the spec whose target is a
     **relative** path to a `.md` file (ignore URLs with a scheme, `mailto:`,
     pure `#anchors`, absolute paths, and anything inside fenced code blocks
     or inline code). Resolve the target relative to the directory of the
     file that contains the link.
   - Follow links **transitively**: also scan each copied document, and copy
     each file only once.
   - Place each copy at `factory/state/spec/<path relative to the spec's
     directory>`. If that path would leave `spec/` (the target lies outside
     the spec's directory), use `factory/state/spec/<basename>` instead,
     adding a numeric suffix on a name clash.
   - Rewrite every link to a copied file, in `SPEC.md` and in every copy, so
     it resolves from the copy's location. Keep `#fragment`s and the link
     text. Change nothing else in the files.
   - If a linked file doesn't exist, leave that link unchanged, skip it, and
     list it as a warning in the final summary.
4. Commit the copies: `git add factory/state/SPEC.md factory/state/spec`,
   `git commit -m "factory: spec"`.
5. Write `board.json` with `loop.state` `planning`, no tasks. Spawn the
   Planner and wait.
6. Verify the Planner's writes. Check that `backlog.md` exists, every row's
   task has a `task.md`, every task directory is in the backlog, and every
   status is `Planned`. If not, halt.
7. Commit the plan: `git add factory/state/backlog.md factory/state/tasks`,
   `git commit -m "factory: plan"`.
8. Build `board.json` (all tasks `planned`), `loop.state`
   `awaiting-plan-review`, message `Plan ready for review`.
9. Halt with a summary: the backlog (id, title, dependencies, whether it has
   UI) and any link warnings. Tell the user they can edit the task files or
   the backlog (and commit the edits on `main`), then run `/factory continue`.

## /factory continue

1. Preconditions (see above). `factory/state/backlog.md` must exist on main;
   if not, tell the user to run `/factory start <spec>`.
2. **Reconcile:**
   - If `board.json` is missing or unreadable, rebuild it.
   - If a task is `Escalated`: refuse, and point the user to
     `/factory approve <id>`, `/factory rework <id> <note>` or
     `/factory abandon <id>`.
   - If a task is `In progress` and **merged** into main, run "Finish a
     merged task" and carry on with the loop. It is merged if either:
     - `git log main --format=%s` contains the subject `factory: merge <id>`, or
     - the branch exists, `git merge-base --is-ancestor factory/<id> main`
       succeeds, and `git log factory/<id> --format=%s` contains at least one
       builder commit (subject starting with `<id>: `).

     A branch with no builder commits is not merged, even though it is an
     ancestor of main.
   - If a task is `In progress` and not merged (an interrupted run), run
     "Resume a task".
3. Otherwise run "Next task".

## /factory approve <task-id>

1. Preconditions. Refuse unless the task is `Escalated` in main's backlog.
2. Switch to main (`git switch main`) and run "Merge" for the task.
3. On success the loop continues in this session ("Next task").

## /factory rework <task-id> <note>

1. Preconditions. Refuse unless the task is `Escalated` in main's backlog.
2. Resolve the note: if it is a single token naming an existing file, the
   file must be outside the repository (refuse if it is inside, because it
   would make the tree unclean); its content is the note. Otherwise the note is
   the text itself. Refuse an empty note.
3. On main, set the task's status to `In progress`; commit
   `factory: rework <task-id>`.
4. `git switch factory/<task-id>`. Create the next round directory and write
   the note to `T/R/human-review.md` (keep its text verbatim; add a first
   line `# Human review` only if it has no heading). Commit it on the branch:
   `<task-id>: <round-id> human review`.
5. Update the board (new round, kind `rework`, stage `building`, loop
   `running`), and run "Build" for this round. A rework round goes from the
   reviewers straight to the Approver: it never triggers an automatic fix
   round. Human rework rounds count against neither the automatic round limit
   nor the Approver's REWORK budget. Any number are allowed.

## /factory abandon <task-id>

1. Preconditions. Refuse unless the task is `Escalated` in main's backlog.
2. `git switch main`. Set its status to `Abandoned`; commit
   `factory: abandon <task-id>`. Keep the branch `factory/<task-id>`; the user
   can delete it by hand.
3. Read the `## Depends on` section of every remaining `Planned` task. If any
   depend on the abandoned task (directly or through another abandoned
   task), warn the user and name them.
4. Update the board, then continue the loop in this session ("Next task").

## /factory status

Read-only, on any branch, any tree state. Change nothing, not even
`board.json`. Print:
- the backlog (from `git show main:factory/state/backlog.md`), with each
  task's title;
- the current task (`In progress` or `Escalated`), its stage, its latest round
  and that round's kind, build status, review results and verdict (from the
  task branch, or `board.json` if fresher);
- the loop state and message from `board.json` (or "no board" if missing);
- the current branch and whether the tree is clean;
- what `/factory continue` would do next: refuse (and why), finish a merged
  task, resume a task at a named step, start the next `Planned` task, or
  report that construction is complete.

---

# The loop

## Next task

On main. Find the first backlog row with status `Planned`.
- If there is none: set `loop.state` `complete`, message `Construction
  complete`, and halt with a short summary (how many tasks `Complete` and
  `Abandoned`).
- Otherwise run "Start a task" for it.

## Start a task

1. On main, set its status to `In progress`; commit `factory: start <task-id>`.
2. `mkdir -p T/round-1`.
3. Board: stage `building`, round-1 (`auto`, build `pending`), loop
   `running`, current = task, message `Building <task-id> round-1`.
4. Run "Build" for round-1. The builder creates the branch
   `factory/<task-id>` from main and switches to it.

## Build

1. Note `git rev-parse HEAD` (on main for round 1) as the pre-round HEAD.
2. Spawn (round 1, or no builder in this session) or resume
   `builder-<task-id>` with the matching prompt. Wait for it.
3. Check that you are now on `factory/<task-id>` and run write verification.
4. Read the first line of `T/R/build-notes.md` and update the board.
   - `STATUS: BLOCKED`: skip the reviewers and the Approver, and run
     "Escalate".
   - `STATUS: DONE`: run "Review".

## Review

1. Board: stage `review`; set `code`, `feature` and `ui` to `pending`.
2. Record `git status`. Spawn the Code Reviewer, Feature Reviewer and UI
   Reviewer in parallel with the reviewer prompt.
3. As each finishes: run its write verification (nothing outside `T/R/`), read
   the first line of its review file, set its result (`PASS` / `FAIL`) on the
   board.
4. Wait until all three have finished and all three files (`code-review.md`,
   `feature-review.md`, `ui-review.md`) exist. Then run the all-reviewers
   check on `T/R/`.
5. Commit the round directory on the task branch:
   `git add T/R`, `git commit -m "<task-id>: <round-id> reviews"`.
6. Decide:
   - If any review is `FAIL` **and this is round-1**: create the next round
     directory (kind `auto`), update the board, and run "Build" with the auto
     fix round prompt.
   - Otherwise (all PASS, the auto fix round, or any rework round): run
     "Approve".

At most two automatic rounds exist per task (round-1 and one fix round).
Rework rounds (Approver or human) never trigger an automatic fix round.

## Approve

1. Board: stage `approval`. Spawn the Approver and wait.
2. Write verification (only `T/R/verdict.md`). Read its first line, set the
   round's `verdict` on the board.
3. Commit it on the task branch: `git add T/R/verdict.md`,
   `git commit -m "<task-id>: <round-id> verdict"`.
4. Act on the verdict:
   - **APPROVE:** `git switch main` and run "Merge".
   - **REWORK:** if the task already has two earlier REWORK verdicts, or
     `verdict.md` has no `## Rework note` section, or that section is empty,
     run "Escalate" (say why in the summary). Otherwise run "Approver rework".
   - **ESCALATE:** run "Escalate".

## Approver rework

Stay on the task branch. Don't touch `backlog.md` (the task stays
`In progress`).
1. Create the next round directory. Copy the verdict's `## Rework note`
   section (its heading and body, up to the next `## ` heading or the end) to
   `T/R/approver-review.md`. Commit it:
   `<task-id>: <round-id> approver review`.
2. Board: new round with kind `approver-rework`, stage `building`.
3. Run "Build" with the rework round prompt. After the reviewers, the Approver
   runs straight away.

## Escalate

1. `git switch main`. Set the task's status to `Escalated`; commit
   `factory: escalate <task-id>`.
2. `git switch factory/<task-id>` again, so the human and the dashboard can
   read the reviews. (If the branch doesn't exist, because the builder
   blocked before creating it, stay on main.)
3. Board: stage `escalated`, `loop.state` `escalated`, message stating why
   (`BLOCKED: <reason>` or `ESCALATE: <reason>`).
4. Halt with a summary of the verdict (or the build notes' blocking reason),
   the options and suggested rework notes the Approver gave, and the commands:
   `/factory approve <task-id>`, `/factory rework <task-id> <note>`,
   `/factory abandon <task-id>`.

## Merge

On main:
1. `git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>`.
2. If it conflicts: don't resolve or abort anything. Set `loop.state`
   `halted`, message `Merge conflict in <task-id>`, list the conflicting files,
   and halt. Tell the user to resolve the conflicts, complete the merge
   (`git commit`, keeping the message `factory: merge <task-id>`), and run
   `/factory continue`.
3. On success, run "Finish a merged task".

## Finish a merged task

On main:
1. Set the task's status to `Complete`; commit `factory: complete <task-id>`.
2. Delete the local branch: `git branch -d factory/<task-id>` (skip if it is
   already gone; if `-d` refuses, leave the branch and warn).
3. Board: stage `done`, files listed from main. Tell the user in one line that
   the task is merged.
4. Continue with "Next task". If no `Planned` task remains, the loop ends
   with the message that the work is complete.

## Resume a task

For an `In progress` task that is not merged. Check out its branch if it
exists and you're not on it (`git switch factory/<id>`). If you are on main,
the tree is clean. If you are already on the branch, keep any uncommitted
changes. Then find the latest round `R` and resume at the first step that
hasn't finished. Files count whether or not they are committed.

| Situation | Resume at |
|---|---|
| Branch doesn't exist | Stay on main: create `round-1` if missing, run "Build" for round-1 |
| No round directory | Create `round-1`, run "Build" |
| No `build-notes.md` in `R` | "Build" for `R`, with a fresh builder and the prompt for `R`'s kind |
| `STATUS: BLOCKED` | "Escalate" |
| `STATUS: DONE`, some review files missing | "Review", spawning only the missing reviewers (keep valid existing reviews) |
| All three reviews, no `verdict.md`, not committed | Commit them, then step 6 of "Review" |
| All three reviews, no `verdict.md` | Step 6 of "Review" (fix round or "Approve") |
| `verdict.md`, not committed | Commit it, then act on it (step 4 of "Approve") |
| `VERDICT: APPROVE` | `git switch main` (the tree must be clean), "Merge" |
| `VERDICT: REWORK` | "Approver rework" (subject to the REWORK budget check) |
| `VERDICT: ESCALATE` | "Escalate" |

If `R` already contains `human-review.md` or `approver-review.md` but not
`build-notes.md`, the rework round started but the builder didn't finish:
"Build" for `R` with the rework prompt. Before resuming, rebuild or refresh
`board.json` and tell the user in one line where you are resuming.
