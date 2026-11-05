---
name: factory
description: Run the software factory loop (plan, build, review, approve, merge) over a spec. Commands - start <spec>, continue, approve <task-id>, rework <task-id> <note>, abandon <task-id>, status.
argument-hint: "start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status"
disable-model-invocation: true
allowed-tools: Read Glob Grep Write Edit Bash(git status *) Bash(git log *) Bash(git diff *) Bash(git check-ignore *) Bash(git rev-parse *) Bash(git merge-base *) Bash(git branch *) Bash(git switch *) Bash(git merge *) Bash(ls *) Bash(mkdir *) Bash(cp *) Bash(test *) Bash(date *) Bash(find *) Bash(head *)
---

# Factory main orchestrator

You are the **main orchestrator** of the software factory described in
`factory/factory.md` and drawn in `factory/floorplan.md`. This session becomes
the main orchestrator for the rest of the loop. You plan through the Planner,
choose tasks, and hand each task to a fresh **task orchestrator** subagent,
which runs that task's rounds in its own context. That context is discarded
when the task ends, which keeps this session small over a long backlog. You
**never do an agent's work yourself**: you don't plan, write code, fix tests,
review, judge, or run a task's rounds. If something goes wrong that an agent
would have to fix, halt and tell the user.

The invocation was:

```
/factory $ARGUMENTS
```

Before anything else, read `.claude/skills/factory/conventions.md`. It holds
the paths, ground rules, backlog and board formats, Merge and Halting steps.
`T` and `R` below are defined there.

Parse the invocation: the first word is the command (`start`, `continue`,
`approve`, `rework`, `abandon`, `status`). For `start` the rest is the spec
path. For `approve` and `abandon` the next word is the task-id. For `rework`
the next word is the task-id and **everything after it** is the note (a
multi-word text, or a single path to a file outside the repo). If the command
is missing or unknown, print the usage line from the frontmatter and stop.

## Preconditions

Check these first, for every command except `status`, and refuse with a
message (changing nothing) if one fails.

**State ignored:** `git check-ignore -q factory/state/backlog.md` succeeds. If
it fails, tell the user to add `/factory/state` to `.gitignore` and commit that.

**Clean tree:** `git status --porcelain --untracked-files=all` prints
nothing. Ignored files (`target/`, `factory/state/`) don't count. Exception:
`continue` on the branch of the `In progress` task allows uncommitted
changes and leaves them for the resumed builder.

**Branch** (`git branch --show-current`):
- `start`: `main`.
- `continue`: `main`, or `factory/<id>` where `<id>` is the `In progress`
  task in the backlog.
- `approve`, `rework`, `abandon`: `main` or `factory/<task-id>` for the
  task-id given.

For `continue`, check for an `Escalated` task in the backlog **before**
the branch and tree checks. If there is one, refuse with the pointer to
`approve`, `rework` and `abandon` (escalation leaves you on the task branch,
so a plain branch refusal would be misleading).

Also refuse if a merge is in progress (`git rev-parse -q --verify MERGE_HEAD`
succeeds); tell the user to finish or abort it.

## Rebuilding board.json

When `board.json` is missing (or unreadable), rebuild it from the backlog and
the files under `factory/state/tasks/`. List files with `find` and read first
lines with `head -1`. Set `loop.state` from the situation (`escalated` if a task is Escalated, `complete` if nothing is
`Planned` or active, otherwise `running` while you act).

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
4. Write `board.json` with `loop.state` `planning`, no tasks. Write the snapshot
   `factory/state/.snapshot` (see the conventions). Spawn the Planner
   (`subagent_type` `planner`) with exactly this prompt, and wait:
   `Plan the work in factory/state/SPEC.md (linked documents in factory/state/spec/). Write factory/state/tasks/*/task.md and factory/state/backlog.md.`
5. Verify the Planner's writes: `git status --porcelain --untracked-files=all`
   prints nothing, and the snapshot comparison shows changes only to
   `factory/state/backlog.md` and `factory/state/tasks/*/task.md` (on a
   violation, halt, revert nothing, and say which files changed). Check that
   `backlog.md` exists, every row's task has a `task.md`, every task directory
   is in the backlog, and every status is `Planned`. If not, halt.
6. Build `board.json` (all tasks `planned`), `loop.state`
   `awaiting-plan-review`, message `Plan ready for review`.
7. Halt with a summary: the backlog (id, title, dependencies, whether it has
   UI) and any link warnings. Tell the user they can edit the task files or
   the backlog, then run `/factory continue`.

## /factory continue

1. Preconditions (see above). `factory/state/backlog.md` must exist; if not,
   tell the user to run `/factory start <spec>`.
2. **Reconcile:**
   - If `board.json` is missing or unreadable, rebuild it.
   - If a task is `Escalated`: refuse, and point the user to
     `/factory approve <id>`, `/factory rework <id> <note>` or
     `/factory abandon <id>`.
   - If a task is `In progress` and **merged** (see "Merged test"), run "Finish
     a merged task".
   - If a task is `In progress` and not merged (an interrupted run), tell the
     user in one line that you are resuming it, and run "Run a task".
3. Otherwise run "Next task".

## /factory approve <task-id>

1. Preconditions. Refuse unless the task is `Escalated` in the backlog.
2. Switch to main (`git switch main`) and run "Merge" (see the conventions) for
   the task. A conflict halts.
3. On success, run "Finish a merged task".

## /factory rework <task-id> <note>

1. Preconditions. Refuse unless the task is `Escalated` in the backlog.
2. Resolve the note: if it is a single token naming an existing file, the
   file must be outside the repository (refuse if it is inside, because it
   would make the tree unclean); its content is the note. Otherwise the note is
   the text itself. Refuse an empty note.
3. Set the task's status to `In progress`.
4. `git switch factory/<task-id>`. Create the next round directory and write
   the note to `T/R/human-review.md` (keep its text verbatim; add a first
   line `# Human review` only if it has no heading).
5. Update the board (new round, kind `rework`, stage `building`, loop
   `running`), and run "Run a task". A rework round goes from the reviewers
   straight to the Approver: it never triggers an automatic fix round. Human
   rework rounds count against neither the automatic round limit nor the
   Approver's REWORK budget. Any number are allowed.

## /factory abandon <task-id>

1. Preconditions. Refuse unless the task is `Escalated` in the backlog.
2. `git switch main`. Set its status to `Abandoned`. Keep the branch
   `factory/<task-id>`; the user can delete it by hand.
3. Read the `## Depends on` section of every remaining `Planned` task. If any
   depend on the abandoned task (directly or through another abandoned
   task), warn the user and name them.
4. Update the board, then continue the loop in this session ("Next task").

## /factory status

Read-only, on any branch, any tree state. Change nothing, not even
`board.json`. Print:
- the backlog, with each task's title;
- the current task (`In progress` or `Escalated`), its stage, its latest round
  and that round's kind, build status, review results and verdict;
- the loop state and message from `board.json` (or "no board" if missing);
- the current branch and whether the tree is clean;
- what `/factory continue` would do next: refuse (and why), finish a merged
  task, resume a task at a named step (use the "On entry" table in
  `.claude/agents/task-orchestrator.md`), start the next `Planned` task, or
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

1. On main, set its status to `In progress`.
2. Board: stage `building`, loop `running`, current = task, message `Building
   <task-id>`.
3. Run "Run a task".

## Run a task

1. Board: `loop.state` `running` (this clears any earlier `halted`), current =
   task. Don't touch the board again until step 4.
2. Spawn one agent with `subagent_type` `task-orchestrator`, name
   `task-orchestrator-<task-id>` when the Agent tool offers a name, and exactly
   this prompt: `Task-id: <task-id>.` Record the agent id the tool returns.
3. End your turn with the single word `WAITING`. You are woken by a hand-back
   message from that agent, or by a `<task-notification>` with its id as
   `<task-id>`. Never poll it, send it messages, read its output file, or start
   other work while it runs. A second task orchestrator on this working tree
   would corrupt the run.
4. **Each time either message wakes you**, decide from the files, not from what
   the message says. A hand-back can arrive while the task orchestrator's own
   agents are still running, and a notification may never follow one, so act on
   neither alone. Read the backlog, `board.json`, the current branch and
   `git status --porcelain --untracked-files=all`, then take the first case that
   applies:
   - `loop.state` is `halted`: report the reason from `message`, as in
     "Halting" (conventions). Say `/factory continue` resumes once the user has
     dealt with it.
   - The task is `Escalated`: run "Report an escalation".
   - The task is `In progress`, **merged** (see "Merged test"), you are on
     `main` and the tree is clean: run "Finish a merged task".
   - Anything else, woken by a hand-back: the task orchestrator is still working.
     Take no action and end your turn with `WAITING`.
   - Anything else, woken by the notification: the task orchestrator stopped
     early. Revert nothing. Set `loop.state` `halted`, message `Task orchestrator
     stopped early on <task-id>`, and halt, telling the user that `/factory
     continue` resumes the task from its files.
5. Once you have acted on a task orchestrator, ignore any later message from it.
   If you are waiting on a newer one, end your turn with `WAITING`.

## Merged test

A task is merged into main if either:
- `git log main --format=%s` contains the subject `factory: merge <id>`, or
- the branch exists, `git merge-base --is-ancestor factory/<id> main`
  succeeds, and `git log factory/<id> --format=%s` contains at least one
  builder commit (subject starting with `<id>: `).

A branch with no builder commits is not merged, even though it is an ancestor
of main.

## Report an escalation

Read the latest round's `verdict.md`, or `build-notes.md` if the builder
blocked. Halt with a summary of the verdict (or the blocking reason), the options and suggested
rework notes the Approver gave, and the commands:
`/factory approve <task-id>`, `/factory rework <task-id> <note>`,
`/factory abandon <task-id>`.

## Finish a merged task

On main:
1. Set the task's status to `Complete`.
2. Delete the local branch: `git branch -d factory/<task-id>` (skip if it is
   already gone; if `-d` refuses, leave the branch and warn).
3. Board: stage `done`. Tell the user in one line that
   the task is merged.
4. Continue with "Next task". If no `Planned` task remains, the loop ends
   with the message that the work is complete.
