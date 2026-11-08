// Factory dashboard: polls ../state/board.json and shows the tasks as a
// kanban board, with a side panel for the files of a task or of the run.
// All text from the board and from files goes in through textContent; the
// only markup inserted is Markdown output that DOMPurify has sanitized.
'use strict';

(() => {
  const POLL_MS = 5000;
  const STAGES = ['planned', 'building', 'review', 'approval', 'escalated', 'done', 'abandoned'];
  const RUN_FILES = ['SPEC.md', 'preflight.md', 'stack.md', 'backlog.md'];
  const VISIBLE_ROUNDS = 3;
  const STATE_ROOT = new URL('../state/', location.href);
  const IMAGE_TYPES = {
    png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif',
    svg: 'image/svg+xml', webp: 'image/webp',
  };
  const MARKDOWN_EXT = new Set(['md', 'markdown']);
  const PANEL_MIN = 280;
  const PANEL_KEY = 'factory-dashboard-panel-width';

  const els = {
    loopState: document.getElementById('loop-state'),
    loopMessage: document.getElementById('loop-message'),
    runFiles: document.getElementById('run-files'),
    updated: document.getElementById('updated'),
    stale: document.getElementById('stale'),
    board: document.getElementById('board'),
    panel: document.getElementById('panel'),
    handle: document.getElementById('panel-handle'),
    panelContext: document.getElementById('panel-context'),
    panelPath: document.getElementById('panel-path'),
    panelClose: document.getElementById('panel-close'),
    fileList: document.getElementById('file-list'),
    viewer: document.getElementById('viewer'),
    note: document.getElementById('viewer-note'),
    content: document.getElementById('viewer-content'),
  };

  const state = {
    board: null,        // last board that parsed
    boardText: null,    // its raw text, to skip re-rendering an unchanged board
    fetchedAt: null,    // when that board was last confirmed
    stale: null,        // reason the latest fetch failed, or null
    runFiles: [],       // the RUN_FILES that exist
    panel: null,        // { context, path, hash, loaded } while the panel is open
    columns: new Map(), // stage -> { root, count, body }
    cards: new Map(),   // task id -> card element
    expandedRounds: new Set(), // task ids whose earlier rounds are shown
    openGroups: new Map(),     // file group key -> open (user's choice)
    fileListKey: null,         // what the file list was last built from
  };
  let loadSeq = 0;

  // ---------- helpers ----------

  function el(tag, props = {}, ...children) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(props)) {
      if (value === undefined || value === null || value === false) continue;
      if (key === 'className') node.className = value;
      else if (key === 'text') node.textContent = value;
      else node.setAttribute(key, value === true ? '' : String(value));
    }
    for (const child of children) {
      if (child === null || child === undefined) continue;
      node.append(child);
    }
    return node;
  }

  function stateUrl(path) {
    return new URL(path.split('/').map(encodeURIComponent).join('/'), STATE_ROOT);
  }

  function extension(path) {
    const name = path.split('/').pop();
    const dot = name.lastIndexOf('.');
    return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
  }

  function fileKind(path) {
    const ext = extension(path);
    if (MARKDOWN_EXT.has(ext)) return 'markdown';
    if (IMAGE_TYPES[ext]) return 'image';
    return 'text';
  }

  function str(value, fallback = '') {
    return value === undefined || value === null ? fallback : String(value);
  }

  function formatTime(date) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function formatStamp(value) {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? str(value) : date.toLocaleString();
  }

  function errorText(err) {
    return err && err.message ? err.message : String(err);
  }

  // A byte checksum, to tell whether an image changed between polls.
  function checksum(buffer) {
    const bytes = new Uint8Array(buffer);
    let hash = 0x811c9dc5;
    for (let i = 0; i < bytes.length; i++) {
      hash ^= bytes[i];
      hash = Math.imul(hash, 0x01000193);
    }
    return `${bytes.length}:${hash >>> 0}`;
  }

  // ---------- board data ----------

  function normalizeBoard(raw) {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
      throw new Error('the top level is not an object');
    }
    const loop = raw.loop && typeof raw.loop === 'object' ? raw.loop : {};
    const tasks = (Array.isArray(raw.tasks) ? raw.tasks : [])
      .filter((t) => t && typeof t === 'object')
      .map((t) => ({
        ...t,
        id: str(t.id, '(no id)'),
        rounds: (Array.isArray(t.rounds) ? t.rounds : []).filter((r) => r && typeof r === 'object'),
        files: (Array.isArray(t.files) ? t.files : []).filter((f) => typeof f === 'string'),
      }));
    return { updated: raw.updated, loop, tasks };
  }

  function findTask(id) {
    return state.board ? state.board.tasks.find((t) => t.id === id) : undefined;
  }

  function isActive(task) {
    return !['Complete', 'Abandoned'].includes(task.status)
      && !['done', 'abandoned'].includes(task.stage);
  }

  // Why an active task should stand out, or null.
  function alertReason(task) {
    if (!isActive(task)) return null;
    const last = task.rounds[task.rounds.length - 1];
    if (last && last.build === 'BLOCKED') return 'BLOCKED';
    if (last && last.verdict === 'ESCALATE') return 'ESCALATE';
    if (task.stage === 'escalated' || task.status === 'Escalated') return 'ESCALATED';
    return null;
  }

  // ---------- polling ----------

  async function poll() {
    try {
      await Promise.all([refreshBoard(), refreshRunFiles()]);
      renderHeader();
      if (state.panel) {
        renderPanelChrome();
        await loadFile();
      }
    } catch (err) {
      console.error('dashboard poll failed', err);
    } finally {
      setTimeout(poll, POLL_MS);
    }
  }

  async function refreshBoard() {
    let text;
    try {
      const res = await fetch(stateUrl('board.json'), { cache: 'no-store' });
      if (!res.ok) throw new Error(`board.json: HTTP ${res.status} ${res.statusText}`.trim());
      text = await res.text();
    } catch (err) {
      state.stale = err instanceof TypeError
        ? `cannot reach the server (${errorText(err)})`
        : errorText(err);
      return;
    }
    if (text !== state.boardText) {
      let board;
      try {
        board = normalizeBoard(JSON.parse(text));
      } catch (err) {
        state.stale = `board.json is not valid: ${errorText(err)}`;
        return;
      }
      state.board = board;
      state.boardText = text;
      renderBoard();
    }
    state.stale = null;
    state.fetchedAt = new Date();
  }

  async function refreshRunFiles() {
    const found = await Promise.all(RUN_FILES.map(async (name) => {
      try {
        const res = await fetch(stateUrl(name), { method: 'HEAD', cache: 'no-store' });
        return res.ok ? name : null;
      } catch {
        return undefined; // unreachable: keep what we knew
      }
    }));
    if (found.includes(undefined)) return;
    const names = found.filter(Boolean);
    if (names.join('\n') !== state.runFiles.join('\n')) {
      state.runFiles = names;
      renderRunFiles();
    }
  }

  // ---------- header ----------

  function loopClass(loopState) {
    if (loopState === 'running' || loopState === 'planning') return 's-running';
    if (loopState === 'complete') return 's-complete';
    if (loopState === 'escalated' || loopState === 'halted') return 's-alert';
    if (loopState.startsWith('awaiting')) return 's-wait';
    return '';
  }

  function renderHeader() {
    const loop = state.board ? state.board.loop : {};
    const loopState = state.board ? str(loop.state, 'unknown') : 'no board';
    els.loopState.textContent = loopState;
    els.loopState.className = `loop-state ${loopClass(loopState)}`;
    els.loopMessage.textContent = str(loop.message);

    const parts = [];
    if (state.board && state.board.updated) parts.push(`board updated ${formatStamp(state.board.updated)}`);
    if (state.fetchedAt) parts.push(`checked ${formatTime(state.fetchedAt)}`);
    els.updated.textContent = parts.join(' · ');

    if (state.stale) {
      els.stale.hidden = false;
      els.stale.textContent = state.board
        ? `Stale: ${state.stale}. Showing the last board read.`
        : `No board: ${state.stale}`;
    } else {
      els.stale.hidden = true;
      els.stale.textContent = '';
    }
    document.body.classList.toggle('is-stale', Boolean(state.stale && state.board));
  }

  function renderRunFiles() {
    const buttons = state.runFiles.map((name) => {
      const button = el('button', { type: 'button', className: 'run-file', text: name });
      button.dataset.path = name;
      button.addEventListener('click', () => openPanel({ kind: 'run' }, name));
      return button;
    });
    els.runFiles.replaceChildren(...buttons);
    markActiveRunFile();
  }

  function markActiveRunFile() {
    const path = state.panel ? state.panel.path : null;
    for (const button of els.runFiles.children) {
      button.classList.toggle('active', button.dataset.path === path);
    }
  }

  // ---------- board ----------

  function column(stage) {
    let col = state.columns.get(stage);
    if (col) return col;
    const count = el('span', { className: 'column-count' });
    const body = el('div', { className: 'column-body' });
    const root = el('section', { className: 'column', 'aria-label': stage },
      el('div', { className: 'column-head' }, el('span', { text: stage }), count), body);
    els.board.append(root);
    col = { root, count, body };
    state.columns.set(stage, col);
    return col;
  }

  function renderBoard() {
    const byStage = new Map(STAGES.map((s) => [s, []]));
    for (const task of state.board.tasks) {
      const stage = STAGES.includes(task.stage) ? task.stage : 'other';
      if (!byStage.has(stage)) byStage.set(stage, []);
      byStage.get(stage).push(task);
    }
    state.cards.clear();
    for (const [stage, tasks] of byStage) {
      const col = column(stage);
      col.count.textContent = String(tasks.length);
      const cards = tasks.map(renderCard);
      col.body.replaceChildren(...(cards.length ? cards : [el('div', { className: 'empty', text: 'none' })]));
      if (stage === 'other') col.root.hidden = tasks.length === 0;
    }
    markSelectedCard();
  }

  function renderCard(task) {
    const reason = alertReason(task);
    const loop = state.board.loop;
    const classes = ['card'];
    if (reason) classes.push('alert');
    if (task.stage === 'abandoned' || task.status === 'Abandoned') classes.push('inactive');

    const top = el('div', { className: 'card-top' },
      el('span', { className: 'card-id', text: task.id }),
      reason ? el('span', { className: 'badge', text: reason }) : null,
      isActive(task) && loop.current === task.id ? el('span', { className: 'badge current', text: 'current' }) : null);

    const card = el('article', { className: classes.join(' '), tabindex: '0', role: 'button' },
      top,
      el('div', { className: 'card-title', text: str(task.title, task.id) }),
      el('div', { className: 'card-status', text: str(task.status, 'no status') }),
      renderRounds(task));
    card.setAttribute('aria-label', `${task.id} ${str(task.title)}, ${str(task.status)}`);
    card.addEventListener('click', () => openTask(task.id));
    card.addEventListener('keydown', (ev) => {
      if (ev.target === card && (ev.key === 'Enter' || ev.key === ' ')) {
        ev.preventDefault();
        openTask(task.id);
      }
    });
    state.cards.set(task.id, card);
    return card;
  }

  function renderRounds(task) {
    const rounds = task.rounds;
    if (!rounds.length) return null;
    const rows = rounds.map((round, i) => renderRound(task, round, i === rounds.length - 1));
    const wrap = el('div', { className: 'rounds' });
    const hidden = rows.length - VISIBLE_ROUNDS;
    if (hidden > 0) {
      const summary = el('summary', { text: `${hidden} earlier round${hidden === 1 ? '' : 's'}` });
      summary.addEventListener('click', (ev) => ev.stopPropagation());
      const details = el('details', {}, summary, ...rows.slice(0, hidden));
      details.open = state.expandedRounds.has(task.id);
      details.addEventListener('toggle', () => {
        if (details.open) state.expandedRounds.add(task.id);
        else state.expandedRounds.delete(task.id);
      });
      wrap.append(details, ...rows.slice(hidden));
    } else {
      wrap.append(...rows);
    }
    return wrap;
  }

  function chip(label, value, tone) {
    return el('span', { className: `chip ${tone}`, text: `${label} ${value}` });
  }

  // BLOCKED is loud only where it is the task's current state.
  function buildChip(build, current) {
    const value = str(build, 'pending');
    const blocked = current ? 'blocked' : 'bad';
    const tone = value === 'BLOCKED' ? blocked : value === 'DONE' ? 'ok' : 'pending';
    return chip('build', value, tone);
  }

  function reviewChip(key, result) {
    const value = str(result, 'pending');
    const tone = value === 'PASS' ? 'ok' : value === 'FAIL' ? 'bad' : 'pending';
    return chip(key, value, tone);
  }

  function verdictChip(task, round, isLast) {
    const value = round.verdict;
    let node;
    if (value === 'APPROVE') node = chip('verdict', value, 'ok');
    else if (value === 'REWORK') node = chip('verdict', value, 'warn');
    else if (value === 'ESCALATE') node = chip('verdict', value, 'bad');
    else if (value !== null && value !== undefined) node = chip('verdict', str(value), '');
    else if (isLast && task.stage === 'approval') node = chip('verdict', 'pending', 'pending');
    else node = chip('verdict', '–', 'none');
    node.classList.add('verdict');
    return node;
  }

  function renderRound(task, round, isLast) {
    const reviews = round.reviews && typeof round.reviews === 'object' ? round.reviews : {};
    const label = [str(round.id, 'round'), str(round.kind)].filter(Boolean).join(' ');
    return el('div', { className: 'round' },
      el('span', { className: 'round-label', text: label }),
      buildChip(round.build, isLast && isActive(task)),
      ...Object.entries(reviews).map(([key, result]) => reviewChip(key, result)),
      verdictChip(task, round, isLast));
  }

  function markSelectedCard() {
    const ctx = state.panel ? state.panel.context : null;
    for (const [id, card] of state.cards) {
      card.classList.toggle('selected', Boolean(ctx && ctx.kind === 'task' && ctx.id === id));
    }
  }

  // ---------- panel ----------

  function openTask(id) {
    const task = findTask(id);
    const files = task ? task.files : [];
    const first = files.includes('task.md') ? 'task.md' : (files[0] || 'task.md');
    openPanel({ kind: 'task', id }, `tasks/${id}/${first}`);
  }

  function openPanel(context, path, hash = '') {
    releaseContent();
    state.panel = { context, path, hash, loaded: null };
    state.openGroups.delete(groupKey(context, path));
    els.panel.hidden = false;
    document.body.classList.add('panel-open');
    els.content.replaceChildren();
    els.note.hidden = true;
    els.viewer.scrollTop = 0;
    renderPanelChrome();
    loadFile();
  }

  function closePanel() {
    releaseContent();
    state.panel = null;
    state.fileListKey = null;
    loadSeq++;
    els.panel.hidden = true;
    document.body.classList.remove('panel-open');
    els.content.replaceChildren();
    markSelectedCard();
    markActiveRunFile();
  }

  function releaseContent() {
    const loaded = state.panel && state.panel.loaded;
    if (loaded && loaded.objectUrl) URL.revokeObjectURL(loaded.objectUrl);
  }

  function taskPrefix(ctx) {
    return ctx.kind === 'task' ? `tasks/${ctx.id}/` : '';
  }

  // The file-list group a path belongs to, e.g. tasks/<id>/round-2.
  function groupKey(context, path) {
    const prefix = taskPrefix(context);
    const rest = path.startsWith(prefix) ? path.slice(prefix.length) : path;
    const slash = rest.indexOf('/');
    return slash < 0 ? null : prefix + rest.slice(0, slash);
  }

  function renderPanelChrome() {
    const { context, path } = state.panel;
    if (context.kind === 'task') {
      const task = findTask(context.id);
      els.panelContext.textContent = task && task.title ? `${context.id} · ${task.title}` : context.id;
    } else {
      els.panelContext.textContent = 'Run files';
    }
    const prefix = taskPrefix(context);
    els.panelPath.textContent = prefix && path.startsWith(prefix) ? path.slice(prefix.length) : path;
    renderFileList();
    markSelectedCard();
    markActiveRunFile();
  }

  function panelFiles(context) {
    if (context.kind === 'run') return state.runFiles.slice();
    const task = findTask(context.id);
    const files = task ? task.files.slice() : [];
    const i = files.indexOf('task.md');
    if (i > 0) files.unshift(...files.splice(i, 1));
    return files;
  }

  function fileButton(label, path, topLevel) {
    const classes = ['file'];
    if (topLevel) classes.push('top-level');
    if (state.panel.path === path) classes.push('active');
    const button = el('button', { type: 'button', className: classes.join(' '), text: label, title: path });
    button.addEventListener('click', () => {
      if (state.panel && state.panel.path === path) return;
      openPanel(state.panel.context, path);
    });
    return button;
  }

  function renderFileList() {
    const { context, path: active } = state.panel;
    const prefix = taskPrefix(context);
    const files = panelFiles(context);

    const groups = new Map(); // directory -> [file]
    const nodes = [];
    for (const file of files) {
      const slash = file.indexOf('/');
      if (slash < 0) {
        nodes.push({ file });
        continue;
      }
      const dir = file.slice(0, slash);
      if (!groups.has(dir)) {
        groups.set(dir, []);
        nodes.push({ dir });
      }
      groups.get(dir).push(file);
    }
    const dirs = [...groups.keys()];
    const lastDir = dirs[dirs.length - 1];
    const activeRel = active.startsWith(prefix) ? active.slice(prefix.length) : null;
    const activeDir = activeRel && activeRel.includes('/') ? activeRel.slice(0, activeRel.indexOf('/')) : null;

    const listKey = [prefix, active, ...files].join('\n');
    if (listKey === state.fileListKey) return;
    state.fileListKey = listKey;

    const children = nodes.map((node) => {
      if (node.file) return fileButton(node.file, prefix + node.file, true);
      const key = `${prefix}${node.dir}`;
      const summary = el('summary', { text: `${node.dir}/` });
      const details = el('details', { className: 'file-group' }, summary,
        ...groups.get(node.dir).map((file) => fileButton(file.slice(node.dir.length + 1), prefix + file, false)));
      const chosen = state.openGroups.get(key);
      details.open = chosen !== undefined ? chosen : (node.dir === activeDir || node.dir === lastDir);
      details.addEventListener('toggle', () => state.openGroups.set(key, details.open));
      return details;
    });
    if (!children.length) {
      children.push(el('div', { className: 'empty', text: 'No files listed.' }));
    }
    const scroll = els.fileList.scrollTop;
    els.fileList.replaceChildren(...children);
    els.fileList.scrollTop = scroll;
  }

  function showNote(text) {
    els.note.textContent = text;
    els.note.hidden = false;
  }

  // Fetches the open file and shows it if it changed since it was last shown.
  async function loadFile() {
    const panel = state.panel;
    if (!panel) return;
    const seq = ++loadSeq;
    const { path } = panel;
    const url = stateUrl(path);
    const kind = fileKind(path);
    let payload;
    try {
      const res = await fetch(url, { cache: 'no-store' });
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`.trim());
      payload = kind === 'image' ? await res.arrayBuffer() : await res.text();
    } catch (err) {
      if (seq !== loadSeq) return;
      const shown = panel.loaded ? ' Showing the last version read.' : '';
      showNote(`Could not load ${path}: ${errorText(err)}.${shown}`);
      return;
    }
    if (seq !== loadSeq || state.panel !== panel) return;
    els.note.hidden = true;

    const key = kind === 'image' ? checksum(payload) : payload;
    if (panel.loaded && panel.loaded.key === key) return;

    const refresh = Boolean(panel.loaded);
    const scroll = els.viewer.scrollTop;
    releaseContent();
    let objectUrl = null;
    let node;
    if (kind === 'image') {
      objectUrl = URL.createObjectURL(new Blob([payload], { type: IMAGE_TYPES[extension(path)] }));
      node = el('img', { className: 'whole', src: objectUrl, alt: path });
    } else if (kind === 'markdown') {
      node = renderMarkdown(payload, url) || renderText(payload);
    } else {
      node = renderText(payload);
    }
    els.content.replaceChildren(node);
    panel.loaded = { key, objectUrl };
    if (refresh) {
      els.viewer.scrollTop = scroll;
    } else if (panel.hash) {
      scrollToAnchor(panel.hash);
    }
  }

  function renderText(text) {
    return el('pre', { className: 'text', text });
  }

  // ---------- Markdown ----------

  function renderMarkdown(text, fileUrl) {
    const { marked, DOMPurify } = window;
    if (!marked || typeof marked.parse !== 'function' || !DOMPurify || typeof DOMPurify.sanitize !== 'function') {
      return null;
    }
    let fragment;
    try {
      const html = marked.parse(text, { gfm: true, async: false });
      if (typeof html !== 'string') return null;
      fragment = DOMPurify.sanitize(html, { RETURN_DOM_FRAGMENT: true });
    } catch (err) {
      console.error('Markdown rendering failed', err);
      return null;
    }
    const root = el('div', { className: 'md' });
    root.append(fragment);
    moveAnchors(root);
    nameHeadings(root);
    rewriteUrls(root, fileUrl);
    return root;
  }

  // Ids from a file must not clash with the page's own, so anchors live in
  // data-anchor and are looked up inside the viewer only.
  function moveAnchors(root) {
    for (const node of root.querySelectorAll('[id]')) {
      node.dataset.anchor = node.getAttribute('id');
      node.removeAttribute('id');
    }
    for (const node of root.querySelectorAll('a[name]')) {
      if (!node.dataset.anchor) node.dataset.anchor = node.getAttribute('name');
      node.removeAttribute('name');
    }
  }

  // GitHub-style heading slugs, so links like #some-heading work.
  function nameHeadings(root) {
    const seen = new Map();
    for (const heading of root.querySelectorAll('h1, h2, h3, h4, h5, h6')) {
      if (heading.dataset.anchor) continue;
      const base = heading.textContent.trim().toLowerCase()
        .replace(/[^\p{L}\p{N}\s_-]/gu, '')
        .replace(/\s/g, '-');
      const n = seen.get(base) || 0;
      seen.set(base, n + 1);
      heading.dataset.anchor = n ? `${base}-${n}` : base;
    }
  }

  function resolveUrl(raw, base) {
    try {
      return new URL(raw, base);
    } catch {
      return null;
    }
  }

  // The path of a URL inside the run's state directory, or null if it is
  // elsewhere or names a directory.
  function runPath(url) {
    if (url.origin !== location.origin || !url.pathname.startsWith(STATE_ROOT.pathname)) return null;
    const rel = url.pathname.slice(STATE_ROOT.pathname.length);
    if (!rel || rel.endsWith('/')) return null;
    try {
      return rel.split('/').map(decodeURIComponent).join('/');
    } catch {
      return null;
    }
  }

  // Relative URLs in a file are relative to that file, not to this page.
  function rewriteUrls(root, fileUrl) {
    for (const attr of ['src', 'poster']) {
      for (const node of root.querySelectorAll(`[${attr}]`)) {
        const abs = resolveUrl(node.getAttribute(attr), fileUrl);
        if (abs) node.setAttribute(attr, abs.href);
        else node.removeAttribute(attr);
      }
    }
    for (const node of root.querySelectorAll('[srcset]')) node.removeAttribute('srcset');
    for (const link of root.querySelectorAll('a[href], area[href]')) wireLink(link, fileUrl);
  }

  function wireLink(link, fileUrl) {
    const abs = resolveUrl(link.getAttribute('href'), fileUrl);
    if (!abs) {
      link.removeAttribute('href');
      return;
    }
    link.setAttribute('href', abs.href);
    const path = runPath(abs);
    if (path === null) {
      link.setAttribute('target', '_blank');
      link.setAttribute('rel', 'noopener noreferrer');
      return;
    }
    link.addEventListener('click', (ev) => {
      if (ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey) return;
      ev.preventDefault();
      followRunLink(path, abs.hash);
    });
  }

  function followRunLink(path, hash) {
    const panel = state.panel;
    if (panel && panel.path === path) {
      if (hash) scrollToAnchor(hash);
      else els.viewer.scrollTop = 0;
      return;
    }
    const match = /^tasks\/([^/]+)\//.exec(path);
    openPanel(match ? { kind: 'task', id: match[1] } : { kind: 'run' }, path, hash);
  }

  function scrollToAnchor(hash) {
    let name = hash.replace(/^#/, '');
    try {
      name = decodeURIComponent(name);
    } catch {
      // keep the raw name
    }
    const target = [...els.content.querySelectorAll('[data-anchor]')]
      .find((node) => node.dataset.anchor === name || node.dataset.anchor === name.toLowerCase());
    if (!target) return;
    target.scrollIntoView({ block: 'start' });
    target.classList.add('anchor-target');
    setTimeout(() => target.classList.remove('anchor-target'), 1500);
  }

  // ---------- resizing ----------

  function setPanelWidth(px) {
    const max = Math.max(PANEL_MIN, window.innerWidth * 0.9);
    const width = Math.round(Math.min(max, Math.max(PANEL_MIN, px)));
    document.documentElement.style.setProperty('--panel-w', `${width}px`);
    return width;
  }

  function savePanelWidth(width) {
    try {
      localStorage.setItem(PANEL_KEY, String(width));
    } catch {
      // storage unavailable: the width just isn't remembered
    }
  }

  function currentPanelWidth() {
    return els.panel.getBoundingClientRect().width;
  }

  function setUpResizing() {
    try {
      const saved = Number(localStorage.getItem(PANEL_KEY));
      if (saved > 0) setPanelWidth(saved);
    } catch {
      // ignore
    }
    let dragging = false;
    let grab = 0; // pointer offset from the panel's left edge
    els.handle.addEventListener('pointerdown', (ev) => {
      if (ev.button !== 0) return;
      dragging = true;
      grab = ev.clientX - els.panel.getBoundingClientRect().left;
      els.handle.setPointerCapture(ev.pointerId);
      els.handle.classList.add('dragging');
      document.body.classList.add('resizing');
      ev.preventDefault();
    });
    els.handle.addEventListener('pointermove', (ev) => {
      if (dragging) setPanelWidth(window.innerWidth - (ev.clientX - grab));
    });
    const stop = () => {
      if (!dragging) return;
      dragging = false;
      els.handle.classList.remove('dragging');
      document.body.classList.remove('resizing');
      savePanelWidth(currentPanelWidth());
    };
    els.handle.addEventListener('pointerup', stop);
    els.handle.addEventListener('pointercancel', stop);
    els.handle.addEventListener('keydown', (ev) => {
      const step = ev.key === 'ArrowLeft' ? 32 : ev.key === 'ArrowRight' ? -32 : 0;
      if (!step) return;
      ev.preventDefault();
      savePanelWidth(setPanelWidth(currentPanelWidth() + step));
    });
  }

  // ---------- start ----------

  function start() {
    for (const stage of STAGES) column(stage).body.append(el('div', { className: 'empty', text: 'none' }));
    els.panelClose.addEventListener('click', closePanel);
    document.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape' && state.panel) closePanel();
    });
    setUpResizing();
    poll();
  }

  start();
})();
