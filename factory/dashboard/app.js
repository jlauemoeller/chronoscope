// Factory dashboard: polls ../state/board.json and renders a kanban board.
'use strict';

const BOARD_URL = '../state/board.json';
const TASKS_URL = '../state/tasks/';
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
const REVIEWERS = ['code', 'feature', 'ui'];

let board = null;          // last good board.json
let lastSignature = '';    // avoids re-rendering an unchanged board
let openTask = null;       // id of the task shown in the panel
let openFile = null;       // file shown in the panel

const $ = (id) => document.getElementById(id);

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value == null || value === false) continue;
    if (key === 'class') node.className = value;
    else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value);
  }
  for (const child of children.flat()) {
    if (child == null || child === false) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return node;
}

async function poll() {
  try {
    const res = await fetch(BOARD_URL, { cache: 'no-store' });
    if (!res.ok) throw new Error(`board.json: HTTP ${res.status}`);
    const text = await res.text();
    const next = JSON.parse(text);   // a half-written file throws; keep the last good board
    board = next;
    if (text !== lastSignature) {
      lastSignature = text;
      render();
    }
    setUpdated(false);
  } catch (err) {
    setUpdated(true, err.message);
    if (!board) renderEmpty(err.message);
  }
}

function setUpdated(stale, reason) {
  const node = $('updated');
  node.classList.toggle('stale', stale);
  const when = board && board.updated ? new Date(board.updated).toLocaleString() : '—';
  node.textContent = stale ? `stale (${reason}) · updated ${when}` : `updated ${when}`;
}

function renderEmpty(message) {
  $('loop-state').textContent = 'no board';
  $('loop-state').className = 'pill';
  $('loop-message').textContent = message;
  renderColumns([]);
}

function render() {
  const loop = board.loop || {};
  const state = loop.state || 'unknown';
  $('loop-state').textContent = state;
  $('loop-state').className = `pill ${state}`;
  $('loop-message').textContent = loop.message || '';
  $('loop-message').title = loop.message || '';
  renderColumns(board.tasks || []);
  if (openTask) {
    const task = findTask(openTask);
    if (task) renderPanelFiles(task); else closePanel();
  }
}

function renderColumns(tasks) {
  const current = board && board.loop ? board.loop.current : null;
  const columns = STAGES.map(([stage, label]) => {
    const inStage = tasks.filter((t) => t.stage === stage);
    const cards = inStage.length
      ? inStage.map((t) => renderCard(t, t.id === current))
      : [el('div', { class: 'empty' })];
    return el('section', { class: 'column' },
      el('div', { class: 'column-head' }, el('span', {}, label), el('span', {}, inStage.length || '')),
      el('div', { class: 'cards' }, cards));
  });
  $('board').replaceChildren(...columns);
}

function lastRound(task) {
  const rounds = task.rounds || [];
  return rounds[rounds.length - 1];
}

function renderCard(task, isCurrent) {
  const latest = lastRound(task);
  const blocked = latest && latest.build === 'BLOCKED';
  const escalated = task.stage === 'escalated' || (latest && latest.verdict === 'ESCALATE');
  const alert = blocked || escalated;
  const flag = alert ? (blocked ? 'BLOCKED' : 'ESCALATED') : null;

  return el('button', {
    class: ['card', isCurrent && 'current', alert && 'alert'].filter(Boolean).join(' '),
    type: 'button',
    onclick: () => openPanel(task.id),
  },
    el('div', { class: 'card-id' }, task.id),
    el('div', { class: 'card-title' }, task.title || task.id),
    flag && el('div', { class: 'flag' }, `▲ ${flag}`),
    (task.rounds || []).length > 0 && el('div', { class: 'rounds' }, task.rounds.map(renderRound)));
}

function renderRound(round) {
  const reviews = round.reviews || {};
  const chip = (label, value, extra = '') =>
    el('span', { class: `chip ${value || ''} ${extra}`.trim(), title: `${label}: ${value || '—'}` },
      value ? (extra === 'verdict' ? value : `${label} ${symbol(value)}`) : label);
  return el('div', { class: 'round' },
    el('span', { class: 'round-id', title: round.kind || '' },
      round.id.replace('round-', 'r'),
      round.kind && round.kind !== 'auto' ? el('span', { class: 'kind' }, round.kind === 'rework' ? ' H' : ' A') : null),
    el('span', { class: 'chips' },
      round.build === 'BLOCKED' ? chip('build', 'BLOCKED') : null,
      REVIEWERS.map((r) => chip(r, reviews[r])),
      chip('verdict', round.verdict, 'verdict')));
}

function symbol(value) {
  return { PASS: '✓', FAIL: '✗', pending: '…', DONE: '✓', BLOCKED: '✗' }[value] || value;
}

function findTask(id) {
  return board && (board.tasks || []).find((t) => t.id === id);
}

function openPanel(id) {
  const task = findTask(id);
  if (!task) return;
  if (openTask !== id) openFile = null;
  openTask = id;
  $('panel-id').textContent = task.id;
  $('panel-title').textContent = task.title || task.id;
  $('panel').hidden = false;
  renderPanelFiles(task);
  showFile(openFile || (task.files || [])[0] || null);
}

function closePanel() {
  openTask = null;
  openFile = null;
  $('panel').hidden = true;
}

function renderPanelFiles(task) {
  const buttons = (task.files || []).map((file) => el('button', {
    type: 'button',
    class: file === openFile ? 'active' : null,
    title: file,
    onclick: () => showFile(file),
  }, file));
  $('panel-files').replaceChildren(...(buttons.length ? buttons : [el('div', { class: 'muted', style: 'padding:6px 14px' }, 'No files yet')]));
}

async function showFile(file) {
  openFile = file;
  const task = findTask(openTask);
  if (task) renderPanelFiles(task);
  const doc = $('panel-doc');
  if (!file) { doc.replaceChildren(el('p', { class: 'muted' }, 'Select a file.')); return; }
  const url = TASKS_URL + encodeURIComponent(openTask) + '/' + file.split('/').map(encodeURIComponent).join('/');
  doc.replaceChildren(el('p', { class: 'muted' }, 'Loading…'));
  try {
    const res = await fetch(url, { cache: 'no-store' });
    if (!res.ok) {
      throw new Error(res.status === 404
        ? 'Not in the working tree (it may live on another branch).'
        : `HTTP ${res.status}`);
    }
    const text = await res.text();
    if (openFile !== file) return;   // another file was clicked meanwhile
    doc.replaceChildren(...renderText(file, text));
  } catch (err) {
    if (openFile === file) doc.replaceChildren(el('p', { class: 'muted' }, `${file}: ${err.message}`));
  }
}

function renderText(file, text) {
  if (file.endsWith('.md') && window.marked && window.DOMPurify) {
    const wrapper = el('div');
    wrapper.innerHTML = window.DOMPurify.sanitize(window.marked.parse(text));
    return [wrapper];
  }
  return [el('pre', {}, text)];
}

$('panel-close').addEventListener('click', closePanel);
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closePanel(); });

poll();
setInterval(poll, POLL_MS);
