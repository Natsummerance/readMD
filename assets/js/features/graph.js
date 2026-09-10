// assets/js/features/graph.js
// ReadMD Canvas 2D 极速力导向知识图谱与反向链接抽屉 (Zero Dependencies, 60 FPS)

(function () {
  'use strict';

  const _t = (key, fallback) => {
    try {
      if (typeof i18n === 'function') {
        const val = i18n(key);
        if (val && val !== key) return val;
      }
    } catch (_) {}
    return fallback;
  };

  const _escapeHtml = (str) => {
    if (str == null) return '';
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  };

  let _modal = null;
  let _canvas = null;
  let _ctx = null;
  let _tooltip = null;
  let _backlinksPanel = null;

  let _nodes = [];
  let _edges = [];
  let _nodesById = new Map();
  let _animId = null;
  let _animating = false;

  let _panX = 0;
  let _panY = 0;
  let _zoom = 1.0;
  let _isPanning = false;
  let _panStartX = 0;
  let _panStartY = 0;

  let _draggedNode = null;
  let _hoveredNode = null;
  let _currentFilePath = null;
  let _currentDirectory = null;

  // ---------------------------------------------------------------- UI 模态框构建
  function _createModalIfNeeded() {
    if (_modal) return;

    _modal = document.createElement('div');
    _modal.id = 'graph-modal';
    _modal.className = 'graph-modal hidden';
    _modal.innerHTML = `
      <div class="graph-backdrop"></div>
      <div class="graph-dialog">
        <div class="graph-header">
          <div class="graph-title-row">
            <span class="graph-title-icon"><svg class="graph-title-svg" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" style="vertical-align:middle;"><circle cx="6" cy="6" r="3"/><circle cx="18" cy="8" r="3"/><circle cx="12" cy="18" r="3"/><path d="M8.5 7.5l7 1.5M7.5 8.5l3.5 7M16.5 10l-3.5 6"/></svg></span>
            <h3 class="graph-title" data-i18n="graph.title">${_t('graph.title')}</h3>
            <span class="graph-stats-badge" id="graph-stats-badge">${_t('graph.statsSimple', {nodes: 0, links: 0}) || '0 节点 · 0 关系'}</span>
          </div>
          <div class="graph-toolbar">
            <button class="graph-tool-btn" id="graph-btn-zoom-in" title="${_t('graph.zoomIn')}">+</button>
            <button class="graph-tool-btn" id="graph-btn-zoom-out" title="${_t('graph.zoomOut')}">-</button>
            <button class="graph-tool-btn" id="graph-btn-reset" title="${_t('graph.reset')}">&#x21bb;</button>
            <button class="graph-tool-btn graph-close-btn" id="graph-btn-close" title="${_t('toolbar.close')}">&times;</button>
          </div>
        </div>
        <div class="graph-body">
          <canvas id="graph-canvas"></canvas>
          <div id="graph-tooltip" class="graph-tooltip hidden"></div>
          <div id="graph-loading" class="graph-loading hidden">
            <div class="graph-spinner"></div>
            <span>${_t('graph.loading', '正在构建关系网络...')}</span>
          </div>
        </div>
      </div>
    `;
    document.body.appendChild(_modal);

    _canvas = _modal.querySelector('#graph-canvas');
    _ctx = _canvas.getContext('2d');
    _tooltip = _modal.querySelector('#graph-tooltip');

    _modal.querySelector('.graph-backdrop').addEventListener('click', closeGraphModal);
    _modal.querySelector('#graph-btn-close').addEventListener('click', closeGraphModal);

    _modal.querySelector('#graph-btn-zoom-in').addEventListener('click', () => _zoomAtCenter(1.25));
    _modal.querySelector('#graph-btn-zoom-out').addEventListener('click', () => _zoomAtCenter(0.8));
    _modal.querySelector('#graph-btn-reset').addEventListener('click', _resetView);

    window.addEventListener('resize', () => {
      if (!_modal.classList.contains('hidden')) _resizeCanvas();
    });

    _initCanvasEvents();
  }

  function _createBacklinksPanelIfNeeded() {
    if (_backlinksPanel) return;

    _backlinksPanel = document.createElement('div');
    _backlinksPanel.id = 'backlinks-panel';
    _backlinksPanel.className = 'backlinks-panel hidden';
    _backlinksPanel.innerHTML = `
      <div class="backlinks-header">
        <span class="backlinks-title-icon"><svg class="backlinks-title-svg" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" style="vertical-align:middle;"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg></span>
        <span class="backlinks-title" data-i18n="graph.backlinks">${_t('graph.backlinks', '反向链接')}</span>
        <button class="backlinks-close-btn" id="backlinks-btn-close">&times;</button>
      </div>
      <div class="backlinks-content" id="backlinks-content">
        <div class="backlinks-empty">${_t('graph.noLinks', '暂无关联笔记')}</div>
      </div>
    `;
    document.body.appendChild(_backlinksPanel);

    _backlinksPanel.querySelector('#backlinks-btn-close').addEventListener('click', () => {
      _backlinksPanel.classList.add('hidden');
    });
  }

  // ---------------------------------------------------------------- 物理力导向引擎
  function _initSimulation(data) {
    _nodes = (data.nodes || []).map((n, i) => {
      const angle = (i / Math.max(data.nodes.length, 1)) * Math.PI * 2;
      const radius = 100 + Math.random() * 150;
      return {
        ...n,
        x: Math.cos(angle) * radius,
        y: Math.sin(angle) * radius,
        vx: 0,
        vy: 0,
        radius: Math.max(6, Math.min(6 + (n.degree || 0) * 2, 22)),
        is_current: _currentFilePath && n.path && n.path === _currentFilePath,
      };
    });

    _nodesById = new Map();
    _nodes.forEach(n => _nodesById.set(n.id, n));

    _edges = (data.edges || []).map(e => {
      return {
        ...e,
        sourceNode: _nodesById.get(e.source),
        targetNode: _nodesById.get(e.target),
      };
    }).filter(e => e.sourceNode && e.targetNode);

    _resetView();
    _startSimulation();
  }

  function _stepPhysics() {
    const kRep = 1200;
    const kAtt = 0.04;
    const targetLen = 80;
    const kGrav = 0.015;
    const damping = 0.86;

    let totalEnergy = 0;

    // 1. 库仑排斥力 (所有节点对)
    for (let i = 0; i < _nodes.length; i++) {
      const n1 = _nodes[i];
      for (let j = i + 1; j < _nodes.length; j++) {
        const n2 = _nodes[j];
        const dx = n2.x - n1.x;
        const dy = n2.y - n1.y;
        const distSq = dx * dx + dy * dy + 1.0;
        const dist = Math.sqrt(distSq);

        if (dist < 450) {
          const force = kRep / distSq;
          const fx = (dx / dist) * force;
          const fy = (dy / dist) * force;
          n1.vx -= fx;
          n1.vy -= fy;
          n2.vx += fx;
          n2.vy += fy;
        }
      }
    }

    // 2. 弹簧胡克引力 (连线两端)
    for (let i = 0; i < _edges.length; i++) {
      const e = _edges[i];
      const s = e.sourceNode;
      const t = e.targetNode;
      const dx = t.x - s.x;
      const dy = t.y - s.y;
      const dist = Math.sqrt(dx * dx + dy * dy) + 0.1;
      const force = (dist - targetLen) * kAtt;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;

      s.vx += fx;
      s.vy += fy;
      t.vx -= fx;
      t.vy -= fy;
    }

    // 3. 向心重力与速度位置积分
    for (let i = 0; i < _nodes.length; i++) {
      const n = _nodes[i];
      if (n === _draggedNode) continue; // 拖拽中固定

      n.vx -= n.x * kGrav;
      n.vy -= n.y * kGrav;

      n.vx *= damping;
      n.vy *= damping;

      // 限速
      const speed = Math.sqrt(n.vx * n.vx + n.vy * n.vy);
      if (speed > 16) {
        n.vx = (n.vx / speed) * 16;
        n.vy = (n.vy / speed) * 16;
      }

      n.x += n.vx;
      n.y += n.vy;

      totalEnergy += speed;
    }

    return totalEnergy;
  }

  function _startSimulation() {
    if (_animating) return;
    _animating = true;

    function loop() {
      const energy = _stepPhysics();
      _render();

      if (energy > 0.05 || _draggedNode) {
        _animId = requestAnimationFrame(loop);
      } else {
        _animating = false;
        _render(); // 稳定后画最后一帧
      }
    }

    _animId = requestAnimationFrame(loop);
  }

  function _wakeSimulation() {
    if (!_animating) {
      _startSimulation();
    }
  }

  // ---------------------------------------------------------------- Canvas 渲染管线
  function _resizeCanvas() {
    if (!_canvas) return;
    const rect = _canvas.parentElement.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    _canvas.width = rect.width * dpr;
    _canvas.height = rect.height * dpr;
    _canvas.style.width = rect.width + 'px';
    _canvas.style.height = rect.height + 'px';
    _render();
  }

  function _resetView() {
    if (!_canvas) return;
    const rect = _canvas.getBoundingClientRect();
    _panX = rect.width / 2;
    _panY = rect.height / 2;
    _zoom = 1.0;
    _render();
  }

  function _zoomAtCenter(factor) {
    if (!_canvas) return;
    const rect = _canvas.getBoundingClientRect();
    const cx = rect.width / 2;
    const cy = rect.height / 2;
    _panX = cx - (cx - _panX) * factor;
    _panY = cy - (cy - _panY) * factor;
    _zoom = Math.max(0.15, Math.min(_zoom * factor, 4.0));
    _render();
  }

  function _render() {
    if (!_ctx || !_canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const width = _canvas.width / dpr;
    const height = _canvas.height / dpr;

    _ctx.save();
    _ctx.scale(dpr, dpr);
    _ctx.clearRect(0, 0, width, height);

    // 坐标系位移与缩放
    _ctx.translate(_panX, _panY);
    _ctx.scale(_zoom, _zoom);

    const isDark = document.body.classList.contains('dark') ||
      window.matchMedia('(prefers-color-scheme: dark)').matches;

    const edgeColor = isDark ? 'rgba(255, 255, 255, 0.16)' : 'rgba(0, 0, 0, 0.14)';
    const edgeHighlight = '#38bdf8';
    const deadEdgeColor = 'rgba(239, 68, 68, 0.45)';

    // 1. 绘制连线
    for (let i = 0; i < _edges.length; i++) {
      const e = _edges[i];
      const s = e.sourceNode;
      const t = e.targetNode;

      const isConnectedToHover = _hoveredNode && (s === _hoveredNode || t === _hoveredNode);
      const isDead = t.is_deadlink;

      _ctx.beginPath();
      if (isDead) {
        _ctx.setLineDash([4, 4]);
        _ctx.strokeStyle = deadEdgeColor;
        _ctx.lineWidth = 1.2;
      } else if (isConnectedToHover) {
        _ctx.setLineDash([]);
        _ctx.strokeStyle = edgeHighlight;
        _ctx.lineWidth = 2.0;
      } else {
        _ctx.setLineDash([]);
        _ctx.strokeStyle = edgeColor;
        _ctx.lineWidth = 1.0;
      }

      _ctx.moveTo(s.x, s.y);
      _ctx.lineTo(t.x, t.y);
      _ctx.stroke();
    }
    _ctx.setLineDash([]);

    // 2. 绘制节点
    for (let i = 0; i < _nodes.length; i++) {
      const n = _nodes[i];
      const isHovered = n === _hoveredNode;
      const isCurrent = n.is_current;
      const isDead = n.is_deadlink;

      let fill = isDark ? '#64748b' : '#94a3b8';
      let stroke = isDark ? '#94a3b8' : '#cbd5e1';

      if (isDead) {
        fill = '#ef4444';
        stroke = '#b91c1c';
      } else if (isCurrent) {
        fill = '#3b82f6';
        stroke = '#60a5fa';
      } else if (n.degree > 3) {
        fill = '#0284c7';
        stroke = '#38bdf8';
      }

      // 外发光/焦点圈
      if (isCurrent || isHovered) {
        _ctx.beginPath();
        _ctx.arc(n.x, n.y, n.radius + 5, 0, Math.PI * 2);
        _ctx.fillStyle = isCurrent ? 'rgba(59, 130, 246, 0.25)' : 'rgba(56, 189, 248, 0.25)';
        _ctx.fill();
      }

      // 节点圆本体
      _ctx.beginPath();
      _ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
      _ctx.fillStyle = fill;
      _ctx.fill();
      _ctx.strokeStyle = stroke;
      _ctx.lineWidth = 1.8;
      _ctx.stroke();

      // 节点文本
      if (_zoom > 0.45 || isHovered || isCurrent || n.degree > 2) {
        _ctx.font = '11px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
        _ctx.fillStyle = isDark ? '#e2e8f0' : '#1e293b';
        _ctx.textAlign = 'center';
        _ctx.textBaseline = 'top';
        const label = n.label.length > 20 ? n.label.slice(0, 18) + '…' : n.label;
        _ctx.fillText(label, n.x, n.y + n.radius + 4);
      }
    }

    _ctx.restore();
  }

  // ---------------------------------------------------------------- 交互事件处理
  function _screenToWorld(sx, sy) {
    return {
      x: (sx - _panX) / _zoom,
      y: (sy - _panY) / _zoom,
    };
  }

  function _findNodeAt(sx, sy) {
    const w = _screenToWorld(sx, sy);
    for (let i = _nodes.length - 1; i >= 0; i--) {
      const n = _nodes[i];
      const dx = w.x - n.x;
      const dy = w.y - n.y;
      if (dx * dx + dy * dy <= (n.radius + 4) * (n.radius + 4)) {
        return n;
      }
    }
    return null;
  }

  function _initCanvasEvents() {
    _canvas.addEventListener('mousedown', e => {
      const rect = _canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      const hit = _findNodeAt(sx, sy);
      if (hit) {
        _draggedNode = hit;
        hit.vx = 0;
        hit.vy = 0;
        _wakeSimulation();
      } else {
        _isPanning = true;
        _panStartX = sx - _panX;
        _panStartY = sy - _panY;
      }
    });

    window.addEventListener('mousemove', e => {
      if (!_canvas || _modal.classList.contains('hidden')) return;
      const rect = _canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      if (_draggedNode) {
        const w = _screenToWorld(sx, sy);
        _draggedNode.x = w.x;
        _draggedNode.y = w.y;
        _draggedNode.vx = 0;
        _draggedNode.vy = 0;
        _wakeSimulation();
        return;
      }

      if (_isPanning) {
        _panX = sx - _panStartX;
        _panY = sy - _panStartY;
        _render();
        return;
      }

      // 悬停检测
      const hit = _findNodeAt(sx, sy);
      if (hit !== _hoveredNode) {
        _hoveredNode = hit;
        _canvas.style.cursor = hit ? 'pointer' : 'default';
        _render();

        if (hit) {
          _tooltip.innerHTML = `
            <strong>${_escapeHtml(hit.label)}</strong><br>
            <span class="muted">${_escapeHtml(hit.path || (_t('graph.deadlinkUncreated') || '（未创建文档）'))}</span><br>
            <span>${_t('graph.inOutLinks', {out: hit.link_count, in: hit.backlink_count}) || `出链: ${hit.link_count} · 入链: ${hit.backlink_count}`}</span>
          `;
          _tooltip.style.left = `${sx + 15}px`;
          _tooltip.style.top = `${sy + 15}px`;
          _tooltip.classList.remove('hidden');
        } else {
          _tooltip.classList.add('hidden');
        }
      }
    });

    window.addEventListener('mouseup', () => {
      _draggedNode = null;
      _isPanning = false;
    });

    _canvas.addEventListener('wheel', e => {
      e.preventDefault();
      const rect = _canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      const delta = e.deltaY < 0 ? 1.15 : 0.85;
      const newZoom = Math.max(0.15, Math.min(_zoom * delta, 4.0));

      _panX = sx - (sx - _panX) * (newZoom / _zoom);
      _panY = sy - (sy - _panY) * (newZoom / _zoom);
      _zoom = newZoom;

      _render();
    }, { passive: false });

    _canvas.addEventListener('click', e => {
      const rect = _canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      const hit = _findNodeAt(sx, sy);
      if (hit && hit.path) {
        closeGraphModal();
        if (typeof window.loadFile === 'function') {
          window.loadFile(hit.path);
        }
      }
    });
  }

  function _hasActiveDocument() {
    if (typeof state === 'undefined' || !state) return false;
    return (state.mode === 'file' || state.mode === 'virtual') && !!state.original;
  }

  // ---------------------------------------------------------------- 公开 API 与接口
  async function openGraphModal(directory) {
    if (!_hasActiveDocument()) {
      if (typeof showToast === 'function') {
        const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
        showToast(_t('toast.openDocumentToUse', '请先打开文档'));
      }
      return;
    }
    _createModalIfNeeded();
    _modal.classList.remove('hidden');
    _resizeCanvas();

    const loading = _modal.querySelector('#graph-loading');
    loading.classList.remove('hidden');

    _currentDirectory = directory || null;

    try {
      let data = null;
      if (window.pywebview && window.pywebview.api && window.pywebview.api.get_links_graph) {
        const res = await window.pywebview.api.get_links_graph(directory || '');
        if (res && res.ok) data = res.graph;
      } else {
        const url = `/api/links/graph?dir=${encodeURIComponent(directory || '')}&max_nodes=500`;
        const res = await fetch(url);
        const json = await res.json();
        if (json && json.ok) data = json.graph;
      }

      if (data) {
        const badge = _modal.querySelector('#graph-stats-badge');
        if (badge && data.stats) {
          badge.textContent = _t('graph.statsBadge', {
            nodes: data.stats.total_nodes,
            edges: data.stats.total_edges,
            deadlinks: data.stats.deadlinks_count
          }) || `${data.stats.total_nodes} 节点 · ${data.stats.total_edges} 关系 · ${data.stats.deadlinks_count} 死链`;
        }
        _initSimulation(data);
      }
    } catch (err) {
      console.error('Failed to load knowledge graph:', err);
    } finally {
      loading.classList.add('hidden');
    }
  }

  function closeGraphModal() {
    if (!_modal) return;
    _modal.classList.add('hidden');
    if (_animId) {
      cancelAnimationFrame(_animId);
      _animId = null;
    }
    _animating = false;
  }

  const WIKILINK_RE = /\[\[([^\]\n|#]+)(?:#([^\]\n|]+))?(?:\|([^\]\n]+))?\]\]/;

  function hasGraphElements(explicitContent) {
    if (typeof explicitContent === 'string') {
      return explicitContent.includes('[[') && WIKILINK_RE.test(explicitContent);
    }
    if (typeof state === 'undefined' || !state || state.mode === 'welcome' || !_hasActiveDocument()) return false;
    const content = state.fixed || state.original || (state.tabs && state.tabs.find(t => t.id === state.activeTabId)?.content) || '';
    return typeof content === 'string' && content.includes('[[') && WIKILINK_RE.test(content);
  }

  function setTopGraphButtonVisible(visible) {
    const btn = document.getElementById('btn-graph');
    if (!btn) return;
    const hasDoc = _hasActiveDocument();
    btn.disabled = !hasDoc;
    if (visible && hasDoc) {
      btn.classList.remove('hidden');
    } else {
      btn.classList.add('hidden');
    }
  }

  function updateGraphButtonVisibility(forceVisible) {
    if (forceVisible === false) {
      setTopGraphButtonVisible(false);
      return;
    }
    if (forceVisible === true) {
      setTopGraphButtonVisible(true);
      return;
    }
    setTopGraphButtonVisible(hasGraphElements());
  }

  async function refreshBacklinks(filePath) {
    _currentFilePath = filePath || null;
    _createBacklinksPanelIfNeeded();

    if (!filePath) {
      _backlinksPanel.querySelector('#backlinks-content').innerHTML = `
        <div class="backlinks-empty">${_t('graph.noLinks', '暂无关联笔记')}</div>
      `;
      updateGraphButtonVisibility(false);
      return;
    }

    try {
      let resData = null;
      if (window.pywebview && window.pywebview.api && window.pywebview.api.get_backlinks) {
        const res = await window.pywebview.api.get_backlinks(filePath);
        if (res && res.ok) resData = res;
      } else {
        const url = `/api/links/backlinks?path=${encodeURIComponent(filePath)}`;
        const res = await fetch(url);
        const json = await res.json();
        if (json && json.ok) resData = json;
      }

      const backlinks = (resData && resData.backlinks) || [];
      const forwardLinks = (resData && resData.forward_links) || [];
      const contentEl = _backlinksPanel.querySelector('#backlinks-content');

      const hasConnections = (backlinks.length > 0) || (forwardLinks.length > 0) || hasGraphElements();
      setTopGraphButtonVisible(hasConnections);

      if (!backlinks.length && !forwardLinks.length) {
        contentEl.innerHTML = `<div class="backlinks-empty">${_t('graph.noLinks', '暂无关联笔记')}</div>`;
        return;
      }

      let html = '';
      if (backlinks.length > 0) {
        html += `<div class="backlinks-group-title">${_t('graph.backlinks', '反向链接')} (${backlinks.length})</div>`;
        for (const item of backlinks) {
          const title = item.source_title || item.source_path.split(/[\\/]/).pop();
          html += `
            <div class="backlink-item" data-path="${_escapeHtml(item.source_path)}" data-line="${item.line_no}">
              <div class="backlink-title">${_escapeHtml(title)}</div>
              <div class="backlink-context">${_t('graph.linePrefix', {line: item.line_no}) || ('行 ' + item.line_no)}${item.alias ? ' · ' + _escapeHtml(item.alias) : ''}</div>
            </div>
          `;
        }
      }

      if (forwardLinks.length > 0) {
        html += `<div class="backlinks-group-title">${_t('graph.outgoing', '正向出链')} (${forwardLinks.length})</div>`;
        for (const item of forwardLinks) {
          const target = item.target_clean || item.target_raw;
          const isDead = !item.target_path;
          html += `
            <div class="backlink-item ${isDead ? 'deadlink' : ''}" data-path="${_escapeHtml(item.target_path || '')}">
              <div class="backlink-title">${_escapeHtml(target)}</div>
              <div class="backlink-context">${_escapeHtml(item.alias || '')}</div>
            </div>
          `;
        }
      }

      contentEl.innerHTML = html;

      // 绑定点击跳转
      contentEl.querySelectorAll('.backlink-item').forEach(el => {
        el.addEventListener('click', () => {
          const targetPath = el.dataset.path;
          if (targetPath && typeof window.loadFile === 'function') {
            window.loadFile(targetPath);
          }
        });
      });
    } catch (err) {
      console.error('Failed to fetch backlinks:', err);
      updateGraphButtonVisibility();
    }
  }

  function toggleBacklinksDrawer() {
    if (!_hasActiveDocument()) {
      if (typeof showToast === 'function') {
        const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
        showToast(_t('toast.openDocumentToUse', '请先打开文档'));
      }
      return;
    }
    _createBacklinksPanelIfNeeded();
    _backlinksPanel.classList.toggle('hidden');
    if (!_backlinksPanel.classList.contains('hidden') && _currentFilePath) {
      refreshBacklinks(_currentFilePath);
    }
  }

  // 挂载到全局
  window.ReadMDGraph = {
    open: openGraphModal,
    close: closeGraphModal,
    refreshBacklinks: refreshBacklinks,
    toggleDrawer: toggleBacklinksDrawer,
    updateVisibility: updateGraphButtonVisibility,
    hasGraph: hasGraphElements,
  };
})();
