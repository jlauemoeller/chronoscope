// Factory stats: the side panel that shows an archived task's files, read
// from ../archive/cycle-<n>/tasks/<task-id>/. It works like the dashboard's
// panel, without polling, since the archive does not change.
// All text from files goes in through textContent; the only markup inserted
// is Markdown output that DOMPurify has sanitized.
'use strict';

window.FactoryPanel = (() => {
  const ARCHIVE_ROOT = new URL('../archive/', location.href);
  const IMAGE_TYPES = {
    png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif',
    svg: 'image/svg+xml', webp: 'image/webp',
  };
  const MARKDOWN_EXT = new Set(['md', 'markdown']);
  const PANEL_MIN = 280;
  const PANEL_KEY = 'factory-stats-panel-width';
  const TASK_PATH = /^cycle-(\d+)\/tasks\/([^/]+)\/(.+)$/;

  const els = {
    panel: document.getElementById('panel'),
    handle: document.getElementById('panel-handle'),
    context: document.getElementById('panel-context'),
    path: document.getElementById('panel-path'),
    close: document.getElementById('panel-close'),
    fileList: document.getElementById('file-list'),
    viewer: document.getElementById('viewer'),
    note: document.getElementById('viewer-note'),
    content: document.getElementById('viewer-content'),
  };

  // Set by init(): the task's file list and title, and who to tell about
  // the open task.
  let source = { files: () => [], title: () => '', changed: () => {} };
  let current = null;              // { cycle, task, file, objectUrl } while open
  const openGroups = new Map();    // file group key -> open (user's choice)
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
    for (const child of children) if (child != null) node.append(child);
    return node;
  }

  function archivePath(cycle, task, file) {
    return `cycle-${cycle}/tasks/${task}/${file}`;
  }

  function archiveUrl(path) {
    return new URL(path.split('/').map(encodeURIComponent).join('/'), ARCHIVE_ROOT);
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

  function errorText(err) {
    return err && err.message ? err.message : String(err);
  }

  // ---------- opening and closing ----------

  // Opens a file of an archived task. file is relative to the task's
  // directory; hash names a heading to scroll to.
  function open(cycle, task, file = 'task.md', hash = '') {
    const sameTask = current && current.cycle === cycle && current.task === task;
    if (sameTask && current.file === file) {
      if (hash) scrollToAnchor(hash);
      return;
    }
    release();
    if (!sameTask) openGroups.clear();
    current = { cycle, task, file, objectUrl: null };
    els.panel.hidden = false;
    document.body.classList.add('panel-open');
    els.content.replaceChildren();
    els.note.hidden = true;
    els.viewer.scrollTop = 0;
    els.context.textContent = [task, source.title(cycle, task)].filter(Boolean).join(' · ');
    els.path.textContent = `cycle ${cycle} · ${file}`;
    setHash(archivePath(cycle, task, file) + (hash ? `@${hash.replace(/^#/, '')}` : ''));
    renderFileList();
    source.changed({ cycle, task, file });
    loadFile(hash);
  }

  function close() {
    if (!current) return;
    release();
    current = null;
    loadSeq++;
    els.panel.hidden = true;
    document.body.classList.remove('panel-open');
    els.content.replaceChildren();
    setHash('');
    source.changed(null);
  }

  function release() {
    if (current && current.objectUrl) URL.revokeObjectURL(current.objectUrl);
  }

  // The open file is kept in the address, so a link to it opens it again.
  function setHash(value) {
    const url = new URL(location.href);
    url.hash = value;
    history.replaceState(null, '', value ? url.href : url.href.replace(/#$/, ''));
  }

  function openFromHash() {
    const raw = location.hash.replace(/^#/, '');
    if (!raw) return false;
    let value = raw;
    try {
      value = decodeURIComponent(raw);
    } catch {
      // keep the raw value
    }
    const at = value.lastIndexOf('@');
    const path = at > 0 ? value.slice(0, at) : value;
    const m = TASK_PATH.exec(path);
    if (!m) return false;
    open(Number(m[1]), m[2], m[3], at > 0 ? value.slice(at + 1) : '');
    return true;
  }

  // ---------- file list ----------

  function fileButton(label, file, topLevel) {
    const classes = ['file'];
    if (topLevel) classes.push('top-level');
    if (current.file === file) classes.push('active');
    const button = el('button', { type: 'button', className: classes.join(' '), text: label, title: file });
    button.addEventListener('click', () => open(current.cycle, current.task, file));
    return button;
  }

  // Files at the task's top level, then one folder per round, each open if
  // it holds the open file or is the last round.
  function renderFileList() {
    const files = source.files(current.cycle, current.task);
    const groups = new Map();
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
    const activeDir = current.file.includes('/') ? current.file.slice(0, current.file.indexOf('/')) : null;

    const children = nodes.map((node) => {
      if (node.file) return fileButton(node.file, node.file, true);
      const details = el('details', { className: 'file-group' }, el('summary', { text: `${node.dir}/` }),
        ...groups.get(node.dir).map((file) => fileButton(file.slice(node.dir.length + 1), file, false)));
      const chosen = openGroups.get(node.dir);
      details.open = chosen !== undefined ? chosen : (node.dir === activeDir || node.dir === lastDir);
      details.addEventListener('toggle', () => openGroups.set(node.dir, details.open));
      return details;
    });
    if (!children.length) children.push(el('div', { className: 'empty', text: 'No files listed.' }));
    const scroll = els.fileList.scrollTop;
    els.fileList.replaceChildren(...children);
    els.fileList.scrollTop = scroll;
    const active = els.fileList.querySelector('.file.active');
    if (active) active.scrollIntoView({ block: 'nearest' });
  }

  // ---------- viewer ----------

  function showNote(text) {
    els.note.textContent = text;
    els.note.hidden = false;
  }

  async function loadFile(hash) {
    const opened = current;
    const seq = ++loadSeq;
    const path = archivePath(opened.cycle, opened.task, opened.file);
    const url = archiveUrl(path);
    const kind = fileKind(path);
    let payload;
    try {
      const res = await fetch(url, { cache: 'no-store' });
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`.trim());
      payload = kind === 'image' ? await res.arrayBuffer() : await res.text();
    } catch (err) {
      if (seq === loadSeq) showNote(`Could not load archive/${path}: ${errorText(err)}.`);
      return;
    }
    if (seq !== loadSeq || current !== opened) return;
    let node;
    if (kind === 'image') {
      opened.objectUrl = URL.createObjectURL(new Blob([payload], { type: IMAGE_TYPES[extension(path)] }));
      node = el('img', { className: 'whole', src: opened.objectUrl, alt: path });
    } else if (kind === 'markdown') {
      node = renderMarkdown(payload, url) || renderText(payload);
    } else {
      node = renderText(payload);
    }
    els.content.replaceChildren(node);
    if (hash) scrollToAnchor(hash);
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

  // The archive path of a URL that names a task file, or null.
  function taskPath(url) {
    if (url.origin !== location.origin || !url.pathname.startsWith(ARCHIVE_ROOT.pathname)) return null;
    let rel;
    try {
      rel = url.pathname.slice(ARCHIVE_ROOT.pathname.length).split('/').map(decodeURIComponent).join('/');
    } catch {
      return null;
    }
    return rel.endsWith('/') ? null : TASK_PATH.exec(rel);
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
    const m = taskPath(abs);
    if (!m) {
      link.setAttribute('target', '_blank');
      link.setAttribute('rel', 'noopener noreferrer');
      return;
    }
    link.addEventListener('click', (ev) => {
      if (ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey) return;
      ev.preventDefault();
      open(Number(m[1]), m[2], m[3], abs.hash);
    });
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

  // files(cycle, task) and title(cycle, task) describe a task; changed is
  // told { cycle, task, file } on each open, and null on close.
  function init(options) {
    source = { ...source, ...options };
    openFromHash();
  }

  els.close.addEventListener('click', close);
  document.addEventListener('keydown', (ev) => {
    if (ev.key === 'Escape' && current) close();
  });
  window.addEventListener('hashchange', openFromHash);
  setUpResizing();

  return { init, open, close };
})();
