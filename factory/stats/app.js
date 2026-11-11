// Factory stats: loads the files that extract.py writes to data/ and draws
// the stats board. All text from the data goes in through textContent.
// Tasks, reviews and verdicts open their archived files in the side panel
// (panel.js).
'use strict';

(() => {
  const DATA_ROOT = new URL('data/', location.href);
  const FILES = ['tasks', 'rounds', 'reviews', 'agents', 'files', 'meta'];
  const SMALL_CYCLE = 5;          // cycles with fewer tasks get a caveat
  const ROUND_SHADES = [0.3, 0.55, 0.8, 1];
  const REVIEWER_ORDER = ['code', 'feature', 'ui', 'security', 'privacy', 'api'];
  // Agents that work on a task; the rest orchestrate the cycle.
  const WORK_ROLES = ['builder', ...REVIEWER_ORDER, 'approver'];
  const ROLE_ORDER = [...WORK_ROLES, 'task-orchestrator', 'main', 'planner', 'other'];
  const ROLE_LABELS = {
    builder: 'Builder', code: 'Code reviewer', feature: 'Feature reviewer', ui: 'UI reviewer',
    security: 'Security reviewer', privacy: 'Privacy reviewer', api: 'API reviewer', approver: 'Approver',
    'task-orchestrator': 'Task orchestrators', main: 'Main session (/factory)', planner: 'Planner',
    other: 'Other agents',
  };
  const VERDICT_KINDS = [
    ['approve', 'APPROVE', 'var(--ok-mark)'],
    ['rework', 'REWORK', 'var(--warn-mark)'],
    ['escalate', 'ESCALATE', 'var(--bad-mark)'],
  ];

  let stats = null;
  let selected = null; // "<cycle>/<task>" open in the panel, or null

  // ---------- formatting ----------

  const fmt = {
    int: (v) => d3.format(',')(v),
    f1: (v) => d3.format('.1f')(v),
    f2: (v) => d3.format('.2f')(v),
    pct: (v) => `${d3.format('.0f')(v)}%`,
    text: (v) => String(v),
    // chronoscope's formats: 45s, 4m 3s, 3h 4m (truncated), and 850, 12.3k, 4.5M.
    dur: (secs) => {
      const s = Math.floor(secs);
      if (s < 60) return `${s}s`;
      if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
      return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
    },
    tok: (n) => {
      if (n < 1000) return String(Math.round(n));
      const k = Math.floor((n + 50) / 100);
      if (k < 10000) return `${Math.floor(k / 10)}.${k % 10}k`;
      const m = Math.floor((n + 50000) / 100000);
      return `${Math.floor(m / 10)}.${m % 10}M`;
    },
  };
  const isWork = (a) => WORK_ROLES.includes(a.role);

  const r2 = (v) => Math.round(v * 100) / 100;
  const pct = (a, b) => (b ? Math.round((100 * a) / b) : 0);
  const plural = (n, word) => `${n} ${word}${n === 1 ? '' : 's'}`;

  function listWords(items) {
    if (items.length < 2) return items.join('');
    return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
  }

  // ---------- derived figures ----------

  function kpis({ tasks, rounds, reviews, agents }) {
    const tokens = d3.sum(agents, (a) => a.tokens);
    const orchestration = d3.sum(agents.filter((a) => !isWork(a)), (a) => a.tokens);
    const usage = {
      tokens,
      cache_read: d3.sum(agents, (a) => a.cache_read),
      tokens_per_task: d3.median(tasks, (t) => t.tokens) || 0,
      secs_per_task: d3.median(tasks, (t) => t.secs) || 0,
      effort_per_task: d3.median(tasks, (t) => t.effort_secs) || 0,
      orchestration_pct: pct(orchestration, tokens),
    };
    const verdicts = rounds.filter((r) => r.verdict !== 'none');
    const reworks = verdicts.filter((r) => r.verdict === 'REWORK').length;
    const first = reviews.filter((v) => v.round === 1);
    const firstPass = tasks.filter((t) => t.first_pass === 1).length;
    return {
      tasks: tasks.length,
      cycles: new Set(tasks.map((t) => t.cycle)).size,
      rounds: rounds.length,
      avg_rounds: r2(rounds.length / tasks.length),
      first_pass: firstPass,
      first_pass_pct: pct(firstPass, tasks.length),
      auto_fix_pct: pct(rounds.filter((r) => r.round === 1 && r.verdict === 'none').length, tasks.length),
      approver_runs: verdicts.length,
      rework_verdicts: reworks,
      rework_pct: pct(reworks, verdicts.length),
      escalations: verdicts.filter((r) => r.verdict === 'ESCALATE').length,
      human_rounds: rounds.filter((r) => r.kind === 'rework').length,
      approved_over_fail: tasks.filter((t) => t.approved_over_fail === 1).length,
      budget_spent: tasks.filter((t) => t.reworks >= 2).length,
      must_fix_r1: r2(d3.mean(first, (v) => v.must_fix) || 0),
      nits_r1: r2(d3.mean(first, (v) => v.nits) || 0),
      ...usage,
    };
  }

  function perCycle({ tasks, rounds, reviews, agents }) {
    const cycles = [...new Set(tasks.map((t) => t.cycle))].sort((a, b) => a - b);
    return cycles.map((c) => {
      const ts = tasks.filter((t) => t.cycle === c);
      const rs = rounds.filter((r) => r.cycle === c);
      const first = reviews.filter((v) => v.cycle === c && v.round === 1);
      const as = agents.filter((a) => a.cycle === c);
      const byRounds = d3.range(1, 5).map((n) => ts.filter((t) => Math.min(t.rounds, 4) === n).length);
      return {
        cycle: c,
        tasks: ts.length,
        avg_rounds: r2(rs.length / ts.length),
        first_pass_pct: pct(ts.filter((t) => t.first_pass === 1).length, ts.length),
        by_rounds: byRounds,
        approve: rs.filter((r) => r.verdict === 'APPROVE').length,
        rework: rs.filter((r) => r.verdict === 'REWORK').length,
        escalate: rs.filter((r) => r.verdict === 'ESCALATE').length,
        must_fix_r1: r2(d3.mean(first, (v) => v.must_fix) || 0),
        work_tokens: d3.sum(as.filter(isWork), (a) => a.tokens),
        orchestration_tokens: d3.sum(as.filter((a) => !isWork(a)), (a) => a.tokens),
        tokens_per_task: Math.round(d3.mean(ts, (t) => t.tokens) || 0),
        secs_per_task: Math.round(d3.median(ts, (t) => t.secs) || 0),
      };
    });
  }

  function headline(cycles, { rounds, tasks }) {
    const first = cycles[0];
    const last = cycles[cycles.length - 1];
    const reworkCycles = rounds.filter((r) => r.verdict === 'REWORK').map((r) => r.cycle);
    const lastRework = reworkCycles.length ? Math.max(...reworkCycles) : null;
    const since = lastRework == null ? 0 : tasks.filter((t) => t.cycle > lastRework).length;
    const small = cycles.filter((c) => c.tasks < SMALL_CYCLE);
    return {
      first_cycle: first.cycle,
      first_tasks: first.tasks,
      first_avg_rounds: first.avg_rounds,
      last_cycle: last.cycle,
      last_tasks: last.tasks,
      last_avg_rounds: last.avg_rounds,
      rework_since: lastRework == null
        ? 'none of them so far.'
        : since
          ? `all of them by cycle ${lastRework}; the ${since} tasks since then needed none.`
          : `the last of them in cycle ${lastRework}.`,
      small_cycles_note: small.length
        ? `${small.length === 1 ? 'Cycle' : 'Cycles'} ${listWords(small.map((c) => `${c.cycle} (${plural(c.tasks, 'task')})`))} had few tasks, so their points move a lot on little evidence.`
        : '',
    };
  }

  function reworkBudget(tasks) {
    return [0, 1, 2].map((n) => ({ reworks: n, tasks: tasks.filter((t) => t.reworks === n).length }));
  }

  function overrides({ tasks, rounds }) {
    return tasks.filter((t) => t.approved_over_fail === 1).map((t) => {
      const final = rounds.find((r) => r.cycle === t.cycle && r.task === t.task && r.round === t.rounds);
      return { cycle: t.cycle, task: t.task, round: t.rounds, failing: final ? final.failing : '', reworks: t.reworks };
    });
  }

  function reviewerNames(reviews) {
    const rank = (name) => (REVIEWER_ORDER.includes(name) ? REVIEWER_ORDER.indexOf(name) : 99);
    return [...new Set(reviews.map((v) => v.reviewer))].sort((a, b) => rank(a) - rank(b));
  }

  function reviewerStats(reviews) {
    const out = [];
    reviewerNames(reviews).forEach((name) => ['round 1', 'later'].forEach((phase) => {
      const vs = reviews.filter((v) => v.reviewer === name && v.phase === phase);
      if (!vs.length) return;
      const mustFix = d3.sum(vs, (v) => v.must_fix);
      const nits = d3.sum(vs, (v) => v.nits);
      out.push({
        reviewer: name,
        phase,
        reviews: vs.length,
        fail_pct: pct(vs.filter((v) => v.verdict === 'FAIL').length, vs.length),
        must_fix: mustFix,
        must_fix_avg: r2(mustFix / vs.length),
        nits,
        nits_avg: r2(nits / vs.length),
        found_late: d3.sum(vs, (v) => v.found_late),
        evidence_avg: Math.round((10 * d3.sum(vs, (v) => v.evidence_files)) / vs.length) / 10,
        secs_avg: d3.mean(vs, (v) => v.secs),
        tokens_avg: d3.mean(vs, (v) => v.tokens),
      });
    }));
    return out;
  }

  function persistence(reviews) {
    const key = (v, round) => `${v.cycle}|${v.task}|${v.reviewer}|${round}`;
    const byKey = new Map(reviews.map((v) => [key(v, v.round), v]));
    return reviewerNames(reviews).map((name) => {
      const next = reviews
        .filter((v) => v.reviewer === name && v.verdict === 'FAIL')
        .map((v) => byKey.get(key(v, v.round + 1)))
        .filter(Boolean);
      const again = next.filter((v) => v.verdict === 'FAIL').length;
      return { reviewer: name, fails: next.length, cleared: next.length - again, still_failing: again, still_pct: pct(again, next.length) };
    }).filter((r) => r.fails > 0);
  }

  // Tasks grouped by the rounds they needed, with the median of a figure.
  function byRounds(tasks, field) {
    const maxRounds = d3.max(tasks, (t) => t.rounds) || 1;
    return d3.range(1, maxRounds + 1).map((n) => {
      const ts = tasks.filter((t) => t.rounds === n && t[field] != null);
      return { rounds: n, tasks: ts, median: ts.length ? d3.median(ts, (t) => t[field]) : null };
    });
  }

  function roleStats(agents) {
    const total = d3.sum(agents, (a) => a.tokens);
    // Orchestrators' time includes waiting for the agents they started, so
    // it is never added to other agents' time.
    const row = (label, as, cls, addTime = true) => ({
      label,
      cls,
      turns: as.length,
      agents: new Set(as.map((a) => `${a.session}/${a.agent}`)).size,
      secs: addTime ? d3.sum(as, (a) => a.secs) : null,
      tokens: d3.sum(as, (a) => a.tokens),
      share: pct(d3.sum(as, (a) => a.tokens), total),
      output: d3.sum(as, (a) => a.output),
      cache_read: d3.sum(as, (a) => a.cache_read),
    });
    const roles = ROLE_ORDER.filter((r) => agents.some((a) => a.role === r));
    const rows = roles.map((r) => row(ROLE_LABELS[r] || r, agents.filter((a) => a.role === r)));
    const work = roles.filter((r) => WORK_ROLES.includes(r)).length;
    return [
      ...rows.slice(0, work),
      row('Task work', agents.filter(isWork), 'subtotal'),
      ...rows.slice(work),
      row('Orchestration', agents.filter((a) => !isWork(a)), 'subtotal', false),
      row('All agents', agents, 'total', false),
    ];
  }

  function derive(data) {
    const cycles = perCycle(data);
    return {
      data,
      kpis: kpis(data),
      cycles,
      headline: headline(cycles, data),
      reworkBudget: reworkBudget(data.tasks),
      overrides: overrides(data),
      reviewerStats: reviewerStats(data.reviews),
      persistence: persistence(data.reviews),
      timeByRounds: byRounds(data.tasks, 'secs'),
      tokensByRounds: byRounds(data.tasks, 'tokens'),
    };
  }

  // ---------- chart helpers ----------

  function el(tag, props = {}, ...children) {
    const node = document.createElement(tag);
    Object.entries(props).forEach(([k, v]) => {
      if (k === 'className') node.className = v;
      else if (k === 'text') node.textContent = v;
      else node.setAttribute(k, v);
    });
    children.forEach((c) => node.append(c));
    return node;
  }

  function tooltip(box) {
    const t = el('div', { className: 'tip' });
    t.hidden = true;
    box.append(t);
    return {
      show(ev, lines) {
        t.replaceChildren(...lines.map((line, i) => el('div', { className: i ? 'tip-row' : 'tip-head', text: line })));
        t.hidden = false;
        const r = box.getBoundingClientRect();
        let x = ev.clientX - r.left + 14;
        let y = ev.clientY - r.top + 14;
        if (x + t.offsetWidth > box.clientWidth) x = Math.max(0, ev.clientX - r.left - t.offsetWidth - 14);
        if (y + t.offsetHeight > box.clientHeight) y = Math.max(0, ev.clientY - r.top - t.offsetHeight - 10);
        t.style.left = `${x}px`;
        t.style.top = `${y}px`;
      },
      hide() { t.hidden = true; },
    };
  }

  function hover(sel, tip, lines) {
    sel.on('pointerenter pointermove', (ev, d) => tip.show(ev, lines(d))).on('pointerleave', () => tip.hide());
  }

  function textWidth(svg, strings, cls) {
    const g = svg.append('g');
    const w = d3.max(strings, (s) => g.append('text').attr('class', cls).text(s).node().getComputedTextLength()) || 0;
    g.remove();
    return w;
  }

  function frame(container, height, label) {
    container.replaceChildren();
    const W = Math.max(260, container.clientWidth || 600);
    const svg = d3.select(container).append('svg')
      .attr('width', W).attr('height', height).attr('viewBox', `0 0 ${W} ${height}`)
      .attr('role', 'img').attr('aria-label', label);
    return { svg, W, tip: tooltip(container) };
  }

  function bottomAxis(svg, x, y, gridTop, tickFormat) {
    const ticks = Math.max(2, Math.floor((x.range()[1] - x.range()[0]) / 70));
    const axis = d3.axisBottom(x).tickSize(-(y - gridTop)).tickPadding(6);
    if (typeof tickFormat === 'function') axis.ticks(ticks).tickFormat(tickFormat);
    else axis.ticks(ticks, tickFormat);
    svg.append('g').attr('transform', `translate(0,${y})`).call(axis);
  }

  function rowLabels(g, r, labW, h) {
    const labY = r.sub ? h / 2 - 3 : h / 2 + 4;
    g.append('text').attr('class', 'row-label').attr('x', labW + 4).attr('y', labY).attr('text-anchor', 'end').text(r.label);
    if (r.sub) g.append('text').attr('class', 'row-sub').attr('x', labW + 4).attr('y', labY + 14).attr('text-anchor', 'end').text(r.sub);
  }

  // Horizontal bars, stacked within a row.
  // rows: [{label, sub, end, bars: [{value, color, opacity, tip}]}]
  function stackedBars(container, { rows, label, valueFmt = fmt.int, tickFormat = 'd' }) {
    const barH = 22, rowStep = barH + 16, top = 4, axisH = 24;
    const H = top + rows.length * rowStep + axisH;
    const { svg, W, tip } = frame(container, H, label);
    const labW = textWidth(svg, rows.flatMap((r) => [r.label, r.sub || '']), 'row-label');
    const endW = textWidth(svg, rows.map((r) => r.end || ''), 'end-label');
    const max = d3.max(rows, (r) => d3.sum(r.bars, (b) => b.value)) || 1;
    const x = d3.scaleLinear().domain([0, max]).nice().range([labW + 12, W - endW - 10]);
    bottomAxis(svg, x, H - axisH + 4, top, tickFormat);
    rows.forEach((r, i) => {
      const g = svg.append('g').attr('transform', `translate(0,${top + i * rowStep})`);
      rowLabels(g, r, labW, barH);
      let acc = 0;
      r.bars.forEach((b) => {
        const x0 = x(acc);
        const w = x(acc + b.value) - x0;
        acc += b.value;
        if (b.value <= 0) return;
        g.append('rect').attr('x', x0).attr('width', w).attr('height', barH).attr('rx', 3)
          .style('fill', b.color).style('fill-opacity', b.opacity ?? 1)
          .style('stroke', 'var(--bg)').style('stroke-width', 2);
        if (w > 22) {
          g.append('text').attr('class', 'in-label').attr('x', x0 + w / 2).attr('y', barH / 2 + 4).attr('text-anchor', 'middle')
            .style('fill', (b.opacity ?? 1) > 0.5 ? 'var(--surface)' : 'var(--text)').text(valueFmt(b.value));
        }
        hover(g.append('rect').attr('class', 'hit').attr('x', x0).attr('y', -2).attr('width', Math.max(w, 10)).attr('height', barH + 4).datum(b), tip, (d) => d.tip);
      });
      if (r.end) g.append('text').attr('class', 'end-label').attr('x', x(acc) + 6).attr('y', barH / 2 + 4).text(r.end);
    });
  }

  // Horizontal bars, grouped under one another within a row.
  function groupedBars(container, { rows, label, valueFmt, tickFormat }) {
    const barH = 13, gap = 3, rowGap = 16, top = 4, axisH = 24;
    const inner = (r) => r.bars.length * barH + (r.bars.length - 1) * gap;
    const H = top + d3.sum(rows, (r) => inner(r) + rowGap) + axisH;
    const { svg, W, tip } = frame(container, H, label);
    const labW = textWidth(svg, rows.flatMap((r) => [r.label, r.sub || '']), 'row-label');
    const endW = textWidth(svg, rows.flatMap((r) => r.bars.map((b) => valueFmt(b.value))), 'end-label');
    const max = d3.max(rows, (r) => d3.max(r.bars, (b) => b.value)) || 1;
    const x = d3.scaleLinear().domain([0, max]).nice().range([labW + 12, W - endW - 10]);
    bottomAxis(svg, x, H - axisH + 4, top, tickFormat);
    let y = top;
    rows.forEach((r) => {
      const g = svg.append('g').attr('transform', `translate(0,${y})`);
      rowLabels(g, r, labW, inner(r));
      r.bars.forEach((b, i) => {
        const by = i * (barH + gap);
        const w = Math.max(b.value > 0 ? 2 : 0, x(b.value) - x(0));
        g.append('rect').attr('x', x(0)).attr('y', by).attr('width', w).attr('height', barH).attr('rx', 3).style('fill', b.color);
        g.append('text').attr('class', 'end-label').attr('x', x(0) + w + 6).attr('y', by + barH / 2 + 4).text(valueFmt(b.value));
        hover(g.append('rect').attr('class', 'hit').attr('x', x(0)).attr('y', by - 1).attr('width', Math.max(w, 10)).attr('height', barH + 2).datum(b), tip, (d) => d.tip);
      });
      y += inner(r) + rowGap;
    });
  }

  // A small line chart over cycles. points: [{x, y, n}]
  function lineChart(container, { points, label, name, valueFmt, max }) {
    const H = 168, top = 22, bottom = 40;
    const { svg, W, tip } = frame(container, H, label);
    const y = d3.scaleLinear().domain([0, max ?? (d3.max(points, (d) => d.y) || 1)]).nice().range([H - bottom, top]);
    const yTicks = y.ticks(4);
    const left = textWidth(svg, yTicks.map(valueFmt), 'axis-label') + 8;
    const x = d3.scalePoint().domain(points.map((d) => d.x)).range([left + 14, W - 20]);
    svg.append('g').attr('transform', `translate(${left},0)`)
      .call(d3.axisLeft(y).tickValues(yTicks).tickFormat(valueFmt).tickSize(-(W - left - 6)).tickPadding(6));
    const xg = svg.append('g').attr('transform', `translate(0,${H - bottom + 16})`);
    points.forEach((d) => {
      xg.append('text').attr('class', 'axis-label').attr('x', x(d.x)).attr('text-anchor', 'middle').text(`C${d.x}`);
      xg.append('text').attr('class', 'axis-label').attr('x', x(d.x)).attr('y', 14).attr('text-anchor', 'middle').text(`n=${d.n}`);
    });
    svg.append('path').datum(points).attr('d', d3.line().x((d) => x(d.x)).y((d) => y(d.y)))
      .style('fill', 'none').style('stroke', 'var(--series-1)').style('stroke-width', 2);
    const last = points.length - 1;
    points.forEach((d, i) => {
      svg.append('circle').attr('cx', x(d.x)).attr('cy', y(d.y)).attr('r', i === last ? 5 : 4)
        .style('fill', 'var(--series-1)').style('stroke', 'var(--bg)').style('stroke-width', 2);
      if (i === 0 || i === last) {
        svg.append('text').attr('class', 'val-label').attr('x', x(d.x)).attr('y', y(d.y) - 10).attr('text-anchor', 'middle').text(valueFmt(d.y));
      }
      hover(svg.append('circle').attr('class', 'hit').attr('cx', x(d.x)).attr('cy', y(d.y)).attr('r', 14).datum(d), tip,
        (p) => [`Cycle ${p.x}`, `${name}: ${valueFmt(p.y)}`, plural(p.n, 'task')]);
    });
  }

  // One dot per task along a value axis, a row per round count, with medians.
  // value(task) is the position; show(value) labels it; unit names the axis.
  function stripChart(container, { rows, label, value, show, unit, tickFormat }) {
    const rowH = 34, top = 6, axisH = 34;
    const H = top + rows.length * rowH + axisH;
    const { svg, W, tip } = frame(container, H, label);
    const labels = rows.map((r) => plural(r.rounds, 'round'));
    const ends = rows.map((r) => (r.median == null ? '' : `median ${show(r.median)}`));
    const labW = textWidth(svg, labels, 'row-label');
    const endW = textWidth(svg, ends, 'end-label');
    const all = rows.flatMap((r) => r.tasks);
    const x = d3.scaleLinear().domain([0, d3.max(all, value) || 1]).nice().range([labW + 14, W - endW - 14]);
    bottomAxis(svg, x, H - axisH + 4, top, tickFormat);
    svg.append('text').attr('class', 'axis-label').attr('x', x.range()[1]).attr('y', H - 2).attr('text-anchor', 'end').text(unit);
    rows.forEach((r, i) => {
      const cy = top + i * rowH + rowH / 2;
      svg.append('text').attr('class', 'row-label').attr('x', labW + 4).attr('y', cy + 4).attr('text-anchor', 'end').text(labels[i]);
      r.tasks.forEach((d, j) => {
        const dy = cy + ((j % 3) - 1) * 6;
        svg.append('circle').attr('cx', x(value(d))).attr('cy', dy).attr('r', 4.5)
          .style('fill', 'var(--series-1)').style('fill-opacity', 0.7).style('stroke', 'var(--bg)').style('stroke-width', 1.5);
        hover(svg.append('circle').attr('class', 'hit link').attr('cx', x(value(d))).attr('cy', dy).attr('r', 9).datum(d)
          .on('click', (ev, p) => openTask(p.cycle, p.task)), tip,
        (p) => [p.task, `Cycle ${p.cycle}, ${plural(p.rounds, 'round')}`,
          `Agent time ${fmt.dur(p.secs)} (effort ${fmt.dur(p.effort_secs)})`, `${fmt.tok(p.tokens)} tokens`, 'Click to open the task']);
      });
      if (r.median == null) return;
      const mx = x(value({ secs: r.median, tokens: r.median }));
      svg.append('line').attr('x1', mx).attr('x2', mx).attr('y1', cy - 12).attr('y2', cy + 12)
        .style('stroke', 'var(--text)').style('stroke-width', 2);
      svg.append('text').attr('class', 'end-label').attr('x', x.range()[1] + 8).attr('y', cy + 4).text(ends[i]);
    });
  }

  // ---------- sections ----------

  function renderValues(s) {
    const scopes = { kpis: s.kpis, headline: s.headline };
    document.querySelectorAll('[data-v]').forEach((node) => {
      const [scope, field] = node.dataset.v.split('.');
      const v = scopes[scope] ? scopes[scope][field] : null;
      node.textContent = v == null ? '—' : fmt[node.dataset.fmt || (typeof v === 'number' ? 'int' : 'text')](v);
    });
  }

  function renderRounds(s) {
    const legend = document.getElementById('rounds-legend');
    legend.replaceChildren(...ROUND_SHADES.map((op, i) => {
      const sw = el('span', { className: 'sw' });
      sw.style.background = 'var(--series-1)';
      sw.style.opacity = String(op);
      return el('li', {}, sw, i === 3 ? '4 or more rounds' : plural(i + 1, 'round'));
    }));
    stackedBars(document.getElementById('rounds-chart'), {
      label: 'Tasks per cycle by the number of rounds they needed',
      rows: s.cycles.map((c) => ({
        label: `Cycle ${c.cycle}`,
        sub: plural(c.tasks, 'task'),
        end: `avg ${fmt.f2(c.avg_rounds)}`,
        bars: c.by_rounds.map((n, i) => ({
          value: n, color: 'var(--series-1)', opacity: ROUND_SHADES[i],
          tip: [`Cycle ${c.cycle}`, `${n} of ${c.tasks} tasks needed ${i === 3 ? '4 or more rounds' : plural(i + 1, 'round')}`],
        })),
      })),
    });
  }

  function renderTrends(s) {
    document.querySelectorAll('[data-trend]').forEach((node) => {
      const field = node.dataset.trend;
      lineChart(node, {
        points: s.cycles.map((c) => ({ x: c.cycle, y: c[field], n: c.tasks })),
        label: node.dataset.label,
        name: node.closest('.cell').querySelector('h3').textContent,
        valueFmt: fmt[node.dataset.fmt],
        max: node.dataset.max ? Number(node.dataset.max) : undefined,
      });
    });
  }

  function renderApprover(s) {
    stackedBars(document.getElementById('verdict-chart'), {
      label: 'Approver verdicts per cycle',
      rows: s.cycles.map((c) => ({
        label: `Cycle ${c.cycle}`,
        sub: plural(c.approve + c.rework + c.escalate, 'verdict'),
        bars: VERDICT_KINDS.map(([k, name, color]) => ({ value: c[k], color, opacity: 0.9, tip: [`Cycle ${c.cycle}`, `${c[k]} ${name}`] })),
      })),
    });
    const words = ['No REWORK', 'One REWORK', 'Both REWORKs'];
    groupedBars(document.getElementById('budget-chart'), {
      label: 'Tasks by how many REWORK verdicts they received',
      valueFmt: fmt.int,
      tickFormat: 'd',
      rows: s.reworkBudget.map((r) => ({
        label: words[r.reworks],
        bars: [{ value: r.tasks, color: 'var(--series-1)', tip: [words[r.reworks], plural(r.tasks, 'task')] }],
      })),
    });
    document.getElementById('overrides-body').replaceChildren(...s.overrides.map((r) => taskRow(r.cycle, r.task,
      el('td', { className: 'num', text: String(r.cycle) }),
      el('td', { className: 'id' }, taskLink(r.cycle, r.task, r.task)),
      el('td', { className: 'num', text: String(r.round) }),
      el('td', {}, ...r.failing.split(' ').filter(Boolean).map((name) =>
        fileLink(r.cycle, r.task, `round-${r.round}/${name}-review.md`, name, `Open the round-${r.round} ${name} review`))),
      el('td', { className: 'num', text: String(r.reworks) }))));
  }

  function renderReviewers(s) {
    const names = [...new Set(s.reviewerStats.map((r) => r.reviewer))];
    document.querySelectorAll('[data-reviewer]').forEach((node) => {
      const field = node.dataset.reviewer;
      const metric = field === 'must_fix_avg' ? 'must-fix' : 'nits';
      groupedBars(node, {
        label: node.dataset.label,
        valueFmt: fmt.f2,
        rows: names.map((name) => {
          const phases = ['round 1', 'later'].map((p) => s.reviewerStats.find((r) => r.reviewer === name && r.phase === p));
          return {
            label: name,
            sub: plural(d3.sum(phases, (p) => (p ? p.reviews : 0)), 'review'),
            bars: phases.map((p, i) => ({
              value: p ? p[field] : 0,
              color: i ? 'var(--series-2)' : 'var(--series-1)',
              tip: p ? [`${name}, ${p.phase}`, `${fmt.f2(p[field])} ${metric} per review`, `${plural(p.reviews, 'review')}, ${p.fail_pct}% FAIL`]
                : [name, 'No reviews in these rounds'],
            })),
          };
        }),
      });
    });
    fillTable('reviewer-body', s.reviewerStats, (r) => [
      [r.reviewer], [r.phase], [fmt.int(r.reviews), 'num'], [fmt.pct(r.fail_pct), 'num'],
      [fmt.int(r.must_fix), 'num'], [fmt.f2(r.must_fix_avg), 'num'], [fmt.int(r.nits), 'num'],
      [fmt.f2(r.nits_avg), 'num'], [fmt.int(r.found_late), 'num'], [fmt.f1(r.evidence_avg), 'num'],
      [r.secs_avg == null ? '—' : fmt.dur(r.secs_avg), 'num'], [r.tokens_avg == null ? '—' : fmt.tok(r.tokens_avg), 'num'],
    ]);
    fillTable('persistence-body', s.persistence, (r) => [
      [r.reviewer], [fmt.int(r.fails), 'num'], [fmt.int(r.cleared), 'num'],
      [fmt.int(r.still_failing), 'num'], [fmt.pct(r.still_pct), 'num'],
    ]);
  }

  function renderTiming(s) {
    stripChart(document.getElementById('timing-chart'), {
      rows: s.timeByRounds,
      label: 'Agent time per task, one dot per task, grouped by rounds needed, with the median marked',
      value: (t) => t.secs / 60,
      show: (secs) => fmt.dur(secs),
      unit: 'agent minutes',
      tickFormat: 'd',
    });
  }

  function renderTokens(s) {
    stackedBars(document.getElementById('tokens-chart'), {
      label: 'Tokens per cycle, split into task work and orchestration',
      valueFmt: fmt.tok,
      tickFormat: fmt.tok,
      rows: s.cycles.map((c) => ({
        label: `Cycle ${c.cycle}`,
        sub: plural(c.tasks, 'task'),
        end: fmt.tok(c.work_tokens + c.orchestration_tokens),
        bars: [
          { value: c.work_tokens, color: 'var(--series-1)', tip: [`Cycle ${c.cycle}`, `Task work: ${fmt.tok(c.work_tokens)} tokens`, `${fmt.tok(c.tokens_per_task)} per task on average`] },
          { value: c.orchestration_tokens, color: 'var(--series-2)', tip: [`Cycle ${c.cycle}`, `Orchestration: ${fmt.tok(c.orchestration_tokens)} tokens`] },
        ],
      })),
    });
    stripChart(document.getElementById('task-tokens-chart'), {
      rows: s.tokensByRounds,
      label: 'Tokens per task, one dot per task, grouped by rounds needed, with the median marked',
      value: (t) => t.tokens,
      show: fmt.tok,
      unit: 'tokens',
      tickFormat: fmt.tok,
    });
  }

  // Fills a cycle selector, keeping its choice, and returns the chosen cycle
  // ('' for all of them).
  function cyclePick(id, s) {
    const pick = document.getElementById(id);
    const cycles = s.cycles.map((c) => String(c.cycle));
    const current = cycles.includes(pick.value) ? pick.value : '';
    pick.replaceChildren(new Option('All cycles', ''), ...cycles.map((c) => new Option(`Cycle ${c}`, c)));
    pick.value = current;
    return current;
  }

  function renderRoles(s) {
    const current = cyclePick('role-cycle-pick', s);
    const agents = s.data.agents.filter((a) => !current || String(a.cycle) === current);
    document.getElementById('role-body').replaceChildren(...roleStats(agents).map((r) => {
      const row = el('tr', r.cls ? { className: r.cls } : {});
      [[r.label], [fmt.int(r.turns), 'num'], [fmt.int(r.agents), 'num'], [r.secs == null ? '—' : fmt.dur(r.secs), 'num'],
        [fmt.tok(r.tokens), 'num'], [fmt.pct(r.share), 'num'], [fmt.tok(r.turns ? r.tokens / r.turns : 0), 'num'],
        [fmt.tok(r.output), 'num'], [fmt.tok(r.cache_read), 'num']]
        .forEach(([text, cls]) => row.append(el('td', cls ? { className: cls, text } : { text })));
      return row;
    }));
  }

  function fillTable(id, rows, cells) {
    document.getElementById(id).replaceChildren(...rows.map((r) => el('tr', {},
      ...cells(r).map(([text, cls]) => el('td', cls ? { className: cls, text: String(text) } : { text: String(text) })))));
  }

  // ---------- links into the archive ----------

  function taskKey(cycle, task) {
    return `${cycle}/${task}`;
  }

  function hasFile(cycle, task, file) {
    return (stats.data.files[taskKey(cycle, task)] || []).includes(file);
  }

  function openTask(cycle, task, file = 'task.md', hash = '') {
    window.FactoryPanel.open(cycle, task, file, hash);
  }

  // A button that opens one archived file of a task. children default to
  // the label text.
  function fileLink(cycle, task, file, label, title, { className = 'link text-link', hash = '', children } = {}) {
    const button = el('button', { type: 'button', className, title });
    button.append(...(children || [label]));
    button.addEventListener('click', () => openTask(cycle, task, file, hash));
    return button;
  }

  function taskLink(cycle, task, label) {
    return fileLink(cycle, task, 'task.md', label, `Open ${task}`);
  }

  function taskRow(cycle, task, ...cells) {
    const row = el('tr', {}, ...cells);
    row.dataset.key = taskKey(cycle, task);
    row.classList.toggle('selected', row.dataset.key === selected);
    return row;
  }

  function markSelected(ctx) {
    selected = ctx ? taskKey(ctx.cycle, ctx.task) : null;
    document.querySelectorAll('tr[data-key]').forEach((row) => row.classList.toggle('selected', row.dataset.key === selected));
  }

  function usageText(secs, tokens) {
    return secs == null ? '' : `: ${fmt.dur(secs)} agent time, ${fmt.tok(tokens)} tokens`;
  }

  // One round of a task: its build notes, each review, and the Approver's
  // verdict (at its rework note for a REWORK) or the human's rework note.
  function roundChip(round, reviews) {
    const { cycle, task } = round;
    const dir = `round-${round.round}`;
    const chip = el('span', { className: 'round' },
      fileLink(cycle, task, `${dir}/build-notes.md`, `R${round.round}`,
        `Round ${round.round}${usageText(round.secs, round.tokens)}. Build${usageText(round.build_secs, round.build_tokens)}. Open the build notes`,
        { className: 'link round-n' }));
    reviews.forEach((v) => {
      const pass = v.verdict === 'PASS';
      chip.append(fileLink(cycle, task, `${dir}/${v.reviewer}-review.md`, v.reviewer[0].toUpperCase(),
        `${v.reviewer}: ${v.verdict}, ${v.must_fix} must-fix, ${v.nits} nits${usageText(v.secs, v.tokens)}. Open the review`,
        { className: `link chip ${pass ? 'chip-pass' : 'chip-fail'}` }));
    });
    if (round.kind === 'rework' && hasFile(cycle, task, `${dir}/human-review.md`)) {
      chip.append(fileLink(cycle, task, `${dir}/human-review.md`, 'human', 'A human asked for this round. Open the rework note',
        { className: 'link verdict v-human' }));
    }
    if (round.verdict !== 'none') {
      const rework = round.verdict === 'REWORK';
      chip.append(fileLink(cycle, task, `${dir}/verdict.md`, round.verdict,
        `Approver${usageText(round.approver_secs, round.approver_tokens)}. ${rework ? 'Open the verdict at its rework note' : 'Open the verdict'}`,
        { className: `link verdict v-${round.verdict}`, hash: rework ? 'rework-note' : '' }));
    }
    return chip;
  }

  function taskTrail(t, { rounds, reviews }) {
    const own = (r) => r.cycle === t.cycle && r.task === t.task;
    return el('div', { className: 'trail' }, ...rounds.filter(own).map((r) =>
      roundChip(r, reviews.filter((v) => own(v) && v.round === r.round))));
  }

  function renderTasks(s) {
    const current = cyclePick('cycle-pick', s);
    const shown = s.data.tasks.filter((t) => !current || String(t.cycle) === current);
    const rows = [];
    d3.groups(shown, (t) => t.cycle).forEach(([cycle, ts]) => {
      rows.push(...ts.map((t) => taskRow(t.cycle, t.task,
        el('td', { className: 'num', text: String(t.cycle) }),
        el('td', { className: 'id' }, taskLink(t.cycle, t.task, t.task)),
        el('td', { className: 'title' }, taskLink(t.cycle, t.task, t.title)),
        el('td', { className: 'num', text: String(t.rounds) }),
        el('td', { className: 'num', text: String(t.reworks) }),
        el('td', { className: 'num', text: String(t.must_fix_total) }),
        el('td', { className: 'num', text: String(t.nits_total) }),
        el('td', { className: 'num', text: t.secs == null ? '—' : fmt.dur(t.secs), title: t.secs == null ? '' : `Effort ${fmt.dur(t.effort_secs)}: the agents' times summed` }),
        el('td', { className: 'num', text: t.tokens == null ? '—' : fmt.tok(t.tokens), title: t.tokens == null ? '' : `${fmt.tok(t.cache_read)} cache reads` }),
        el('td', {}, taskTrail(t, s.data)))));
      rows.push(totalRow(`Cycle ${cycle}`, String(cycle), ts, 'subtotal'));
    });
    // With one cycle picked, its subtotal is already the total.
    if (!current && shown.length) rows.push(totalRow('All cycles', '', shown, 'total'));
    document.getElementById('task-body').replaceChildren(...rows);
  }

  // A sum row under a cycle's tasks, or under all of them. Agent time and
  // tokens add up the tasks' own figures; orchestration is not included.
  function totalRow(label, cycle, ts, cls) {
    const sum = (field) => d3.sum(ts, (t) => t[field] || 0);
    const timed = ts.some((t) => t.secs != null);
    const firstPass = ts.filter((t) => t.first_pass === 1).length;
    return el('tr', { className: cls },
      el('td', { className: 'num', text: cycle }),
      el('td', { text: label }),
      el('td', { text: plural(ts.length, 'task') }),
      el('td', { className: 'num', text: fmt.int(sum('rounds')) }),
      el('td', { className: 'num', text: fmt.int(sum('reworks')) }),
      el('td', { className: 'num', text: fmt.int(sum('must_fix_total')) }),
      el('td', { className: 'num', text: fmt.int(sum('nits_total')) }),
      el('td', { className: 'num', text: timed ? fmt.dur(sum('secs')) : '—', title: timed ? `Effort ${fmt.dur(sum('effort_secs'))}: the agents' times summed` : '' }),
      el('td', { className: 'num', text: timed ? fmt.tok(sum('tokens')) : '—', title: timed ? `${fmt.tok(sum('cache_read'))} cache reads` : '' }),
      el('td', { className: 'sum-note', text: `${fmt.f2(sum('rounds') / ts.length)} rounds per task, ${fmt.pct(pct(firstPass, ts.length))} first-pass` }));
  }

  function renderCharts() {
    if (!stats) return;
    renderRounds(stats);
    renderTrends(stats);
    renderApprover(stats);
    renderReviewers(stats);
    renderTiming(stats);
    renderTokens(stats);
  }

  function renderAll() {
    renderValues(stats);
    renderCharts();
    renderRoles(stats);
    renderTasks(stats);
    const meta = stats.data.meta;
    document.getElementById('generated').textContent =
      `Cycles ${meta.cycles[0]}–${meta.cycles[meta.cycles.length - 1]} · data generated ${new Date(meta.generated).toLocaleString()}`;
  }

  // ---------- loading ----------

  async function loadFile(name) {
    const url = new URL(`${name}.json`, DATA_ROOT);
    const res = await fetch(url, { cache: 'no-store' });
    if (!res.ok) throw new Error(`${url.pathname}: HTTP ${res.status}`);
    return res.json();
  }

  async function load() {
    const note = document.getElementById('load-error');
    try {
      const files = await Promise.all(FILES.map(loadFile));
      stats = derive(Object.fromEntries(FILES.map((f, i) => [f, files[i]])));
      note.hidden = true;
      renderAll();
      const titles = new Map(stats.data.tasks.map((t) => [taskKey(t.cycle, t.task), t.title]));
      window.FactoryPanel.init({
        files: (cycle, task) => stats.data.files[taskKey(cycle, task)] || [],
        title: (cycle, task) => titles.get(taskKey(cycle, task)) || '',
        changed: markSelected,
      });
    } catch (err) {
      note.textContent = `Could not load the stats data (${err.message}). Run python3 factory/stats/extract.py, then reload.`;
      note.hidden = false;
    }
  }

  function watchWidth() {
    let lastWidth = 0;
    let pending = 0;
    new ResizeObserver(([entry]) => {
      const w = Math.round(entry.contentRect.width);
      if (w === lastWidth) return;
      lastWidth = w;
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(renderCharts);
    }).observe(document.getElementById('page'));
  }

  document.getElementById('cycle-pick').addEventListener('change', () => stats && renderTasks(stats));
  document.getElementById('role-cycle-pick').addEventListener('change', () => stats && renderRoles(stats));
  watchWidth();
  load();
})();
