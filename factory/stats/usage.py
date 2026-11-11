"""Token use and agent time of the factory's Claude Code sessions.

Used by extract.py. Reads the session transcripts under ~/.claude/projects:
each factory run is a main session (the /factory orchestrator) with one
subagent transcript per agent it, or a task orchestrator, started.

Agent time is the time an agent was working, never the wall clock between
events, and follows chronoscope's session selector:

- A main session's agent time is its "Agent" figure in chronoscope: the
  sum of its Assistant turns' durations, each from the human prompt that
  opened the turn to the turn's last turn_duration entry (or that entry's
  durationMs when a timestamp is missing).
- Subagent transcripts have no turn_duration entries. An agent's turn runs
  from the prompt it was given (its task prompt, or the message that resumed
  it for a later round) to its last entry before it handed its report back.
  Like a main session's turn, it includes the time it waited for its own
  background commands or agents. Waiting to be resumed after a hand-back is
  not agent time.

Tokens are counted as chronoscope counts them: the latest usage of each
assistant message id, per transcript. "tokens" is input + output + cache
writes; cache reads are kept apart.
"""
import json
import re
from datetime import datetime, timedelta
from pathlib import Path

ROLES = {
    'builder': 'builder', 'approver': 'approver', 'planner': 'planner',
    'task-orchestrator': 'task-orchestrator',
    'code-reviewer': 'code', 'feature-reviewer': 'feature', 'ui-reviewer': 'ui',
    'security-reviewer': 'security', 'privacy-reviewer': 'privacy', 'api-reviewer': 'api',
}
PROMPT = re.compile(r'Task-id:\s*(\S+?)\.?\s+(?:Round-id:\s*round-(\d+))?')
ROUND = re.compile(r'Round-id:\s*round-(\d+)')
HANDBACK = 'SubagentHandback'
LONG_TURN = timedelta(hours=3)


def when(entry):
    try:
        return datetime.fromisoformat(entry['timestamp'].replace('Z', '+00:00'))
    except (KeyError, TypeError, ValueError, AttributeError):
        return None


def text_of(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return '\n'.join(b.get('text', '') for b in content if isinstance(b, dict) and b.get('type') == 'text')
    return ''


def has_result(content):
    return isinstance(content, list) and any(isinstance(b, dict) and b.get('type') == 'tool_result' for b in content)


def is_local_command(entry, content):
    if 'origin' in entry or entry.get('isMeta') or entry.get('isCompactSummary') or has_result(content):
        return False
    start = text_of(content).lstrip()
    return start.startswith(('<command-name>', '<local-command-stdout>', '<local-command-stderr>'))


def is_human_prompt(entry):
    """chronoscope's definition (Entry::human_prompt_text)."""
    content = (entry.get('message') or {}).get('content')
    if content is None or is_local_command(entry, content):
        return False
    if has_result(content) or entry.get('isCompactSummary') or entry.get('isMeta'):
        return False
    if 'origin' in entry:
        return isinstance(entry['origin'], dict) and entry['origin'].get('kind') == 'human'
    return isinstance(content, str) or any(isinstance(b, dict) and b.get('type') == 'text' for b in content)


def is_human_queued(entry):
    """A queued_command attachment typed by a human (inject::is_human_queued)."""
    a = entry.get('attachment') or {}
    if a.get('type') != 'queued_command':
        return False
    origin = a.get('origin') if isinstance(a.get('origin'), dict) else entry.get('origin')
    return isinstance(origin, dict) and origin.get('kind') == 'human'


def entries(path):
    with open(path, encoding='utf-8', errors='replace') as f:
        for line in f:
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if isinstance(e, dict):
                yield e


class Usage:
    """Latest usage per assistant message id, and which segment saw it first."""

    def __init__(self):
        self.latest = {}
        self.segment = {}

    def add(self, entry, segment):
        msg = entry.get('message') or {}
        mid, usage = msg.get('id'), msg.get('usage')
        if not mid or not isinstance(usage, dict):
            return
        self.latest[mid] = {
            'input': usage.get('input_tokens') or 0,
            'output': usage.get('output_tokens') or 0,
            'cache_read': usage.get('cache_read_input_tokens') or 0,
            'cache_write': usage.get('cache_creation_input_tokens') or 0,
        }
        self.segment.setdefault(mid, segment)

    def totals(self, segment=None):
        out = dict(input=0, output=0, cache_read=0, cache_write=0)
        for mid, u in self.latest.items():
            if segment is None or self.segment[mid] == segment:
                for k in out:
                    out[k] += u[k]
        return out


def main_session(path):
    """chronoscope's agent time and tokens for a main transcript.
    Returns (segments [(start, end)], usage)."""
    usage = Usage()
    prompt_ts = None
    turn = None            # [start, duration] of the open Assistant turn
    turns = []
    compaction_pending = False
    for e in entries(path):
        kind = e.get('type')
        ts = when(e)
        if kind == 'user' and e.get('isCompactSummary') and compaction_pending:
            compaction_pending = False
            continue
        if kind == 'user' and is_human_prompt(e):
            turn = None
            prompt_ts = ts
        elif kind == 'attachment' and is_human_queued(e):
            turn = None
            prompt_ts = ts
        elif (kind == 'system' and e.get('subtype') == 'compact_boundary') or \
                (kind == 'user' and e.get('isCompactSummary') and not compaction_pending):
            turn = None
            prompt_ts = None
            compaction_pending = kind == 'system'
            continue
        elif kind == 'assistant':
            if turn is None:
                turn = [prompt_ts or ts, None]
                turns.append(turn)
            usage.add(e, 0)
        elif kind == 'system' and e.get('subtype') == 'turn_duration' and turn is not None:
            span = ts - turn[0] if ts and turn[0] and ts >= turn[0] else None
            ms = e.get('durationMs')
            if span is not None:
                turn[1] = span
            elif isinstance(ms, (int, float)):
                turn[1] = timedelta(milliseconds=ms)
    segments = [(t[0], t[0] + t[1]) for t in turns if t[1] is not None and t[0] is not None]
    return segments, usage


def subagent(path):
    """Turns of a subagent transcript: [{start, end, round, text}], and usage.

    A turn ends when the agent hands its report back. The next entry after
    that, a prompt or a reply, starts the next turn. A prompt for another
    round also starts one, should an agent have been resumed without
    handing back."""
    usage = Usage()
    turns = []
    working = False

    def start(ts, text, m):
        prev = turns[-1]['round'] if turns else None
        turns.append(dict(start=ts, end=ts, round=int(m.group(1)) if m else prev,
                          named=bool(m), replied=False, text=text[:300]))

    for e in entries(path):
        kind = e.get('type')
        ts = when(e)
        if not ts:
            continue
        content = (e.get('message') or {}).get('content') if kind in ('user', 'assistant') else None
        if kind == 'user' and not has_result(content) and not e.get('isCompactSummary'):
            text = text_of(content)
            m = ROUND.search(text)
            if not working:
                start(ts, text, m)
                working = True
                continue
            cur = turns[-1]
            if m and cur['named'] and int(m.group(1)) != cur['round']:
                start(ts, text, m)
                continue
            # A resume can come as several entries, the round in a later one.
            if m and not cur['named'] and not cur['replied']:
                cur.update(round=int(m.group(1)), named=True, text=text[:300])
        if not working:
            if kind != 'assistant':
                continue
            start(ts, '', None)
            working = True
        turns[-1]['end'] = max(turns[-1]['end'], ts)
        if kind == 'assistant':
            turns[-1]['replied'] = True
            usage.add(e, len(turns) - 1)
            msg = e.get('message') or {}
            calls = [b.get('name') for b in msg.get('content') or [] if isinstance(b, dict) and b.get('type') == 'tool_use']
            if HANDBACK in calls:
                working = False
    return turns, usage


def factory_sessions(projects):
    """Main transcripts with at least one factory subagent, and all their
    subagents: those of other types (say, one that rebuilt the agent
    definitions) get the role "other"."""
    found = {}
    for meta_path in projects.glob('*/*/subagents/*.meta.json'):
        try:
            if json.loads(meta_path.read_text()).get('agentType') in ROLES:
                main = meta_path.parent.parent.with_suffix('.jsonl')
                if main.exists():
                    found[main] = []
        except (OSError, ValueError):
            continue
    for main, agents in found.items():
        for path in sorted((main.parent / main.stem / 'subagents').glob('*.jsonl')):
            try:
                meta = json.loads(path.with_name(path.stem + '.meta.json').read_text())
            except (OSError, ValueError):
                meta = {}
            agents.append((path, meta))
    return found


def read_runs(projects):
    """One row per agent turn (and one per main session), with intervals."""
    runs = []
    for main, agents in sorted(factory_sessions(projects).items()):
        session = main.stem
        segs, usage = main_session(main)
        runs.append(dict(session=session, project=main.parent.name, transcript=str(main), agent='main', role='main',
                         task=None, round=None, intervals=segs, tokens=usage.totals(), turn=None))
        for path, meta in agents:
            if not path.exists():
                continue
            turns, usage = subagent(path)
            first = next((t['text'] for t in turns if PROMPT.search(t['text'])), '')
            m = PROMPT.search(first)
            task = m.group(1) if m else None
            for i, t in enumerate(turns):
                runs.append(dict(session=session, project=main.parent.name, transcript=str(main),
                                 agent=path.stem.removeprefix('agent-'),
                                 role=ROLES.get(meta.get('agentType'), 'other'),
                                 task=task if meta.get('agentType') in ROLES else None, round=t['round'],
                                 intervals=[(t['start'], t['end'])], tokens=usage.totals(i), turn=i + 1))
    return runs


def union_seconds(intervals):
    total, cur = 0.0, None
    for s, e in sorted(intervals):
        if cur and s <= cur[1]:
            cur[1] = max(cur[1], e)
        else:
            if cur:
                total += (cur[1] - cur[0]).total_seconds()
            cur = [s, e]
    if cur:
        total += (cur[1] - cur[0]).total_seconds()
    return total


def seconds(intervals):
    return sum((e - s).total_seconds() for s, e in intervals)


def total(tokens):
    return tokens['input'] + tokens['output'] + tokens['cache_write']


def chronoscope_cache():
    """chronoscope's cached session stats, by transcript path, with the main
    file size and subagent count the entry was computed for."""
    path = Path.home() / '.cache' / 'chronoscope' / 'stats-v2.json'
    try:
        data = json.loads(path.read_text())
    except (OSError, ValueError):
        return {}
    out = {}
    for key, value in data.items():
        if not key.startswith('claude:') or '.jsonl:' not in key:
            continue
        cut = key.index('.jsonl:') + len('.jsonl')
        rest = key[cut + 1:].split(':')
        try:
            out[key[len('claude:'):cut]] = dict(value, size=int(rest[0]), subagents=int(rest[2]))
        except (IndexError, ValueError):
            continue
    return out


def check_chronoscope(runs, notes):
    """Compares each session's agent time and tokens with chronoscope's
    cache, where its entry is for the files as they are now."""
    cache = chronoscope_cache()
    checked = 0
    for transcript in sorted({r['transcript'] for r in runs if r['cycle'] is not None and r.get('transcript')}):
        entry = cache.get(transcript)
        main = Path(transcript)
        subs = list((main.parent / main.stem / 'subagents').glob('*.jsonl'))
        if not entry or not main.exists() or entry['size'] != main.stat().st_size or entry['subagents'] != len(subs):
            continue
        checked += 1
        own = [r for r in runs if r.get('transcript') == transcript]
        ms = round(seconds(next(r for r in own if r['role'] == 'main')['intervals']) * 1000)
        tokens = {k: sum(r['tokens'][k] for r in own) for k in ('input', 'output', 'cache_read', 'cache_write')}
        if abs(ms - entry['agent_time']) > 1:
            notes.append(f'{main.stem}: agent time {ms} ms, chronoscope says {entry["agent_time"]} ms')
        if any(tokens[k] != entry['tokens'].get(k) for k in tokens):
            notes.append(f'{main.stem}: tokens {tokens}, chronoscope says {entry["tokens"]}')
    return checked


def window_check(cycles, by_cycle):
    """Each cycle's time window: from 12 h before its first branch commit (but
    not before the previous cycle's last merge) to 1 h after its last merge."""
    windows, prev_end = {}, None
    for c in cycles:
        ts = by_cycle[c].values()
        starts = [t['started_at'] for t in ts if t.get('started_at')]
        ends = [t['merged_at'] for t in ts if t.get('merged_at')]
        lo = min(starts) - timedelta(hours=12) if starts else None
        if prev_end and (lo is None or lo < prev_end):
            lo = prev_end
        hi = max(ends) + timedelta(hours=1) if ends else None
        windows[c] = (lo, hi)
        if ends:
            prev_end = max(ends)
    return windows


def attribute(runs, tasks, notes):
    """Sets each run's cycle (None when it is no part of an archived cycle).

    A run that names a task belongs to the cycle that has that task, inside
    the cycle's window and before the task's merge: task ids recur across
    cycles and runs, and an unarchived run reuses them. Main sessions and
    planners take the cycle most of their session's runs belong to."""
    by_cycle = {}
    for t in tasks:
        by_cycle.setdefault(t['cycle'], {})[t['id']] = t
    cycles = sorted(by_cycle)
    windows = window_check(cycles, by_cycle)

    def fits(run, c):
        task = by_cycle[c].get(run['task'])
        if not task:
            return False
        start = run['intervals'][0][0] if run['intervals'] else None
        lo, hi = windows[c]
        if task.get('merged_at'):
            hi = task['merged_at'] + timedelta(hours=1)
        return start is None or ((lo is None or start >= lo) and (hi is None or start <= hi))

    for run in runs:
        run['cycle'] = None
        if run['task']:
            run['cycle'] = next((c for c in cycles if fits(run, c)), None)
    sessions = {}
    for run in runs:
        if run['cycle'] is not None:
            sessions.setdefault(run['session'], []).append(run['cycle'])
    for session, cs in sessions.items():
        major = max(set(cs), key=cs.count)
        if len(set(cs)) > 1:
            notes.append(f'session {session} has agents in cycles {sorted(set(cs))}; counted under each')
        for run in runs:
            if run['session'] == session and not run['task']:
                run['cycle'] = major
    # A session with no task turns at all (a planner run on its own) belongs
    # to the cycle whose window holds its start, if it ran in a project
    # directory that holds matched sessions (not, say, a trial copy).
    projects = {r['project'] for r in runs if r['session'] in sessions}
    for session in {r['session'] for r in runs} - set(sessions):
        own = [r for r in runs if r['session'] == session]
        if any(r['task'] for r in own) or own[0]['project'] not in projects:
            continue
        starts = [i[0] for r in own for i in r['intervals']]
        start = min(starts) if starts else None
        c = next((c for c in cycles if start and windows[c][0] and windows[c][1]
                  and windows[c][0] <= start <= windows[c][1]), None)
        for r in own:
            r['cycle'] = c
    for run in runs:
        if run['cycle'] is None:
            continue
        rounds = len(by_cycle[run['cycle']][run['task']]['rounds']) if run['task'] else 0
        if run['task'] and run['role'] != 'task-orchestrator' and not (run['round'] and 1 <= run['round'] <= rounds):
            notes.append(f'cycle {run["cycle"]} {run["task"]}: {run["role"]} turn for round {run["round"]}, '
                         f'which the board does not have; counted under the task only')
        long = [e - s for s, e in run['intervals'] if e - s > LONG_TURN and run['role'] != 'main']
        if long:
            notes.append(f'cycle {run["cycle"]} {run["task"]} {run["role"]}: a turn of {long[0]}, counted in full')


def check_coverage(runs, tasks, problems, notes):
    """Every review, verdict and build of an archived round must have its
    agent among the runs of that cycle. A review or verdict run twice (a
    restarted agent) is noted; both runs count."""
    have = {(r['cycle'], r['task'], r['round'], r['role']) for r in runs if r['cycle'] is not None}
    seen = {}
    for r in runs:
        if r['cycle'] is not None and r['role'] not in ('builder', 'main', 'planner', 'task-orchestrator', 'other'):
            key = (r['cycle'], r['task'], r['round'], r['role'])
            seen.setdefault(key, set()).add(r['agent'])
    for (c, task, rd, role), agents in sorted(seen.items(), key=lambda kv: str(kv[0])):
        n = len(agents)
        if n > 1:
            notes.append(f'cycle {c} {task} round-{rd}: {n} {role} agents (restarted); all are counted')
    for t in tasks:
        for r in t['rounds']:
            key = (t['cycle'], t['id'], r['n'])
            for role in ['builder', *r['reviews']] + (['approver'] if r['verdict'] else []):
                if (*key, role) not in have:
                    problems.append(f'cycle {t["cycle"]} {t["id"]} round-{r["n"]}: no {role} transcript')


def to_json(run):
    return {**run, 'intervals': [[s.isoformat(), e.isoformat()] for s, e in run['intervals']]}


def from_json(row):
    parse = datetime.fromisoformat
    return {**row, 'intervals': [(parse(s), parse(e)) for s, e in row['intervals']]}


def merge_stored(runs, archive, cycles, notes):
    """Keeps each cycle's runs in archive/cycle-<n>/usage.json, so the
    figures outlive the transcripts (Claude Code deletes old sessions). An
    agent whose transcript is gone keeps its stored runs; every other agent
    is measured again."""
    out = []
    for c in cycles:
        found = [r for r in runs if r['cycle'] == c]
        path = archive / f'cycle-{c}' / 'usage.json'
        stored = []
        if path.exists():
            try:
                stored = [from_json(r) for r in json.loads(path.read_text())['runs']]
            except (OSError, ValueError, KeyError) as err:
                notes.append(f'{path}: unreadable ({err}); ignored')
        have = {(r['session'], r['agent']) for r in found}
        kept = [r for r in stored if (r['session'], r['agent']) not in have]
        if kept:
            agents = len({(r['session'], r['agent']) for r in kept})
            notes.append(f'cycle {c}: {agents} agents no longer have transcripts; using their stored figures')
        merged = found + kept
        if merged:
            path.write_text(json.dumps({'runs': [to_json(r) for r in merged]}, ensure_ascii=False))
        else:
            notes.append(f'cycle {c}: no session transcripts found; no tokens or agent time')
        out += merged
    return out
