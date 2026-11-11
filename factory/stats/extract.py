"""Extract factory performance data from factory/archive for the stats board.

Reads every archived cycle (factory/archive/cycle-*) and the Claude Code
transcripts of the factory's sessions, and writes the data behind the stats
board (factory/stats/index.html):

  tasks.json    one row per task: rounds, REWORK verdicts, finding totals,
                agent time, tokens, and a text trail of its rounds
  rounds.json   one row per round: kind, Approver verdict, failing reviews,
                agent time and tokens
  reviews.json  one row per review: verdict, must-fix, nits, found-late nits,
                evidence files, agent time and tokens
  agents.json   one row per agent turn: role, task, round, agent time and
                tokens, plus one row per main (/factory) session
  files.json    the archived files of each task, keyed "<cycle>/<task-id>",
                for the board's file panel (which reads them from
                factory/archive)
  meta.json     when the data was generated, and from which cycles

Usage:
  python3 factory/stats/extract.py [out-dir] [--archive DIR] [--repo DIR]
                                   [--projects DIR]
  python3 -m http.server -d factory      # then open /stats/

The output defaults to factory/stats/data (not tracked), the archive to
factory/archive, the repo (for merge commits) to the git repository that
holds this script, and the transcripts to ~/.claude/projects. Run it again
after archiving a cycle.

How things are counted:
- Rounds, kinds, review states and verdicts come from each cycle's
  board.json. Each review's first line and each verdict.md must agree with
  it; any mismatch is printed and the exit status is 1.
- Findings are the [must-fix] and [nit] items under a review's
  "## Findings" heading only. Sections that re-check earlier findings
  ("## Previous findings" and the like) are not counted again. Every FAIL
  review must parse to at least one must-fix and every PASS review to none.
- Agent time and tokens are measured as chronoscope measures them (see
  usage.py), never as wall-clock time. A task's agent time is the time at
  least one of its builder, reviewers or Approver was working: their turns
  overlap, so they are merged, not summed ("effort" is the sum). The main
  session, task orchestrators and the planner count as orchestration of
  their cycle, not of a task.
- Agent turns are matched to archived tasks and rounds by their prompt
  ("Task-id: ... Round-id: ..."), within the cycle's span of merge commits
  on main. Every archived build, review and verdict must have its agent's
  transcript; any that is missing is a problem. The matched turns are
  stored in archive/cycle-<n>/usage.json and used once the transcripts are
  gone (Claude Code deletes sessions after cleanupPeriodDays, 30 by
  default).
"""
import argparse
import json
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path

import usage

TAG = re.compile(r'^\s*(?:\d+\.|-|\*)\s*\*{0,2}\[(must-fix|nit)\]\*{0,2}(.*)$')
HEADING = re.compile(r'^##\s+(.*?)\s*$')


def sections(text):
    """Yield (heading, lines) for each ## section, with '' for the preamble."""
    heading, lines = '', []
    for line in text.splitlines():
        m = HEADING.match(line)
        if m:
            yield heading, lines
            heading, lines = m.group(1), []
        else:
            lines.append(line)
    yield heading, lines


def first_line_verdict(text):
    first = text.splitlines()[0].strip() if text else ''
    return first.removeprefix('VERDICT:').strip() if first.startswith('VERDICT:') else None


def parse_review(path):
    text = path.read_text()
    must = nit = late = 0
    for heading, lines in sections(text):
        if heading.lower() != 'findings':
            continue
        for line in lines:
            m = TAG.match(line)
            if not m:
                continue
            if m.group(1) == 'must-fix':
                must += 1
            else:
                nit += 1
                late += 'found late' in m.group(2).lower()
    return dict(verdict=first_line_verdict(text), must=must, nit=nit, late=late)


def git(repo, *args):
    return subprocess.run(['git', '-C', str(repo), *args],
                          capture_output=True, text=True, check=True).stdout


def merges_on_main(repo):
    out = git(repo, 'log', '--first-parent', 'main', '--reverse', '--format=%H %aI %s')
    merges = []
    for line in out.splitlines():
        sha, when, subject = line.split(' ', 2)
        m = re.match(r'factory: merge (\S+)', subject)
        if m:
            merges.append((m.group(1), datetime.fromisoformat(when), sha))
    return merges


def branch_commits(repo, sha):
    try:
        return int(git(repo, 'rev-list', '--count', f'{sha}^1..{sha}^2').strip())
    except subprocess.CalledProcessError:
        return None


def read_tasks(archive, problems):
    cycles = sorted((p for p in archive.iterdir() if p.is_dir() and p.name.startswith('cycle-')),
                    key=lambda p: int(p.name.split('-')[1]))
    tasks = []
    for cdir in cycles:
        cycle = int(cdir.name.split('-')[1])
        board = json.loads((cdir / 'board.json').read_text())
        for t in board['tasks']:
            tdir = cdir / 'tasks' / t['id']
            rounds = []
            for n, r in enumerate(t['rounds'], 1):
                rdir = tdir / r['id']
                where = f'cycle {cycle} {t["id"]} {r["id"]}'
                reviews = {}
                for key, state in r['reviews'].items():
                    path = rdir / f'{key}-review.md'
                    if not path.exists():
                        problems.append(f'{where}: no {path.name}')
                        continue
                    rv = parse_review(path)
                    evidence = rdir / key
                    rv['evidence'] = sum(1 for p in evidence.rglob('*') if p.is_file()) if evidence.is_dir() else 0
                    if rv['verdict'] != state:
                        problems.append(f'{where} {key}: file says {rv["verdict"]}, board says {state}')
                    if (rv['verdict'] == 'FAIL') != (rv['must'] > 0):
                        problems.append(f'{where} {key}: {rv["verdict"]} with {rv["must"]} must-fix')
                    reviews[key] = rv
                vfile = rdir / 'verdict.md'
                verdict = first_line_verdict(vfile.read_text()) if vfile.exists() else None
                if verdict != r.get('verdict'):
                    problems.append(f'{where}: verdict.md says {verdict}, board says {r.get("verdict")}')
                rounds.append(dict(n=n, kind=r['kind'], verdict=r.get('verdict'), reviews=reviews))
            tasks.append(dict(cycle=cycle, id=t['id'], title=t.get('title') or '', rounds=rounds))
    return tasks


def add_merges(tasks, repo, problems):
    """Match archived tasks to their merges on main: the archived ids, in
    order, must be one unbroken run of merges (ids recur across cycles).
    Sets each task's merge time, first branch commit and commit count."""
    merges = merges_on_main(repo)
    ids = [t['id'] for t in tasks]
    found = []
    for start in range(len(merges) - len(ids) + 1):
        if [m[0] for m in merges[start:start + len(ids)]] == ids:
            found = merges[start:start + len(ids)]
            break
    if not found:
        problems.append('merges on main do not match the archived tasks')
    for t in tasks:
        t['commits'] = t['merged_at'] = t['started_at'] = None
    for t, (_, when, sha) in zip(tasks, found):
        t['merged_at'] = when
        t['commits'] = branch_commits(repo, sha)
        first = git(repo, 'log', '--format=%aI', f'{sha}^1..{sha}^2').split()
        t['started_at'] = datetime.fromisoformat(first[-1]) if first else None


WORK = {'builder', 'approver', *usage.ROLES.values()} - {'planner', 'task-orchestrator'}


def add_usage(tasks, runs):
    """Agent time (seconds) and tokens per task, round and review."""
    def figures(rs, merge=True):
        intervals = [i for r in rs for i in r['intervals']]
        tok = [r['tokens'] for r in rs]
        secs = usage.union_seconds(intervals) if merge else usage.seconds(intervals)
        return dict(secs=int(secs) if rs else None,
                    tokens=sum(usage.total(x) for x in tok) if rs else None,
                    cache_read=sum(x['cache_read'] for x in tok) if rs else None)
    index = {}
    for r in runs:
        index.setdefault((r['cycle'], r['task']), []).append(r)
    for t in tasks:
        own = index.get((t['cycle'], t['id']), [])
        work = [r for r in own if r['role'] in WORK]
        t['usage'] = figures(work)
        t['usage']['effort_secs'] = figures(work, merge=False)['secs']
        t['usage']['orchestration'] = figures([r for r in own if r['role'] == 'task-orchestrator'])
        for rd in t['rounds']:
            mine = [r for r in work if r['round'] == rd['n']]
            rd['usage'] = figures(mine)
            rd['usage']['build'] = figures([r for r in mine if r['role'] == 'builder'])
            rd['usage']['approver'] = figures([r for r in mine if r['role'] == 'approver'])
            for key, rv in rd['reviews'].items():
                rv['usage'] = figures([r for r in mine if r['role'] == key])


def rows(tasks):
    task_rows, round_rows, review_rows = [], [], []
    for t in tasks:
        rs = t['rounds']
        for r in rs:
            failing = [k for k, v in r['reviews'].items() if v['verdict'] == 'FAIL']
            round_rows.append(dict(
                cycle=t['cycle'], task=t['id'], round=r['n'], kind=r['kind'],
                verdict=r['verdict'] or 'none', failing_reviews=len(failing),
                failing=' '.join(failing), reviewers=len(r['reviews']),
                secs=r['usage']['secs'], tokens=r['usage']['tokens'], cache_read=r['usage']['cache_read'],
                build_secs=r['usage']['build']['secs'], build_tokens=r['usage']['build']['tokens'],
                approver_secs=r['usage']['approver']['secs'], approver_tokens=r['usage']['approver']['tokens']))
            for k, v in r['reviews'].items():
                review_rows.append(dict(
                    cycle=t['cycle'], task=t['id'], round=r['n'],
                    phase='round 1' if r['n'] == 1 else 'later', reviewer=k,
                    verdict=v['verdict'], must_fix=v['must'], nits=v['nit'],
                    found_late=v['late'], evidence_files=v['evidence'],
                    secs=v['usage']['secs'], tokens=v['usage']['tokens'], cache_read=v['usage']['cache_read']))
        last = rs[-1]
        trail = ' › '.join(
            f'R{r["n"]} ' + ' '.join(f'{k[0].upper()}{"✓" if v["verdict"] == "PASS" else "✗"}'
                                     for k, v in r['reviews'].items())
            + (f' → {r["verdict"]}' if r['verdict'] else '')
            for r in rs)
        task_rows.append(dict(
            cycle=t['cycle'], task=t['id'], title=t['title'], rounds=len(rs),
            reworks=sum(r['verdict'] == 'REWORK' for r in rs),
            first_pass=int(rs[0]['verdict'] == 'APPROVE'),
            approved_over_fail=int(last['verdict'] == 'APPROVE'
                                   and any(v['verdict'] == 'FAIL' for v in last['reviews'].values())),
            reviewers=' '.join(rs[0]['reviews']),
            must_fix_total=sum(v['must'] for r in rs for v in r['reviews'].values()),
            nits_total=sum(v['nit'] for r in rs for v in r['reviews'].values()),
            secs=t['usage']['secs'], effort_secs=t['usage']['effort_secs'],
            tokens=t['usage']['tokens'], cache_read=t['usage']['cache_read'],
            orchestration_secs=t['usage']['orchestration']['secs'],
            orchestration_tokens=t['usage']['orchestration']['tokens'],
            branch_commits=t['commits'], merged_at=t['merged_at'] and t['merged_at'].isoformat(),
            trail=trail))
    return task_rows, round_rows, review_rows


def agent_rows(runs):
    out = []
    for r in sorted(runs, key=lambda r: (r['cycle'], r['intervals'][0][0].timestamp() if r['intervals'] else 0)):
        tok = r['tokens']
        out.append(dict(
            cycle=r['cycle'], session=r['session'], agent=r['agent'], role=r['role'], task=r['task'],
            round=r['round'], turn=r['turn'], secs=int(usage.seconds(r['intervals'])),
            tokens=usage.total(tok), **tok,
            start=r['intervals'][0][0].isoformat() if r['intervals'] else None))
    return out


def read_usage(projects, archive, tasks, problems, notes):
    """The agent turns of the archived cycles, matched to their tasks."""
    runs = usage.read_runs(projects) if projects.is_dir() else []
    usage.attribute(runs, tasks, notes)
    left_out = {}
    for r in runs:
        if r['cycle'] is None:
            agg = left_out.setdefault(r['session'], [0, 0, r['project']])
            agg[0] += 1
            agg[1] += usage.total(r['tokens'])
    for session, (n, tok, project) in sorted(left_out.items()):
        notes.append(f'left out: {n} agent turns of session {session} ({project}), '
                     f'{tok:,} tokens, outside every archived cycle')
    checked = usage.check_chronoscope(runs, notes)
    print(f'checked {checked} sessions against chronoscope\'s stats cache')
    cycles = sorted({t['cycle'] for t in tasks})
    runs = usage.merge_stored(runs, archive, cycles, notes)
    covered = {r['cycle'] for r in runs}
    usage.check_coverage(runs, [t for t in tasks if t['cycle'] in covered], problems, notes)
    return runs


def task_files(archive, tasks):
    """Every file under each task's directory, task.md first, then by path."""
    out = {}
    for t in tasks:
        tdir = archive / f'cycle-{t["cycle"]}' / 'tasks' / t['id']
        files = sorted((p.relative_to(tdir).as_posix() for p in tdir.rglob('*') if p.is_file()),
                       key=lambda f: (f != 'task.md', f.split('/')))
        out[f'{t["cycle"]}/{t["id"]}'] = files
    return out


def main():
    here = Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('out', type=Path, nargs='?', default=here / 'data',
                    help='directory to write the JSON files to (default: factory/stats/data)')
    ap.add_argument('--archive', type=Path, default=here.parent / 'archive')
    ap.add_argument('--repo', type=Path, default=None)
    ap.add_argument('--projects', type=Path, default=Path.home() / '.claude' / 'projects',
                    help='Claude Code session transcripts (default: ~/.claude/projects)')
    args = ap.parse_args()
    repo = args.repo or Path(git(here, 'rev-parse', '--show-toplevel').strip())

    problems, notes = [], []
    tasks = read_tasks(args.archive, problems)
    add_merges(tasks, repo, problems)
    runs = read_usage(args.projects, args.archive, tasks, problems, notes)
    add_usage(tasks, runs)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / 'agents.json').write_text(json.dumps(agent_rows(runs), ensure_ascii=False, indent=0))
    print(f'agents.json: {len(runs)} rows')
    for name, data in zip(('tasks', 'rounds', 'reviews'), rows(tasks)):
        (args.out / f'{name}.json').write_text(json.dumps(data, ensure_ascii=False, indent=0))
        print(f'{name}.json: {len(data)} rows')
    (args.out / 'files.json').write_text(json.dumps(task_files(args.archive, tasks), ensure_ascii=False))
    meta = dict(generated=datetime.now().astimezone().isoformat(timespec='seconds'),
                cycles=sorted({t['cycle'] for t in tasks}))
    (args.out / 'meta.json').write_text(json.dumps(meta))
    print(f'wrote {args.out}')
    for n in notes:
        print(f'note: {n}', file=sys.stderr)
    for p in problems:
        print(f'problem: {p}', file=sys.stderr)
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
