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
Refuse if `factory/state/backlog.md`, `factory/state/tasks/` or
`factory/state/preflight.md` already exist (tell the user to archive or remove
`factory/state/` first; the factory never deletes a previous run). Write the
path of <spec> to `factory/state/spec-source`. Copy <spec> to
`factory/state/SPEC.md`. Also copy every Markdown file that <spec> links to
with a relative link (following links transitively) into
`factory/state/spec/`, and rewrite those links so they resolve from the
copies. Then spawn the Planner. It runs the preflight first (see
"Preflight"). When the Planner is done:
- If it wrote `backlog.md`, the orchestrator builds `board.json`, and halts
  with a summary of the backlog, the preflight assumptions, and the stack
  profile (including anything under **Not available**), so the human can
  review (and optionally edit) the plan. Building starts with
  `/factory continue`.
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
would take next. In the preflight state it prints the preflight result and
its open issues and offered tools instead of the backlog.

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

- **The Planner** first runs the preflight (see "Preflight"), and stops there
  unless the result is `PREFLIGHT: READY`. It then divides
  `factory/state/SPEC.md` (with its linked documents in
  `factory/state/spec/`) into a suitable number of tasks that together
  implement the work described in them. Each task becomes a directory
  `factory/state/tasks/<task-id>/`. In each task directory the Planner writes
  a `task.md` file containing everything from the spec that a builder needs
  to implement the task. Each `task.md` has these sections:
  - a title;
  - a goal;
  - the relevant spec excerpts;
  - acceptance criteria;
  - a `UI` section (or `UI: none` when the task has no UI changes);
  - a `Reviews` section listing the keys of the reviewers that run on the task
    (see "The reviewer roster"), with one line for each specialist saying why
    it applies. A specialist applies only when the task touches its concern
    as the baseline in `preflight.md` defines it. `code` and `feature` are
    always listed, and `ui` is listed
    exactly when the `UI` section isn't `UI: none`. For each specialist it
    lists, the Planner writes the spec requirements that reviewer will check
    into the acceptance criteria, so the Builder knows them before it starts;
  - the ids of the tasks it depends on.

  When all tasks are written, the Planner writes `factory/state/backlog.md`,
  the execution plan. It contains a Markdown table with one row per task and
  two cells per row: the task-id and the task's status (`Planned` for every
  task at this point). The tasks must be in implementation order from top to
  bottom, so that earlier tasks don't depend on later ones.

  The Planner can assume the stack's base toolchain is installed (such as the
  compiler or runtime and the package manager), but not that the project
  exists or that any other tool is installed. If the project doesn't exist,
  the first task scaffolds it. The project must let the UI Reviewer render its
  views from outside the repo, as `stack.md` describes. For example, a Rust
  TUI exposes a library target with renderable widgets that a scratch crate
  can depend on, and a web app has a build the reviewer can serve and open in
  a headless browser.

  The Planner works on the main branch and writes only `preflight.md`,
  `stack.md`, `backlog.md` and the `task.md` files, and it doesn't commit. The Planner runs
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
  done until every command in the test gate in `factory/state/stack.md`
  passes.

  UI changes must come with snapshot tests of the rendered UI, using the
  tooling under **UI tests** in `stack.md`. Snapshots must be accepted before
  the Builder commits, and it never commits pending snapshot files.

  Before it reports `STATUS: DONE`, the Builder self-checks its work against
  `task.md`, the way the reviewers will:
  - every acceptance criterion is met and covered by a test that would fail
    if the behaviour were wrong;
  - every UI state and size in the `UI` section has a snapshot;
  - for each specialist reviewer in the task's `Reviews` section, the work
    holds up against that reviewer's focus in the roster;
  - for each reviewer the task lists, it has run the commands that `stack.md`
    gives for that key under **Reviewer checks**, and the build notes address
    each result or say why it doesn't apply;
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
  acceptance criteria in `factory/state/tasks/<task-id>/task.md`. It produces
  a review with all its findings, and marks it PASS or FAIL (see "Review
  findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/feature-review.md`. Apart from
  evidence in `feature/` next to it, this is the only file the Feature
  Reviewer may create or change. The Feature
  Reviewer runs claude-opus-5-5 with High effort.

- **The UI Reviewer** reviews the changes on the task branch to determine
  whether the implementation matches the UI specified in
  `factory/state/tasks/<task-id>/task.md`. It inspects the Builder's
  snapshots. Any tests it runs in the repo run in the no-update mode from
  `stack.md`. It also renders views itself, as **Rendering** in `stack.md`
  describes:
  - It creates a throwaway project outside the repo, in its own scratch
    directory (see "Rules"). The project depends on the repo's project, and
    its build outputs stay in the scratch directory.
  - It renders the views at fixed sizes.
  - It never adds code to the repo.

  It produces a review with all its findings, and marks it PASS or FAIL (see
  "Review findings"). The review is written to
  `factory/state/tasks/<task-id>/<round-id>/ui-review.md`. Any text snapshots
  that document issues go in `factory/state/tasks/<task-id>/<round-id>/ui/`.
  These are the only files the UI Reviewer may create or change. It runs only
  on tasks that list `ui` in their `Reviews` section. The UI Reviewer runs
  claude-opus-5-5 with High effort.

- **Specialist reviewers** (every roster row except `code` and `feature`: the
  UI Reviewer above, plus security, privacy, API and any rows added later)
  each review one concern, on the tasks that list them. See "The reviewer
  roster".

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
  3. If at least one review is FAIL, and the Approver can write a single
     suitable rework note, and rework rounds remain, REWORK.
  4. Otherwise ESCALATE.

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
  there.
- **Assumptions:** smaller gaps, with the assumption the plan makes for each.
  It always includes the baseline for each specialist reviewer the plan is
  likely to use (see "Specialist reviewers"), taken from the spec where the
  spec states it.
- **Tools on offer:** each recommended tool that isn't installed, what it
  would check, the reviewer or gate section it serves, and the install
  command.
- **Stack profile changes:** what changed from the previous cycle's profile,
  if there was one.

A spec or environment issue always blocks. A tool is on offer only once: the
halt shows it whether the result is `TOOLS` or `BLOCKED`. When the Planner
finds a `preflight.md` from an earlier run, the human has had the offer for
every tool under its **Tools on offer**. The Planner marks each of those
tools that is still missing as `declined` in `stack.md`. A declined tool is
never offered again, in this cycle or a later one (see "The stack profile").
So a preflight with no major issues and only declined tools left gives
`READY`, and `/factory continue` moves on to planning.

### The stack profile

`factory/state/stack.md` is the only place that names the project's tools.
Everything else in this prompt refers to it, so the factory works the same on
any stack. The Planner writes it during the preflight. If an earlier cycle's
profile exists (the most recently modified `factory/archive/*/stack.md`), the
Planner starts from it: it keeps its choices, the human's edits and the
declined tools, re-checks every tool, and changes only what the project or the
spec now needs. Otherwise it writes the profile from `SPEC.md` and, when the
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

A task's `Reviews` section says which reviewers run on it. A `task.md`
without one runs `code`, `feature`, and `ui` unless its `UI` section is
`UI: none`.

Every reviewer follows the same contract:
- It reviews the changes on the task branch (`git diff main...HEAD`) within
  its focus, against `task.md`, `SPEC.md` and the documents in
  `factory/state/spec/`.
- Any tests it runs in the repo run in the no-update mode from `stack.md`. It
  also runs the tools that `stack.md` lists for its key under **Reviewer
  checks**. Any work outside the repo happens in its own scratch directory
  (see "Rules").
- It writes its review to
  `factory/state/tasks/<task-id>/<round-id>/<key>-review.md`, and any
  evidence, such as rendered views or tool output, to `<key>/` in the same
  directory. These are the only files it may create or change.
- It labels its findings as described in "Review findings". Anything it
  notices outside its focus is a nit, except that every reviewer raises a
  security or data-loss risk as must-fix.

Specialist reviewers (every row except `code` and `feature`) run
claude-opus-5-5 with High effort. To add a review concern, add a row here, a
description under "Specialist reviewers", and its agent file.

### Specialist reviewers

Each specialist judges its concern against a baseline: the scope of that
concern for this project. The baseline comes from the spec, or else from
**Assumptions** in `preflight.md`. The specialist states the baseline at the
top of its review. A finding that matters only under a wider baseline is a
nit. Each description says what the reviewer reads beyond the diff and what
is must-fix in its concern. Anything else in the concern is a nit.

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
   stays must-fix.
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

- Start a task: on main, mark it `In progress`. Create `round-1/` and spawn a 
  builder named `builder-<task-id>`, giving it the task-id and round-id.

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
  `build-notes.md`, `base-commit`, `human-review.md`, `approver-review.md`,
  and, for each reviewer the task lists, `<key>-review.md` and `<key>/**`, is
  a violation.

For the Approver, anything other than `verdict.md` is a violation. For the
Planner, anything other than `preflight.md`, `stack.md`, `backlog.md` and
`tasks/*/task.md` is a violation. On a violation, halt (loop state `halted`, 
with a reason) without reverting anything, and tell the user what changed.

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
- Each round shows as a row of reviewer chips, one for each key in the
  round's `reviews` in roster order, plus a verdict chip. The chips fill in as
  results land.
- ESCALATE and BLOCKED tasks stand out.
- A header shows the loop state and its message.
- Clicking a card opens a resizable panel that lists the task's Markdown files (from
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

- `loop.state`: `planning`, `awaiting-preflight` (the preflight stopped; when
  rebuilding, this is the state whenever `preflight.md` exists and
  `backlog.md` doesn't), `awaiting-plan-review`, `running`, `escalated`,
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
- `reviews`: one key for each reviewer in the task's `Reviews` section, in
  roster order (see "The reviewer roster").
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
  `preflight.md` starts with `PREFLIGHT: READY`, `PREFLIGHT: TOOLS` or
  `PREFLIGHT: BLOCKED`.
- The builder commits and works on a feature branch (`factory/<task-id>`) and
  tests against the acceptance criteria in
  `factory/state/tasks/<task-id>/task.md`.
- The factory never pushes changes to an origin. Only the human user pushes.
- Agents never share scratch space. The session scratchpad and `$TMPDIR` are
  shared by agents running at the same time, so the definition of every agent
  that can work outside the repo (Builder, reviewers, Approver) tells it to:
  create its own directory with `mktemp -d`, named
  `<role>-<task-id>-<round-id>.XXXXXX`, before any such work; keep all exports,
  copies, probes and backups there, with build outputs sent to a folder inside
  it (see **Scratch builds** in `stack.md`); never use, overwrite or delete a 
  path it didn't create in this run; and delete its directory when it finishes.
- Wait for all reviews: a review round is done only when the review file of
  every reviewer the task lists exists in the round directory. Never resume
  the builder mid-round.
- Spawn the builder with a name (`builder-<task-id>`) and resume that same
  builder for each new round when it is still available (see "Builder
  reuse").
- The orchestrator creates a new round directory at the beginning of each
  round. Before spawning any agent for the round, it writes the round's base
  commit to `base-commit` in that directory. For round 1 that's the commit
  `main` points at. For later rounds it's the `HEAD` of the task branch.
- Only one task is ever `In progress` or `Escalated` at a time.

## Building the factory

The factory is the set of files below. "Factory file system" shows where they
sit. Build them once from this document. The orchestrator never creates, 
regenerates or edits them at runtime. Commit them before the first `/factory start`, 
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
- rebuilding `board.json` from `backlog.md` and the task files when it is
  missing;
- choosing the next task, and spawning and acting on the task orchestrator
  (see "Waiting and stopping").

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
  - the `base-commit` file;
  - how to count earlier REWORK verdicts;
  - the exact first line of each file that has one: reviews, verdicts, build
    notes and `preflight.md`. Any other first line is an error that halts the
    loop.
- **The preflight state:** how to recognise it from the files, and how to
  re-copy the spec from `spec-source`.
- **The reviewer roster:** the keys, and how to read a task's reviewers from
  its `Reviews` section, including the default when there is none.
- **`board.json`:** its format and how to update it (see "Updating
  board.json").
- **Shared steps:** the Merge and Halting steps.

### The task orchestrator

`.claude/agents/task-orchestrator.md` defines the subagent that runs one
task. The main orchestrator spawns a fresh one for each task. Its first
instruction is to read `conventions.md`. It then covers:
- a table that works out where to start from the task's files, so that new,
  reworked and interrupted tasks all enter the same way;
- the spawn prompts for the Builder, the reviewers and the Approver, and
  builder reuse (see "Builder reuse");
- write verification after each of those agents;
- the Build, Review, Approve, Approver rework and Escalate steps.

### The role agents

`.claude/agents/` holds one subagent definition per role:
- `planner.md`, `builder.md` and `approver.md`;
- one `<key>-reviewer.md` for each row of the reviewer roster.

Each definition uses the format under "Example files", and the model and
effort given in its role description. It says what the agent reads, what it
does, and which files it may write. For agents that can work outside the
repo, it also gives the scratch-directory rule (see "Rules").

The content comes from the role's description under "The Floor Plan", plus
the sections that role depends on:
- **The Planner:** "Preflight", "The stack profile" and "The reviewer
  roster".
- **The reviewers:** "The reviewer roster", "Specialist reviewers", "Review
  findings" and "Later rounds".

### The dashboard

`factory/dashboard/index.html` is the static page described under "The
dashboard", with optional `style.css` and `app.js` files next to it.

### Git ignore entries

Add the entries `/factory/state` and `/factory/archive` to `.gitignore`, with
no trailing slash, and ignore nothing else under `factory/`. So the
dashboard, `factory.md` and `floorplan.md` are tracked, and the state of a
run never is. When a run is finished, the human moves `factory/state/` to a
folder under `factory/archive/`.
