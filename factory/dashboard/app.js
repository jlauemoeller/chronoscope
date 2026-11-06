'use strict';

// The factory dashboard: polls ../state/board.json and shows the tasks as a
// kanban board. Served with `python3 -m http.server -d factory`, opened at
// /dashboard/.

(() => {
  const STATE_URL = new URL('../state/', location.href);
  const BOARD_URL = new URL('board.json', STATE_URL);
  const POLL_MS = 5000;
  const STAGES = [
    ['planned', 'Planned'], ['building', 'Building'], ['review', 'Review'],
    ['approval', 'Approval'], ['escalated', 'Escalated'], ['done', 'Done'],
    ['abandoned', 'Abandoned'],
  ];
  const RUN_FILES = ['SPEC.md', 'preflight.md', 'stack.md', 'backlog.md'];
  const IMAGE_RE = /\.(png|jpe?g|gif|svg|webp|bmp|avif)$/i;
  const MARKDOWN_RE = /\.(md|markdown)$/i;
  const KIND_LABELS = { 'approver-rework': 'approver rework', rework: 'human rework' };

  const view = {
    board: null,          // the last board that loaded
    error: null,          // why the last fetch failed, or null
    runFiles: [],         // run files that exist
    panel: null,          // { kind: 'task', id } or { kind: 'run' }
    openUrl: null,        // URL of the file shown in the panel
  };

  // ------------------------------------------------------------ helpers --

  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  function pathUrl(base, path) {
    return new URL(path.split('/').map(encodeURIComponent).join('/'), base);
  }

  function taskUrl(id, file) {
    return pathUrl(new URL(`tasks/${encodeURIComponent(id)}/`, STATE_URL), file);
  }

  function statePath(url) {
    const rel = url.pathname.startsWith(STATE_URL.pathname)
      ? url.pathname.slice(STATE_URL.pathname.length) : url.pathname;
    return decodeURIComponent(rel);
  }

  function findTask(id) {
    return view.board ? view.board.tasks.find((t) => t.id === id) : undefined;
  }

  function isActive(task) {
    return task.status !== 'Complete' && task.status !== 'Abandoned';
  }

  function latestRound(task) {
    const rounds = task.rounds || [];
    return rounds.length ? rounds[rounds.length - 1] : null;
  }

  function alertReason(task) {
    if (!isActive(task)) return null;
    const round = latestRound(task);
    if (round && round.build === 'BLOCKED') return 'BLOCKED';
    if (round && round.verdict === 'ESCALATE') return 'ESCALATE';
    if (task.status === 'Escalated') return 'ESCALATED';
    return null;
  }

  // ------------------------------------------------------------ fetching --

  async function fetchBoard() {
    try {
      const res = await fetch(BOARD_URL, { cache: 'no-store' });
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`.trim());
      const data = await res.json();
      if (!data || typeof data.loop !== 'object' || !Array.isArray(data.tasks)) {
        throw new Error('board.json has no loop or tasks');
      }
      view.board = data;
      view.error = null;
    } catch (err) {
      view.error = err && err.message ? err.message : String(err);
    }
  }

  async function fetchRunFiles() {
    const found = await Promise.all(RUN_FILES.map(async (name) => {
      try {
        const res = await fetch(new URL(name, STATE_URL), { method: 'HEAD', cache: 'no-store' });
        return res.ok ? name : null;
      } catch (err) {
        return null;
      }
    }));
    view.runFiles = found.filter(Boolean);
  }

  let lastSignature = null;

  async function poll() {
    await Promise.all([fetchBoard(), fetchRunFiles()]);
    // Re-render only when something changed, so focus and scrolling survive.
    const signature = JSON.stringify([view.board, view.error, view.runFiles]);
    if (signature === lastSignature) return;
    lastSignature = signature;
    renderAll();
  }

  // -------------------------------------------------------------- header --

  function renderLoop() {
    const loop = (view.board && view.board.loop) || {};
    const state = $('loop-state');
    state.textContent = loop.state || (view.board ? 'unknown' : 'no board');
    state.className = `loop-state ${loop.state || ''}`;
    $('loop-message').textContent = loop.message || '';
  }

  function renderUpdated() {
    const updated = view.board && view.board.updated;
    $('updated').textContent = updated ? `updated ${new Date(updated).toLocaleString()}` : '';
  }

  function renderStale() {
    const stale = $('stale');
    if (!view.error) {
      stale.hidden = true;
      return;
    }
    stale.hidden = false;
    stale.textContent = view.board
      ? `Stale: showing the last board. Fetching board.json failed: ${view.error}`
      : `No board yet. Fetching board.json failed: ${view.error}`;
  }

  function renderRunFiles() {
    const nav = $('run-files');
    nav.replaceChildren(...view.runFiles.map((name) => {
      const button = el('button', 'run-file', name);
      button.type = 'button';
      button.addEventListener('click', () => openRunFile(name));
      return button;
    }));
  }

  // --------------------------------------------------------------- board --

  function chip(label, value) {
    const cls = String(value || 'none').toLowerCase();
    return el('span', `chip ${cls}`, `${label} ${value || 'none'}`);
  }

  function verdictChip(task, round) {
    if (round.verdict) return chip('verdict', round.verdict);
    const pending = isActive(task) && round === latestRound(task) && round.build !== 'BLOCKED';
    return pending ? chip('verdict', 'pending') : el('span', 'chip none', 'no verdict');
  }

  function roundRow(task, round) {
    const row = el('span', 'round');
    const kind = KIND_LABELS[round.kind];
    row.append(el('span', 'round-id', kind ? `${round.id} (${kind})` : round.id));
    row.append(chip('build', round.build || 'pending'));
    for (const [key, value] of Object.entries(round.reviews || {})) {
      row.append(chip(key, value));
    }
    row.append(verdictChip(task, round));
    return row;
  }

  function card(task) {
    const node = el('button', 'card');
    node.type = 'button';
    const reason = alertReason(task);
    if (reason) node.classList.add('alert');
    if (view.board.loop && view.board.loop.current === task.id) node.classList.add('current');
    if (view.panel && view.panel.kind === 'task' && view.panel.id === task.id) {
      node.classList.add('selected');
    }
    node.append(el('span', 'card-id', task.id));
    node.append(el('span', 'card-title', task.title || task.id));
    const round = latestRound(task);
    node.append(el('span', 'card-status', round ? `${task.status} · ${round.id}` : task.status));
    if (reason) node.append(el('span', 'card-flag', reason));
    if ((task.rounds || []).length) {
      const rounds = el('span', 'rounds');
      rounds.append(...task.rounds.map((r) => roundRow(task, r)));
      node.append(rounds);
    }
    node.addEventListener('click', () => openTask(task.id));
    return node;
  }

  function column(stage, label, tasks) {
    const col = el('section', 'column');
    col.setAttribute('aria-label', label);
    const head = el('div', 'column-head');
    head.append(el('span', '', label), el('span', 'column-count', String(tasks.length)));
    const cards = el('div', 'cards');
    if (tasks.length) cards.append(...tasks.map(card));
    else cards.append(el('div', 'empty', '—'));
    col.append(head, cards);
    return col;
  }

  function renderBoard() {
    const tasks = view.board ? view.board.tasks : [];
    $('board').replaceChildren(...STAGES.map(([stage, label]) =>
      column(stage, label, tasks.filter((t) => t.stage === stage))));
  }

  // --------------------------------------------------------------- panel --

  function panelFiles() {
    if (!view.panel) return [];
    if (view.panel.kind === 'run') {
      return view.runFiles.map((name) => ({ label: name, url: new URL(name, STATE_URL) }));
    }
    const task = findTask(view.panel.id);
    return task ? (task.files || []).map((f) => ({ label: f, url: taskUrl(task.id, f) })) : [];
  }

  function renderPanel() {
    const panel = $('panel');
    if (!view.panel) {
      panel.hidden = true;
      return;
    }
    panel.hidden = false;
    const task = view.panel.kind === 'task' ? findTask(view.panel.id) : null;
    $('panel-title').textContent = view.panel.kind === 'run'
      ? 'Run files' : `${view.panel.id}${task && task.title ? ` · ${task.title}` : ''}`;
    $('panel-files').replaceChildren(...panelFiles().map(fileItem));
  }

  function fileItem(file) {
    const li = el('li');
    const button = el('button', '', file.label);
    button.type = 'button';
    if (view.openUrl && view.openUrl.href === file.url.href) button.classList.add('open');
    button.addEventListener('click', () => showFile(file.url, file.label));
    li.append(button);
    return li;
  }

  function openTask(id) {
    if (!view.panel || view.panel.kind !== 'task' || view.panel.id !== id) {
      view.openUrl = null;
      $('panel-view').replaceChildren(el('p', 'hint', 'Choose a file.'));
    }
    view.panel = { kind: 'task', id };
    renderAll();
  }

  function openRunFile(name) {
    view.panel = { kind: 'run' };
    renderAll();
    showFile(new URL(name, STATE_URL), name);
  }

  function closePanel() {
    view.panel = null;
    view.openUrl = null;
    renderAll();
  }

  // ---------------------------------------------------------- file view --

  function canRenderMarkdown() {
    return typeof window.marked !== 'undefined' && typeof window.marked.parse === 'function'
      && typeof window.DOMPurify !== 'undefined';
  }

  function markdownNode(text, url) {
    if (!canRenderMarkdown()) return el('pre', 'plain', text);
    const node = el('div', 'markdown');
    node.innerHTML = window.DOMPurify.sanitize(window.marked.parse(text));
    for (const a of node.querySelectorAll('a[href]')) {
      const target = new URL(a.getAttribute('href'), url);
      if (target.origin !== location.origin) {
        a.target = '_blank';
        a.rel = 'noopener noreferrer';
      }
    }
    return node;
  }

  async function fileBody(url, label) {
    if (IMAGE_RE.test(label)) {
      const img = el('img');
      img.src = `${url.href}${url.search ? '&' : '?'}t=${Date.now()}`;
      img.alt = label;
      return img;
    }
    const res = await fetch(url, { cache: 'no-store' });
    if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`.trim());
    const text = await res.text();
    return MARKDOWN_RE.test(label) ? markdownNode(text, url) : el('pre', 'plain', text);
  }

  async function showFile(url, label) {
    view.openUrl = url;
    renderPanel();
    const target = $('panel-view');
    let body;
    try {
      body = await fileBody(url, label);
    } catch (err) {
      body = el('p', 'hint', `Could not load ${label}: ${err.message || err}`);
    }
    if (view.openUrl !== url) return; // another file was chosen meanwhile
    target.replaceChildren(el('div', 'file-name', label), body);
    target.scrollTop = 0;
  }

  // Relative links between Markdown files (such as SPEC.md to spec/x.md)
  // open in the panel instead of leaving the dashboard.
  function onViewClick(event) {
    const a = event.target.closest('a[href]');
    if (!a || !view.openUrl) return;
    const href = a.getAttribute('href');
    if (href.startsWith('#')) return;
    const target = new URL(href, view.openUrl);
    if (target.origin !== location.origin || !MARKDOWN_RE.test(target.pathname)) return;
    event.preventDefault();
    target.hash = '';
    showFile(target, statePath(target));
  }

  // -------------------------------------------------------- panel resize --

  const WIDTH_KEY = 'factory-dashboard-panel-width';

  function setPanelWidth(px) {
    const width = Math.max(280, Math.min(px, window.innerWidth - 40));
    $('panel').style.setProperty('--panel-width', `${width}px`);
    try { localStorage.setItem(WIDTH_KEY, String(width)); } catch (err) { /* optional */ }
  }

  function restorePanelWidth() {
    let saved = null;
    try { saved = Number(localStorage.getItem(WIDTH_KEY)); } catch (err) { /* optional */ }
    if (saved) setPanelWidth(saved);
  }

  function initResize() {
    const handle = $('panel-resize');
    handle.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      handle.setPointerCapture(event.pointerId);
      const move = (e) => setPanelWidth(window.innerWidth - e.clientX);
      const up = () => {
        handle.removeEventListener('pointermove', move);
        handle.removeEventListener('pointerup', up);
      };
      handle.addEventListener('pointermove', move);
      handle.addEventListener('pointerup', up);
    });
    handle.addEventListener('keydown', (event) => {
      const width = $('panel').getBoundingClientRect().width;
      if (event.key === 'ArrowLeft') setPanelWidth(width + 32);
      if (event.key === 'ArrowRight') setPanelWidth(width - 32);
    });
  }

  // ---------------------------------------------------------------- main --

  function $(id) {
    return document.getElementById(id);
  }

  function renderAll() {
    renderLoop();
    renderUpdated();
    renderStale();
    renderRunFiles();
    renderBoard();
    renderPanel();
  }

  function init() {
    $('panel-close').addEventListener('click', closePanel);
    $('panel-view').addEventListener('click', onViewClick);
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && view.panel) closePanel();
    });
    initResize();
    restorePanelWidth();
    poll();
    setInterval(poll, POLL_MS);
  }

  init();
})();
