// Factory dashboard: polls ../state/board.json and renders it as a kanban board.
'use strict';

const BOARD_URL = '../state/board.json';
const STATE_URL = '../state/';
const POLL_MS = 5000;
const STAGES = ['planned', 'building', 'review', 'approval', 'escalated', 'done', 'abandoned'];
const KIND_LABELS = { auto: 'auto', 'approver-rework': 'approver rework', rework: 'human rework' };
const IMAGE_RE = /\.(png|jpe?g|gif|svg|webp)$/i;
const WIDTH_KEY = 'factory-dashboard-panel-width';

const ui = {
  board: document.getElementById('board'),
  loopState: document.getElementById('loop-state'),
  loopMessage: document.getElementById('loop-message'),
  runFiles: document.getElementById('run-files'),
  updated: document.getElementById('updated'),
  panel: document.getElementById('panel'),
  handle: document.getElementById('panel-handle'),
  panelTitle: document.getElementById('panel-title'),
  panelSubtitle: document.getElementById('panel-subtitle'),
  panelFiles: document.getElementById('panel-files'),
  panelView: document.getElementById('panel-view'),
  panelClose: document.getElementById('panel-close'),
};

const view = {
  boardText: null,
  board: null,
  // What the panel shows: { type: 'task', id } or { type: 'run' }, or null.
  selection: null,
  // The file open in the panel: { url, name, text }.
  file: null,
};

// ---------- helpers ----------

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined && text !== null) node.textContent = String(text);
  return node;
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function encodePath(path) {
  return path.split('/').map(encodeURIComponent).join('/');
}

function taskFileUrl(taskId, path) {
  return `${STATE_URL}tasks/${encodeURIComponent(taskId)}/${encodePath(path)}`;
}

function runFileUrl(name) {
  return STATE_URL + encodePath(name);
}

function formatTime(iso) {
  if (!iso) return '';
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? String(iso) : date.toLocaleString();
}

function latestRound(task) {
  const rounds = Array.isArray(task.rounds) ? task.rounds : [];
  return rounds.length ? rounds[rounds.length - 1] : null;
}

function alertFor(task) {
  const latest = latestRound(task);
  if (latest && latest.build === 'BLOCKED') return 'BLOCKED';
  if (task.status === 'Escalated' || task.stage === 'escalated') return 'ESCALATED';
  if (latest && latest.verdict === 'ESCALATE') return 'ESCALATED';
  return null;
}

// ---------- header ----------

function renderHeader(board) {
  const loop = (board && board.loop) || {};
  const state = loop.state || 'unknown';
  ui.loopState.textContent = state;
  ui.loopState.className = `badge state-${state.replace(/[^a-z-]/g, '')}`;
  ui.loopMessage.textContent = loop.message || '';
  ui.updated.textContent = board && board.updated ? `Updated ${formatTime(board.updated)}` : '';

  ui.runFiles.replaceChildren();
  const files = board && Array.isArray(board.files) ? board.files : [];
  for (const name of files) {
    const button = el('button', 'file-button', name);
    button.type = 'button';
    if (view.selection && view.selection.type === 'run' && view.file && view.file.name === name) {
      button.classList.add('active');
    }
    button.addEventListener('click', () => openRunFile(name));
    ui.runFiles.append(button);
  }
}

function renderNoBoard(reason) {
  ui.loopState.textContent = 'no board';
  ui.loopState.className = 'badge state-none';
  ui.loopMessage.textContent = reason;
  ui.updated.textContent = '';
  ui.runFiles.replaceChildren();
  ui.board.replaceChildren(...STAGES.map((stage) => renderColumn(stage, [])));
}

// ---------- board ----------

function renderChip(label, className, title) {
  const chip = el('span', `chip ${className}`, label);
  if (title) chip.title = title;
  return chip;
}

function renderRound(round) {
  const row = el('div', 'round');
  const label = el('span', 'round-label');
  label.append(el('span', null, round.id || ''), document.createTextNode(' '));
  label.append(el('span', 'round-kind', KIND_LABELS[round.kind] || round.kind || ''));
  row.append(label);

  const build = round.build || 'pending';
  if (build === 'BLOCKED') {
    row.append(renderChip('BLOCKED', 'build-blocked', 'Builder blocked: reviewers skipped'));
  } else if (build === 'DONE') {
    row.append(renderChip('build', 'build-done', 'Build: DONE'));
  } else {
    row.append(renderChip('build', 'pending', 'Build: pending'));
  }

  const reviews = round.reviews && typeof round.reviews === 'object' ? round.reviews : {};
  for (const [key, value] of Object.entries(reviews)) {
    if (build === 'BLOCKED') {
      row.append(renderChip(key, 'skipped', `${key}: skipped (builder blocked)`));
    } else if (value === 'PASS') {
      row.append(renderChip(key, 'pass', `${key}: PASS`));
    } else if (value === 'FAIL') {
      row.append(renderChip(key, 'fail', `${key}: FAIL`));
    } else {
      row.append(renderChip(key, 'pending', `${key}: pending`));
    }
  }

  const verdict = round.verdict;
  if (verdict) {
    row.append(renderChip(verdict, `verdict-${verdict.toLowerCase()}`, `Verdict: ${verdict}`));
  } else {
    row.append(renderChip('verdict', build === 'BLOCKED' ? 'skipped' : 'pending', 'Verdict: none yet'));
  }
  return row;
}

function renderCard(task, board) {
  const card = el('div', 'card');
  card.tabIndex = 0;
  card.setAttribute('role', 'button');
  card.dataset.id = task.id;
  const alert = alertFor(task);
  if (alert) card.classList.add('alert');
  if (board.loop && board.loop.current === task.id) card.classList.add('current');
  if (view.selection && view.selection.type === 'task' && view.selection.id === task.id) {
    card.classList.add('selected');
  }

  card.append(el('div', 'card-id mono', task.id));
  card.append(el('div', 'card-title', task.title || task.id));
  const meta = el('div', 'card-meta');
  meta.append(el('span', null, task.status || ''));
  if (alert) meta.append(el('span', 'badge alert-badge', alert));
  card.append(meta);

  const rounds = Array.isArray(task.rounds) ? task.rounds : [];
  if (rounds.length) {
    const list = el('div', 'rounds');
    for (const round of rounds) list.append(renderRound(round));
    card.append(list);
  }

  const open = () => openTask(task.id);
  card.addEventListener('click', open);
  card.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      open();
    }
  });
  return card;
}

function renderColumn(stage, cards) {
  const column = el('section', 'column');
  const head = el('div', 'column-head');
  head.append(el('span', null, stage), el('span', null, cards.length));
  column.append(head);
  const list = el('div', 'cards');
  if (cards.length) list.append(...cards);
  else list.append(el('div', 'empty', '—'));
  column.append(list);
  return column;
}

function renderBoard(board) {
  const tasks = Array.isArray(board.tasks) ? board.tasks : [];
  const byStage = new Map(STAGES.map((stage) => [stage, []]));
  for (const task of tasks) {
    const stage = byStage.has(task.stage) ? task.stage : 'planned';
    byStage.get(stage).push(renderCard(task, board));
  }
  ui.board.replaceChildren(...STAGES.map((stage) => renderColumn(stage, byStage.get(stage))));
}

// ---------- panel ----------

function findTask(id) {
  const tasks = view.board && Array.isArray(view.board.tasks) ? view.board.tasks : [];
  return tasks.find((task) => task.id === id) || null;
}

function panelEntries() {
  if (!view.selection) return [];
  if (view.selection.type === 'run') {
    const files = view.board && Array.isArray(view.board.files) ? view.board.files : [];
    return files.map((name) => ({ name, url: runFileUrl(name) }));
  }
  const task = findTask(view.selection.id);
  const files = task && Array.isArray(task.files) ? task.files : [];
  return files.map((name) => ({ name, url: taskFileUrl(view.selection.id, name) }));
}

function renderPanelHead() {
  if (!view.selection) return;
  if (view.selection.type === 'run') {
    ui.panelTitle.textContent = 'Run files';
    ui.panelSubtitle.textContent = 'factory/state/';
    return;
  }
  const task = findTask(view.selection.id);
  ui.panelTitle.textContent = task ? `${task.id} · ${task.title || ''}` : view.selection.id;
  ui.panelSubtitle.textContent = task
    ? [task.status, task.stage, task.branch].filter(Boolean).join(' · ')
    : 'No longer on the board';
}

function renderPanelFiles() {
  const entries = panelEntries();
  ui.panelFiles.replaceChildren();
  if (!entries.length) {
    ui.panelFiles.append(el('li', 'panel-note', 'No files yet.'));
    return;
  }
  for (const entry of entries) {
    const item = el('li');
    const button = el('button', 'file-button mono', entry.name);
    button.type = 'button';
    if (view.file && view.file.url === entry.url) button.classList.add('active');
    button.addEventListener('click', () => showFile(entry.name, entry.url));
    item.append(button);
    ui.panelFiles.append(item);
  }
}

function openPanel(selection) {
  view.selection = selection;
  ui.panel.hidden = false;
  renderPanelHead();
  renderPanelFiles();
  if (view.board) renderAll(view.board);
}

function openTask(id) {
  const sameTask = view.selection && view.selection.type === 'task' && view.selection.id === id;
  if (!sameTask) {
    view.file = null;
    ui.panelView.replaceChildren(el('p', 'panel-note', 'Choose a file.'));
  }
  openPanel({ type: 'task', id });
  if (!sameTask) {
    const task = findTask(id);
    if (task && Array.isArray(task.files) && task.files.includes('task.md')) {
      showFile('task.md', taskFileUrl(id, 'task.md'));
    }
  }
}

function openRunFile(name) {
  openPanel({ type: 'run' });
  showFile(name, runFileUrl(name));
}

function closePanel() {
  view.selection = null;
  view.file = null;
  ui.panel.hidden = true;
  ui.panelView.replaceChildren();
  if (view.board) renderAll(view.board);
}

let markedReady = false;

function setupMarked() {
  if (markedReady || !window.marked || typeof window.marked.parse !== 'function') return markedReady;
  window.marked.use({
    renderer: {
      // Raw HTML in a Markdown file is shown as text, never run.
      html(token) {
        const text = typeof token === 'string' ? token : (token && token.text) || '';
        return escapeHtml(text);
      },
    },
  });
  markedReady = true;
  return true;
}

function safeUrl(value, base) {
  try {
    const url = new URL(value, base);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url : null;
  } catch (error) {
    return null;
  }
}

// Resolve links and images against the file, not the dashboard page. Relative
// links open in the panel; anything that isn't http(s) is dropped.
function fixLinks(container, fileUrl) {
  const base = new URL(fileUrl, window.location.href);
  for (const img of container.querySelectorAll('img')) {
    const url = safeUrl(img.getAttribute('src') || '', base);
    if (url) img.src = url.href;
    else img.removeAttribute('src');
  }
  for (const link of container.querySelectorAll('a[href]')) {
    const raw = link.getAttribute('href');
    if (raw.startsWith('#')) continue;
    const url = safeUrl(raw, base);
    if (!url) {
      link.removeAttribute('href');
      continue;
    }
    link.href = url.href;
    if (url.origin === window.location.origin) {
      link.addEventListener('click', (event) => {
        event.preventDefault();
        const name = decodeURIComponent(url.pathname.split('/').pop() || url.pathname);
        showFile(name, url.pathname + url.search);
      });
    } else {
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
    }
  }
}

function renderFileContent(name, url, text) {
  if (/\.md$/i.test(name) && setupMarked()) {
    const container = el('div', 'markdown');
    try {
      container.innerHTML = window.marked.parse(text);
      fixLinks(container, url);
      ui.panelView.replaceChildren(container);
      return;
    } catch (error) {
      // fall through to plain text
    }
  }
  ui.panelView.replaceChildren(el('pre', null, text));
}

async function showFile(name, url, refresh = false) {
  const opened = !refresh;
  if (opened) {
    view.file = { name, url, text: null };
    renderPanelFiles();
    if (view.board) renderHeader(view.board);
  }
  if (IMAGE_RE.test(name)) {
    if (opened) {
      const img = el('img');
      img.alt = name;
      img.src = `${url}${url.includes('?') ? '&' : '?'}t=${Date.now()}`;
      ui.panelView.replaceChildren(img);
    }
    return;
  }
  let text;
  try {
    const response = await fetch(url, { cache: 'no-store' });
    if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
    text = await response.text();
  } catch (error) {
    if (view.file && view.file.url === url && view.file.text !== '\u0000missing') {
      view.file.text = '\u0000missing';
      ui.panelView.replaceChildren(el('p', 'panel-note', `Can't load ${name}: ${error.message}`));
    }
    return;
  }
  if (!view.file || view.file.url !== url || view.file.text === text) return;
  view.file.text = text;
  renderFileContent(name, url, text);
}

// ---------- resizing ----------

function setPanelWidth(width) {
  const clamped = Math.max(280, Math.min(width, window.innerWidth - 60));
  document.documentElement.style.setProperty('--panel-width', `${Math.round(clamped)}px`);
  return clamped;
}

function restorePanelWidth() {
  try {
    const saved = Number(window.localStorage.getItem(WIDTH_KEY));
    if (saved > 0) setPanelWidth(saved);
  } catch (error) {
    // storage unavailable: keep the default width
  }
}

function setupResize() {
  let dragging = false;
  let width = null;
  ui.handle.addEventListener('pointerdown', (event) => {
    dragging = true;
    ui.handle.classList.add('dragging');
    ui.handle.setPointerCapture(event.pointerId);
    event.preventDefault();
  });
  ui.handle.addEventListener('pointermove', (event) => {
    if (!dragging) return;
    width = setPanelWidth(window.innerWidth - event.clientX);
  });
  const stop = () => {
    if (!dragging) return;
    dragging = false;
    ui.handle.classList.remove('dragging');
    if (width !== null) {
      try {
        window.localStorage.setItem(WIDTH_KEY, String(Math.round(width)));
      } catch (error) {
        // storage unavailable: the width lasts for this page only
      }
    }
  };
  ui.handle.addEventListener('pointerup', stop);
  ui.handle.addEventListener('pointercancel', stop);
}

// ---------- polling ----------

function renderAll(board) {
  renderHeader(board);
  renderBoard(board);
}

async function poll() {
  let text;
  try {
    const response = await fetch(BOARD_URL, { cache: 'no-store' });
    if (!response.ok) throw new Error(response.status === 404 ? 'No board yet' : `HTTP ${response.status}`);
    text = await response.text();
  } catch (error) {
    view.boardText = null;
    view.board = null;
    renderNoBoard(`${error.message}: waiting for factory/state/board.json`);
    return;
  }
  if (text !== view.boardText) {
    let board;
    try {
      board = JSON.parse(text);
      if (!board || typeof board !== 'object') throw new Error('not an object');
    } catch (error) {
      renderNoBoard(`board.json is not valid JSON (${error.message})`);
      return;
    }
    view.boardText = text;
    view.board = board;
    renderAll(board);
    if (view.selection) {
      renderPanelHead();
      renderPanelFiles();
    }
  }
  if (view.selection && view.file) {
    showFile(view.file.name, view.file.url, true);
  }
}

function start() {
  restorePanelWidth();
  setupResize();
  ui.panelClose.addEventListener('click', closePanel);
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && !ui.panel.hidden) closePanel();
  });
  renderNoBoard('Loading…');
  poll();
  window.setInterval(poll, POLL_MS);
}

start();
