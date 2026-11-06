'use strict';

(() => {
  const BOARD_URL = '../state/board.json';
  const TASKS_URL = '../state/tasks/';
  const POLL_MS = 5000;
  const STAGES = ['planned', 'building', 'review', 'approval', 'escalated', 'done', 'abandoned'];
  const ROSTER = ['code', 'feature', 'security', 'privacy', 'api', 'ui'];

  let board = null;
  let boardJson = '';
  let selectedTask = null;
  let selectedFile = null;
  let docText = null;
  let markedReady = false;

  const $ = (id) => document.getElementById(id);

  function el(tag, cls, text) {
    const node = document.createElement(tag);
    if (cls) node.className = cls;
    if (text !== undefined && text !== null) node.textContent = text;
    return node;
  }

  function lower(value) {
    return String(value ?? '').toLowerCase();
  }

  function relTime(iso) {
    const t = Date.parse(iso);
    if (Number.isNaN(t)) return iso;
    const s = Math.max(0, Math.round((Date.now() - t) / 1000));
    if (s < 60) return `${s}s ago`;
    if (s < 3600) return `${Math.floor(s / 60)}m ago`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
    return new Date(t).toLocaleString();
  }

  function rosterOrder(keys) {
    const rank = (k) => (ROSTER.includes(k) ? ROSTER.indexOf(k) : ROSTER.length);
    return [...keys].sort((a, b) => rank(a) - rank(b));
  }

  // ---- Polling --------------------------------------------------------

  async function poll() {
    try {
      const res = await fetch(BOARD_URL, { cache: 'no-store' });
      if (!res.ok) {
        throw new Error(res.status === 404
          ? 'No board.json yet. Run /factory start <spec>.'
          : `Could not load board.json (HTTP ${res.status})`);
      }
      const text = await res.text();
      const next = JSON.parse(text);
      board = next;
      renderHeader(null);
      if (text !== boardJson) {
        boardJson = text;
        renderBoard();
      }
      if (selectedTask) await renderPanel();
    } catch (err) {
      renderHeader(err.message || String(err));
      if (!board) renderBoard();
    }
  }

  // ---- Header ---------------------------------------------------------

  function renderHeader(error) {
    const state = $('loop-state');
    const loop = (board && board.loop) || {};
    if (error) {
      state.textContent = 'offline';
      state.className = 'state offline';
      $('loop-message').textContent = error;
    } else {
      state.textContent = loop.state || 'unknown';
      state.className = `state ${lower(loop.state)}`;
      $('loop-message').textContent = loop.message || '';
    }
    $('loop-current').textContent = loop.current ? `current ${loop.current}` : '';
    $('updated').textContent = board && board.updated ? `updated ${relTime(board.updated)}` : '';
    document.title = loop.state ? `Factory · ${loop.state}` : 'Factory board';
  }

  // ---- Board ----------------------------------------------------------

  function alertOf(task) {
    if (task.status === 'Complete' || task.status === 'Abandoned') return null;
    const rounds = task.rounds || [];
    const last = rounds[rounds.length - 1];
    if (last && last.build === 'BLOCKED') return 'BLOCKED';
    if ((last && last.verdict === 'ESCALATE') || task.stage === 'escalated' || task.status === 'Escalated') {
      return 'ESCALATED';
    }
    return null;
  }

  function chip(label, value, extraClass) {
    const v = value === null || value === undefined ? '' : String(value);
    const node = el('span', `chip ${extraClass || lower(v)}`, v ? `${label} ${v}` : label);
    node.title = v ? `${label}: ${v}` : label;
    return node;
  }

  function renderRound(round) {
    const row = el('div', 'round');
    const label = el('span', 'round-label', round.id);
    if (round.kind && round.kind !== 'auto') {
      label.append(' ', el('span', 'kind', round.kind === 'approver-rework' ? 'approver rework' : 'human rework'));
    }
    row.append(label);
    if (round.build && round.build !== 'DONE') row.append(chip('build', round.build));
    const reviews = round.reviews || {};
    for (const key of rosterOrder(Object.keys(reviews))) row.append(chip(key, reviews[key]));
    if (round.verdict) row.append(chip('verdict', round.verdict));
    else row.append(chip('verdict', null, 'empty-verdict'));
    return row;
  }

  function renderCard(task) {
    const card = el('div', 'card');
    card.tabIndex = 0;
    card.setAttribute('role', 'button');
    card.dataset.id = task.id;
    const alert = alertOf(task);
    if (alert) card.classList.add('alert');
    if (board.loop && board.loop.current === task.id) card.classList.add('current');
    if (selectedTask === task.id) card.classList.add('selected');

    const id = el('div', 'card-id', task.id);
    if (alert) id.append(el('span', 'flag', alert));
    card.append(id, el('div', 'card-title', task.title || task.id));

    const rounds = el('div', 'rounds');
    for (const r of task.rounds || []) rounds.append(renderRound(r));
    card.append(rounds);

    const open = () => openPanel(task.id);
    card.addEventListener('click', open);
    card.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        open();
      }
    });
    return card;
  }

  function renderBoard() {
    const root = $('board');
    const tasks = (board && board.tasks) || [];
    const columns = STAGES.map((stage) => {
      const col = el('section', 'column');
      const items = tasks.filter((t) => (STAGES.includes(t.stage) ? t.stage : 'planned') === stage);
      const head = el('div', 'column-head');
      head.append(el('span', null, stage), el('span', null, String(items.length)));
      const body = el('div', 'column-body');
      if (items.length === 0) body.append(el('div', 'empty', '—'));
      for (const t of items) body.append(renderCard(t));
      col.append(head, body);
      return col;
    });
    root.replaceChildren(...columns);
  }

  // ---- Panel ----------------------------------------------------------

  function taskById(id) {
    return ((board && board.tasks) || []).find((t) => t.id === id) || null;
  }

  function fileUrl(taskId, file) {
    const path = String(file).split('/').map(encodeURIComponent).join('/');
    return `${TASKS_URL}${encodeURIComponent(taskId)}/${path}`;
  }

  function openPanel(taskId) {
    selectedTask = taskId;
    const task = taskById(taskId);
    const files = (task && task.files) || [];
    selectedFile = files.includes('task.md') ? 'task.md' : files[0] || null;
    docText = null;
    $('panel').hidden = false;
    for (const c of document.querySelectorAll('.card')) {
      c.classList.toggle('selected', c.dataset.id === taskId);
    }
    renderPanel();
  }

  function closePanel() {
    selectedTask = null;
    selectedFile = null;
    docText = null;
    $('panel').hidden = true;
    for (const c of document.querySelectorAll('.card.selected')) c.classList.remove('selected');
  }

  async function renderPanel() {
    const task = taskById(selectedTask);
    if (!task) {
      closePanel();
      return;
    }
    $('panel-id').textContent = task.id;
    $('panel-title').textContent = task.title || '';

    const files = task.files || [];
    const nav = $('panel-files');
    const buttons = files.map((f) => {
      const b = el('button', `file${f === selectedFile ? ' active' : ''}`, f);
      b.type = 'button';
      b.addEventListener('click', () => {
        selectedFile = f;
        docText = null;
        renderPanel();
      });
      return b;
    });
    nav.replaceChildren(...buttons);

    const doc = $('panel-doc');
    if (!selectedFile) {
      doc.replaceChildren(el('p', 'muted', 'No files yet.'));
      return;
    }
    const wanted = selectedFile;
    try {
      const res = await fetch(fileUrl(task.id, wanted), { cache: 'no-store' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const text = await res.text();
      if (wanted !== selectedFile || task.id !== selectedTask) return;
      if (text === docText) return;
      docText = text;
      const scroll = doc.scrollTop;
      doc.replaceChildren(renderMarkdown(text));
      doc.scrollTop = scroll;
    } catch (err) {
      if (wanted !== selectedFile) return;
      docText = null;
      doc.replaceChildren(el('p', 'muted', `Could not load ${wanted}: ${err.message || err}`));
    }
  }

  // ---- Markdown -------------------------------------------------------

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]));
  }

  function setupMarked() {
    if (markedReady) return true;
    const m = window.marked;
    if (!m || typeof m.parse !== 'function') return false;
    try {
      // Show raw HTML in documents as text instead of rendering it.
      m.use({
        gfm: true,
        renderer: {
          html(token) {
            return escapeHtml(typeof token === 'string' ? token : token.raw ?? token.text ?? '');
          },
        },
      });
    } catch (_) {
      return false;
    }
    markedReady = true;
    return true;
  }

  function sanitize(fragment) {
    for (const n of fragment.querySelectorAll('script, style, iframe, object, embed, form')) n.remove();
    for (const n of fragment.querySelectorAll('*')) {
      for (const attr of [...n.attributes]) {
        const name = attr.name.toLowerCase();
        const value = attr.value.trim().toLowerCase();
        if (name.startsWith('on')) n.removeAttribute(attr.name);
        else if ((name === 'href' || name === 'src') && /^(javascript|data|vbscript):/.test(value)) {
          n.removeAttribute(attr.name);
        }
      }
      if (n.tagName === 'A') {
        n.target = '_blank';
        n.rel = 'noopener noreferrer';
      }
    }
    return fragment;
  }

  function renderMarkdown(text) {
    if (setupMarked()) {
      try {
        const tpl = document.createElement('template');
        tpl.innerHTML = window.marked.parse(text);
        return sanitize(tpl.content);
      } catch (_) {
        // Fall through to preformatted text.
      }
    }
    const pre = el('pre', 'raw');
    pre.textContent = text;
    return pre;
  }

  // ---- Resizable panel ------------------------------------------------

  function setupResize() {
    const panel = $('panel');
    const handle = $('panel-resize');
    try {
      const saved = Number(localStorage.getItem('factory.panelWidth'));
      if (saved > 0) panel.style.width = `${Math.min(saved, window.innerWidth)}px`;
    } catch (_) {
      // Storage unavailable: keep the default width.
    }
    handle.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      handle.setPointerCapture(e.pointerId);
      handle.classList.add('dragging');
      const move = (ev) => {
        const width = Math.min(Math.max(window.innerWidth - ev.clientX, 280), window.innerWidth - 40);
        panel.style.width = `${width}px`;
      };
      const up = () => {
        handle.classList.remove('dragging');
        handle.removeEventListener('pointermove', move);
        handle.removeEventListener('pointerup', up);
        handle.removeEventListener('pointercancel', up);
        try {
          localStorage.setItem('factory.panelWidth', String(parseInt(panel.style.width, 10)));
        } catch (_) {
          // Storage unavailable: the width just isn't remembered.
        }
      };
      handle.addEventListener('pointermove', move);
      handle.addEventListener('pointerup', up);
      handle.addEventListener('pointercancel', up);
    });
  }

  // ---- Start ----------------------------------------------------------

  $('panel-close').addEventListener('click', closePanel);
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && selectedTask) closePanel();
  });
  setupResize();
  poll();
  setInterval(poll, POLL_MS);
})();
