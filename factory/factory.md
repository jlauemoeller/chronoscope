Build a visual "software factory" in this repo using a Claude skill,
subagents, json, and markdown files. The factory should be a single skill
`/factory` that runs the loop described below.

## Deliverables

Building the factory means creating these files (see "Factory file system"):

- `.claude/skills/factory/SKILL.md` — the main orchestrator instructions,
  covering every command below. The skill is user-invoked only
  (`disable-model-invocation: true`) and takes the command and its arguments
  as arguments. It also holds the preconditions, rebuilding `board.json`,
  running the Planner, choosing the next task, and spawning and acting on the
  task orchestrator. It tells the main orchestrator to read `conventions.md`
  before anything else.
- `.claude/skills/factory/conventions.md` — everything both orchestrator levels
  need, so that it is written once:
  - the paths and names, including `T` for the task directory and `R` for the
    round directory, and the list of agent types;
  - the ground rules: never push or rewrite history, the destructive git
    commands that are never allowed, one active task, the state staying out of
    git, prompts that contain only ids and paths, waiting through completion
    notifications, and one `board.json` writer at a time;
  - the `backlog.md` format and how to change a status;
  - the exact commands for checking changes to the state (see "Write
    verification");
  - the round rules: how a round's kind follows from its files, how to find the
    latest and next round, how to count earlier REWORK verdicts, and the exact
    first lines, where any other first line is an error that halts;
  - the `board.json` format and how to update it (see "The orchestrator");
  - the Merge and Halting steps.
- `.claude/agents/task-orchestrator.md` — the task orchestrator: the spawn
  prompts, builder reuse, write verification for the Builder, reviewers and
  Approver, the table of where to start from a task's files, and the Build,
  Review, Approve, Approver rework and Escalate steps. It tells the task
  orchestrator to read `conventions.md` first.
- `.claude/agents/{planner,builder,code-reviewer,feature-reviewer,ui-reviewer,approver}.md`
  — one subagent definition per role, with the model and effort given below.
- `factory/dashboard/index.html` (plus optional `style.css` / `app.js`) — the
  static dashboard.
- `.gitignore` entries `/factory/state` and `/factory/archive`, with no trailing
  slash, and nothing else under `factory/`; the dashboard is tracked. The state
  of a run is never committed. When a run is finished, the human user moves
  `factory/state/` to a folder under `factory/archive/`.

The factory is built once; the orchestrator never regenerates these files at
runtime. Commit the factory files before the first `/factory start`, so the
working tree is clean.

## Skill Commands

Every command except `status` requires `factory/state/` to be ignored by git,
and a clean working tree: no uncommitted changes and no untracked files that
git doesn't ignore. Each command also requires a particular branch:
- `start`: main.
- `continue`: main, or the branch of the `In progress` task (see below).
- `approve`, `rework` and `abandon`: main or `factory/<task-id>`.

If a precondition fails, the command refuses with a message and does nothing.

`factory/state/` is not in git, so there is one copy of it, the same on every
branch. The orchestrator and agents read and edit its files directly, and
nothing under it is ever committed.

/factory start <spec>
Starts the factory. The session using the command becomes the orchestrator.
Refuse if `factory/state/backlog.md` or `factory/state/tasks/` already exist
(tell the user to archive or remove `factory/state/` first; the factory never
deletes a previous run). Copy <spec> to `factory/state/SPEC.md`. Also copy
every Markdown file that <spec> links to with a relative link (following
links transitively) into `factory/state/spec/`, and rewrite those links so
they resolve from the copies. Then spawn the Planner. When the Planner is
done, the orchestrator builds `board.json`, and halts with a summary of the backlog so the human can
review (and optionally edit) the plan. Building starts with
`/factory continue`.

/factory continue
Restart a stopped loop. The session using the command becomes the
orchestrator. Before choosing work it reconciles state:
- If `board.json` is missing, rebuild it from `backlog.md` and the files in
  `factory/state/tasks/`.
- If a task is `In progress` and its branch `factory/<task-id>` is already
  merged into main (the user resolved a merge conflict), mark it `Complete`,
  delete the branch, and carry on.
- If a task is `In progress` and not merged (an interrupted run), check out
  its branch (or stay on it) and resume at the right step of its latest
  round. Work out the step from which files exist in the round directory
  (build notes, reviews, verdict). If the latest round's verdict is REWORK,
  start the Approver rework round it asks for (see "The orchestrator").
  When `continue` runs on this branch, it allows uncommitted changes and
  leaves them for the resumed builder.
- If a task is `Escalated`, refuse and point the user to
  `/factory approve`, `/factory rework`, or `/factory abandon`.

Otherwise start the next available task: the first task in `backlog.md`
with status `Planned`. If there is none, halt with a message stating that
construction is complete.

/factory approve <task-id>
Merge an `Escalated` task into main. A human reviewer uses this command after
reviewing the task. Refuse if the task isn't `Escalated`. The task branch
already holds the code and documentation. Switch to main and merge the branch
with
`git merge --no-ff -m "factory: merge <task-id>"`. In the event of a merge
conflict, stop and notify the human user. Once the conflicts are resolved,
the user completes the merge and restarts the loop with `/factory continue`.
If the merge succeeds, mark the task `Complete`, delete the local
branch `factory/<task-id>`, and continue the loop in the same session.

/factory rework <task-id> <note>
Send an `Escalated` task back to the builder with feedback. Refuse if the
task isn't `Escalated`. The orchestrator marks the task `In progress`,
checks out `factory/<task-id>`, and creates the next round
directory. The note is given as text or as the name of a file outside the
repo (a file inside the repo would make the tree unclean). The orchestrator
copies it to
`factory/state/tasks/<task-id>/<round-id>/human-review.md`. It then hands
control to the builder with instructions to revise its work according to the
review. After the builder finishes, the reviewers review the round, and then
the Approver runs straight away; a rework round never triggers an automatic
fix round. Human rework rounds count against neither the automatic round
limit nor the Approver's rework limit, and any number of them are allowed.

/factory abandon <task-id>
Give up on an `Escalated` task. Switch to main and mark the task `Abandoned` in
`backlog.md`. Keep the unmerged branch `factory/<task-id>` so no work is
lost; the user can delete it by hand. Warn the user if later tasks depend on
the abandoned one. Then continue the loop in the same session.

/factory status
Read-only and allowed on any branch. Prints the backlog, the current task,
its stage and round, the loop state, and the action that `/factory continue`
would take next.

## Identifiers

Tasks are identified by a three-digit, zero-padded serial number that
increases by 1 from `001`, followed by a two-to-three word "fingerprint" of
the task. The serial number and words are separated by dashes. Example:
`002-apply-selected-theme`.

Rounds are identified by the word `round` and a number that increases
monotonically from 1, separated by a dash. Example: `round-4`.

Branches are named `factory/<task-id>`.

## The Floor Plan

Read [floorplan.md](floorplan.md) to see the factory floor plan.
The agent roles are:

- **The Planner** divides `factory/state/SPEC.md` (with its linked documents
  in `factory/state/spec/`) into a suitable number of tasks that together
  implement the work described in them. Each task becomes a directory
  `factory/state/tasks/<task-id>/`. In each task directory the Planner writes
  a `task.md` file containing everything from the spec that a builder needs
  to implement the task. Each `task.md` has these sections:
  - a title;
  - a goal;
  - the relevant spec excerpts;
  - acceptance criteria;
  - a `UI` section (or `UI: none` when the task has no UI changes);
  - the ids of the tasks it depends on.

  When all tasks are written, the Planner writes `factory/state/backlog.md`,
  the execution plan. It contains a Markdown table with one row per task and
  two cells per row: the task-id and the task's status (`Planned` for every
  task at this point). The tasks must be in implementation order from top to
  bottom, so that earlier tasks don't depend on later ones.

  The Planner can assume the toolchain is installed, but not that the project
  exists. If it doesn't, the first task scaffolds it. The crate must expose a
  library target with renderable widgets so the UI Reviewer can render them
  from a scratch crate.

  The Planner works on the main branch and writes only `backlog.md` and the
  `task.md` files, and it doesn't commit. The Planner runs
  claude-opus-5-5 with xhigh effort.

- **The Builder** implements one task, given to it by the orchestrator as a
  task-id and a round-id. It never reads or writes `backlog.md`.
  - **Round 1:** before modifying any files, it creates the branch
    `factory/<task-id>` from main and switches to it. If the branch already
    exists, it checks it out instead.
  - **Later rounds:** it reads every review file in the previous round
    directory and addresses all of them. In a rework round it also reads
    the `human-review.md` or `approver-review.md` in the current round
    directory. Where that note conflicts with a review, the note wins.

  It implements the task from `factory/state/tasks/<task-id>/task.md` and
  tests the result against that file's acceptance criteria. The work is not
  done until the test gate passes:

  ```
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  ```

  UI changes must come with `insta` snapshot tests that use Ratatui's
  `TestBackend`. Snapshots must be accepted (`.snap`) before the Builder
  commits; it never commits `.snap.new` files.

  Before it reports `STATUS: DONE`, the Builder self-checks its work against
  `task.md`, the way the reviewers will:
  - every acceptance criterion is met and covered by a test that would fail
    if the behaviour were wrong;
  - every UI state and terminal size in the `UI` section has a snapshot;
  - in later rounds, every review finding has been addressed.

  At the end of each round the Builder writes
  `factory/state/tasks/<task-id>/<round-id>/build-notes.md`. Its first line
  is `STATUS: DONE`, or `STATUS: BLOCKED` if it couldn't pass the test gate
  or the task is unclear. The file then summarises the changes, the
  self-check (one line per acceptance criterion and UI state, with the test
  that covers it), the test gate results, and (if blocked) the reason. The
  Builder commits its work on the task branch, with messages prefixed
  `<task-id>: `. The build notes are not committed. Apart from them, it never
  writes under `factory/state/`. The Builder runs claude-sonnet-5-5 with
  High effort.

- **The Code Reviewer** reviews the changes on the task branch
  (`git diff main...HEAD`). It checks that they follow best practices for the
  language and frameworks, and any additional guidelines in
  `factory/state/SPEC.md`. It re-runs the test gate with `INSTA_UPDATE=no`,
  so that failing snapshots don't write files into the repo. It produces a
  review with all its findings, and marks it PASS or FAIL (see "Review
  findings"). The review is written
  to `factory/state/tasks/<task-id>/<round-id>/code-review.md`, and this is
  the only file the Code Reviewer may create or change. The Code Reviewer
  runs claude-opus-5-5 with High effort.

- **The Feature Reviewer** reviews the changes on the task branch to
  determine whether the implementation matches the feature description and
  acceptance criteria in `factory/state/tasks/<task-id>/task.md`. It produces
  a review with all its findings, and marks it PASS or FAIL (see "Review
  findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/feature-review.md`, and this is
  the only file the Feature Reviewer may create or change. The Feature
  Reviewer runs claude-opus-5-5 with High effort.

- **The UI Reviewer** reviews the changes on the task branch to determine
  whether the implementation matches the UI specified in
  `factory/state/tasks/<task-id>/task.md`. It inspects the Builder's `insta`
  snapshots. Any tests it runs in the repo run with `INSTA_UPDATE=no`. It
  also renders views itself:
  - It creates a throwaway crate outside the repo, in its own scratch
    directory (see "Rules"). The crate has a path dependency on the project
    and its own `CARGO_TARGET_DIR`.
  - It renders the views with Ratatui's `TestBackend` at fixed sizes.
  - It never adds code to the repo.

  It produces a review with all its findings, and marks it PASS or FAIL (see
  "Review findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/ui-review.md`. Any text snapshots
  that document issues go in `factory/state/tasks/<task-id>/<round-id>/ui/`.
  These are the only files the UI Reviewer may create or change. If the task
  has `UI: none`, the UI Reviewer writes a PASS review saying so. The UI
  Reviewer runs claude-opus-5-5 with High effort.

- **The Approver** reads `task.md`, the build notes, and all reviews from the
  current round. It may also read earlier rounds for context. It produces a
  final verdict by judging whether any remaining review issues are minor
  enough to ignore, can be fixed by another round, or need human review. It
  writes its findings and verdict to
  `factory/state/tasks/<task-id>/<round-id>/verdict.md` and marks the
  verdict APPROVE, REWORK or ESCALATE. This is the only file the Approver
  may create or change.
  - **APPROVE** means the orchestrator can merge the changes into main
    without human review.
  - **REWORK** means the Approver sends the task back to the builder for
    another round, without human review. The verdict must contain a
    `## Rework note` section: one concrete set of instructions for the
    builder, written like a `/factory rework` note.
  - **ESCALATE** means work must stop until a human review is complete.
    When it can, the Approver lays out the options for the human and
    suggests a rework note for each.

  The Approver decides in this order:
  1. If every remaining issue is minor enough to ignore, APPROVE.
  2. If the issues allow two or more distinct solutions that lead to
     significantly different outcomes (for example different functionality
     or behaviour), ESCALATE. This is an A/B choice for the human, and the
     priority order below never settles it.
  3. If at least one review is FAIL, the Approver can write a single
     suitable rework note, and rework rounds remain, REWORK.
  4. Otherwise ESCALATE.

  The Approver may issue at most two REWORK verdicts per task in total. It
  counts them from the `verdict.md` files of earlier rounds. Human rework
  rounds don't reset the count, and an unused REWORK is still available
  after a human rework.

  When reviewers conflict, the Approver resolves the conflict with this
  priority order, from most to least important:
  1. Security, whichever reviewer raises it.
  2. Correct, idiomatic, best-practice code (Code Reviewer).
  3. Feature implementation (Feature Reviewer).
  4. UI (UI Reviewer).

  A rework note follows the higher priority where two reviews conflict. It
  says which demand it drops and why. The Approver runs claude-opus-5-5 with
  xhigh effort.

### Review findings

Each reviewer labels every finding `[must-fix]` or `[nit]`, and describes
precisely the change needed for each one.
- **must-fix:** a failing test gate; an unmet acceptance criterion or spec
  requirement; a bug or wrong behaviour; a security or data-loss risk; a
  missing or ineffective test for required behaviour; a UI that contradicts
  the task's `UI` section; a breach of a guideline in `SPEC.md`; or a clear
  violation of language or framework best practice that would cause real
  problems.
- **nit:** anything else worth mentioning, such as style preferences, small
  naming or documentation issues, minor inefficiencies off the hot path, and
  cosmetic details the spec doesn't pin down.

A review is FAIL if it has at least one must-fix finding, and PASS otherwise.
A PASS review still lists its nits. Reviewers don't raise a nit to must-fix
to force another round, or lower a must-fix to let the work pass.

## The orchestrator

The orchestrator runs the main loop of the factory, described above and shown
in [floorplan.md](floorplan.md). It spawns agents, chooses their tasks, and
sets up their context. It never does an agent's work itself. Agent prompts
contain only the task-id, the round-id, and the paths to read; all other
context comes from files.

The orchestrator has two levels, so that a long backlog doesn't accumulate in
one context:
- The **main orchestrator** is the session that ran `/factory`. It handles the
  commands and their preconditions, the Planner, reconciliation, choosing the
  next task, and the backlog bookkeeping around a task (`Start`, `Complete`,
  and the `approve`, `rework` and `abandon` commands). It reports halts to the
  human user.
- The **task orchestrator** is a subagent, spawned fresh for each task and
  discarded when the task ends. It runs claude-opus-5-5 with High effort, and
  its tools are `Read, Grep, Glob, Write, Edit, Bash, Agent, SendMessage,
  ToolSearch`. The role agents don't get `Agent`, so nesting stops there. Its
  prompt is exactly `Task-id: <task-id>.`, and it always enters by working out
  the next step from the task's files, so new, reworked and interrupted tasks
  start the same way. It runs the task's rounds, and stops when the task is
  merged, escalated or halted.

Only the task orchestrator spawns the Builder, reviewers and Approver. Only the
main orchestrator spawns the Planner and task orchestrators, and it never runs
two task orchestrators at once.

"The orchestrator" below means whichever level does the step. The split is:
- **Starting a task:** the main orchestrator marks it `In progress`. The task
  orchestrator creates `round-1/` and spawns the builder.
- **`/factory rework`:** the main orchestrator does the backlog change, the new
  round directory and `human-review.md`, then spawns a task orchestrator.
- **Merging after APPROVE:** the task orchestrator merges and stops on `main`,
  leaving the task `In progress`. The main orchestrator marks it `Complete` and
  deletes the branch.
- **`/factory approve`:** the main orchestrator merges, using the same Merge step
  from `conventions.md`.
- **Escalating:** the task orchestrator marks the task `Escalated`, stays on the
  branch it is on, and stops. The main orchestrator reads
  `verdict.md`, or `build-notes.md` if the builder blocked, and shows the user
  the summary and commands.

### Waiting and stopping

The task orchestrator waits for its agents by ending its turn with the single
word `WAITING`, and is woken when they report. It never polls or sleeps. Its run
is over when it ends a turn with no agent running, so it does that only in one
of three end states, with a one-line final message:
- **Merged:** the merge succeeded and it is on `main` (`merged <task-id>`).
- **Escalated:** the Escalate step has finished (`escalated <task-id>`).
- **Halted:** `loop.state` is `halted`, with the reason in `message`
  (`halted <task-id>`). If agents are still running when it must halt, it waits
  for them first, and only then sets `halted`.

It makes no further changes after reaching an end state.

The main orchestrator runs a task in this order:
1. Set `loop.state` to `running`, which clears any earlier `halted`, and
   `current` to the task. It doesn't touch the board again until step 4.
2. Spawn the task orchestrator.
3. End its turn with `WAITING`. It is woken by a hand-back message from that
   agent, or by a `<task-notification>` that carries that agent's id. It never
   polls the task orchestrator, sends it messages, reads its output file or
   starts other work.
4. **Each time either message wakes it**, decide from `backlog.md`,
   `board.json`, the current branch and `git status`, not from what the message
   says. A hand-back can arrive while the task orchestrator's own agents are
   still running, and a notification may never follow one (a `claude -p` run can
   exit first), so it acts on neither alone. It takes the first case that
   applies:
   - `loop.state` is `halted`: report the reason. `/factory continue` resumes.
   - The task is `Escalated`: show the summary and commands.
   - The task is `In progress` and merged, on `main`, with a clean tree: mark it
     `Complete` and carry on with the next task.
   - Anything else, woken by a hand-back: the task orchestrator is still
     working. Take no action and end the turn with `WAITING`.
   - Anything else, woken by the notification: the task orchestrator stopped
     early. Revert nothing, set `loop.state` to `halted` with the message `Task
     orchestrator stopped early on <task-id>`, and tell the user that
     `/factory continue` resumes the task from its files.
5. Once it has acted on a task orchestrator, it ignores any later message from
   that agent.

A task is merged if `git log main --format=%s` contains the subject
`factory: merge <id>`, or if its branch exists, is an ancestor of main
(`git merge-base --is-ancestor`), and has at least one builder commit (subject
starting with `<id>: `). A branch with no builder commits is not merged, even
though it is an ancestor of main.

### Updating board.json

A board for a large spec is tens of kilobytes, so rewriting it through the Write
tool on every update would fill the orchestrators' contexts. Never print the
file or leave it half-edited. Change only `updated`, `loop`, `agents` and the
current task's entry, with a short one-shot `python3 -I -c` command that loads
the JSON, changes those fields, writes `board.json.tmp` next to it and moves it
into place with `os.replace`. Use the Write tool only to create or rebuild the
file, and don't leave helper scripts behind.

### Responsibilities

The orchestrator uses a single working tree:
- On **main**, it runs the Planner, merges, and starts tasks.
- On **`factory/<task-id>`**, it runs the build and review rounds.

Its responsibilities are:

- Maintain `factory/state/board.json`, which reflects the current state of
  the loop (schema below). The orchestrator is the owner and sole writer of
  this file: the task orchestrator while it runs, the main orchestrator
  otherwise, never both at once. The file isn't tracked by git, and it can
  always be rebuilt from `backlog.md` and the task files. Update it every time
  the orchestrator takes back control from a subagent, and on every stage
  change (see "Updating board.json").

- Maintain `factory/state/backlog.md`, which reflects the state of the known
  tasks. Statuses are `Planned`, `In progress`, `Escalated`, `Complete` and
  `Abandoned`. The orchestrator and the Planner are the owners and sole
  writers of this file. It isn't tracked by git, so it can be edited on any
  branch.

- Spawn the Planner (see `/factory start`) and wait for it to finish.

- Start a task: on main, mark it `In progress`. Create `round-1/` and spawn a builder named
  `builder-<task-id>`, giving it the task-id and round-id.

- When the builder finishes:
  - If the build notes say `STATUS: BLOCKED`, skip the reviewers and handle
    the task like an ESCALATE.
  - Otherwise spawn the three reviewers in parallel and set each of their
    states to `pending` in `board.json`.

  Wait for the reviewers to finish. As each one finishes, check its output and
  update its state in `board.json` to PASS or FAIL according to the first line
  of its review. To check the output, confirm that the agent created or changed
  only its allowed files (see "Write verification").

- When all the reviews for the round are in:
  - If any review is FAIL and this is round 1, create the next round
    directory and resume the builder with instructions to address the
    reviews.
  - Otherwise spawn the Approver and wait for its verdict.

  The orchestrator creates at most two automatic rounds per task (round 1
  and one fix round). The Approver can add up to two Approver rework rounds
  through REWORK verdicts. Any further rounds come only from the human user
  through `/factory rework`. Neither kind of rework round triggers an
  automatic fix round.

- When the Approver has produced a verdict:
  - **APPROVE:** switch to main and merge the branch with
    `git merge --no-ff -m "factory: merge <task-id>"`. If the merge
    conflicts, stop and notify the human user. Once the conflicts are
    resolved, the user completes the merge and restarts the loop with
    `/factory continue`.
  - **REWORK:** stay on the task branch. Don't touch `backlog.md`, because
    the task stays `In progress`. Create the next round directory, copy the
    verdict's `## Rework note` section into it as `approver-review.md`, and
    resume the builder with instructions to revise its work according to the
    note. After the builder finishes, the reviewers review the round, and
    then the Approver runs straight away. If the verdict is REWORK but the
    task already has two earlier REWORK verdicts, or the verdict has no
    rework note, handle it like an ESCALATE.
  - **ESCALATE:** mark the task `Escalated` and stay on the branch. Set the loop
    state to `escalated` and halt with a
    summary of the verdict and the commands the user can run next.

- When work has been merged into main, mark the task `Complete` and delete the
  local task branch. If no `Planned` tasks remain, the loop ends
  with a message to the user stating that the work is complete. Otherwise the
  loop continues with the next task.

### Builder reuse

Within one task orchestrator run, resume the same `builder-<task-id>` (via
SendMessage) for each new round of that task. A task orchestrator that hasn't
spawned that builder itself (after `continue` or `rework`) spawns a fresh
`builder-<task-id>`. It rebuilds its context from `task.md`, the earlier round
directories, and the branch's git log.

### Write verification

After each subagent finishes, the orchestrator checks two things:
- **Outside the state,** it runs `git status`. Changes to files that git ignores
  (for example `target/`) don't count. The Builder must leave a clean tree, with
  nothing under `factory/state/` in its commits. Every other agent changes
  nothing here.
- **Inside the state,** `git status` can't see changes, so it compares checksums.
  Just before spawning an agent, it writes a `cksum` listing of every file under
  `factory/state/` to `factory/state/tasks/<task-id>/.snapshot`, or
  `factory/state/.snapshot` for the Planner, leaving out `board.json*` and
  `.snapshot` files. Afterwards it runs the same listing and compares it with
  `diff`. The Builder may change only its `build-notes.md`.

Reviewers run in parallel, so:
- After each reviewer finishes, any change outside the current round
  directory is a violation.
- Once all of them are done, any file in the round directory other than
  `code-review.md`, `feature-review.md`, `ui-review.md`, `ui/**`,
  `build-notes.md`, `human-review.md` and `approver-review.md` is a
  violation.

For the Approver, anything other than `verdict.md` is a violation. For the
Planner, anything other than `backlog.md` and `tasks/*/task.md` is a
violation. On a violation, halt (loop state `halted`, with a reason) without
reverting anything, and tell the user what changed.

## The dashboard

A simple kanban-style dashboard at `factory/dashboard/index.html` displays
the state of `factory/state/board.json`. It is a static page (with optional
CSS/JS files next to it), served with `python3 -m http.server -d factory`
and opened at `/dashboard/`. Every 5 seconds it fetches
`../state/board.json` with `cache: 'no-store'`, so it never shows stale
data.

- The columns are the stages: planned, building, review, approval,
  escalated, done, abandoned.
- Tasks are cards that move across the columns.
- Each round shows as a row of reviewer chips (code, feature, ui) plus a
  verdict chip, which fill in as results land.
- ESCALATE and BLOCKED tasks stand out.
- A header shows the loop state and its message.
- Clicking a card opens a panel that lists the task's Markdown files (from
  the card's `files`). Clicking a file renders it. Use a small Markdown
  library with a pinned version from cdnjs, or fall back to preformatted
  text.
- Clean and minimal.

## Factory file system

```
.claude/
  skills/factory/SKILL.md          # main orchestrator instructions
  skills/factory/conventions.md    # shared by both orchestrator levels
  agents/                          # one file per agent
    task-orchestrator.md
    planner.md
    builder.md
    code-reviewer.md
    feature-reviewer.md
    ui-reviewer.md
    approver.md
factory/
  factory.md                       # This prompt
  floorplan.md                     # The factory floor plan
  dashboard/
    index.html                     # visual board, polls ../state/board.json
  archive/                         # finished runs, moved here by hand (git-ignored)
  state/                           # everything below is git-ignored
    board.json                     # current loop status
    backlog.md                     # queue of tasks
    SPEC.md                        # The spec file defining the work
    spec/                          # documents linked from SPEC.md
    tasks/                         # Tasks created by the planner
      001-scaffold-crate/          # one directory per task
        .snapshot                  # checksums for write verification
        task.md                    # Spec for the task
        round-1/                   # one directory per round
          build-notes.md           # Builder: STATUS: DONE | BLOCKED
          code-review.md           # one file per reviewer
          feature-review.md
          ui-review.md
          ui/                      # optional UI Reviewer snapshots
          verdict.md               # Approver verdict (if run this round)
        round-3/
          approver-review.md       # only in Approver rework rounds
          ...
        round-5/
          human-review.md          # only in human rework rounds
          ...
```

## Example files

.claude/agents/<name>.md (Claude Code subagent syntax; check the frontmatter
field names against the current Claude Code docs)

```markdown
---
name: example-agent
description: One line on when the orchestrator should use this agent.
tools: Read, Grep, Glob, Bash, Write
model: claude-opus-5-5
effort: high
---

What this agent reads, what it does, and which file it writes.
```

factory/state/board.json

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

- `loop.state`: `planning`, `awaiting-plan-review`, `running`, `escalated`,
  `halted` (merge conflict, write violation, error; the reason goes in
  `message`), or `complete`.
- `agents`: builder agent ids, keyed by name. An id is only usable by the task
  orchestrator that spawned it; a later one spawns a fresh builder and replaces
  the id.
- `stage`: `planned`, `building`, `review`, `approval`, `escalated`, `done`,
  or `abandoned`.
- `kind`: `auto`, `approver-rework` (started by a REWORK verdict) or
  `rework` (started by `/factory rework`).
- `build`: `pending`, `DONE` or `BLOCKED`.
- Review values: `pending`, `PASS` or `FAIL`.
- `verdict`: `null`, `APPROVE`, `REWORK` or `ESCALATE`.
- `files` paths are relative to the task directory.

## Rules

- Agents get context from files, not the conversation, and write only their
  designated outputs.
- Each review starts with the line `VERDICT: PASS` or `VERDICT: FAIL`, and
  is FAIL only if it has a must-fix finding.
  `verdict.md` starts with `VERDICT: APPROVE`, `VERDICT: REWORK` or
  `VERDICT: ESCALATE`.
  `build-notes.md` starts with `STATUS: DONE` or `STATUS: BLOCKED`.
- The builder commits and works on a feature branch (`factory/<task-id>`) and
  tests against the acceptance criteria in
  `factory/state/tasks/<task-id>/task.md`.
- The factory never pushes changes to an origin. Only the human user pushes.
- Agents never share scratch space. The session scratchpad and `$TMPDIR` are
  shared by agents running at the same time, so the definition of every agent
  that can work outside the repo (Builder, reviewers, Approver) tells it to:
  create its own directory with `mktemp -d`, named
  `<role>-<task-id>-<round-id>.XXXXXX`, before any such work; keep all exports,
  copies, probes and backups there, with `CARGO_TARGET_DIR` set to a `target`
  folder inside it; never use, overwrite or delete a path it didn't create in
  this run; and delete its directory when it finishes.
- Wait for all reviews: a review round is done only when every reviewer's
  file exists in the round directory. Never resume the builder mid-round.
- Spawn the builder with a name (`builder-<task-id>`) and resume that same
  builder for each new round when it is still available (see "Builder
  reuse").
- The orchestrator creates a new round directory at the beginning of each
  round.
- Only one task is ever `In progress` or `Escalated` at a time.
