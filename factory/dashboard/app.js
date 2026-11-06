// The factory dashboard: polls ../state/board.json and shows it as a kanban
// board, with a panel for a task's files and the run's own files.
'use strict';

(function () {
  const POLL_MS = 5000;
  const STAGES = [
    ['planned', 'Planned'],
    ['building', 'Building'],
    ['review', 'Review'],
    ['approval', 'Approval'],
    ['escalated', 'Escalated'],
    ['done', 'Done'],
    ['abandoned', 'Abandoned'],
  ];
  const RUN_FILES = ['SPEC.md', 'preflight.md', 'stack.md', 'backlog.md'];
  const MARKDOWN = /\.(md|markdown)$/i;
  const IMAGE = /\.(png|jpe?g|gif|svg|webp|avif|bmp)$/i;
  const STATE_BASE = new URL('../state/', window.location.href);
  const WIDTH_KEY = 'factory-dashboard-panel-width';

  const view = {
    loopState: document.getElementById('loop-state'),
    loopMessage: document.getElementById('loop-message'),
    runFiles: document.getElementById('run-files'),
    stale: document.getElementById('stale'),
    updated: document.getElementById('updated'),
    board: document.getElementById('board'),
    panel: document.getElementById('panel'),
    panelTitle: document.getElementById('panel-title'),
    panelFiles: document.getElementById('panel-files'),
    panelClose: document.getElementById('panel-close'),
    resize: document.getElementById('panel-resize'),
    viewerPath: document.getElementById('viewer-path'),
    viewerStatus: document.getElementById('viewer-status'),
    viewer: document.getElementById('viewer'),
  };

  const state = {
    board: null, // the last board fetched successfully
    staleReason: null, // why the last fetch failed, or null
    runFiles: [], // the run files that exist
    panel: null, // { kind: 'task', id } or { kind: 'run' }
    openPath: null, // the open file, relative to factory/state/
    openHash: '',
    openSignature: null, // what the open file looked like when last shown
    objectUrl: null, // the blob URL of an image being shown
  };

  // ---------------------------------------------------------------- helpers

  function h(tag, attrs, ...children) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs || {})) {
      if (value === null || value === undefined || value === false) continue;
      if (key === 'className') node.className = value;
      else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
      else node.setAttribute(key, value === true ? '' : value);
    }
    for (const child of children.flat()) {
      if (child === null || child === undefined || child === false) continue;
      node.append(child instanceof Node ? child : String(child));
    }
    return node;
  }

  function stateUrl(path) {
    return new URL(path.split('/').map(encodeURIComponent).join('/'), STATE_BASE);
  }

  async function fetchNoStore(url, init) {
    const response = await fetch(url, Object.assign({ cache: 'no-store' }, init));
    if (!response.ok) throw new Error('HTTP ' + response.status + ' for ' + url.pathname);
    return response;
  }

  function formatTime(iso) {
    const date = new Date(iso);
    return isNaN(date) ? String(iso || '') : date.toLocaleString();
  }

  function slugify(text) {
    return text.trim().toLowerCase().replace(/[^\p{L}\p{N}\s-]/gu, '').replace(/\s/g, '-');
  }

  function storageGet(key) {
    try { return window.localStorage.getItem(key); } catch (e) { return null; }
  }

  function storageSet(key, value) {
    try { window.localStorage.setItem(key, value); } catch (e) { /* not available */ }
  }

  // ---------------------------------------------------------------- polling

  async function fetchBoard() {
    try {
      const response = await fetchNoStore(new URL('board.json', STATE_BASE));
      const board = await response.json();
      if (!board || typeof board !== 'object') throw new Error('board.json is not an object');
      state.board = board;
      state.staleReason = null;
    } catch (error) {
      state.staleReason = error.message || String(error);
    }
  }

  async function fetchRunFiles() {
    const found = await Promise.all(RUN_FILES.map(async (name) => {
      try {
        await fetchNoStore(stateUrl(name), { method: 'HEAD' });
        return name;
      } catch (error) {
        return null;
      }
    }));
    state.runFiles = found.filter(Boolean);
  }

  async function poll() {
    await Promise.all([fetchBoard(), fetchRunFiles()]);
    renderHeader();
    renderBoard();
    if (state.panel) {
      if (!state.openPath && panelFiles().length) {
        const files = panelFiles();
        const first = files.find((file) => file.label === 'task.md') || files[0];
        openFile(first.path, '');
      } else {
        renderPanelFiles();
        await loadOpenFile(false);
      }
    }
    window.setTimeout(poll, POLL_MS);
  }

  // ---------------------------------------------------------------- header

  function renderHeader() {
    const loop = (state.board && state.board.loop) || {};
    const loopState = loop.state || (state.board ? 'unknown' : 'no board');
    view.loopState.textContent = loopState;
    view.loopState.className = 'state ' + String(loopState).replace(/\s/g, '-');
    view.loopMessage.textContent = loop.message || '';
    view.updated.textContent = state.board && state.board.updated
      ? 'updated ' + formatTime(state.board.updated) : '';
    renderStale();
    renderRunFileButtons();
  }

  function renderStale() {
    if (!state.staleReason) {
      view.stale.hidden = true;
      return;
    }
    view.stale.hidden = false;
    view.stale.textContent = state.board
      ? 'stale: ' + state.staleReason
      : 'no board: ' + state.staleReason;
  }

  function renderRunFileButtons() {
    view.runFiles.replaceChildren(...state.runFiles.map((name) =>
      h('button', { type: 'button', className: 'run-file', onclick: () => openRunPanel(name) }, name)));
  }

  // ---------------------------------------------------------------- board

  function tasks() {
    return (state.board && Array.isArray(state.board.tasks)) ? state.board.tasks : [];
  }

  function findTask(id) {
    return tasks().find((task) => task.id === id) || null;
  }

  function renderBoard() {
    if (!state.board) {
      view.board.replaceChildren(h('p', { className: 'notice' },
        'Waiting for factory/state/board.json. Start a run with /factory start <spec>.'));
      return;
    }
    view.board.replaceChildren(...STAGES.map(([stage, label]) => renderColumn(stage, label)));
  }

  function renderColumn(stage, label) {
    const cards = tasks().filter((task) => task.stage === stage);
    const list = h('ul', { className: 'cards' },
      cards.length ? cards.map(renderCard) : h('li', { className: 'empty-column' }, '—'));
    return h('section', { className: 'column', 'aria-label': label },
      h('div', { className: 'column-head' }, h('span', {}, label), h('span', {}, String(cards.length))),
      list);
  }

  function latestRound(task) {
    const rounds = Array.isArray(task.rounds) ? task.rounds : [];
    return rounds.length ? rounds[rounds.length - 1] : null;
  }

  function alertFlag(task) {
    const active = task.status === 'In progress' || task.status === 'Escalated';
    if (!active) return null;
    const latest = latestRound(task);
    if (latest && latest.build === 'BLOCKED') return 'BLOCKED';
    if (latest && latest.verdict === 'ESCALATE') return 'ESCALATE';
    if (task.status === 'Escalated') return 'ESCALATED';
    return null;
  }

  function renderCard(task) {
    const flag = alertFlag(task);
    const loop = state.board.loop || {};
    const classes = ['card'];
    if (flag) classes.push('alert');
    if (loop.current === task.id) classes.push('current');
    if (state.panel && state.panel.kind === 'task' && state.panel.id === task.id) classes.push('selected');
    const open = () => openTaskPanel(task.id);
    const rounds = Array.isArray(task.rounds) ? task.rounds : [];
    return h('li', {
      className: classes.join(' '),
      tabindex: '0',
      role: 'button',
      'aria-label': 'Open ' + task.id,
      onclick: open,
      onkeydown: (event) => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); open(); }
      },
    },
    h('div', { className: 'card-head' },
      h('span', { className: 'card-id' }, task.id),
      h('span', { className: 'card-status' }, task.status || '')),
    h('div', { className: 'card-title' }, task.title || task.id),
    flag ? h('div', { className: 'card-flag' }, flag) : null,
    rounds.map((round, index) => renderRound(round, index === rounds.length - 1)));
  }

  function renderRound(round, isLatest) {
    const label = round.kind && round.kind !== 'auto' ? round.id + ' · ' + round.kind : round.id;
    const chips = [];
    if (round.build && round.build !== 'DONE') {
      chips.push(chip('build ' + round.build, round.build === 'BLOCKED' ? 'fail' : ''));
    }
    for (const [key, value] of Object.entries(round.reviews || {})) {
      chips.push(chip(key + ' ' + value, resultClass(value)));
    }
    chips.push(verdictChip(round, isLatest));
    return h('div', { className: 'round' }, h('span', { className: 'round-id' }, label), chips);
  }

  function verdictChip(round, isLatest) {
    if (round.verdict) return chip('verdict ' + round.verdict, 'verdict ' + resultClass(round.verdict));
    if (round.build === 'BLOCKED' || !isLatest) return chip('verdict —', 'verdict none');
    return chip('verdict pending', 'verdict');
  }

  function resultClass(value) {
    if (value === 'PASS' || value === 'APPROVE') return 'pass';
    if (value === 'FAIL' || value === 'ESCALATE') return 'fail';
    if (value === 'REWORK') return 'warn';
    return '';
  }

  function chip(text, extra) {
    return h('span', { className: 'chip ' + (extra || '') }, text);
  }

  // ---------------------------------------------------------------- panel

  function openTaskPanel(id) {
    const task = findTask(id);
    const files = (task && task.files) || [];
    const first = files.includes('task.md') ? 'task.md' : files[0];
    state.panel = { kind: 'task', id: id };
    showPanel();
    openFile(first ? 'tasks/' + id + '/' + first : null, '');
  }

  function openRunPanel(name) {
    state.panel = { kind: 'run' };
    showPanel();
    openFile(name, '');
  }

  function showPanel() {
    view.panel.hidden = false;
    renderBoard();
  }

  function closePanel() {
    state.panel = null;
    state.openPath = null;
    state.openSignature = null;
    view.panel.hidden = true;
    view.viewer.replaceChildren();
    renderBoard();
  }

  function panelFiles() {
    if (!state.panel) return [];
    if (state.panel.kind === 'run') return state.runFiles.map((name) => ({ label: name, path: name }));
    const task = findTask(state.panel.id);
    const files = (task && Array.isArray(task.files)) ? task.files : [];
    return files.map((file) => ({ label: file, path: 'tasks/' + state.panel.id + '/' + file }));
  }

  function renderPanelFiles() {
    if (state.panel.kind === 'run') {
      view.panelTitle.textContent = 'Run files';
    } else {
      const task = findTask(state.panel.id);
      view.panelTitle.textContent = task ? task.id + ': ' + (task.title || '') : state.panel.id;
    }
    view.panelFiles.replaceChildren(...panelFiles().map((file) =>
      h('li', {}, h('button', {
        type: 'button',
        className: 'file' + (file.path === state.openPath ? ' active' : ''),
        onclick: () => openFile(file.path, ''),
      }, file.label))));
  }

  function openFile(path, hash) {
    state.openPath = path;
    state.openHash = hash || '';
    state.openSignature = null;
    renderPanelFiles();
    view.viewerPath.textContent = path || '';
    view.viewerStatus.textContent = '';
    view.viewer.replaceChildren(path ? h('p', {}, 'Loading…') : h('p', {}, 'No files yet.'));
    view.viewer.scrollTop = 0;
    if (path) loadOpenFile(true);
  }

  // Fetches the open file and shows it again only when it changed, keeping
  // the scroll position, so polling never makes the panel jump.
  async function loadOpenFile(isNew) {
    const path = state.openPath;
    if (!path) return;
    const url = stateUrl(path);
    try {
      const response = await fetchNoStore(url);
      const isImage = IMAGE.test(path);
      const body = isImage ? await response.blob() : await response.text();
      if (path !== state.openPath) return;
      const signature = isImage ? body.size + ':' + body.type : body;
      view.viewerStatus.textContent = '';
      if (!isNew && signature === state.openSignature) return;
      state.openSignature = signature;
      const scroll = view.viewer.scrollTop;
      view.viewer.replaceChildren(renderFile(path, body, url));
      if (isNew) scrollToHash(state.openHash);
      else view.viewer.scrollTop = scroll;
    } catch (error) {
      if (path !== state.openPath) return;
      view.viewerStatus.textContent = error.message || String(error);
      if (isNew) view.viewer.replaceChildren(h('p', {}, 'This file is not available.'));
    }
  }

  function renderFile(path, body, url) {
    if (state.objectUrl) URL.revokeObjectURL(state.objectUrl);
    state.objectUrl = null;
    if (IMAGE.test(path)) {
      state.objectUrl = URL.createObjectURL(body);
      return h('img', { src: state.objectUrl, alt: path });
    }
    if (MARKDOWN.test(path)) return renderMarkdown(body, url);
    return h('pre', { className: 'plain' }, body);
  }

  // ---------------------------------------------------------------- markdown

  let markdownParser = null;

  function parser() {
    if (markdownParser || !window.marked || !window.marked.Marked) return markdownParser;
    markdownParser = new window.marked.Marked({ gfm: true, async: false });
    // Raw HTML in a file is shown as text, never run.
    markdownParser.use({ renderer: { html: (token) => escapeHtml(token.text || token.raw || '') } });
    return markdownParser;
  }

  function escapeHtml(text) {
    return String(text).replace(/[&<>"']/g, (c) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
  }

  function renderMarkdown(text, fileUrl) {
    const md = parser();
    if (!md) return h('pre', { className: 'plain' }, text);
    let html;
    try {
      html = md.parse(text);
    } catch (error) {
      return h('pre', { className: 'plain' }, text);
    }
    const container = h('div', { className: 'markdown' });
    container.innerHTML = html;
    addHeadingIds(container);
    container.querySelectorAll('a[href]').forEach((link) => fixLink(link, fileUrl));
    container.querySelectorAll('img[src]').forEach((image) => fixImage(image, fileUrl));
    return container;
  }

  function addHeadingIds(container) {
    const seen = new Map();
    container.querySelectorAll('h1, h2, h3, h4, h5, h6').forEach((heading) => {
      const base = slugify(heading.textContent || '');
      const count = seen.get(base) || 0;
      seen.set(base, count + 1);
      heading.id = count ? base + '-' + count : base;
    });
  }

  // Links resolve against the file, not the page. A link to another file of
  // the run opens it in the panel; anything else opens in a new tab.
  function fixLink(link, fileUrl) {
    const raw = link.getAttribute('href');
    if (raw.startsWith('#')) {
      link.addEventListener('click', (event) => {
        event.preventDefault();
        scrollToHash(raw);
      });
      return;
    }
    let target;
    try { target = new URL(raw, fileUrl); } catch (error) { link.removeAttribute('href'); return; }
    if (target.protocol !== 'http:' && target.protocol !== 'https:') {
      link.removeAttribute('href');
      return;
    }
    link.href = target.href;
    if (target.origin === window.location.origin && target.pathname.startsWith(STATE_BASE.pathname)) {
      const path = decodeURIComponent(target.pathname.slice(STATE_BASE.pathname.length));
      link.addEventListener('click', (event) => {
        event.preventDefault();
        if (path === state.openPath) scrollToHash(target.hash);
        else openFile(path, target.hash);
      });
      return;
    }
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
  }

  function fixImage(image, fileUrl) {
    let source;
    try { source = new URL(image.getAttribute('src'), fileUrl); } catch (error) { image.remove(); return; }
    if (source.protocol !== 'http:' && source.protocol !== 'https:') { image.remove(); return; }
    image.src = source.href;
    image.loading = 'lazy';
  }

  function scrollToHash(hash) {
    if (!hash || hash === '#') return;
    let id = hash.slice(1);
    try { id = decodeURIComponent(id); } catch (error) { /* keep it as it is */ }
    const target = view.viewer.querySelector('[id="' + CSS.escape(id) + '"]');
    if (target) target.scrollIntoView({ block: 'start' });
  }

  // ---------------------------------------------------------------- resizing

  function applyWidth(width) {
    const clamped = Math.max(320, Math.min(width, window.innerWidth));
    view.panel.style.setProperty('--panel-width', clamped + 'px');
    return clamped;
  }

  function setUpResize() {
    const saved = Number(storageGet(WIDTH_KEY));
    if (saved) applyWidth(saved);
    view.resize.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      view.resize.setPointerCapture(event.pointerId);
      view.resize.classList.add('dragging');
    });
    view.resize.addEventListener('pointermove', (event) => {
      if (!view.resize.hasPointerCapture(event.pointerId)) return;
      applyWidth(window.innerWidth - event.clientX);
    });
    const stop = (event) => {
      if (!view.resize.hasPointerCapture(event.pointerId)) return;
      view.resize.releasePointerCapture(event.pointerId);
      view.resize.classList.remove('dragging');
      storageSet(WIDTH_KEY, String(Math.round(view.panel.getBoundingClientRect().width)));
    };
    view.resize.addEventListener('pointerup', stop);
    view.resize.addEventListener('pointercancel', stop);
    view.resize.addEventListener('keydown', (event) => {
      const step = event.key === 'ArrowLeft' ? 40 : event.key === 'ArrowRight' ? -40 : 0;
      if (!step) return;
      event.preventDefault();
      const width = applyWidth(view.panel.getBoundingClientRect().width + step);
      storageSet(WIDTH_KEY, String(Math.round(width)));
    });
  }

  // ---------------------------------------------------------------- start

  view.panelClose.addEventListener('click', closePanel);
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && state.panel) closePanel();
  });
  setUpResize();
  poll();
})();
