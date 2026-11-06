---
name: factory
description: "Run the software factory: plan a spec into tasks, then build, review and approve them one by one. Commands: start <spec>, continue, approve <task-id>, rework <task-id> <note>, abandon <task-id>, status."
argument-hint: "start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status"
disable-model-invocation: true
---

# /factory

Arguments: `$ARGUMENTS`

First read `.claude/skills/factory/conventions.md` in full. It defines the
paths (`T`, `R`, `B`), the ground rules, the backlog format, rounds, the
board script, write verification and the Merge and Halting steps that this
file uses.

You are the **main orchestrator**. You handle the commands and their
preconditions, the Planner, reconciliation, choosing the next task, and the
backlog bookkeeping around a task. You spawn the Planner and task
orchestrators, never two task orchestrators at once, and never any other
agent. You report halts to the user. You never do an agent's work yourself.

The first word of the arguments is the command. Anything else is refused
with the list of commands below.

## Preconditions

Every command except `status` checks these, in this order, and refuses (a
short message, and no other action: no board update, no git command that
changes anything) when one fails:

1. **The state is ignored:** `git check-ignore -q factory/state`
   *(exit status is the answer: 0 means ignored)*. Otherwise refuse: add
   `/factory/state` to `.gitignore` and commit it.
2. **Clean tree:** `git status --porcelain --untracked-files=all` prints
   nothing. Otherwise refuse, and list what is uncommitted.
3. **Branch** (`git branch --show-current`):
   - `start`: `main`;
   - `continue`: `main`, or the branch of the `In progress` task;
   - `approve`, `rework`, `abandon`: `main` or `factory/<task-id>`.

`continue` differs: it checks for an `Escalated` task before the tree and
the branch, and on the branch of the `In progress` task it allows
uncommitted changes (see `/factory continue`).

## /factory start <spec>

1. Preconditions, on `main`.
2. Refuse if `factory/state/backlog.md`, `factory/state/tasks/` or
   `factory/state/preflight.md` exists: tell the user to archive
   `factory/state/` (move it to a folder under `factory/archive/`) or remove
   it first. The factory never deletes a previous run.
3. Refuse if `<spec>` is missing or isn't a file.
4. `mkdir -p factory/state`. Copy the spec as "Copying the spec" in the
   conventions describes, writing `spec-source`. If an interrupted `start`
   left `SPEC.md` or `spec/` behind, replace them as a re-copy does.
5. Run the Planner (see "The Planner").

## /factory continue

The session becomes the main orchestrator and reconciles the state before
choosing work. In order:

1. Precondition 1 (the state is ignored).
2. Read the backlog rows, if `backlog.md` exists. If a task is `Escalated`,
   refuse with: `<task-id> is escalated. Review it, then run /factory approve <task-id>, /factory rework <task-id> <note>, or /factory abandon <task-id>.`
   This comes before the branch and tree checks, because an escalation
   leaves the session on the task branch.
3. Branch and tree: if a task is `In progress` and the current branch is
   `factory/<that task-id>`, carry on, leaving any uncommitted changes for
   the resumed builder. Otherwise apply preconditions 2 and 3 (clean tree,
   on `main`).
4. **The preflight state** (see the conventions): re-copy the spec from
   `spec-source` ("Copying the spec"), then run the Planner (see
   "The Planner") and act on its result as `start` does. Nothing else in
   this list applies. If `spec-source` exists but neither `preflight.md` nor
   `backlog.md` does, a `start` was interrupted before the preflight
   finished: do the same.
5. If there is no `backlog.md`, refuse: there is no run; start one with
   `/factory start <spec>`.
6. If `board.json` is missing or unreadable (`B show` fails), run
   `B rebuild`.
7. If `loop.state` (from `B show`) is `awaiting-plan-review`, the human may
   have edited the plan. Check it (see "Checking the plan"); if it fails,
   halt as described there. Then run `B rebuild`, which rebuilds the board
   from `backlog.md` and the task files.
8. If a task is `In progress`:
   - **Merged** (the merged check in the conventions; the user resolved a
     merge conflict): if you aren't on `main`, `git switch main` (halt if the
     tree isn't clean), then mark it `Complete` (see "Completing a task") and
     go on to step 9.
   - **Not merged** (an interrupted run): if its branch exists and you are
     on `main`, `git switch factory/<task-id>`; otherwise stay where you are.
     Then run the task (see "Running a task", from step 2). The task
     orchestrator works out the step from the round files, including
     starting the Approver rework round that a REWORK verdict asks for.
9. Otherwise choose the next task (see "Choosing the next task").

## /factory approve <task-id>

A human reviewer runs this after reviewing an escalated task.

1. Preconditions, on `main` or `factory/<task-id>`.
2. Refuse unless the task is `Escalated`.
3. The task branch already holds the code and documentation. Do the Merge
   step from the conventions.
   - **Conflict:** mark the task `In progress`, then halt with the Merge
     step's message and tell the user how to finish: resolve the conflicts
     on `main`, commit the merge, then `/factory continue`, which finds the
     task merged and marks it `Complete`.
   - **Success:** mark the task `Complete` (see "Completing a task"), then
     continue the loop in this session (see "Choosing the next task").

## /factory rework <task-id> <note>

Sends an escalated task back to the builder with the human's feedback.

1. Preconditions, on `main` or `factory/<task-id>`.
2. Refuse unless the task is `Escalated`.
3. The note is everything after the task-id. If it is a single word that
   names an existing file (expand a leading `~`), the note is that file; it
   must be outside the repository (`realpath` of the file doesn't start
   with `git rev-parse --show-toplevel` followed by `/`), or refuse,
   because a file in the repo would make the tree unclean. Otherwise the
   note is the text itself. Refuse an empty note.
4. Mark the task `In progress`.
5. Check out the task branch: `git switch factory/<task-id>`, or, if it
   doesn't exist (`git rev-parse -q --verify refs/heads/factory/<task-id>`
   fails), `git switch -c factory/<task-id> main`.
6. Create the next round (see "Rounds" in the conventions): `mkdir -p R`;
   write the note to `R/human-review.md` (`cp -- "<file>" R/human-review.md`,
   or the text verbatim with the Write tool); then
   `git rev-parse HEAD > R/base-commit`.
7. `B task <task-id>`, then run the task (see "Running a task", from step 2).
   The task orchestrator spawns a fresh builder for the round; the reviewers
   and then the Approver follow, with no automatic fix round. Human rework
   rounds count against neither the automatic round limit nor the
   Approver's REWORK limit, and any number of them are allowed.

## /factory abandon <task-id>

1. Preconditions, on `main` or `factory/<task-id>`.
2. Refuse unless the task is `Escalated`.
3. `git switch main` (if not on it).
4. Mark the task `Abandoned`. Keep the unmerged branch `factory/<task-id>`
   so no work is lost; tell the user they can delete it by hand.
5. Warn the user about tasks that depend on it: run the **Dependents of a
   task** command from "Plan and dependency commands" in the conventions,
   and name each later task it lists.
6. Continue the loop in this session (see "Choosing the next task").

## /factory status

Read-only, allowed on any branch, with no preconditions. Change nothing:
don't run `B` with any command except `show`, don't write snapshots, and
don't switch branches.

- No `backlog.md`, `preflight.md` or `spec-source`: say there is no run.
- **The preflight state:** print the first line of `preflight.md`, its
  **Spec issues** and **Environment issues**, and its **Tools on offer**
  with their install commands. Next action: `/factory continue` re-copies
  the spec and runs the preflight again.
- Otherwise print:
  - the backlog (task-id, title from the `# ` line of each `task.md`,
    status);
  - the loop state and message (`B show`; if the board is missing, say so:
    `continue` rebuilds it);
  - the current task (the `In progress` or `Escalated` one), with its stage,
    latest round, that round's kind and its build, review and verdict values
    (`B show <task-id>`);
  - the action `/factory continue` would take next, following its steps
    above: refuse because a task is escalated (and the three commands);
    check the plan and start the first task; mark a merged task `Complete`;
    resume the `In progress` task, naming the step its files point to (for
    example "spawn the reviewers for round-2", from the entry table in
    `.claude/agents/task-orchestrator.md`); start the first `Planned` task;
    or report that construction is complete.

## The Planner

Used by `start` and by `continue` in the preflight state, on `main`.

1. `B loop --state planning --message "Planner running" --no-current`.
2. Take the Planner snapshot (`factory/state/.snapshot`, see "Write
   verification"), and note `HEAD` and the branch.
3. Spawn the Planner with the Agent tool: `subagent_type: "planner"`,
   `run_in_background: true`, `description: "Factory Planner"`, and exactly
   this prompt:

   ```text
   Spec: factory/state/SPEC.md. Linked documents: factory/state/spec/.
   ```

4. End your turn with `WAITING`. Act on the first message that says the
   Planner finished (its hand-back, or a `<task-notification>` for it), and
   ignore any later one from it.
5. Write verification for the Planner (see the conventions). On a
   violation, halt.
6. Act on the result:
   - **`backlog.md` exists.** The first line of `preflight.md` must be
     `PREFLIGHT: READY`; otherwise halt (the Planner planned without a
     ready preflight). Check the plan (see "Checking the plan"); if it
     fails, halt as described there. Otherwise
     `B rebuild --state awaiting-plan-review --message "Plan ready for review: <n> tasks"`
     and halt with the **plan-review summary**:
     - the backlog: each task-id, its title and its dependencies;
     - the **Assumptions** from `preflight.md`;
     - the stack profile from `stack.md`, section by section, including
       everything under **Not available**;
     - the next steps: review the plan, and optionally edit
       `factory/state/backlog.md`, the `task.md` files and `stack.md`.
       Removing a task from `backlog.md` also means deleting its task
       directory. Building starts with `/factory continue`.
   - **No `backlog.md`, and `preflight.md` starts with `PREFLIGHT: TOOLS` or
     `PREFLIGHT: BLOCKED`.** The preflight stopped.
     `B rebuild --message "<first line of preflight.md>: see preflight.md"`
     (the state becomes `awaiting-preflight`), and halt with the
     **preflight summary**:
     - the result line;
     - each spec issue: where it is, what is wrong, and what would resolve
       it. Fix the spec at the path in `factory/state/spec-source`. If the
       Planner is wrong about an issue, add a sentence to the spec that
       settles it. If the spec is in this repository, commit the change;
     - each environment issue, and what would fix it;
     - each tool on offer: what it would check, and its install command,
       for the user to run themselves (in Claude Code, for example, with
       `! <command>`). You never install anything. A tool the user doesn't
       install is declined, and the next preflight doesn't offer it again;
     - the stack profile from `stack.md`, including **Not available**;
     - then: run `/factory continue`.
   - **Anything else** (no `preflight.md`, an invalid first line, or
     `PREFLIGHT: READY` without a `backlog.md`): halt, saying what the
     Planner left.

## Checking the plan

Run after the Planner writes a backlog, and when `continue` leaves the plan
review. The plan is sound when every check below passes. Collect every
problem, then, if there are any, run
`B rebuild --state awaiting-plan-review --message "Plan check failed: <short summary>"`
and halt: list each problem, and say to fix the plan (removing a task from
`backlog.md` also means deleting its task directory), then run
`/factory continue`, which checks it again. The state stays
`awaiting-plan-review` so that it does.

The commands named below are under "Plan and dependency commands" in the
conventions. Run them from there, exactly as written.

1. **Backlog and directories match.** Every backlog row has
   `factory/state/tasks/<task-id>/task.md`, and every directory in
   `factory/state/tasks/` appears in the backlog exactly once. Compare the
   backlog rows with the **Task directories** command.
2. **Sections.** Each `task.md` starts with `# <task-id>: ` and has every
   section. The **Missing sections** command prints each problem.
3. **Statuses.** Every row's status is `Planned`.
4. **Dependencies.** Every task-id in a task's `## Depends on` section names
   a task earlier in this backlog, or a task from an earlier run that is
   already merged (the merged check). The **Dependencies** command prints
   each task's dependencies.
5. **Identifiers.** Every task-id matches the task-id pattern in the
   conventions, no serial appears twice in the backlog, and every serial is
   above the highest serial the factory used before this run (the
   **Highest used serial** command).

   Compare serials as numbers (`10#<serial>` in bash). This run's tasks have
   no branches yet while the plan is being checked, so every branch counts
   as an earlier run's.

## Running a task

1. On `main` with a clean tree, mark the task `In progress` (a `Planned`
   task; a resumed or reworked one already is).
2. `B loop --state running --current <task-id> --message "Running <task-id>"`.
   This clears any earlier `halted`. Don't touch the board again until
   step 5.
3. Spawn the task orchestrator with the Agent tool:
   `subagent_type: "task-orchestrator"`, `run_in_background: true`,
   `description: "Factory task <task-id>"`, and exactly this prompt:

   ```text
   Task-id: <task-id>.
   ```

   Remember the agent id the Agent tool returns.
4. End your turn with `WAITING`. You are woken by a hand-back message from
   that agent, or by a `<task-notification>` carrying its agent id. Never
   poll it, send it messages, read its output file, or start other work.
5. **Each time either message wakes you,** decide from `backlog.md`,
   `B show <task-id>`, `git branch --show-current` and
   `git status --porcelain --untracked-files=all`, not from what the message
   says. A hand-back can arrive while the task orchestrator's own agents are
   still running, and a notification may never follow one. Take the first
   case that applies:
   - `loop.state` is `halted`: tell the user the reason from
     `loop.message`. `/factory continue` resumes.
   - The task is `Escalated`: show the escalation summary (see
     "Escalations").
   - The task is `In progress`, the merged check passes, you are on `main`,
     and the tree is clean: mark it `Complete` (see "Completing a task") and
     choose the next task (see "Choosing the next task").
   - Anything else, woken by a hand-back: the task orchestrator is still
     working. Take no action and end your turn with `WAITING`.
   - Anything else, woken by the notification: the task orchestrator stopped
     early. Revert nothing.
     `B loop --state halted --message "Task orchestrator stopped early on <task-id>"`,
     and tell the user that `/factory continue` resumes the task from its
     files.
6. Once you have acted on a task orchestrator (any case except the
   `WAITING` one), ignore every later message from it.

## Completing a task

On `main`, after the merge:

1. Mark the task `Complete` in the backlog.
2. Delete its branch: `git branch -d factory/<task-id>` (see the Merge step
   for the one case where `-D` is allowed). A branch that is already gone is
   fine.
3. `B task <task-id>`.

## Choosing the next task

You must be on `main` with a clean tree. Take the first backlog row with
status `Planned` and run it (see "Running a task"). If there is none, run
`B loop --state complete --message "Construction is complete" --no-current`
and tell the user that construction is complete, with the backlog's final
statuses (and any `Abandoned` tasks, whose branches are kept).

## Escalations

When a task is `Escalated`, the session is on the branch the task
orchestrator stopped on (normally `factory/<task-id>`). Read the latest
round (`B show <task-id>`) and show the user:

- **The reason:**
  - the build is `BLOCKED`: the Builder blocked. Summarise the reason from
    `R/build-notes.md`, and warn that the branch holds unfinished work that
    didn't pass the test gate (its last commit is marked
    `WIP (blocked)`);
  - the verdict is `ESCALATE`: the Approver escalated. Summarise
    `R/verdict.md`: the open issues, the options it lays out with the
    suggested rework note for each, and whether it thinks approving the
    branch as it is would be reasonable;
  - the verdict is `REWORK`: the REWORK limit was reached (the count in
    "Rounds" is above 2), or the verdict had no rework note. Summarise the
    verdict's findings.
- **Where to look:** the round files under `T/<round-id>/` (also on the
  dashboard), and the change with `git diff main...factory/<task-id>`.
- **The commands:**
  - `/factory approve <task-id>`: merge the branch as it is;
  - `/factory rework <task-id> <note>`: another round with your note (text,
    or the path of a file outside the repository);
  - `/factory abandon <task-id>`: give up on the task and keep its branch.

## The dashboard

The board can be watched while the factory runs: serve it with
`python3 -m http.server -d factory` and open `/dashboard/`. Mention this at
the first halt of a run.
