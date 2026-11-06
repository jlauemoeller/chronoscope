---
name: factory
description: "Runs the software factory: a Planner splits a spec into tasks, and each task is built, reviewed and approved on its own branch. Commands: start <spec>, continue, approve <task-id>, rework <task-id> <note>, abandon <task-id>, status."
disable-model-invocation: true
argument-hint: "start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status"
---

# The factory: main orchestrator

First, read `.claude/skills/factory/conventions.md` in full. It defines the
paths, the ground rules, the spawn prompts, the git checks, write
verification, rounds, the roster, the board, the factory script and the Merge
and Halting steps that this file refers to. Follow it throughout.

The command line is: `$ARGUMENTS`

The session running this command becomes the **main orchestrator**. It
handles the commands and their preconditions, the Planner, reconciliation,
choosing the next task, and the backlog bookkeeping around a task. It spawns
only the `planner` and `task-orchestrator` agents, one at a time, and never
does an agent's work itself. It reports every halt to the human.

`factory.py <command>` means
`python3 -I .claude/skills/factory/factory.py <command>`, run in full from the
repository root.

## The command

The first word of the command line is the command: `start`, `continue`,
`approve`, `rework`, `abandon` or `status`. Anything else, or nothing, prints
this usage and stops:

```
/factory start <spec>              plan a spec and halt for the plan review
/factory continue                  restart a stopped loop
/factory approve <task-id>         merge an escalated task
/factory rework <task-id> <note>   send an escalated task back with a note
/factory abandon <task-id>         give up on an escalated task
/factory status                    show the loop (read-only)
```

## Preconditions

Every command except `status` checks these, and refuses with a message and
does nothing else (no board change, no git change) if one fails:

1. **State ignored:** `git check-ignore -q factory/state` (plain git check)
   exits 0. Otherwise: "`factory/state/` isn't ignored by git. Add
   `/factory/state` to `.gitignore` and commit it."
2. **Branch** (`git branch --show-current`):
   - `start`: `main`;
   - `continue`: `main`, or `factory/<task-id>` of the `In progress` task;
   - `approve`, `rework`, `abandon`: `main` or `factory/<task-id>` of the
     task named in the command.
3. **Clean tree:** `git status --porcelain=v1 --untracked-files=all` prints
   nothing: no uncommitted changes and no untracked files that git doesn't
   ignore. Otherwise name the files and ask the human to commit or remove
   them.

`continue` checks for an `Escalated` task before the branch and the tree,
and has one exception to the clean tree (see `/factory continue`).

## /factory start <spec>

1. Check the preconditions (branch `main`).
2. Refuse if `factory/state/backlog.md`, `factory/state/tasks/` or
   `factory/state/preflight.md` exists: "`factory/state/` holds a previous
   run. Move it to a folder under `factory/archive/` (or remove it) first;
   the factory never deletes a run."
3. Copy the spec: `factory.py spec-copy <spec>`, with `<spec>` exactly as
   given. If it fails (for example, the spec doesn't exist), refuse with its
   error. Keep its `missing:` lines for the halt summary.
4. Create the board: `factory.py board rebuild --state planning --message
   "Preflight and planning"`.
5. Spawn the Planner and act on its result (see "The Planner").

## /factory continue

Restart a stopped loop. Reconcile the state in this order before choosing
work:

1. **State ignored** (precondition 1).
2. Run `factory.py tasks`.
   - `phase: none`: refuse: "There is no factory run in `factory/state/`.
     Use `/factory start <spec>`."
   - `active:` shows an `Escalated` task: refuse, whatever the branch or the
     tree: "<task-id> is escalated. Review it, then run
     `/factory approve <task-id>`, `/factory rework <task-id> <note>` or
     `/factory abandon <task-id>`." (An escalation leaves the session on the
     task branch, so a branch refusal would hide the real reason.)
3. **The preflight state** (`phase: preflight`): check the branch (`main`)
   and the clean tree. Re-copy the spec with `factory.py spec-copy` (no
   argument), which replaces `SPEC.md` and `spec/` from the path in
   `spec-source`. Then `factory.py board rebuild --state planning --message
   "Preflight (again) and planning"`, spawn the Planner, and act on its
   result as `start` does (see "The Planner"). Nothing else in this list
   applies.
4. **Branch:** `main`, or `factory/<task-id>` of the task that `active:`
   shows as `In progress`. Otherwise refuse.
5. **Tree:** it must be clean, with one exception. On the task branch, when
   the latest round has no review files and no verdict
   (`factory.py round state <task-id>` shows every review and the verdict as
   `missing`, or the task has no round yet), uncommitted changes are the
   Builder's own: leave them for the resumed builder. If the tree isn't
   clean and a review or a verdict exists, the changes came from a reviewer
   or the Approver: halt with `factory.py board set --state halted --message
   "Uncommitted changes on factory/<task-id> after review"` and ask the
   human to inspect them and commit or remove them, because otherwise a
   builder would commit changes that no reviewer saw. On `main`, a dirty tree
   is refused as usual.
6. **Board missing** (`factory.py board show` prints `"board": "missing"` or
   `"unreadable"`): rebuild it with `factory.py board rebuild`.
7. **Plan review** (`loop.state` is `awaiting-plan-review`): the human may
   have edited the plan. Check it (see "Checking the plan"), and rebuild the
   board with `factory.py board rebuild --state awaiting-plan-review
   --message "Plan checked"`. If the check found problems, halt as that
   section says.
8. **An `In progress` task** (from `active:`):
   - If `factory.py merged <task-id>` prints `yes` (the human resolved a
     merge conflict), switch to `main` if needed and complete the task (see
     "Completing a task"). Then carry on with step 9.
   - Otherwise it is an interrupted run. If its branch exists (plain git
     check in the conventions), check it out with
     `git switch factory/<task-id>`, or stay on it; if it doesn't exist yet,
     stay on `main` (its round-1 builder creates it). Then run the task (see
     "Running a task"). The task orchestrator works out the step to resume
     at from the round's files, including a REWORK verdict that still needs
     its Approver rework round.
9. **Next task:** see "Choosing the next task".

## /factory approve <task-id>

A human reviewer uses this after reviewing an escalated task.

1. Check the preconditions (branch `main` or `factory/<task-id>`).
2. Refuse unless `factory.py tasks` shows the task as `Escalated`.
3. Run the Merge step from the conventions. The task branch already holds
   the code and documentation.
4. On a merge conflict: first `factory.py backlog-set <task-id> "In progress"`,
   then halt as the Merge step says, with `--current <task-id>`, and tell
   the human to resolve the conflicts, complete the merge, and run
   `/factory continue`, which finds the task merged and marks it `Complete`.
5. On success, complete the task (see "Completing a task"), then continue
   the loop in this session (see "Choosing the next task").

## /factory rework <task-id> <note>

Send an escalated task back to the builder with feedback.

1. Check the preconditions (branch `main` or `factory/<task-id>`).
2. Refuse unless `factory.py tasks` shows the task as `Escalated`.
3. The note is everything after the task-id (drop one pair of surrounding
   quotes). If it is the path of an existing file (expand `~`), it must be
   outside the repository: a file inside it would make the tree unclean, so
   refuse and ask for the note as text or as a file outside the repo.
   Otherwise the note is the text itself. Refuse an empty note.
4. Work in this order, so that an interrupted `rework` can simply be run
   again:
   1. Check out the task branch: `git switch factory/<task-id>` if it exists
      (or stay on it), otherwise create it from main with
      `git switch -c factory/<task-id> main`.
   2. Choose the round directory: if `factory.py round latest <task-id>`
      names a round with no `build-notes.md` (an earlier `rework` was
      interrupted), reuse it. Otherwise create
      `factory/state/tasks/<task-id>/<next>/`, where `<next>` is
      `factory.py round next <task-id>`.
   3. Copy the note to `factory/state/tasks/<task-id>/<round-id>/human-review.md`,
      replacing any earlier one: `cp` a note file, or write a text note
      exactly as given. Then write the round's base commit:
      `git rev-parse HEAD > factory/state/tasks/<task-id>/<round-id>/base-commit`.
   4. Only then mark the task: `factory.py backlog-set <task-id> "In progress"`.
5. Run the task (see "Running a task"). The task orchestrator spawns a fresh
   builder, which revises its work according to the note. After the builder
   and the reviewers, the Approver runs straight away; a rework round never
   triggers an automatic fix round. Human rework rounds count against neither
   the automatic round limit nor the Approver's rework limit.

## /factory abandon <task-id>

Give up on an escalated task.

1. Check the preconditions (branch `main` or `factory/<task-id>`).
2. Refuse unless `factory.py tasks` shows the task as `Escalated`.
3. Switch to `main`: `git switch main` (unless already on it).
4. `factory.py backlog-set <task-id> Abandoned`, then
   `factory.py board set --state running --message "Abandoned <task-id>"
   --clear-current --task <task-id>`.
5. Keep the unmerged branch `factory/<task-id>`, so no work is lost; tell the
   human it is kept and that they can delete it by hand.
6. Run `factory.py dependents <task-id>`. If later tasks depend on the
   abandoned one, warn the human and name them: they will be built without
   it.
7. Continue the loop in this session (see "Choosing the next task").

## /factory status

Read-only and allowed on any branch, with no preconditions. It never writes
anything: no board rebuild, no snapshot, no git change.

1. Run `factory.py tasks` and `factory.py board show`.
2. `phase: none`: say there is no run, and that `/factory start <spec>`
   starts one.
3. `phase: preflight`: instead of the backlog, print the preflight result
   (`factory.py first-line factory/state/preflight.md`), its open spec and
   environment issues, and the tools on offer with their install commands,
   from `factory/state/preflight.md`. Print the loop state. The next action
   is: re-copy the spec and run the preflight again.
4. `phase: backlog`: print
   - the backlog: each task's id, status, stage and latest round;
   - the current task (`loop.current`, or the `active:` task), its stage,
     and its latest round's state (`factory.py round state <task-id>`);
   - the loop state and message (or that the board is missing);
   - the action `/factory continue` would take next, following its steps:
     a refusal (escalated task, wrong branch, dirty tree), rebuilding a
     missing board, re-checking the plan, completing a merged task, resuming
     the `In progress` task (and at which step, from its round state: start
     round 1, build, review with the reviewers still missing, fix round,
     Approver, Approver rework round, merge, escalate), starting the next
     `Planned` task, or declaring construction complete.

## The Planner

Used by `start` and by the preflight restart in `continue`, on `main`.

1. Write the snapshot `factory/state/.snapshot` (see "Write verification" in
   the conventions), and record the branch and `HEAD`.
2. Spawn the `planner` agent with exactly the prompt
   `Spec: factory/state/SPEC.md. Linked documents: factory/state/spec/.`
   End the turn with `WAITING` until its completion notification arrives.
3. Check what it changed (the `planner` row of "What each agent may
   change"). On a violation, halt as the conventions say.
4. Act on its result:
   - **It wrote `backlog.md`:**
     1. `factory.py first-line factory/state/preflight.md` must print
        `READY`. Otherwise halt: the Planner planned without a ready
        preflight.
     2. Check the plan (see "Checking the plan").
     3. `factory.py board rebuild --state awaiting-plan-review --message
        "Plan ready for review"`.
     4. If the check found problems, halt as "Checking the plan" says.
        Otherwise halt for the plan review with a summary of: the backlog
        (each task's id and title, in order), the **Assumptions** from
        `preflight.md`, and the stack profile `factory/state/stack.md`
        section by section, including everything under **Not available**.
        Tell the human they can review and edit the plan (`backlog.md` and
        the `task.md` files, and `stack.md`), that removing a task from
        `backlog.md` also means deleting its task directory, and that
        building starts with `/factory continue`.
   - **Otherwise the preflight stopped.** `factory.py first-line
     factory/state/preflight.md` must print `TOOLS` or `BLOCKED` (anything
     else, including a missing file, is an error: halt). Run
     `factory.py board rebuild --state awaiting-preflight --message
     "Preflight <result>: see preflight.md"`. Halt with a summary of
     `preflight.md` (its result, spec issues, environment issues and tools on
     offer), the stack profile including **Not available**, any linked
     documents `spec-copy` reported missing, and these next steps:
     - For each spec issue: fix the spec at the path in
       `factory/state/spec-source`. If the Planner is wrong about an issue,
       add a sentence to the spec that settles it. If the spec is in the
       repo, commit the change.
     - For each tool on offer: its install command, which the human runs
       themselves (in Claude Code, for example, with `! <command>`). The
       factory never installs anything. A tool they don't install is
       declined, and the next preflight doesn't offer it again.
     - Then run `/factory continue`.

## Checking the plan

The plan is checked by the Planner before it finishes, by this orchestrator
after the Planner, and again when `/factory continue` leaves the plan review,
because the human may have edited the plan. The plan is sound when:
- every task in `backlog.md` has a `task.md` with all its sections, and every
  task directory appears in `backlog.md` exactly once;
- every task's status is `Planned`;
- every `Reviews` section lists `code` and `feature`;
- every dependency names a task earlier in this backlog, or a task from an
  earlier run that is already merged;
- every task-id follows the identifier rules, including serials that
  continue from earlier runs.

Run `factory.py plan-check`, which makes these checks. If it prints
`plan: ok`, the plan is sound. Otherwise halt: say what is wrong (its `- `
lines), leave `loop.state` at `awaiting-plan-review` (with
`factory.py board set --state awaiting-plan-review --message "Plan check
failed: fix the plan, then /factory continue"`) so that the next
`/factory continue` checks again, and remind the human that removing a task
from `backlog.md` also means deleting its task directory.

## Choosing the next task

On `main`, run `factory.py tasks` and take `next:`, the first task in
`backlog.md` with status `Planned`.
- If it is `none`, construction is complete:
  `factory.py board set --state complete --message "Construction complete"
  --clear-current`, and tell the human the work is complete.
- Otherwise start it: `factory.py backlog-set <task-id> "In progress"`, then
  run the task.

## Running a task

1. `factory.py board set --state running --message "Working on <task-id>"
   --current <task-id> --task <task-id>`. This clears any earlier `halted`.
   Don't touch the board again until step 4.
2. Spawn the `task-orchestrator` agent with exactly the prompt
   `Task-id: <task-id>.` Note its agent id.
3. End the turn with `WAITING`. You are woken by a hand-back message from
   that agent, or by a `<task-notification>` that carries its agent id.
   Never poll the task orchestrator, send it messages, read its output file
   or start other work.
4. **Each time either message wakes you**, decide from `factory.py tasks`,
   `factory.py board show --task <task-id>`, `git branch --show-current` and
   `git status --porcelain=v1 --untracked-files=all`, not from what the
   message says. A hand-back can arrive while the task orchestrator's own
   agents are still running, and a notification may never follow one, so act
   on neither alone. Take the first case that applies:
   - `loop.state` is `halted`: report the reason to the human, with what to
     inspect or fix. `/factory continue` resumes. Stop.
   - The task is `Escalated`: show the escalation summary (below). Stop.
   - The task is `In progress`, `factory.py merged <task-id>` prints `yes`,
     the branch is `main` and the tree is clean: complete the task (see
     "Completing a task") and carry on with the next task (see "Choosing the
     next task").
   - Anything else, woken by a hand-back: the task orchestrator is still
     working. Take no action and end the turn with `WAITING`.
   - Anything else, woken by the notification: the task orchestrator stopped
     early. Revert nothing, run `factory.py board set --state halted
     --message "Task orchestrator stopped early on <task-id>"`, and tell the
     human that `/factory continue` resumes the task from its files. Stop.
5. Once you have acted on a task orchestrator (any case but "still
   working"), ignore every later message from that agent id: take no action
   for it, and if you are waiting on another agent, end the turn with
   `WAITING`.

### Completing a task

On `main`, after its branch is merged:
1. `factory.py backlog-set <task-id> Complete`.
2. If the branch exists, delete it with `git branch -d factory/<task-id>`. If
   git refuses, halt instead of forcing.
3. `factory.py board set --task <task-id> --clear-current`.

### The escalation summary

The task orchestrator leaves the session on the task branch. Read the
latest round (`factory.py round state <task-id>`), and its `verdict.md`, or
its `build-notes.md` if the builder blocked. Give the reason:
- **The Builder blocked** (`build: BLOCKED`): summarise the reason from the
  build notes, and warn that the branch holds unfinished work that didn't
  pass the gate (its last commit is a `WIP (blocked)` commit).
- **The Approver escalated** (`verdict: ESCALATE`): summarise the verdict,
  the options it lays out with the rework note it suggests for each, and
  whether it says approving the branch as it is would be reasonable.
- **The REWORK limit was reached** (`verdict: REWORK` and
  `factory.py rework-count <task-id> <round-id>` prints 2 or more):
  summarise the verdict and its rework note.
- **A REWORK verdict had no rework note** (`verdict: REWORK` and
  `factory.py rework-note <task-id> <round-id>` prints `rework-note: none`):
  summarise the verdict.

Then list the commands:
- `/factory approve <task-id>`: merge the branch as it is;
- `/factory rework <task-id> <note>`: another round with the note (text, or
  the path of a file outside the repository);
- `/factory abandon <task-id>`: give up on the task and keep its branch.
