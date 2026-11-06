Build a visual "software factory" in this repo using a Claude skill,
subagents, JSON and Markdown files. The factory is a single skill, `/factory`,
that runs the loop described below. A Planner splits a spec into tasks. For
each task, a Builder implements it, reviewers check it, and an Approver
decides whether it merges, goes back for another round, or waits for a human.

Most of this document describes how the factory behaves: its commands, its
roles, the orchestrator's loop, the dashboard and the rules. The last
section, "Building the factory", lists the files to create and which parts of
this document go into each one.

## Skill Commands

Every command except `status` requires `factory/state/` to be ignored by git,
and a clean working tree: no uncommitted changes and no untracked files that
git doesn't ignore, whatever git's `status.showUntrackedFiles` setting. Each
command also requires a particular branch:
- `start`: main.
- `continue`: main, or the branch of the `In progress` task (see below).
- `approve`, `rework` and `abandon`: main or `factory/<task-id>`.

If a precondition fails, the command refuses with a message and does nothing.
`continue` checks for an `Escalated` task before it checks the branch or the
tree. An escalation leaves the session on the task branch, so a refusal for
the wrong branch would hide the real reason. Instead it refuses with the
pointer to `approve`, `rework` and `abandon` (see `/factory continue`).

`factory/state/` is not in git, so there is one copy of it, the same on every
branch. The orchestrator and agents read and edit its files directly, and
nothing under it is ever committed.

/factory start <spec>
Starts the factory. The session using the command becomes the orchestrator.
Refuse if `factory/state/backlog.md`, `factory/state/tasks/` or
`factory/state/preflight.md` already exist (tell the user to archive or remove
`factory/state/` first; the factory never deletes a previous run). Copy the
spec with the factory script (see "The factory script"). It writes the
absolute path of <spec>, with any `~` expanded, to
`factory/state/spec-source`. It copies <spec> to `factory/state/SPEC.md`,
and every Markdown file that <spec> links to with a relative link
(following links transitively) into `factory/state/spec/`. It rewrites those
links so they resolve from the copies, and reports any linked document that
is missing. Then spawn the Planner. It runs the preflight first (see
"Preflight"). When the Planner is done:
- If it wrote `backlog.md`, the orchestrator checks the plan (see "Checking
  the plan"), builds `board.json`, and halts with a summary of the backlog,
  the preflight assumptions, and the stack profile (including anything under
  **Not available**), so the human can review (and optionally edit) the
  plan. Building starts with `/factory continue`. This first halt also says
  how to open the dashboard (see "The dashboard").
- Otherwise the preflight stopped. The orchestrator builds `board.json` with
  `loop.state` `awaiting-preflight`, and halts with a summary of
  `preflight.md` and the next steps:
  - For each spec issue, fix the spec at the path in `spec-source`. If the
    Planner is wrong about an issue, add a sentence to the spec that settles
    it. If the spec is in the repo, commit the change.
  - For each tool on offer, the install command, which the user runs
    themselves (in Claude Code, for example, with `! <command>`). The
    orchestrator never installs anything. A tool the user doesn't install is
    declined, and the next preflight doesn't offer it again.
  - Then run `/factory continue`.

/factory continue
Restart a stopped loop. The session using the command becomes the
orchestrator. Before choosing work it reconciles state:
- If `preflight.md` exists and `backlog.md` doesn't (the preflight state),
  re-copy the spec from the path in `spec-source`, replacing `SPEC.md` and
  `spec/` the same way `start` does. Then spawn the Planner again and act on
  its result as `start` does. Nothing else in this list applies.
- If `board.json` is missing, rebuild it from `backlog.md` and the files in
  `factory/state/tasks/`.
- If every task is `Planned`, no task has a round directory, and
  `loop.state` isn't `awaiting-plan-review`, the human hasn't seen the plan
  yet. For example, the Planner step may have halted after `backlog.md` was
  written. Check the plan (see "Checking the plan"), rebuild `board.json`,
  and halt with the plan summary, as `start` does. Nothing else in this list
  applies.
- If `loop.state` is `awaiting-plan-review`, the human may have edited the
  plan. Check it again (see "Checking the plan"), and rebuild `board.json`
  from `backlog.md` and the files in `factory/state/tasks/`.
- If a task is `In progress` and its branch `factory/<task-id>` is already
  merged into main (the user resolved a merge conflict), mark it `Complete`,
  delete the branch, and carry on.
- If a task is `In progress` and not merged (an interrupted run), check out
  its branch (or stay on it) and resume at the right step of its latest
  round. Work out the step from which files exist in the round directory
  (build notes, reviews, verdict). If the latest round's verdict is REWORK,
  start the Approver rework round it asks for (see "The orchestrator").
  When `continue` runs on this branch, it allows uncommitted changes only
  while the latest round has no `reviews-started` marker (see "Write
  verification"). They must then be the Builder's own, and it leaves them
  for the resumed builder. Once the marker exists, the changes came from a
  reviewer or the Approver. Then `continue` halts and asks the human to
  inspect them and commit or remove them, because otherwise a builder would
  commit changes that no reviewer saw.
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
conflict, mark the task `In progress`, then stop and notify the human user.
Once the conflicts are resolved, the user completes the merge and restarts
the loop with `/factory continue`, which finds the task merged and marks it
`Complete`. If the merge succeeds, mark the task `Complete`, delete the local
branch `factory/<task-id>`, and continue the loop in the same session.

/factory rework <task-id> <note>
Send an `Escalated` task back to the builder with feedback. Refuse if the
task isn't `Escalated`. The note is given as text or as the name of a file
outside the repo (a file inside the repo would make the tree unclean). The
orchestrator works in this order, so that an interrupted `rework` can simply
be run again:
1. Check out `factory/<task-id>`, creating it from main if it doesn't exist.
2. Create the next round directory. If the latest round directory has no
   build notes (an earlier `rework` was interrupted), reuse it instead.
3. Copy the note to
   `factory/state/tasks/<task-id>/<round-id>/human-review.md`, replacing any
   earlier one, and then write the round's `base-commit`.
4. Only then mark the task `In progress`.

It then spawns a task orchestrator, which spawns a fresh builder for the
round. The Builder's definition says how it revises its work according to
the note. After the builder finishes, the reviewers review the round, and
then
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
would take next. In the preflight state it prints the preflight result and
its open issues and offered tools instead of the backlog.

## Identifiers

Tasks are identified by a serial number, zero-padded to at least three
digits, followed by a two-to-three word "fingerprint" of the task in
lowercase letters and digits. The serial number and words are separated by
dashes. Example: `002-apply-selected-theme`.

A repository never reuses a serial number, so that the merged check and the
branch names can't match a task from an earlier run. The Planner numbers a
run's tasks consecutively from one above the highest serial the factory has
already used, or from `001` if it has used none. It finds the used serials in
git, which every clone has, not in the archive, which may be gone:
- the subjects `factory: merge <id>` in `git log main --format=%s --` (the
  trailing `--` stops git from reading `main` as a path);
- the local branches `factory/<id>`, which include abandoned tasks.

Rounds are identified by the word `round` and a number that increases
monotonically from 1, separated by a dash. Example: `round-4`.

Branches are named `factory/<task-id>`.

## The Floor Plan

Read [floorplan.md](floorplan.md) to see the factory floor plan.
The agent roles are:

- **The Planner** first runs the preflight (see "Preflight"), and stops there
  unless the result is `PREFLIGHT: READY`. It then divides
  `factory/state/SPEC.md` (with its linked documents in
  `factory/state/spec/`) into a suitable number of tasks that together
  implement the work described in them. Each task becomes a directory
  `factory/state/tasks/<task-id>/`. In each task directory the Planner writes
  a `task.md` file containing everything from the spec that a builder needs
  to implement the task.

  The factory script reads `task.md`, so its shape is fixed. The first line
  is `# <task-id>: <title>`. Then come these sections, as `##` headings with
  exactly these names and in this order:
  - `## Goal`: what the task achieves, in a few sentences.
  - `## Spec excerpts`: the relevant parts of the spec, each with where it
    comes from. They are quoted as blockquotes (`> `), so that a heading in
    the spec can never be mistaken for a section of `task.md`.
  - `## Acceptance criteria`.
  - `## UI`: describes every view and state the task changes precisely
    enough to check: what is shown, the layout, styling such as theme roles
    or colours, focus and selection, and the sizes it must work at, with the
    spec's sketches where it has them. When the task has no UI changes, the
    section is the single heading line `## UI: none`.
  - `## Reviews`: one top-level bullet for each reviewer that runs on the
    task (see "The reviewer roster"), starting with its key in backticks,
    such as ``- `security`: parses session files from disk``. It lists only
    the reviewers that run, and nothing else in the section is a bullet.
    `code` and `feature` are always listed, and `ui` is listed exactly when
    the task has UI changes. Each specialist's bullet says why it applies. A
    specialist applies only when the task touches its concern as the
    baseline in `preflight.md` defines it. For each specialist it lists, the
    Planner writes the spec requirements that reviewer will check into the
    acceptance criteria, so the Builder knows them before it starts.
  - `## Depends on`: one top-level bullet for each task-id it depends on, or
    the single bullet `- none`.

  When all tasks are written, the Planner writes `factory/state/backlog.md`,
  the execution plan. It contains a Markdown table with one row per task and
  two cells per row: the task-id and the task's status (`Planned` for every
  task at this point). The tasks must be in implementation order from top to
  bottom, so that earlier tasks don't depend on later ones. Before it
  finishes, the Planner checks its plan (see "Checking the plan").

  The Planner sizes each task so that a builder can finish it, and a reviewer
  can judge it, in one round. It leaves out anything the spec defers, such as
  later versions or non-goals. It avoids tasks that only add code for later
  tasks to use, because reviewers rightly flag unused code.

  The Planner can assume the stack's base toolchain is installed (such as the
  compiler or runtime and the package manager), but not that the project
  exists or that any other tool is installed. If the project doesn't exist,
  the first task scaffolds it. The scaffold includes `.gitignore` entries for
  the stack's build outputs and dependency folders, because every clean-tree
  check assumes they are ignored. It also includes the gate's dev
  dependencies and whatever **Rendering** needs. The project must let the
  UI Reviewer render its views from outside the repo, as `stack.md`
  describes. For example, a Rust
  TUI exposes a library target with renderable widgets that a scratch crate
  can depend on, and a web app has a build the reviewer can serve and open in
  a headless browser.

  The Planner works on the main branch and writes only `preflight.md`,
  `stack.md`, `backlog.md` and the `task.md` files, and it doesn't commit.
  When it runs the gate on main, it follows the same safety rules as the
  reviewers (see "Safety rules"), so the gate's outputs can't make the tree
  unclean. The Planner runs claude-opus-5-5 with xhigh effort.

- **The Builder** implements one task, given to it by the orchestrator as a
  task-id and a round-id. It never reads or writes `backlog.md`.
  - **Round 1:** first of all, even if it then blocks, it creates the branch
    `factory/<task-id>` from main and switches to it. If the branch already
    exists, it checks it out instead.
  - **Later rounds:** it reads the reviews in the previous round directory
    (the `<key>-review.md` of each reviewer the task lists) and addresses
    every finding: every must-fix, and every nit unless its build notes list
    that nit with the reason for leaving it. In a rework round it also reads
    the `human-review.md` or `approver-review.md` in the current round
    directory. Where that note conflicts with a review, the note wins. A
    finding the rework note sets aside is left alone. If the previous round
    has no reviews because the Builder blocked, the note is its only input.
  - **A fresh builder** (in any round, including an interrupted round 1)
    rebuilds its context from `task.md`, the earlier round directories and
    the branch's git log. It carries on from any uncommitted changes it
    finds on the branch, which are an interrupted builder's work.

  It implements the task from `factory/state/tasks/<task-id>/task.md` and
  tests the result against that file's acceptance criteria. It reads the
  baselines of the task's specialist reviewers under **Assumptions** in
  `preflight.md`. It may read `SPEC.md` for context, but where the two
  differ, `task.md` wins. The work is not done until every command in the
  test gate in `factory/state/stack.md` passes.

  UI changes must come with snapshot tests of the rendered UI, using the
  tooling under **UI tests** in `stack.md`. The Builder accepts a snapshot
  only after checking that its content matches the `UI` section, and it
  never commits pending snapshot files. Pending snapshot files are never
  kept: the Builder deletes any it finds, its own or an interrupted
  builder's, and runs the tests again to produce the ones it still needs.

  Before it reports `STATUS: DONE`, the Builder self-checks its work against
  `task.md`, the way the reviewers will:
  - every acceptance criterion is met and covered by a test that would fail
    if the behaviour were wrong;
  - every UI state and size in the `UI` section has a snapshot;
  - for each specialist reviewer in the task's `Reviews` section, the work
    holds up against that reviewer's focus in the roster, its must-fix list
    under "Specialist reviewers", and its baseline;
  - for each reviewer the task lists, it has run the commands that `stack.md`
    gives for that key under **Reviewer checks**, and the build notes address
    each result or say why it doesn't apply;
  - in later rounds, every must-fix finding has been addressed, and every nit
    has been addressed or listed with a reason.

  At the end of each round the Builder writes
  `factory/state/tasks/<task-id>/<round-id>/build-notes.md`. Its first line
  is `STATUS: DONE`, or `STATUS: BLOCKED` if it couldn't pass the test gate
  or the task is unclear. The file then summarises the changes, the
  self-check (one line per acceptance criterion and UI state, with the test
  that covers it), the test gate results, and (if blocked) the reason. The
  Builder commits its work on the task branch, with messages prefixed
  `<task-id>: `. When it is blocked, it still commits what it has, with the
  subject `<task-id>: WIP (blocked) <summary>`, so the tree is clean and the
  partial work is kept and clearly marked. It writes the build notes last
  of all, after committing (see "Rules"). The build notes are not committed.
  Apart from them, it never writes under `factory/state/`.

  When it is resumed with the uncommitted-changes message (see "Write
  verification"), it commits those changes. If they shouldn't be committed,
  it fixes the cause instead, for example by adding a missing `.gitignore`
  entry for build outputs. Either way it leaves a clean tree and updates its
  build notes. It follows the Builder's safety rules (see "Safety rules").
  The Builder runs claude-sonnet-5-5 with High effort.

- **The Code Reviewer** reviews the changes on the task branch
  (`git diff main...HEAD`). It checks that they follow best practices for the
  language and frameworks, and any additional guidelines in
  `factory/state/SPEC.md`. It re-runs the test gate in the no-update mode
  from `stack.md`, so that failing snapshots don't write files into the repo.
  It produces a review with all its findings, and marks it PASS or FAIL (see
  "Review findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/code-review.md`. Apart from
  evidence in `code/` next to it, this is the only file the Code Reviewer may
  create or change. The Code Reviewer
  runs claude-opus-5-5 with High effort.

- **The Feature Reviewer** reviews the changes on the task branch to
  determine whether the implementation matches the feature description and
  acceptance criteria in `factory/state/tasks/<task-id>/task.md`. It gives a
  result for each acceptance criterion. It checks the Builder's self-check
  claims in the build notes against the code and the tests, and looks at the
  edge cases and error paths of the behaviour the task adds. It produces
  a review with all its findings, and marks it PASS or FAIL (see "Review
  findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/feature-review.md`. Apart from
  evidence in `feature/` next to it, this is the only file the Feature
  Reviewer may create or change. The Feature
  Reviewer runs claude-opus-5-5 with High effort.

- **The UI Reviewer** reviews the changes on the task branch to determine
  whether the implementation matches the UI specified in
  `factory/state/tasks/<task-id>/task.md`. It inspects the Builder's
  snapshots, and checks that no pending snapshot files are committed. Any
  tests it runs in the repo run in the no-update mode from `stack.md`. It
  also renders views itself, as **Rendering** in `stack.md`
  describes:
  - It creates a throwaway project outside the repo, in its own scratch
    directory (see "Rules"). The project depends on the repo's project, and
    its build outputs stay in the scratch directory.
  - It renders the views at the sizes in the `UI` section, and also tries a
    few edge cases the snapshots may not cover, such as the smallest size,
    long text and empty lists.
  - It never adds code to the repo.

  It produces a review with all its findings, and marks it PASS or FAIL (see
  "Review findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/ui-review.md`. Any rendered views
  that document issues (text snapshots or screenshots) go in
  `factory/state/tasks/<task-id>/<round-id>/ui/`.
  These are the only files the UI Reviewer may create or change. It runs only
  on tasks that list `ui` in their `Reviews` section. The UI Reviewer runs
  claude-opus-5-5 with High effort.

- **Specialist reviewers** (every roster row except `code` and `feature`: the
  UI Reviewer above, plus security, privacy, API and any rows added later)
  each review one concern, on the tasks that list them. See "The reviewer
  roster".

- **The Approver** reads `task.md`, the build notes, and all reviews from the
  current round, and the reviewers' evidence. It may also read earlier
  rounds, `SPEC.md` and `stack.md` for context. Where `task.md` and `SPEC.md`
  differ, `task.md` wins. It produces a final verdict by judging whether any
  remaining review issues are minor enough to ignore, can be fixed by
  another round, or need human review. It writes its findings and verdict to
  `factory/state/tasks/<task-id>/<round-id>/verdict.md` and marks the
  verdict APPROVE, REWORK or ESCALATE. This is the only file the Approver
  may create or change. It follows the reviewers' safety rules (see "Safety
  rules").
  - **APPROVE** means the orchestrator can merge the changes into main
    without human review.
  - **REWORK** means the Approver sends the task back to the builder for
    another round, without human review. The verdict must contain a
    `## Rework note` section: one concrete set of instructions for the
    builder, written like a `/factory rework` note. The note also lists
    every finding the Approver set aside, as wrong or as minor, so that the
    Builder leaves it alone and the reviewers don't raise it again.
  - **ESCALATE** means work must stop until a human review is complete.
    When it can, the Approver lays out the options for the human and
    suggests a rework note for each. It also says whether approving the
    branch as it is (`/factory approve`) would be reasonable.

  The Approver decides in this order:
  1. If every remaining issue is minor enough to ignore, APPROVE.
  2. If the issues allow two or more distinct solutions that lead to
     significantly different outcomes (for example different functionality
     or behaviour), ESCALATE. This is an A/B choice for the human, and the
     priority order below never settles it.
  3. If at least one review is FAIL, and the Approver can write a single
     suitable rework note, and rework rounds remain, REWORK.
  4. Otherwise ESCALATE.

  A must-fix finding in a hard category (see "Review findings") is never
  minor enough to ignore. The Approver may set one aside only by showing that
  it is wrong, for example because the reviewer misread the spec or the code.
  A must-fix finding in a soft category may be judged minor, and the verdict
  says why. A nit is never on its own a reason to REWORK.

  The Approver may issue at most two REWORK verdicts per task in total. It
  counts them from the `verdict.md` files of earlier rounds. Human rework
  rounds don't reset the count, and an unused REWORK is still available
  after a human rework.

  When reviewers conflict, the Approver resolves the conflict with this
  priority order, from most to least important:
  1. Security and data protection, whichever reviewer raises it.
  2. Correct, idiomatic, best-practice code (Code Reviewer).
  3. Feature implementation (Feature Reviewer).
  4. The other specialist reviewers, in roster order.
  5. UI (UI Reviewer).

  A rework note follows the higher priority where two reviews conflict. It
  says which demand it drops and why. The Approver runs claude-opus-5-5 with
  xhigh effort.

### Preflight

Before it plans, the Planner checks that the spec can be planned and that the
working environment is as good as it can be. It writes the stack profile (see
"The stack profile") and `factory/state/preflight.md`. That file's first line
is the result:
- `PREFLIGHT: READY`: nothing stops planning. The Planner goes on to plan in
  the same run.
- `PREFLIGHT: TOOLS`: nothing stops planning, but recommended tools are on
  offer. The Planner stops so the human can install them.
- `PREFLIGHT: BLOCKED`: at least one major issue. The Planner stops.

The rest of `preflight.md` has these sections:
- **Spec issues:** the major issues. Each names where it is, what is wrong,
  and what would resolve it. A major issue is something a plan can't be built
  on:
  - the spec contradicts itself, or contradicts the existing code, on
    something tasks would depend on;
  - information a plan needs is missing and no reasonable assumption covers
    it, such as an undefined core behaviour, or no way to tell the stack of a
    new project;
  - a document the spec links to is missing;
  - a requirement that no test or reviewer could check as written.
- **Environment issues:** also major. These are a missing tool that a
  formatting, linting or test command in the gate needs, or a gate that fails
  on main. When the project exists, the Planner runs the whole gate on main in
  no-update mode, so a changed gate is checked against the code already
  there. Each issue gives the failing command, its output in brief, and what
  would resolve it.
- **Assumptions:** smaller gaps, with the assumption the plan makes for each.
  It always includes the baseline for each specialist reviewer the plan is
  likely to use (see "Specialist reviewers"), taken from the spec where the
  spec states it. The UI Reviewer's baseline is each task's `UI` section, so
  it isn't listed here.
- **Tools on offer:** each recommended tool that isn't installed, what it
  would check, the reviewer or gate section it serves, and the install
  command. A tool that a formatting, linting or test command needs is never
  on offer: its absence is an environment issue.
- **Stack profile changes:** what changed from the previous cycle's profile,
  if there was one. This stays the comparison when the preflight runs again
  in the same cycle.

A spec or environment issue always blocks. A tool is on offer only once: the
halt shows it whether the result is `TOOLS` or `BLOCKED`. When the Planner
finds a `preflight.md` from an earlier preflight in this cycle, the human
has had the offer for
every tool under its **Tools on offer**. The Planner marks each of those
tools that is still missing as `declined` in `stack.md`. A declined tool is
never offered again, in this cycle or a later one (see "The stack profile").
So a preflight with no major issues and only declined tools left gives
`READY`, and `/factory continue` moves on to planning.

### Checking the plan

The plan is checked three times: by the Planner before it finishes, by the
orchestrator after the Planner, and by the orchestrator again when
`/factory continue` leaves the plan review, because the human may have
edited the plan. The plan is sound when:
- every task in `backlog.md` has a `task.md` with all its sections, and
  every task directory appears in `backlog.md` exactly once;
- every task's status is `Planned`;
- every `Reviews` section lists `code` and `feature`;
- every dependency names a task earlier in this backlog, or a task from an
  earlier run that is already merged;
- every task-id follows "Identifiers", including serials that continue from
  earlier runs.

The factory script makes the mechanical checks (see "The factory script").
The Planner fixes what it finds. When the orchestrator finds a problem, it
halts and says what is wrong, and leaves `loop.state` at
`awaiting-plan-review` so that the next `/factory continue` checks again. The
plan-review halt tells the human that removing a task from `backlog.md` also
means deleting its task directory.

### The stack profile

`factory/state/stack.md` is the only place that names the project's tools.
Everything else in this prompt refers to it, so the factory works the same on
any stack. The Planner writes it during the preflight. If an earlier cycle's
profile exists (the most recently modified `factory/archive/*/stack.md`), the
Planner starts from it: it keeps its choices, the human's edits and the
declined tools, re-checks every tool, and changes only what the project or the
spec now needs. When the preflight runs again in the same cycle, the Planner
starts from the current `stack.md` instead, so the same applies to it.
Otherwise it writes the profile from `SPEC.md` and, when the
project exists, from its manifests, lockfiles, CI configuration and scripts.
It prefers the tools the project already uses. The human sees the profile at
any preflight halt and at the `awaiting-plan-review` halt. After planning,
only the human changes it. It has these sections:

- **Stack:** languages, frameworks, package manager and toolchain versions.
- **Test gate:** the commands, in order, that must all exit 0. They always
  cover formatting, linting with warnings treated as errors, and the tests.
  Where the stack has them, they also cover the automated checks that the
  roster's reviewers rely on, such as a dependency audit or a static analyser.
  For example, a Rust project might use `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings` and `cargo test`.
- **No-update mode:** how to run the gate so that snapshot or golden tests
  fail instead of writing new files (with `insta`, for example,
  `INSTA_UPDATE=no`).
- **UI tests:** the snapshot tooling for UI changes, how a snapshot is
  accepted, and which pending files are never committed (with `insta` and
  Ratatui's `TestBackend`, for example, accepted `.snap` files are committed
  and `.snap.new` files never are). `UI tests: none` when the project has no
  UI.
- **Rendering:** how the UI Reviewer renders views from outside the repo, and
  at which sizes (terminal sizes, viewports).
- **Scratch builds:** how to send build outputs to a scratch directory (with
  Cargo, for example, `CARGO_TARGET_DIR`).
- **Concurrency:** whether the gate's commands can safely run at the same
  time in one checkout, as the reviewers do when they run in parallel. Cargo,
  for example, locks its build directory, so concurrent runs are safe: they
  wait for each other. If the commands can't run concurrently, reviewers run
  the gate with their build outputs in their scratch directories.
- **Reviewer checks:** for each roster key, any extra tools that reviewer runs
  beyond the gate.
- **Not available:** tools that belong in the gate or in a reviewer's checks
  but aren't installed, each with how to install it. A tool the human chose
  not to install is marked `declined`.

Every command under **Test gate** and **Reviewer checks** uses a tool that
the Planner has confirmed runs (for example with a version check), or one
that the first task adds to the project as a dev dependency. Anything else
goes under **Not available**. No agent skips a gate command. If one can't
run, the Builder reports `STATUS: BLOCKED`.

The profile is runtime data, not part of the factory's own files, so
rewriting it between cycles never changes an agent's instructions. Agents
read it each time they run.

### The reviewer roster

Each review concern belongs to one reviewer, identified by a key. Its agent
is `.claude/agents/<key>-reviewer.md`, and its review is `<key>-review.md`.
The roster's order is the order of the reviewer chips on the dashboard and,
after security and data protection, the Approver's priority order.

| Key | Reviewer | Runs on | Focus |
|---|---|---|---|
| `code` | Code Reviewer | every task | As described above. |
| `feature` | Feature Reviewer | every task | As described above. |
| `security` | Security Reviewer | tasks that handle authentication, authorisation, sessions, secrets or untrusted input, run external commands, or add dependencies | Injection, access control, secret handling, unsafe deserialisation, dependency risk, and the security requirements in the spec. |
| `privacy` | Privacy Reviewer | tasks that collect, store, log, process or transmit personal data | The data-protection requirements in the spec and the regime it names (for example GDPR which is also the default): data minimisation, purpose and consent, retention and deletion, access and export, personal data in logs, and transfer to third parties. |
| `api` | API Reviewer | tasks that add or change a public interface: an HTTP or RPC API, a library's public API, a command-line interface, or a file or wire format | The interface matches the spec, stays backwards compatible or is versioned, names things and reports errors consistently, and is documented. |
| `ui` | UI Reviewer | tasks whose `UI` section isn't `UI: none` | As described above. |

A task's `Reviews` section says which reviewers run on it, and `code` and
`feature` always run, whatever it says. A `task.md` without one runs `code`,
`feature`, and `ui` unless its `UI` section is `UI: none`.

Every reviewer follows the same contract:
- It reviews the changes on the task branch (`git diff main...HEAD`) within
  its focus, against `task.md`, `SPEC.md` and the documents in
  `factory/state/spec/`. Where `task.md` and `SPEC.md` differ, `task.md`
  wins: a difference from `SPEC.md` that `task.md` settles isn't a finding.
  If the reviewer thinks `task.md` itself departs from the spec in a way
  that matters, it says so as a nit for the human.
- Any tests it runs in the repo run in the no-update mode from `stack.md`. It
  also runs the tools that `stack.md` lists for its key under **Reviewer
  checks**. Any work outside the repo happens in its own scratch directory
  (see "Rules"). It follows the reviewers' safety rules (see "Safety
  rules").
- It writes its review to
  `factory/state/tasks/<task-id>/<round-id>/<key>-review.md`, and any
  evidence, such as rendered views or tool output, to `<key>/` in the same
  directory. These are the only files it may create or change.
- It labels its findings as described in "Review findings". Anything it
  notices outside its focus is a nit, except that every reviewer raises a
  security or data-loss risk as must-fix.

Specialist reviewers (every row except `code` and `feature`) run
claude-opus-5-5 with High effort. To add a review concern, add a row here and
a description under "Specialist reviewers". Then rebuild the factory files:
besides the new agent file, several of them list the roster's keys (see
"Building the factory").

### Specialist reviewers

Each specialist judges its concern against a baseline: the scope of that
concern for this project. The baseline comes from the spec, or else from
**Assumptions** in `preflight.md`; the UI Reviewer's comes from the task's
`UI` section. The specialist states the baseline at the top of its review. A
finding that matters only under a wider baseline is a nit. Each description
says what the reviewer reads beyond the diff and what is must-fix in its
concern. Anything else in the concern is a nit.

Each must-fix below is in a hard category (see "Review findings"): a bug, a
security or data-loss risk, or an unmet spec requirement. There are two
exceptions, which are soft: a missing snapshot is a missing test, and the
API Reviewer's inconsistent naming or error reporting is best practice.

- **UI Reviewer.** Baseline: the views, states and sizes in the task's `UI`
  section. It reads the Builder's snapshots, and renders the views itself at
  those sizes and at the edge cases it tries. Must-fix: a view that
  contradicts the `UI` section; a listed state or size with no snapshot; a
  pending snapshot file committed to the branch; or a crash, panic or
  unusable layout at any size it tries, edge cases included. Other
  differences outside the `UI` section are nits.

- **Security Reviewer.** Baseline: the threat model, meaning who can reach
  the program's inputs and what they can do with them. A local single-user
  tool reading the user's own files has a narrow one; a public web service
  has a wide one. It reads the changed code paths that take untrusted input,
  run external commands, handle secrets or enforce access, and every added or
  upgraded dependency, from the manifest and lockfile diff. Must-fix: a
  weakness that is exploitable within the threat model; a secret in code,
  logs or the repo; a missing access check that the spec requires; or a
  dependency that a check under **Reviewer checks** reports as vulnerable.
  Any proof-of-concept input runs only inside its scratch directory.
- **Privacy Reviewer.** Baseline: which data is personal, the regime (GDPR
  unless the spec names another), and the program's role, from a controller
  of other people's data to a local tool for the user's own data. It traces
  each item of personal data the change touches: where it is collected,
  stored, logged and sent, and how long it is kept. Must-fix: personal data
  collected, kept, logged or sent beyond what the spec needs or allows; a
  missing deletion, export or consent path that the spec or the regime
  requires; or personal data reaching a third party the spec doesn't name.
- **API Reviewer.** Baseline: which interfaces are public, and the
  compatibility promise for each, such as semantic versioning for a library,
  versioned endpoints, stable command-line flags, or older file formats that
  must stay readable. It compares each changed interface on main
  (`git show main:<path>`) with the branch. Must-fix: a breaking change
  without the version change or migration the promise requires; an interface
  that differs from the spec; inconsistent naming or error reporting within
  one interface; or a public interface missing documentation that the spec
  requires.

### Review findings

Each reviewer labels every finding `[must-fix]` or `[nit]`, and describes
precisely the change needed for each one.
- **must-fix:** a failing test gate; an unmet acceptance criterion or spec
  requirement; a bug or wrong behaviour; a security or data-loss risk; a UI
  that contradicts the task's `UI` section; a breach of a guideline in
  `SPEC.md`; a missing or ineffective test for required behaviour; or a clear
  violation of language or framework best practice that would cause real
  problems. The last two are the *soft* categories and the rest are *hard*.
  Later rounds treat the two kinds differently (see "Later rounds").
- **nit:** anything else worth mentioning, such as style preferences, small
  naming or documentation issues, minor inefficiencies off the hot path, and
  cosmetic details the spec doesn't pin down.

A review is FAIL if it has at least one must-fix finding, and PASS otherwise.
A PASS review still lists its nits. Reviewers don't raise a nit to must-fix
to force another round, or lower a must-fix to let the work pass.

### Later rounds

Round 1 is the place for a complete review, so a reviewer lists everything it
finds there. In every later round (fix, Approver rework and human rework
rounds alike), a reviewer:
1. Reads its own review from the previous round, and the `approver-review.md`
   or `human-review.md` in the current round if there is one.
2. Checks each must-fix finding from its previous review, and states in its
   review whether that finding is resolved. A finding that isn't resolved
   stays must-fix, unless the rework note sets it aside. No reviewer raises
   a finding the rework note sets aside, whoever raised it first.
3. Reviews the task as in round 1, and also reads this round's changes with
   `git diff <base>..HEAD`, where `<base>` is the commit in the round's
   `base-commit` file.
4. Classifies a new finding in code that this round changed as in round 1. A
   new finding in code that this round didn't change is must-fix only in a
   hard category. In a soft category it becomes a nit, marked
   `[nit] (found late)`. A missing test counts as being in changed code when
   the behaviour it should cover changed this round.

A reviewer that has no review in the previous round, or a round with no
`base-commit` file, classifies everything as in round 1.

## The orchestrator

The orchestrator runs the main loop of the factory, described above and shown
in [floorplan.md](floorplan.md). It spawns agents, chooses their tasks, and
sets up their context. It never does an agent's work itself. Agent prompts
contain only the task-id, the round-id, the paths to read, and the fixed
messages this document defines; all other context comes from files. What a
builder does in a later round, for example, comes from its definition and
the round's files, not from its prompt.

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
  start the same way. Before it does, it checks the first line of every
  file in the task's directory that has a fixed one, so that an invalid file
  left by an interrupted run halts before anything acts on it. It runs the
  task's rounds, and stops when the task is merged, escalated or halted.

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
  the summary and commands. The summary gives the reason: the Approver
  escalated, the Builder blocked, the REWORK limit was reached, or a REWORK
  verdict had no rework note. When the Builder blocked, it warns that the
  branch holds unfinished work that didn't pass the gate.

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

### The factory script

The orchestrators leave the factory's mechanics to the factory script,
`.claude/skills/factory/factory.py`. It is a factory file, run as
`python3 -I .claude/skills/factory/factory.py <command> …`, always from the
repository root. It finds the repository from the working directory
(`git rev-parse --show-toplevel`), not from its own location, so it can be
tested against a sample repository without touching the real run. The 
orchestrators keep the judgement: what to do next, and what to tell the human. 
The script does the parsing, so the same files always give the same answer. Its 
commands cover:
- **The board.** A board for a large spec is tens of kilobytes, so
  rewriting it through the Write tool on every update would fill the
  orchestrators' contexts. Every change to `board.json` goes through the
  script instead. It:
  - creates the board, and rebuilds it from `backlog.md` and the task files;
  - updates `updated`, `loop`, `agents` and one task's entry, working that
    entry out from the task's files;
  - prints the parts of the board an orchestrator needs to read: the loop,
    the agents, and one task's status, stage and latest round;
  - writes `board.json.tmp` next to the board and moves it into place with
    `os.replace`, so the file is never half-written.
- **The spec copy** for `start` and the preflight restart (see

`/factory start`).
- **The plan:**
  - the mechanical part of "Checking the plan";
  - the highest serial used before this run (see "Identifiers");
  - the newest earlier cycle's `stack.md` in the archive (see "The stack
    profile").
- **Rounds:**
  - the latest round, the previous round and the next round-id;
  - a round's kind;
  - the number of earlier REWORK verdicts;
  - a verdict's rework note, and whether it has any text.
- **Tasks:**
  - a task's reviewers, with the default and the rule that `code` and
    `feature` always run;
  - the tasks that depend on a task;
  - whether a task is merged.
- **First lines:** whether every file that has a fixed first line has a
  valid one.

The role agents use it as well, and their definitions name the commands:
- the Planner, for the highest serial used, the archived stack profile and
  the plan check;
- the Builder and the reviewers, for their previous round;
- the Approver, for the number of earlier REWORK verdicts.

Every command exits 0 when it succeeds and prints its answer, even when the
answer is "no" or an empty list. A non-zero exit always means an error, with
the reason on stderr, and the orchestrator halts with that reason. The
orchestrators never print the whole board or change it any other way, and
they leave no helper scripts behind.

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
  change (see "The factory script").

- Maintain `factory/state/backlog.md`, which reflects the state of the known
  tasks. Statuses are `Planned`, `In progress`, `Escalated`, `Complete` and
  `Abandoned`. The orchestrator and the Planner are the owners and sole
  writers of this file. It isn't tracked by git, so it can be edited on any
  branch.

- Spawn the Planner (see `/factory start`) and wait for it to finish.

- Start a task: on main, mark it `In progress`. Create `round-1/` and spawn a
  builder (see "Builder reuse"), giving it the task-id and round-id.

- When the builder finishes:
  - If the build notes say `STATUS: BLOCKED`, skip the reviewers and handle
    the task like an ESCALATE.
  - Otherwise spawn the reviewers listed in the task's `Reviews` section (see
    "The reviewer roster") in parallel, and set each of their states to
    `pending` in `board.json`.

  Wait for the reviewers to finish. As each one finishes, check its output and
  update its state in `board.json` to PASS or FAIL according to the first line
  of its review. To check the output, confirm that the agent created or changed
  only its allowed files (see "Write verification").

- When all the reviews for the round are in:
  - If any review is FAIL and this is round 1, create the next round
    directory and resume the builder for it.
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
    resume the builder for it. After the builder finishes, the reviewers
    review the round, and then the Approver runs straight away. If the
    verdict is REWORK but the task already has two earlier REWORK verdicts,
    or the verdict has no rework note, handle it like an ESCALATE, without
    creating the round directory.
  - **ESCALATE:** mark the task `Escalated` and stay on the branch. Set the loop
    state to `escalated` and halt with a
    summary of the verdict and the commands the user can run next.

- When work has been merged into main, mark the task `Complete` and delete the
  local task branch. If no `Planned` tasks remain, the loop ends
  with a message to the user stating that the work is complete. Otherwise the
  loop continues with the next task.

### Builder reuse

Within one task orchestrator run, use one builder for the task. The Agent
tool doesn't name agents, so the task orchestrator records the agent id of
the builder it spawns in `board.json`, under `builder-<task-id>`. For each
new round it resumes that builder by its id, via SendMessage. In Claude Code,
SendMessage is a deferred tool, so the task orchestrator loads its schema
with ToolSearch before its first resume. It spawns a fresh builder, and
replaces the id, when it hasn't spawned one itself in this run (after
`continue` or `rework`) or when resuming fails. A fresh builder, in any
round, rebuilds its context from `task.md`, the earlier round directories,
and the branch's git log, and carries on from any uncommitted changes it
finds.

### Write verification

After each subagent finishes, the orchestrator checks two things:
- **Outside the state,** it runs `git status`. Changes to files that git ignores
  (for example `target/`) don't count. The Builder must leave a clean tree, with
  nothing under `factory/state/` in its commits. If it leaves uncommitted
  changes, the orchestrator resumes it once with the fixed message
  `Task-id: <task-id>. Round-id: <round-id>. Uncommitted changes: commit
  them or fix their cause, then update the build notes.` If the tree still
  isn't clean, it halts. Every other agent changes nothing here.
- **Inside the state,** `git status` can't see changes, so it compares checksums.
  Just before spawning an agent, it writes a `cksum` listing of every file under
  `factory/state/` to `factory/state/tasks/<task-id>/.snapshot`, or
  `factory/state/.snapshot` for the Planner, leaving out `board.json*` and
  `.snapshot` files. Afterwards it runs the same listing and compares it with
  `diff`. The Builder may change only its `build-notes.md`.

Once the Builder's work has passed these checks, and just before the
orchestrator spawns the round's first reviewer, it writes an empty marker
file `reviews-started` in the round directory, and only then the snapshot.
The marker records that from here on the Builder is done with the round.
Uncommitted changes found before it are the Builder's own, and changes found
after it came from a reviewer or the Approver (see `/factory continue`).
When a resumed round re-runs only its missing reviewers, the marker is
already there.

Reviewers run in parallel, so:
- After each reviewer finishes, any change outside the current round
  directory is a violation.
- Once all of them are done, any file in the round directory other than
  `build-notes.md`, `base-commit`, `reviews-started`, `human-review.md`,
  `approver-review.md`, and, for each reviewer the task lists,
  `<key>-review.md` and `<key>/**`, is a violation.

For the Approver, anything other than `verdict.md` is a violation. For the
Planner, anything other than `preflight.md`, `stack.md`, `backlog.md` and
`tasks/*/task.md` is a violation. On a violation, halt (loop state `halted`,
with a reason) without reverting anything. Tell the user what changed, and
that they must inspect those changes and commit or remove them before
`/factory continue`.

## The dashboard

A simple kanban-style dashboard at `factory/dashboard/index.html` displays
the state of `factory/state/board.json`. It is a static page (with optional
CSS/JS files next to it), served with `python3 -m http.server -d factory`
and opened at `/dashboard/`. Every 5 seconds it fetches
`../state/board.json` with `cache: 'no-store'`, so it never shows a cached
board.

- The columns are the stages: planned, building, review, approval,
  escalated, done, abandoned.
- Tasks are cards that move across the columns.
- Each round shows as a row of reviewer chips, one for each key in the
  round's `reviews`, in the order `board.json` gives them (roster order),
  plus a verdict chip. The chips fill in as results land. A chip shows its
  result in words as well as colour, such as `code PASS`.
- ESCALATE and BLOCKED tasks stand out while they are active. A task that
  is `Complete` or `Abandoned` no longer does.
- If a fetch fails, the dashboard keeps showing the last board, marked as
  stale, with the reason.
- A header shows the loop state and its message. It also opens the run's own
  files in the panel when they exist: `SPEC.md`, `preflight.md`, `stack.md`
  and `backlog.md`. That way a preflight halt, which has no tasks yet, can
  still be read on the dashboard.
- Clicking a card opens a resizable panel that lists the task's files (from
  the card's `files`): its Markdown files and the reviewers' evidence. It
  opens `task.md` first. Clicking a file shows it. Markdown is rendered with
  a small Markdown library from cdnjs, pinned to a version and loaded with
  its integrity hash, falling back to preformatted text. Images show as
  images. Other files show as preformatted text that scrolls sideways
  instead of wrapping, so text snapshots keep their layout.
- Links and images inside a rendered file resolve against that file, not
  the dashboard page. A link to another file of the run opens it in the
  panel.
- The open file refreshes on each poll, so a file that changes shows its
  latest version.
- Clean and minimal.

## Factory file system

```
.claude/
  skills/factory/SKILL.md          # main orchestrator instructions
  skills/factory/conventions.md    # shared by both orchestrator levels
  skills/factory/factory.py        # the factory script
  agents/                          # one file per agent
    task-orchestrator.md
    planner.md
    builder.md
    code-reviewer.md               # one <key>-reviewer.md per roster row
    feature-reviewer.md
    security-reviewer.md
    privacy-reviewer.md
    api-reviewer.md
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
    spec-source                    # path of the original spec
    spec/                          # documents linked from SPEC.md
    preflight.md                   # Planner: PREFLIGHT: READY | TOOLS | BLOCKED
    stack.md                       # stack profile: test gate and tools
    tasks/                         # Tasks created by the planner
      001-scaffold-project/        # one directory per task
        .snapshot                  # checksums for write verification
        task.md                    # Spec for the task
        round-1/                   # one directory per round
          base-commit              # commit the round started from
          build-notes.md           # Builder: STATUS: DONE | BLOCKED
          reviews-started          # empty marker: the reviewers were spawned
          code-review.md           # one <key>-review.md per listed reviewer
          feature-review.md
          ui-review.md
          ui/                      # optional evidence, in <key>/
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
field names against the current Claude Code docs). The frontmatter must
parse as YAML, so quote any value that contains `: `, such as a description
with a colon in it.

```markdown
---
name: example-agent
description: "One line on when the factory uses this agent, saying that only the factory spawns it."
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

- `loop.state`: `planning`, `awaiting-preflight` (the preflight stopped; when
  rebuilding, this is the state whenever `preflight.md` exists and
  `backlog.md` doesn't), `awaiting-plan-review`, `running`, `escalated`,
  `halted` (merge conflict, write violation, error; the reason goes in
  `message`), or `complete`. Only the plan-review halt sets
  `awaiting-plan-review`, by passing it explicitly. A rebuild from the files
  never derives it, so a rebuilt board can't make `continue` skip the plan
  review (see `/factory continue`).
- `agents`: builder agent ids, keyed by name. An id is only usable by the task
  orchestrator that spawned it; a later one spawns a fresh builder and replaces
  the id.
- `stage`: `planned`, `building`, `review`, `approval`, `escalated`, `done`,
  or `abandoned`.
- `kind`: `auto`, `approver-rework` (started by a REWORK verdict) or
  `rework` (started by `/factory rework`).
- `build`: `pending`, `DONE` or `BLOCKED`.
- `reviews`: one key for each reviewer in the task's `Reviews` section, in
  roster order (see "The reviewer roster").
- Review values: `pending`, `PASS` or `FAIL`.
- `verdict`: `null`, `APPROVE`, `REWORK` or `ESCALATE`.
- `files`: `task.md`, every Markdown file in the round directories, and every
  file in the reviewers' evidence directories (`round-*/<key>/`). Paths are
  relative to the task directory.

## Rules

- Agents get context from files, not the conversation, and write only their
  designated outputs.
- Each review starts with the line `VERDICT: PASS` or `VERDICT: FAIL`, and
  is FAIL only if it has a must-fix finding.
  `verdict.md` starts with `VERDICT: APPROVE`, `VERDICT: REWORK` or
  `VERDICT: ESCALATE`.
  `build-notes.md` starts with `STATUS: DONE` or `STATUS: BLOCKED`.
  `preflight.md` starts with `PREFLIGHT: READY`, `PREFLIGHT: TOOLS` or
  `PREFLIGHT: BLOCKED`.
- The builder commits and works on a feature branch (`factory/<task-id>`) and
  tests against the acceptance criteria in
  `factory/state/tasks/<task-id>/task.md`.
- The factory never pushes changes to an origin. Only the human user pushes.
- Agents never share scratch space. The session scratchpad and `$TMPDIR` are
  shared by agents running at the same time, so the definition of every agent
  that can work outside the repo (Planner, Builder, reviewers, Approver)
  tells it to:
  create its own directory with `mktemp -d`, named
  `<role>-<task-id>-<round-id>.XXXXXX` (the Planner, which has neither id,
  uses `planner.XXXXXX`), before any such work; keep all exports,
  copies, probes and backups there, with build outputs sent to a folder inside
  it (see **Scratch builds** in `stack.md`); never use, overwrite or delete a
  path it didn't create in this run; and delete its directory when it
  finishes. Shell state doesn't persist between commands, so the agent uses
  the directory's full path each time.
- A command that finds, lists or counts things exits 0 when it succeeds,
  including when it finds nothing, so a non-zero exit always means an error.
  The exception is a plain git check whose exit status is its answer, such
  as `git check-ignore -q`, and the factory files label each one as such.
  The factory script's commands have no such exception (see "The factory
  script").
- Every agent writes each of its result files once, complete, at the end of
  its work, with a single write: never a draft or a stub first. These are
  `preflight.md`, the build notes, the reviews and the verdict. A round's
  result files (build notes, reviews and verdict) are final once they
  exist, because a resumed round treats them as done and the dashboard
  shows them as results. The one exception is a Builder resumed with the
  uncommitted-changes message, which rewrites its build notes, again in a
  single write. A preflight run that runs again replaces `preflight.md` and
  `stack.md`, as "Preflight" describes.
- Wait for all reviews: a review round is done only when the review file of
  every reviewer the task lists exists in the round directory. Never resume
  the builder mid-round.
- Record the builder's agent id under `builder-<task-id>`, and resume that
  same builder for each new round when it is still available (see "Builder
  reuse").
- The orchestrator creates a new round directory at the beginning of each
  round. Before spawning any agent for the round, it writes the round's base
  commit to `base-commit` in that directory. For round 1 that's the commit
  `main` points at. For later rounds it's the `HEAD` of the task branch. It
  writes the round's note (`human-review.md` or `approver-review.md`) first
  and `base-commit` last, so a round directory with a `base-commit` is
  complete. Before it creates an Approver rework round, it checks that the
  verdict has a rework note, so a REWORK without one never leaves a round
  directory behind.
- On re-entry, a round directory without `base-commit` was interrupted while
  it was being created:
  - an Approver rework round gets its note copied again from the previous
    round's verdict, and then its `base-commit`;
  - any other round just gets its `base-commit`.

  A human rework round can't be found like this while the task is
  `In progress`, because `/factory rework` marks the task only after the
  round is complete, and a repeated `rework` reuses the unfinished round
  (see `/factory rework`). Then the work goes on.
- A round's previous round is the latest earlier round directory that has a
  `base-commit`. Directories without one are never a previous round.
- Only one task is ever `In progress` or `Escalated` at a time.

## Safety rules

Every agent and both orchestrator levels follow these rules. Each agent
definition repeats the rules for its role.

- **Nobody** pushes, forces anything, rewrites history, or discards commits
  or uncommitted changes, except that the Builder deletes pending snapshot
  files (see below). That rules out `git push`, `reset`, `rebase`,
  `commit --amend`, `cherry-pick`, `revert`, `stash`, `clean`, `restore`,
  `checkout -- <path>`, `branch -D`, `branch -f`, `tag -d`, `update-ref`,
  `reflog expire` and `gc --prune`. Only the Merge step merges, and branches
  are deleted only with `git branch -d`; if that refuses, the orchestrator
  halts instead of forcing.
- **Nobody** changes the factory's own files while the factory runs: the
  skill and its conventions, the factory script, the agent definitions, the
  dashboard, `factory.md` and `floorplan.md`. In `.gitignore`, only the
  factory's two entries are off limits; tasks may add their own entries.
- **Nobody** installs anything outside the project and their own scratch
  directory. The project's dev dependencies go in its manifest. A tool that
  a scratch project needs, such as the UI Reviewer's rendering project, is
  installed into that scratch directory. Nothing is installed globally.
- **The Builder** never weakens, skips or disables a test or a gate
  command to make the gate pass. It never switches away from its task
  branch, except to create it in round 1. It keeps uncommitted changes it
  finds on the branch and carries on from them. Pending snapshot files are
  the one exception: it deletes them (see "The Floor Plan").
- **The Planner, the reviewers and the Approver** change nothing in the
  repository: no edits, commits, branch switches or stashes. Their tests run
  in the no-update mode from `stack.md`. A command that would still write
  files into the repo that git doesn't ignore runs with its outputs in their
  scratch directory instead.

## Building the factory

The factory is the set of files below. "Factory file system" shows where they
sit. Build them once from this document and `floorplan.md` alone, as if in a
new repository: earlier runs, archives and any existing factory files are
not a source. The orchestrator never creates, regenerates or edits them at
runtime. Commit them before the first `/factory start`, 
because every command except `status` needs a clean working tree. When this 
document changes, rebuild the affected files and commit them before the next run.

### The skill

`.claude/skills/factory/SKILL.md` is what runs when the user types
`/factory`. It is user-invoked only (`disable-model-invocation: true` in its
frontmatter), and it receives the command and its arguments as its
arguments. The session that runs it becomes the main orchestrator. Its first
instruction is to read `conventions.md`. It then covers:
- every command in "Skill Commands", with its preconditions and refusals;
- spawning the Planner and acting on its result, including the preflight
  halt and the restart from it (see "Preflight");
- checking the plan (see "Checking the plan");
- rebuilding `board.json` from `backlog.md` and the task files when it is
  missing, or when the plan review may have changed the plan;
- choosing the next task, and spawning and acting on the task orchestrator
  (see "Waiting and stopping").

Claude Code substitutes `$ARGUMENTS`, `$0`, `$1` and so on in a skill's
body before the model reads it, so any shell snippet that uses `$` (such as
an `awk` field) goes in `conventions.md`, which is read as it is.

### The shared conventions

`.claude/skills/factory/conventions.md` holds everything that both
orchestrator levels need, so that it is written once. Both levels read it
before anything else. It covers:
- **Names and paths:** the state paths, `T` as shorthand for the task
  directory and `R` for the round directory, and the list of agent types.
- **Ground rules:**
  - never push or rewrite history, and the destructive git commands that are
    never allowed;
  - only one active task at a time;
  - the state never goes into git;
  - agent prompts contain only ids and paths;
  - wait for completion notifications instead of polling;
  - only one writer of `board.json` at a time.
- **The backlog:** the format of `backlog.md` and how to change a task's
  status.
- **Write verification:** the exact commands for checking what an agent
  changed (see "Write verification").
- **Rounds:**
  - how a round's kind follows from its files;
  - how to find the latest round and the next one;
  - the `base-commit` file and the `reviews-started` marker;
  - how to count earlier REWORK verdicts;
  - the exact first line of each file that has one: reviews, verdicts, build
    notes and `preflight.md`. Any other first line is an error that halts the
    loop.
- **The preflight state:** how to recognise it from the files, and how to
  re-copy the spec from `spec-source`.
- **The reviewer roster:** the keys, and how to read a task's reviewers from
  its `Reviews` section, including the default when there is none.
- **`board.json`:** its format.
- **The factory script:** each of its commands, and when to use it (see
  "The factory script").
- **Shared steps:** the Merge and Halting steps.

### The factory script

`.claude/skills/factory/factory.py` is the factory script described under
"The factory script". It uses only the Python standard library, and works
out everything it reports or writes from the files in `factory/state/` and
from git. It must be robust to the files the agents and the human write:
- a byte order mark, CRLF line endings and trailing spaces;
- headings inside code fences, which are not headings;
- backticks around task-ids and reviewer keys;
- rounds numbered 10 and above;
- no task directories at all.

It reads `task.md` only by its fixed shape (see "The Floor Plan"): only the
`##` headings with the fixed names count as sections. In `## Reviews` and
`## Depends on`, only top-level bullets count, each by its leading key or
task-id. It treats a first line that isn't one of the allowed ones,
including an empty one, as an error. A rework note counts only if it has
text under its heading.

The spec copy:
- writes the absolute path of the spec to `spec-source`;
- follows relative links to Markdown files transitively, including
  reference-style links and links whose text wraps across lines;
- ignores links in code: inline code, fenced code and indented code
  blocks;
- keeps anchors;
- rewrites every link to the spec itself as a link to `SPEC.md`, so the spec
  is never copied twice;
- gives copies with the same file name distinct names;
- replaces `SPEC.md` and `spec/` when it runs again;
- lists every linked document that is missing.

Before committing the script, run every command against a sample
repository, never the real one. Cover:
- tasks in every stage, and the preflight state with no tasks;
- a `task.md` whose spec excerpts quote headings, and a `Reviews` section
  with explanations in its bullets;
- a spec whose linked documents link to each other, back to the spec and to
  a missing file, with a link that wraps across lines and one in an
  indented code block.

### The task orchestrator

`.claude/agents/task-orchestrator.md` defines the subagent that runs one
task. The main orchestrator spawns a fresh one for each task. Its first
instruction is to read `conventions.md`. It then covers:
- the first-line check of the task's files on entry, then a table that works
  out where to start from those files, so that new, reworked and
  interrupted tasks all enter the same way;
- the spawn prompts for the Builder, the reviewers and the Approver, and
  builder reuse (see "Builder reuse");
- write verification after each of those agents, and the `reviews-started`
  marker;
- the Build, Review, Approve, Approver rework and Escalate steps.

### The role agents

`.claude/agents/` holds one subagent definition per role:
- `planner.md`, `builder.md` and `approver.md`;
- one `<key>-reviewer.md` for each row of the reviewer roster.

Each definition uses the format under "Example files", and the model and
effort given in its role description. It says what the agent reads, what it
does, and which files it may write, and it repeats the safety rules for its
role (see "Safety rules"). For agents that can work outside the repo, it
also gives the scratch-directory rule (see "Rules").

The content comes from the role's description under "The Floor Plan", plus
the sections that role depends on:
- **The Planner:** "Identifiers", "Preflight", "Checking the plan", "The
  stack profile" and "The reviewer roster".
- **The Builder:** "The stack profile", "The reviewer roster" and
  "Specialist reviewers", for its self-check.
- **The reviewers:** "The stack profile" (for no-update mode and
  **Concurrency**), "The reviewer roster", "Specialist reviewers", "Review
  findings" and "Later rounds".
- **The Approver:** "Review findings", for the hard and soft categories.

### The dashboard

`factory/dashboard/index.html` is the static page described under "The
dashboard", with optional `style.css` and `app.js` files next to it.

### Git ignore entries

Add the entries `/factory/state` and `/factory/archive` to `.gitignore`, with
no trailing slash, and ignore nothing else under `factory/`. So the
dashboard, `factory.md` and `floorplan.md` are tracked, and the state of a
run never is. When a run is finished, the human moves `factory/state/` to a
folder under `factory/archive/`.
