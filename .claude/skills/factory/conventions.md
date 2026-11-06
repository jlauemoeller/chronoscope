# Factory conventions

Shared by both orchestrator levels: the **main orchestrator** (the session
running `/factory`, see `SKILL.md`) and the **task orchestrator** (the
`task-orchestrator` subagent). Read this whole file before doing anything
else. Run every command from the repository root.

## Names and paths

| Name | Path |
|---|---|
| state | `factory/state/` (git-ignored; one copy, the same on every branch) |
| board | `factory/state/board.json` |
| backlog | `factory/state/backlog.md` |
| spec copy | `factory/state/SPEC.md` |
| spec source | `factory/state/spec-source` (path of the original spec, one line) |
| linked docs | `factory/state/spec/` |
| preflight | `factory/state/preflight.md` |
| stack profile | `factory/state/stack.md` |
| Planner snapshot | `factory/state/.snapshot` |
| archive | `factory/archive/<run>/` (finished runs, moved there by hand) |
| `T` | `factory/state/tasks/<task-id>/`, the task directory |
| task spec | `T/task.md` |
| task snapshot | `T/.snapshot` |
| `R` | `T/<round-id>/`, the round directory |
| round files | `R/base-commit`, `R/build-notes.md`, `R/<key>-review.md`, `R/<key>/` (evidence), `R/verdict.md`, `R/approver-review.md`, `R/human-review.md` |

Identifiers:
- **task-id:** three-digit zero-padded serial from `001`, a dash, and a
  two-to-three word dashed fingerprint, e.g. `002-apply-selected-theme`.
- **round-id:** `round-<n>`, `n` counting up from 1, e.g. `round-4`.
- **branch:** `factory/<task-id>`.

Agent types (the `subagent_type` to spawn; definitions in `.claude/agents/`):

| Agent | Spawned by | Model, effort |
|---|---|---|
| `planner` | main orchestrator | claude-opus-5-5, xhigh |
| `task-orchestrator` | main orchestrator | claude-opus-5-5, high |
| `builder` (named `builder-<task-id>`) | task orchestrator | claude-sonnet-5-5, high |
| `code-reviewer`, `feature-reviewer`, `security-reviewer`, `privacy-reviewer`, `api-reviewer`, `ui-reviewer` | task orchestrator | claude-opus-5-5, high |
| `approver` | task orchestrator | claude-opus-5-5, xhigh |

The model and effort come from each agent's definition; don't override them
when spawning.

## Ground rules

- **Git safety.** Never push to any remote; only the human pushes. Never
  rewrite history. These are never allowed, for any agent or orchestrator:
  `git push` (in any form), `git reset --hard` or any reset that moves a
  branch, `git rebase`, `git commit --amend`, `git cherry-pick`,
  `git revert`, `git merge --abort`, `git clean`, `git checkout -- <path>`,
  `git restore`, `git stash` (any subcommand), `git branch -D`,
  `git branch -f`, `git tag -d`, `git update-ref`, `git filter-branch`,
  `git filter-repo`, `git reflog expire`, `git gc --prune`, and `git add -f`.
  Delete a merged task branch only with `git branch -d`; if it refuses,
  halt instead of forcing.
- **One active task.** Only one task is ever `In progress` or `Escalated` at
  a time, and the main orchestrator never runs two task orchestrators at once.
- **The state never goes into git.** Nothing under `factory/state/` is ever
  staged or committed. Never delete a previous run's state; the human
  archives it.
- **The factory's own files are fixed.** Never create, regenerate or edit
  `.claude/skills/factory/*`, `.claude/agents/*`, `factory/factory.md`,
  `factory/floorplan.md` or `factory/dashboard/*` at runtime.
- **Never install anything.** Tools are installed by the human.
- **Agent prompts contain only ids and paths:** the task-id, the round-id,
  and the paths to read. All other context comes from files. Use the exact
  prompts given in `SKILL.md` and `task-orchestrator.md`.
- **Wait, don't poll.** Spawned agents run in the background. Wait for them
  by ending your turn with the single word `WAITING`; you are woken when one
  reports. Never sleep, poll, loop on a file, read an agent's output file, or
  send an agent a message to ask how it is doing.
- **One writer of `board.json`.** The task orchestrator writes it while it
  runs; the main orchestrator writes it otherwise (see "board.json").
- **Never do an agent's work.** The orchestrators don't write code, reviews,
  verdicts, build notes, task files or the stack profile, and don't fix an
  agent's output.

## The backlog

`backlog.md` is the execution plan, written by the Planner. It holds one
Markdown table with one row per task and two cells per row, in
implementation order:

```markdown
# Backlog

| Task | Status |
|---|---|
| 001-scaffold-project | Complete |
| 002-apply-selected-theme | In progress |
| 003-save-settings | Planned |
```

Statuses: `Planned`, `In progress`, `Escalated`, `Complete`, `Abandoned`.
List the rows with:

```bash
sed -n 's/^| *\([0-9]\{3\}-[a-z0-9-]*\) *| *\([A-Za-z ]*[a-z]\) *|.*/\1 \2/p' factory/state/backlog.md
```

To change a task's status, use the Edit tool to replace that task's row
`| <task-id> | <old status> |` with `| <task-id> | <new status> |`, and
change nothing else. Then refresh the task's board entry (see
"board.json"). The transitions, and who makes them:

| From | To | Who, when |
|---|---|---|
| `Planned` | `In progress` | main: starting the task |
| `In progress` | `Escalated` | task orchestrator: Escalate step |
| `In progress` | `Complete` | main: after the merge |
| `Escalated` | `In progress` | main: `/factory rework`, or `/factory approve` hitting a merge conflict |
| `Escalated` | `Complete` | main: `/factory approve` |
| `Escalated` | `Abandoned` | main: `/factory abandon` |

The *next available task* is the first row with status `Planned`.

## Write verification

After each agent finishes, check what it changed. Both checks run from the
repository root.

**Outside the state.** `git status --porcelain` must print nothing (changes
to ignored files such as build outputs don't show and don't count). Also
note the branch (`git branch --show-current`) and `HEAD`
(`git rev-parse HEAD`) before spawning:
- The Builder must leave a clean tree on `factory/<task-id>`, and nothing
  under `factory/state/` may be in its commits:
  `git diff --name-only "$(cat R/base-commit)"..HEAD -- factory/state` must
  print nothing.
- Every other agent (Planner, reviewers, Approver) changes nothing in the
  repo: clean tree, same branch, same `HEAD`.

**Inside the state.** `git status` can't see the state, so compare
checksums. Just before spawning an agent, write the listing to its snapshot
file (`SNAP` is `T/.snapshot` for task agents, `factory/state/.snapshot` for
the Planner):

```bash
find factory/state -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | LC_ALL=C sort -z | xargs -0 -r cksum > SNAP
```

After it finishes, print the paths it added, removed or changed:

```bash
find factory/state -type f ! -name 'board.json*' ! -name '.snapshot' -print0 | LC_ALL=C sort -z | xargs -0 -r cksum | diff SNAP - | sed -n 's/^[<>] [0-9]* [0-9]* //p' | LC_ALL=C sort -u
```

Allowed changes (every printed path must be one of these):

| Agent | May change |
|---|---|
| Planner | `factory/state/preflight.md`, `factory/state/stack.md`, `factory/state/backlog.md`, `factory/state/tasks/*/task.md` |
| Builder | `R/build-notes.md` |
| each reviewer, as it finishes | only paths inside `R/` |
| all reviewers, once all are done | inside `R/`, only `<key>-review.md` and `<key>/**` for the keys the task lists; and the files in `R/` (`find R -type f`) are only `build-notes.md`, `base-commit`, `human-review.md`, `approver-review.md`, and those reviewer files |
| Approver | `R/verdict.md` |

Reviewers run in parallel from one snapshot, so the per-reviewer check only
flags paths outside `R/`; the full check runs when the last one is done.

On a violation, use the Halting step with a reason that names the agent
and lists what changed. Revert nothing.

## Rounds

- **Latest round** of a task (empty if none):
  ```bash
  ls -d factory/state/tasks/<task-id>/round-* 2>/dev/null | sed 's/.*round-//' | sort -n | tail -n1
  ```
  The next round is that number plus 1, or `round-1` if there is none.
  Always compare round numbers numerically (`round-10` comes after
  `round-9`).
- **Kind** follows from the round's files: `human-review.md` present means
  `rework` (started by `/factory rework`); `approver-review.md` present means
  `approver-rework` (started by a REWORK verdict); otherwise `auto` (round 1
  or the automatic fix round).
- **`base-commit`.** Each round starts by creating its directory and,
  before any agent is spawned for it, writing its base commit there as one
  full hash: for `round-1` the commit `main` points at, for later rounds the
  `HEAD` of the task branch.
  ```bash
  mkdir R && git rev-parse main > R/base-commit       # round-1
  mkdir R && git rev-parse HEAD > R/base-commit       # later rounds, on factory/<task-id>
  ```
- **Round limits.** At most two automatic rounds per task: `round-1` and one
  fix round, which happens only when `round-1` has a FAIL review. Up to two
  Approver rework rounds come from REWORK verdicts. Further rounds come only
  from `/factory rework`, without limit. Neither kind of rework round ever
  triggers an automatic fix round: after its reviews, the Approver runs.
- **Earlier REWORK verdicts** of a task, before round `N`:
  ```bash
  for d in factory/state/tasks/<task-id>/round-*; do n=${d##*round-}; [ "$n" -lt N ] && [ -f "$d/verdict.md" ] && head -n1 "$d/verdict.md"; done | grep -cx 'VERDICT: REWORK'
  ```
  A REWORK verdict in round `N` is honoured only if this prints `0` or `1`.
  Human rework rounds don't reset the count.
- **First lines.** These files must start with exactly one of these lines
  (compare `head -n1 <file> | tr -d '\r'`, ignoring trailing spaces):

  | File | First line |
  |---|---|
  | `R/<key>-review.md` | `VERDICT: PASS` or `VERDICT: FAIL` |
  | `R/verdict.md` | `VERDICT: APPROVE`, `VERDICT: REWORK` or `VERDICT: ESCALATE` |
  | `R/build-notes.md` | `STATUS: DONE` or `STATUS: BLOCKED` |
  | `preflight.md` | `PREFLIGHT: READY`, `PREFLIGHT: TOOLS` or `PREFLIGHT: BLOCKED` |

  Any other first line, or a missing file after its agent reports, is an
  error: use the Halting step.

## The preflight state

The run is in the **preflight state** when `factory/state/preflight.md`
exists and `factory/state/backlog.md` doesn't. In it, `loop.state` is
`awaiting-preflight` and there are no tasks.

**Copying the spec** (used by `/factory start`, and by `/factory continue`
in the preflight state to re-copy it). It reads the path in `spec-source`,
copies that file to `SPEC.md`, replaces `spec/` with a fresh copy of every
Markdown file the spec links to with a relative link (following links
transitively, each document copied once, flat, renamed `<n>-<name>.md` on a
name clash), and rewrites those links so they resolve from the copies
(`spec/<name>.md` from `SPEC.md`, `<name>.md` between copies, `../SPEC.md`
back to the spec). A linked document that is missing is left as a broken
link and printed as `MISSING`: don't stop for it, because the Planner
reports it as a spec issue. Run exactly:

```bash
python3 -I -c '
import os,re,shutil
S="factory/state";src=os.path.abspath(open(S+"/spec-source").read().strip())
LK=re.compile(r"(\]\(\s*<?|^ {0,3}\[[^\]]+\]:\s*<?)([^\s<>()]+)",re.M)
def rel(t):return t.split("#")[0].lower().endswith(".md") and not re.match(r"[a-zA-Z][\w+.-]*:|[/#]",t)
def tgt(p,t):return os.path.normpath(os.path.join(os.path.dirname(p),t.split("#")[0]))
names={};todo=[src];txt={};miss=[]
while todo:
 p=todo.pop(0);txt[p]=open(p,encoding="utf-8").read()
 for m in LK.finditer(txt[p]):
  t=m.group(2)
  if not rel(t):continue
  q=tgt(p,t)
  if q==src or q in names:continue
  if not os.path.isfile(q):miss.append(t+" (linked from "+p+")");continue
  n=os.path.basename(q);i=2
  while n in names.values():n=str(i)+"-"+os.path.basename(q);i+=1
  names[q]=n;todo.append(q)
def fix(p,sub):
 def f(m):
  t=m.group(2)
  if not rel(t):return m.group(0)
  q=tgt(p,t);fr=t[len(t.split("#")[0]):]
  new=("../SPEC.md" if sub else "SPEC.md") if q==src else ((names[q] if sub else "spec/"+names[q]) if q in names else None)
  return m.group(1)+new+fr if new else m.group(0)
 return LK.sub(f,txt[p])
shutil.rmtree(S+"/spec",ignore_errors=True)
open(S+"/SPEC.md","w",encoding="utf-8").write(fix(src,False))
if names:os.makedirs(S+"/spec")
for q,n in names.items():open(S+"/spec/"+n,"w",encoding="utf-8").write(fix(q,True))
print("copied SPEC.md and",len(names),"linked documents")
for x in miss:print("MISSING",x)'
```

## The reviewer roster

Keys, in roster order (the order of the dashboard chips and, after security
and data protection, the Approver's priority order):

| Key | Agent | Runs on |
|---|---|---|
| `code` | `code-reviewer` | every task |
| `feature` | `feature-reviewer` | every task |
| `security` | `security-reviewer` | tasks that list it |
| `privacy` | `privacy-reviewer` | tasks that list it |
| `api` | `api-reviewer` | tasks that list it |
| `ui` | `ui-reviewer` | tasks that list it |

Each reviewer writes `R/<key>-review.md` and may put evidence in `R/<key>/`.

A task's reviewers are the keys listed as bullet items in the `Reviews`
section of its `task.md`, plus `code` and `feature` always. A `task.md`
with no `Reviews` section runs `code`, `feature`, and `ui` unless its `UI`
section is `UI: none`. Print a task's keys, in roster order, with:

```bash
python3 -I -c '
import re,sys
K=["code","feature","security","privacy","api","ui"];tm=open(sys.argv[1],encoding="utf-8").read()
sec=re.search(r"(?ms)^#+\s*Reviews\b.*?(?=^#|\Z)",tm)
ks=set(re.findall(r"(?m)^\s*[-*]\s+[`*_]*([a-z]+)",sec.group(0))) if sec else (set() if re.search(r"(?mi)^\W*UI:\s*none\W*$",tm) else {"ui"})
print(" ".join(k for k in K if k in ks|{"code","feature"}))' factory/state/tasks/<task-id>/task.md
```

## board.json

`factory/state/board.json` reflects the loop for the dashboard. It isn't
tracked by git and can always be rebuilt from `backlog.md` and the task
files. Format:

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
  `running`, `escalated`, `halted` (the reason goes in `message`), or
  `complete`. `loop.current` is the active task-id or `null`.
- `agents`: builder agent ids keyed by name (`builder-<task-id>`). An id is
  usable only by the task orchestrator that spawned it; a later one spawns a
  fresh builder and replaces the id.
- `tasks`: one entry per backlog row, in backlog order. `status` is the
  backlog status. `stage` is `planned`, `building`, `review`, `approval`,
  `escalated`, `done` or `abandoned`. Each round has `id`, `kind` (`auto`,
  `approver-rework`, `rework`), `build` (`pending`, `DONE`, `BLOCKED`),
  `reviews` (one key per listed reviewer in roster order, each `pending`,
  `PASS` or `FAIL`; empty until the build is `DONE`) and `verdict` (`null`,
  `APPROVE`, `REWORK`, `ESCALATE`). `files` lists the task's Markdown files,
  relative to `T`.

**Never print the file, never rewrite it with the Write tool, and never leave
it half-edited.** Update it every time you take back control from an agent
and on every stage change, with the one-shot command below. It loads the
JSON, sets `updated`, the given `loop` fields and `agents` entries, and
re-derives the given task's whole entry (status from `backlog.md`; title,
rounds, chips, stage and files from the task's files), then writes
`board.json.tmp` and moves it into place with `os.replace`. It changes
nothing else. Arguments, in order: the task-id to refresh (or `-`), the
`loop.state` (or `-` to keep), the `loop.message` (or `-`), `loop.current`
(a task-id, `none` for `null`, or `-`), then any number of
`builder-<task-id>=<agent-id>` pairs (an empty id removes the entry).

```bash
python3 -I -c '
import json,os,re,sys,datetime as D
S="factory/state";P=S+"/board.json";tid,st,msg,cur=sys.argv[1:5]
b=json.load(open(P));L=b.setdefault("loop",{});A=b.setdefault("agents",{});X=b.setdefault("tasks",[])
b["updated"]=D.datetime.now(D.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
if st!="-":L["state"]=st
if msg!="-":L["message"]=msg
if cur!="-":L["current"]=None if cur=="none" else cur
for a in sys.argv[5:]:
 k,v=a.split("=",1)
 if v:A[k]=v
 else:A.pop(k,None)
if tid!="-":
 T=S+"/tasks/"+tid;K=["code","feature","security","privacy","api","ui"]
 hd=lambda f:open(f,encoding="utf-8").readline().strip() if os.path.isfile(f) else ""
 m=re.search(r"(?m)^\|\s*"+re.escape(tid)+r"\s*\|\s*([^|]*?)\s*\|",open(S+"/backlog.md",encoding="utf-8").read())
 status=m.group(1) if m else "Planned";tm=open(T+"/task.md",encoding="utf-8").read()
 sec=re.search(r"(?ms)^#+\s*Reviews\b.*?(?=^#|\Z)",tm)
 ks=set(re.findall(r"(?m)^\s*[-*]\s+[`*_]*([a-z]+)",sec.group(0))) if sec else (set() if re.search(r"(?mi)^\W*UI:\s*none\W*$",tm) else {"ui"})
 ks=[k for k in K if k in ks|{"code","feature"}];rounds=[];files=["task.md"]
 for r in sorted((d for d in os.listdir(T) if re.fullmatch(r"round-\d+",d)),key=lambda d:int(d[6:])):
  d=T+"/"+r;bn=hd(d+"/build-notes.md")[8:] or "pending"
  kind="rework" if os.path.exists(d+"/human-review.md") else "approver-rework" if os.path.exists(d+"/approver-review.md") else "auto"
  rv={k:hd(d+"/"+k+"-review.md")[9:] or "pending" for k in ks} if bn=="DONE" else {}
  rounds.append({"id":r,"kind":kind,"build":bn,"reviews":rv,"verdict":hd(d+"/verdict.md")[9:] or None})
  files+=sorted(os.path.relpath(os.path.join(w,f),T) for w,_,fs in os.walk(d) for f in fs if f.endswith(".md"))
 z=rounds[-1] if rounds else None;rs=z["reviews"].values() if z else []
 if status!="In progress":stage={"Escalated":"escalated","Complete":"done","Abandoned":"abandoned"}.get(status,"planned")
 elif not z or z["build"]=="pending":stage="building"
 elif z["build"]=="BLOCKED" or z["verdict"]=="ESCALATE":stage="escalated"
 elif z["verdict"] or (rs and "pending" not in rs and not (z["id"]=="round-1" and "FAIL" in rs)):stage="approval"
 else:stage="review"
 e=next((t for t in X if t.get("id")==tid),None)
 if e is None:e={"id":tid};X.append(e)
 tl=re.search(r"(?m)^#\s+(.+?)\s*$",tm);tl=re.sub("^"+re.escape(tid)+r"\W*","",tl.group(1)) if tl else tid
 e.update(title=tl,branch="factory/"+tid,status=status,stage=stage,rounds=rounds,files=files)
with open(P+".tmp","w") as f:json.dump(b,f,indent=1)
os.replace(P+".tmp",P)' <task-id|-> <state|-> "<message|->" <current|none|-> [builder-<task-id>=<agent-id> ...]
```

Examples:
- after a reviewer finishes: `... 002-apply-selected-theme - "Reviewing 002-apply-selected-theme round-1" -`
- after spawning a builder: `... 002-apply-selected-theme - "Building 002-apply-selected-theme round-1" - builder-002-apply-selected-theme=<agent-id>`

**Creating or rebuilding.** Only then is the Write tool used. Write
`board.json` with no tasks:

```json
{"updated": "<now, UTC, ISO 8601>", "loop": {"state": "<state>", "message": "<message>", "current": null}, "agents": {}, "tasks": []}
```

Then, unless the run is in the preflight state, fill in every task in
backlog order by running the update command for each one:

```bash
for id in $(sed -n 's/^| *\([0-9]\{3\}-[a-z0-9-]*\) *|.*/\1/p' factory/state/backlog.md); do python3 -I -c '<the update script above>' "$id" - - -; done
```

The `<state>` and `<message>` of a rebuild:
- the preflight state: `awaiting-preflight`, `Preflight: <first line of preflight.md>`;
- every task `Planned`: `awaiting-plan-review`, `Plan ready: review it, then run /factory continue`;
- a task `Escalated`: `escalated`, `Escalated <task-id>`, with `current` set to it;
- a task `In progress`: `halted`, `Rebuilt board; /factory continue resumes <task-id>`, with `current` set to it;
- otherwise (no `Planned` task left): `complete`, `Construction complete`.

Don't leave helper scripts behind: always run these as one-shot commands.

## Merged

A task is **merged** if `main`'s log has the subject `factory: merge <id>`,
or if its branch exists, is an ancestor of `main`, and has at least one
builder commit (subject starting with `<id>: `). A branch with no builder
commits is not merged, even though it is an ancestor of `main`. Exit status
0 means merged:

```bash
id=<task-id>; git log main --format=%s | grep -qxF "factory: merge $id" || { git rev-parse -q --verify "refs/heads/factory/$id" >/dev/null && git merge-base --is-ancestor "factory/$id" main && git log "factory/$id" --format=%s | grep -q "^$id: "; }
```

## Shared steps

### Merge

Used by the task orchestrator after an APPROVE verdict, and by the main
orchestrator for `/factory approve`.

1. `git status --porcelain` must print nothing. Otherwise use the Halting
   step: `Uncommitted changes; cannot merge <task-id>`.
2. `git switch main`
3. `git merge --no-ff -m "factory: merge <task-id>" factory/<task-id>`
4. If the merge fails with conflicts (`git diff --name-only --diff-filter=U`
   lists files): leave the merge in progress exactly as it is. Don't resolve
   it, abort it or commit it. Use the Halting step with the reason
   `Merge conflict merging <task-id> into main (<conflicted files>): resolve
   the conflicts, commit the merge, then run /factory continue`. If it fails
   for any other reason, halt with git's error.
5. On success you are on `main` with a clean tree. The task stays
   `In progress`; the main orchestrator marks it `Complete` and deletes the
   branch with `git branch -d factory/<task-id>`.

### Halting

Used for merge conflicts, write violations, bad first lines, missing
outputs and any other error that stops the loop.

1. If any agent you spawned is still running, spawn nothing new and end your
   turn with `WAITING` until every one has reported. Only then go on.
2. Revert nothing: leave the working tree, branches, `backlog.md` and every
   agent's output exactly as they are.
3. Set `loop.state` to `halted` and `loop.message` to the reason (one line;
   for a write violation, name the agent and list the changed paths), with
   the update command, refreshing the current task if there is one.
4. Stop. The task orchestrator ends with the one line `halted <task-id>`.
   The main orchestrator tells the user the reason, what (if anything) to fix,
   and that `/factory continue` resumes from the files.
