'use strict';
/* ============================================================
   ReadMD Features - AI Assistant & Dialog Import
   ============================================================ */

/* ---------------- AI 助手 ---------------- */

const AI_ACTIONS = {
  quick_read: 'Quick Read', polish: 'Polish', modify: 'Modify',
  expand: 'Expand', continue: 'Continue', translate: 'Translate', ask: 'Ask',
};

const AI_SYSTEM = {
  quick_read: '你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。',
  polish: '你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。',
  modify: '你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。只输出修改后的完整文档，不要加任何解释。',
  expand: '你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。只输出扩充后的完整文档，不要加任何解释。',
  continue: '你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。',
  translate: '你是专业翻译。将用户给出的文档翻译成指定语言，保留 Markdown 结构、表格与代码块，只输出译文。',
  ask: '你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。',
};

function toggleAiPanel() {
  if (moduleBlocked('ai')) return;
  const p = $('ai-panel');
  p.classList.toggle('hidden');
  if (!p.classList.contains('hidden')) {
    updateAiUsage();
    if (!state.ai.config) loadAiOnDemand();
    else { loadAiPrompts(); loadAiSessions(); }
    setTimeout(() => $('ai-prompt') && $('ai-prompt').focus(), 0);
  }
}

async function loadAiOnDemand() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  setAiConnectionState('loading', _t('ai.statusReading') || 'Reading connection...');
  try { await apiFetch('/api/modules/load', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: 'ai' }) }); } catch (e) { /* old servers use the normal poll path */ }
  const cfg = await loadAiConfig();
  if (!cfg) setAiConnectionState('warn', _t('ai.statusOffline') || 'Not connected');
}

function setAiConnectionState(kind, label) {
  const dot = $('ai-connection-dot'), text = $('ai-connection-label');
  if (dot) dot.className = 'conn-dot ' + (kind || '');
  if (text) text.textContent = label || '';
}

function updateAiConnectionSummary() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  const model = $('ai-model');
  const summary = $('ai-model-summary');
  if (summary) summary.textContent = (model && model.value) || (_t('ai.noModel') || 'No model selected');
  if (!p) return setAiConnectionState('warn', _t('ai.noProvider') || 'Please select provider');
  const local = /ollama/i.test(p.name || '');
  if (local || p.has_key || p.key_source) setAiConnectionState('ready', p.name + ' · ' + (_t('ai.statusReady') || 'Ready'));
  else setAiConnectionState('warn', p.name + ' · ' + (_t('ai.needKey') || 'API Key required'));
}
/* ---------------- Prompt 模板 ---------------- */

async function loadAiPrompts() {
  try {
    const r = await apiFetch('/api/ai/prompts');
    if (!r.ok) return;
    state.ai.templates = (await r.json()).templates || [];
    fillAiTemplates();
  } catch (e) { /* ignore */ }
}

function fillAiTemplates() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = $('ai-template');
  if (!sel) return;
  const cur = state.ai.templateId;
  sel.innerHTML = '';
  const none = document.createElement('option');
  none.value = '';
  none.textContent = _t('ai.defaultAction') || 'Default action (no template)';
  sel.appendChild(none);
  (state.ai.templates || []).forEach(t => {
    const o = document.createElement('option');
    o.value = t.id;
    o.textContent = (t.builtin ? '◆ ' : '◇ ') + t.name;
    sel.appendChild(o);
  });
  if (cur && [...sel.options].some(o => o.value === cur)) sel.value = cur;
  else state.ai.templateId = '';
}


function currentAiTemplate() {
  const id = $('ai-template').value;
  return (state.ai.templates || []).find(t => t.id === id) || null;
}

function onAiTemplateChange() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const t = currentAiTemplate();
  state.ai.templateId = t ? t.id : '';
  document.querySelectorAll('.ai-act').forEach(b => {
    b.classList.toggle('active', !!(t && t.action && t.action !== 'custom' && b.dataset.act === t.action));
  });
  if (t && t.action === 'translate') $('ai-prompt').placeholder = _t('ai.promptTranslatePlaceholder') || 'Translate: target language (e.g., English, Japanese)';
  else $('ai-prompt').placeholder = _t('ai.promptDefaultPlaceholder') || 'Additional requirements / questions / translation target (optional)';
}

function openTplModal() {
  $('tpl-modal').classList.remove('hidden');
  if (!state.ai.templates.length) loadAiPrompts();
  renderTplList();
  selectTpl(null);
}

function renderTplList() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const list = $('tpl-list');
  list.innerHTML = '';
  (state.ai.templates || []).forEach(t => {
    const li = document.createElement('li');
    li.textContent = (t.builtin ? '◆ ' : '◇ ') + t.name;
    li.dataset.id = t.id;
    li.title = (_t('ai.actionPrefix') || 'Action: ') + (t.action || 'custom') + (t.user ? (' · ' + (_t('ai.hasUserTpl') || 'Has user message template')) : '');
    li.addEventListener('click', () => selectTpl(t.id));
    list.appendChild(li);
  });
}

function selectTpl(id) {
  const t = (state.ai.templates || []).find(x => x.id === id) || null;
  document.querySelectorAll('#tpl-list li').forEach(li => li.classList.toggle('active', li.dataset.id === id));
  $('tpl-id').value = t ? t.id : '';
  $('tpl-name').value = t ? t.name : '';
  $('tpl-action').value = (t && t.action) || 'custom';
  $('tpl-system').value = t ? (t.system || '') : '';
  $('tpl-user').value = t ? (t.user || '') : '';
  $('tpl-del').disabled = !t;
}

async function saveTplForm() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const t = {
    id: $('tpl-id').value || undefined,
    name: $('tpl-name').value.trim(),
    action: $('tpl-action').value || 'custom',
    system: $('tpl-system').value.trim(),
    user: $('tpl-user').value.trim(),
  };
  if (!t.name) { showToast(_t('toast.tplNameReq') || 'Please enter template name'); return; }
  try {
    const r = await apiFetch('/api/ai/prompts', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'save', template: t }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '保存失败');
    state.ai.templates = d.templates || [];
    fillAiTemplates();
    renderTplList();
    selectTpl(d.saved_id);
    showToast(_t('toast.tplSaved') || 'Template saved');
  } catch (e) { showToast((_t('toast.saveFailed') || 'Save failed: ') + e.message); }
}

async function deleteCurrentTpl() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const id = $('tpl-id').value;
  if (!id) return;
  const cur = (state.ai.templates || []).find(x => x.id === id);
  const msg = cur && cur.builtin ? (_t('toast.tplResetConfirm') || '将重置为默认模板，确定吗？') : (_t('toast.tplDelConfirm') || '确定删除此模板吗？');
  if (!confirm(msg)) return;
  try {
    const r = await apiFetch('/api/ai/prompts', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'delete', id }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '删除失败');
    state.ai.templates = d.templates || [];
    fillAiTemplates();
    renderTplList();
    selectTpl(null);
    showToast(_t('toast.tplDeleted') || 'Template deleted');
  } catch (e) { showToast((_t('toast.deleteFailed') || 'Delete failed: ') + e.message); }
}

/* ---------------- 对话历史管理 ---------------- */

async function loadAiSessions() {
  try {
    const r = await apiFetch('/api/ai/history');
    if (!r.ok) return;
    const d = await r.json();
    state.ai.sessions = d.sessions || [];
    renderAiSessionSelect();
  } catch (e) { /* ignore */ }
}

function renderAiSessionSelect() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const list = $('ai-history-list');
  if (!list) return;
  const q = ($('ai-history-search') && $('ai-history-search').value || '').trim().toLowerCase();
  list.innerHTML = '';
  const rows = (state.ai.sessions || []).filter(s => !q || (s.title || '').toLowerCase().includes(q));
  if (!rows.length) {
    const empty = document.createElement('div');
    empty.className = 'ai-history-empty';
    empty.textContent = _t('ai.historyEmpty') || 'No session history';
    list.appendChild(empty);
    return;
  }
  rows.forEach(s => {
    const row = document.createElement('div'); row.className = 'ai-history-item';
    const load = document.createElement('button'); load.className = 'tb-btn ai-history-load';
    load.innerHTML = '<strong></strong><small></small>';
    load.querySelector('strong').textContent = s.title || (_t('ai.untitledSession') || 'Untitled Session');
    load.querySelector('small').textContent = fmtTime(s.updated) + ' · ' + (s.msgCount || 0) + ' ' + (_t('ai.msgCountUnit') || ' messages');
    load.addEventListener('click', async () => { $('ai-session').value = s.id; await onAiSessionChange(); closeAiModal('ai-history-modal'); });
    const rename = document.createElement('button'); rename.className = 'tb-btn'; rename.textContent = _t('tabs.rename') || 'Rename'; rename.title = _t('ai.renameSession') || 'Rename session';
    rename.addEventListener('click', () => renameAiSession(s));
    const del = document.createElement('button'); del.className = 'tb-btn'; del.textContent = _t('tpl.delete') || 'Del'; del.title = _t('ai.deleteSession') || 'Delete session';
    del.addEventListener('click', async () => {
      if (!window.confirm((_t('ai.confirmDeleteSession') || 'Delete session: ') + (s.title || (_t('ai.untitledSession') || 'Untitled Session')) + '？')) return;
      await deleteAiSessionById(s.id);
    });
    row.append(load, rename, del); list.appendChild(row);
  });
}

async function renameAiSession(summary) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const title = window.prompt(_t('ai.sessionName') || 'Session name', summary.title || '');
  if (title === null) return;
  const next = title.trim().slice(0, 80);
  if (!next) { showToast(_t('toast.sessionNameEmpty') || 'Session name cannot be empty'); return; }
  try {
    const r = await apiFetch('/api/ai/history?id=' + encodeURIComponent(summary.id));
    const d = await r.json(); if (!r.ok || !d.session) throw new Error(d.error || 'Session does not exist');
    d.session.title = next;
    const saved = await apiFetch('/api/ai/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'save', session: d.session }) });
    if (!saved.ok) throw new Error((await saved.json().catch(() => ({}))).error || '保存失败');
    await loadAiSessions(); showToast(_t('toast.sessionRenamed') || 'Session renamed');
  } catch (e) { showToast((_t('toast.renameFailed') || 'Rename failed: ') + e.message); }
}

function fmtTime(ts) {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  const p = n => String(n).padStart(2, '0');
  return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate()) + ' ' + p(d.getHours()) + ':' + p(d.getMinutes());
}

async function onAiSessionChange() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const id = $('ai-session').value;
  if (!id) return;
  try {
    const r = await apiFetch('/api/ai/history?id=' + encodeURIComponent(id));
    if (!r.ok) { showToast(_t('toast.loadSessionFail') || 'Failed to load session'); return; }
    const s = (await r.json()).session;
    if (!s) { showToast(_t('toast.sessionNotExist') || 'Session does not exist'); return; }
    const savedProvider = (state.ai.providers || []).find(p => p.id === s.provider || p.name === s.provider);
    if (savedProvider) {
      $('ai-provider').value = savedProvider.id;
      onAiProviderChange();
      if (s.model) $('ai-model').value = s.model;
      syncAiKey();
    }
    state.ai.messages = s.messages || [];
    state.ai.sessionId = s.id;
    state.ai.raw = '';
    state.ai.usage = null;
    state.ai.sessUsage = s.usage || { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
    updateAiUsage();
    renderAiHistory();
    showToast(_t('toast.sessionLoaded') || 'Session loaded');
  } catch (e) { showToast(_t('toast.loadSessionFail') || 'Failed to load session'); }
}


function renderAiHistory() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const out = $('ai-output');
  out.innerHTML = '';
  const msgs = state.ai.messages || [];
  let uSeq = 0, aSeq = 0;
  msgs.forEach((m, i) => {
    if (m.role === 'user') { uSeq++;
      const ub = document.createElement('div');
      ub.className = 'ai-msg user';
      const tag = document.createElement('div');
      tag.className = 'ai-msg-tag';
      tag.textContent = _t('ai.meTag', { seq: uSeq }) || ('我 · 提问 ' + uSeq);
      const body = document.createElement('div');
      body.className = 'ai-msg-body';
      body.textContent = m.content.length > 3000 ? m.content.slice(0, 3000) + '\n' + (_t('ai.omittedLong') || '... (omitted)') : m.content;
      ub.appendChild(tag); ub.appendChild(body);
      out.appendChild(ub);
    } else if (m.role === 'assistant' && m.content) { aSeq++;
      const ab = document.createElement('div');
      ab.className = 'ai-msg ai';
      const tag = document.createElement('div');
      tag.className = 'ai-msg-tag';
      tag.textContent = (_t('ai.aiTag', { seq: aSeq }) || ('AI · 回答 ' + aSeq)) + (m.model ? ' · ' + m.model : '') + fmtAiUsage(m.usage);
      tag.appendChild(aiAnswerCopyButton(m.content));
      const body = document.createElement('div');
      body.className = 'ai-msg-body';
      const prot = protectMath(m.content);
      body.innerHTML = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
      ab.appendChild(tag); ab.appendChild(body);
      out.appendChild(ab);
    }
  });
  out.scrollTop = out.scrollHeight;
  const last = msgs[msgs.length - 1];
  if (last && last.role === 'assistant') state.ai.raw = last.content || '';
  updateAiRawButtons();
}

async function saveCurrentSession(silent) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const msgs = (state.ai.messages || []).filter(m => m && !m.ephemeral);
  if (!msgs.length) { if (!silent) showToast((state.ai.messages || []).length ? (_t('toast.incognitoNoSave') || 'Current session is incognito, will not save') : (_t('toast.noConversationContent') || 'No conversation content')); return false; }
  const title = ($('ai-prompt').value.trim() || msgs[0].content || (_t('ai.untitledSession') || 'Untitled Session')).slice(0, 40).replace(/\s+/g, ' ');
  const sess = {
    id: state.ai.sessionId || undefined,
    title: title,
    provider: $('ai-provider').value,
    model: $('ai-model').value,
    doc: state.mode === 'file' ? (state.file || '') : (state.sourceName || ''),
    messages: msgs,
    usage: state.ai.sessUsage,
  };
  try {
    const r = await apiFetch('/api/ai/history', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'save', session: sess }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || (_t('toast.saveFailed') || '保存失败'));
    state.ai.sessionId = d.session.id;
    await loadAiSessions();
    $('ai-session').value = state.ai.sessionId;
    if (!silent) showToast(_t('toast.sessionSaved') || 'Session saved');
    return true;
  } catch (e) { if (!silent) showToast((_t('toast.saveFailed') || 'Save failed: ') + e.message); return false; }
}

async function deleteCurrentSession() {
  const id = $('ai-session').value;
  if (!id) return;
  try {
    const r = await apiFetch('/api/ai/history', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'delete', id }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '删除失败');
    if (state.ai.sessionId === id) { state.ai.sessionId = null; state.ai.messages = []; clearAiOutput(); }
    await loadAiSessions();
    showToast(_t('toast.sessionDeleted') || 'Session deleted');
  } catch (e) { showToast((_t('toast.deleteFailed') || 'Delete failed: ') + e.message); }
}

function clearAiContext() {
  if (!(state.ai.messages || []).length) { showToast(_t('toast.noContext') || 'No context available'); return; }
  state.ai.messages = [];
  state.ai.sessionId = null;
  state.ai.raw = '';
  state.ai.usage = null;
  state.ai.sessUsage = { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
  updateAiUsage();
  clearAiOutput();
  $('ai-session').value = '';
  showToast(_t('toast.contextCleared') || 'Context cleared, starting new round');
}


function clearAiOutput() {
  state.ai.raw = '';
  state.ai.aborter = null;
  const out = $('ai-output');
  if (out) out.innerHTML = '';
  updateAiRawButtons();
}

async function loadAiConfig() {
  for (let attempt = 0; attempt < 25; attempt++) {
  try {
    const r = await apiFetch('/api/ai/config');
    if (r.status === 409) { await new Promise(r2 => setTimeout(r2, 800)); continue; }
    if (!r.ok) return null;
    state.ai.config = await r.json();
    const cfg = state.ai.config;
    state.ai.providers = mergeAiProviders(cfg.custom || [], cfg.presets || []);
    fillAiProviders(state.ai.providers, cfg.current || {});
    loadAiPrompts();
    loadAiSessions();
    return cfg;
  } catch (e) { /* ignore */ return null; }
  }
  return null;
}

function mergeAiProviders(custom, presets) {
  return [...(custom || []), ...(presets || [])];
}

function fillAiProviders(merged, current) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = $('ai-provider');
  const curId = (current && (current.provider_id || current.provider)) || (merged[0] && merged[0].id) || '';
  sel.innerHTML = '';
  const customGroup = document.createElement('optgroup'); customGroup.label = _t('ai.customConnections') || 'Custom Connections';
  const presetGroup = document.createElement('optgroup'); presetGroup.label = _t('ai.officialPresets') || 'Official Presets';
  merged.forEach(p => {
    const o = document.createElement('option');
    o.value = p.id;
    o.textContent = p.name;
    (p.custom ? customGroup : presetGroup).appendChild(o);
  });
  if (customGroup.children.length) sel.appendChild(customGroup);
  if (presetGroup.children.length) sel.appendChild(presetGroup);
  if (curId) sel.value = curId;
  onAiProviderChange();
}

function currentAiProvider() {
  const id = $('ai-provider').value;
  return (state.ai.providers || []).find(p => p.id === id || p.name === id) || null;
}

function aiPresetBase(p) {
  return (p && p.base_url) || '';
}

function fillAiModels(models, selected) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = $('ai-model');
  sel.innerHTML = '';
  const list = Array.isArray(models) ? models.filter(Boolean) : [];
  const placeholder = new Option(list.length ? (_t('ai.selectModel') || 'Select model') : (_t('ai.fetchModelsFirst') || 'Fetch models first'), '');
  placeholder.disabled = true; placeholder.selected = !list.length;
  sel.appendChild(placeholder);
  list.forEach(id => sel.appendChild(new Option(id, id)));
  sel.disabled = !list.length;
  if (list.length) sel.value = list.indexOf(selected) >= 0 ? selected : list[0];
  updateAiConnectionSummary();
}

function onAiProviderChange() {
  const p = currentAiProvider();
  if (!p) { fillAiModels([], ''); syncAiKey(); return; }
  const base = aiPresetBase(p);
  $('ai-base-url').value = base;
  const mode = p.mode || (p.format === 'anthropic' ? 'messages' : 'auto');
  $('ai-mode').value = (mode === 'anthropic') ? 'messages' : mode;
  const current = (state.ai.config && state.ai.config.current) || {};
  fillAiModels(p.models, (current.provider_id || current.provider) === p.id ? current.model : '');
  $('ai-provider-name').value = p.name || '';
  $('ai-provider-name').disabled = !p.custom;
  $('ai-provider-delete').disabled = !p.custom;
  syncAiKey();
}

function syncAiKey() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  const inp = $('ai-key');
  const status = $('ai-conn-status');
  if (!p) { inp.value = ''; inp.placeholder = ''; if (status) status.textContent = ''; return; }
  // API Key 不会从后端回传；切换连接时也不保留前一个连接的输入值。
  inp.value = '';
  inp.placeholder = (p.key_source && p.key_source.indexOf('env:') === 0)
    ? (_t('ai.readFromEnv', { env: p.key_source.slice(4) }) || ('已从环境变量 ' + p.key_source.slice(4) + ' 读取，可覆盖'))
    : (p.name.indexOf('Ollama') >= 0 ? (_t('ai.apiKeyOllama') || 'API Key（本地 Ollama 可留空）') : (_t('ai.apiKeyRequired') || 'API Key（必填）'));
  if (status) {
    status.textContent = p.has_key
      ? (p.key_source ? (_t('ai.keyReady', { source: p.key_source }) || ('Key 就绪（' + p.key_source + '）')) : (_t('ai.keyConfigured') || 'Key 已配置'))
      : (p.name.indexOf('Ollama') >= 0 ? (_t('ai.localNoKey') || '本地模型无需 Key') : (_t('ai.noKeyConfigured') || '未配置 Key'));
  }
  updateAiConnectionSummary();
}

function aiAnswerCopyButton(content) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const button = document.createElement('button');
  button.className = 'tb-btn ai-msg-copy'; button.textContent = _t('toast.copiedAnswer') || 'Copy answer';
  button.addEventListener('click', () => copyText(String(content || ''), _t('toast.copiedAnswer') || '已复制回答'));
  return button;
}

async function copyText(value, success) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!value) return;
  try { await navigator.clipboard.writeText(value); showToast(success || (_t('toast.copied') || 'Copied')); }
  catch (e) { const ta = document.createElement('textarea'); ta.value = value; document.body.appendChild(ta); ta.select(); try { document.execCommand('copy'); showToast(success || (_t('toast.copied') || 'Copied')); } catch (e2) { showToast(_t('toast.copyFailed') || 'Copy failed'); } ta.remove(); }
}

async function deleteAiSessionById(id) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!id) return;
  try {
    const r = await apiFetch('/api/ai/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'delete', id }) });
    const d = await r.json(); if (!r.ok || !d.ok) throw new Error(d.error || (_t('toast.deleteFailed') || '删除失败'));
    if (state.ai.sessionId === id) clearAiContext();
    await loadAiSessions(); showToast(_t('toast.sessionDeleted') || 'Session deleted');
  } catch (e) { showToast((_t('toast.deleteFailed') || 'Delete failed: ') + e.message); }
}

async function clearAiSessions() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!(state.ai.sessions || []).length || !window.confirm(_t('toast.clearSessionsConfirm') || '清空全部会话历史？此操作无法撤销。')) return;
  try {
    const r = await apiFetch('/api/ai/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'clear' }) });
    if (!r.ok) throw new Error((await r.json().catch(() => ({}))).error || (_t('toast.clearFailed') || '清空失败'));
    state.ai.sessionId = null; state.ai.sessions = []; fillAiSessions(); showToast(_t('toast.sessionsCleared') || 'Session history cleared');
  } catch (e) { showToast((_t('toast.clearFailed') || 'Clear failed: ') + e.message); }
}

function newAiProvider() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.ai.config) return;
  const custom = state.ai.config.custom || (state.ai.config.custom = []);
  let seq = custom.length + 1;
  const connPrefix = _t('ai.customConnections') || 'Custom Connections';
  let name = connPrefix + ' ' + seq;
  while ((state.ai.providers || []).some(p => p.name === name)) name = connPrefix + ' ' + (++seq);
  const uid = (crypto.randomUUID ? crypto.randomUUID().replace(/-/g, '') : String(Date.now()) + Math.random().toString(16).slice(2));
  const p = { id: 'custom:' + uid, name, custom: true, base_url: '', format: 'openai', mode: 'auto', models: [] };
  custom.push(p);
  state.ai.providers = mergeAiProviders(custom, state.ai.config.presets || []);
  fillAiProviders(state.ai.providers, { provider_id: p.id, model: '' });
  $('ai-provider-name').focus(); $('ai-provider-name').select();
}

async function deleteAiProvider() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p || !p.custom || !state.ai.config) return;
  if (!window.confirm(_t('toast.customConnDeleteConfirm', { name: p.name }) || ('删除自定义连接“' + p.name + '”？此操作不会影响官方预设。'))) return;
  const custom = (state.ai.config.custom || []).filter(c => c.id !== p.id);
  const fallback = (state.ai.config.presets || [])[0] || custom[0] || {};
  const current = { provider_id: fallback.id || '', model: '' };
  try {
    const r = await apiFetch('/api/ai/config', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ providers: custom, current }),
    });
    if (!r.ok) throw new Error((await r.json().catch(() => ({}))).error || (_t('toast.saveFailed') || '保存失败'));
    state.ai.config.custom = custom; state.ai.config.current = current;
    state.ai.providers = mergeAiProviders(custom, state.ai.config.presets || []);
    fillAiProviders(state.ai.providers, current);
    showToast(_t('toast.customConnDeleted') || 'Custom connection deleted');
  } catch (e) { showToast((_t('toast.deleteFailed') || 'Delete failed: ') + e.message); }
}


async function saveAiSelection(silent) {
  const p = currentAiProvider();
  if (!p || !state.ai.config) return;
  const custom = (state.ai.config.custom || []).map(c => Object.assign({}, c));
  const keyVal = $('ai-key').value.trim();
  const baseUrl = $('ai-base-url').value.trim();
  const mode = $('ai-mode').value || 'auto';
  const requestedName = $('ai-provider-name').value.trim() || p.name;
  if (p.custom && requestedName !== p.name && custom.some(c => c.name === requestedName)) {
    showToast(_t('toast.customConnNameExists') || 'Custom connection name already exists'); return;
  }
  let over = custom.find(c => c.id === p.id);
  if (!over) {
    over = Object.assign({}, p);
    delete over.has_key; delete over.key_source;
    if (!String(over.id || '').startsWith('custom:')) {
      const uid = (crypto.randomUUID ? crypto.randomUUID().replace(/-/g, '') : String(Date.now()) + Math.random().toString(16).slice(2));
      over.id = 'custom:' + uid;
    }
    over.custom = true;
    custom.push(over);
  }
  if (p.custom && requestedName !== p.name) {
    over.name = requestedName;
  }
  if (baseUrl) over.base_url = baseUrl;
  else delete over.base_url;
  over.mode = mode;
  if (mode === 'messages') over.format = 'anthropic';
  else over.format = 'openai';
  if (keyVal) over.api_key = keyVal;
  over.models = Array.from($('ai-model').options).map(o => o.value).filter(Boolean);
  if (p.clear_key) over.clear_key = true;
  const current = { provider_id: over.id, model: $('ai-model').value || '' };
  try {
    const r = await apiFetch('/api/ai/config', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ providers: custom, current }),
    });
    if (r.ok) {
      await loadAiConfig();
      const status = $('ai-conn-status');
      if (status) status.textContent = (_t('status.saved') || 'Saved') + '✓';
      if (!silent) showToast(_t('toast.connSettingsSaved') || 'Connection settings saved');
    } else {
      const d = await r.json().catch(() => ({}));
      throw new Error(d.error || 'HTTP ' + r.status);
    }
  } catch (e) {
    showToast((_t('toast.saveFailed') || 'Save failed: ') + e.message);
  }
}

function getAiTargetText() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let sel = '';
  if ($('ai-selection').checked) {
    sel = ((window.getSelection && window.getSelection()) || {}).toString() || '';
    if (!sel) showToast(_t('toast.noTextSelectionNotice') || 'No text selected, will process full text');
  }
  if (sel) return { text: sel, isSelection: true };
  const src = state.mode === 'file'
    ? (state.original || state.fixed || '')
    : (state.fixed || state.original || '');
  return { text: src, isSelection: false };
}

function setAiBusy(b) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  state.ai.busy = b;
  $('ai-run').disabled = b;
  $('ai-stop').disabled = !b;
  $('ai-status').textContent = b ? (_t('ai.generating') || 'Generating...') : '';
}

function updateAiRawButtons() {
  const has = !!state.ai.raw;
  $('ai-apply').disabled = !has;
  $('ai-copy').disabled = !has;
  $('ai-saveas').disabled = !has;
}

async function loadAiModels() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const baseUrl = $('ai-base-url').value.trim();
  const key = $('ai-key').value.trim();
  const mode = $('ai-mode').value || 'auto';
  if (!baseUrl) { showToast(_t('toast.enterBaseUrlFirst') || 'Please enter Base URL first'); return; }
  const p = currentAiProvider();
  const local = p && p.name.indexOf('Ollama') >= 0;
  if (!local && !key && !(p && p.has_key)) { showToast(_t('toast.enterApiKeyFirst') || 'Please enter API Key first'); return; }
  const btn = $('ai-models-btn');
  const old = btn.textContent;
  btn.disabled = true; btn.textContent = _t('toast.fetchingModels') || 'Fetching...';
  const status = $('ai-conn-status');
  try {
    const q = new URLSearchParams({ provider: (p && p.id) || '', base_url: baseUrl, key: key, mode: mode });
    const r = await apiFetch('/api/ai/models?' + q.toString());
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error || ('HTTP ' + r.status));
    const ids = d.models || [];
    if (ids.length) {
      p.models = ids;
      fillAiModels(ids, $('ai-model').value);
      await saveAiSelection(true);
      if (status) status.textContent = (_t('toast.fetchedModels', { count: ids.length }) || ('已获取 ' + ids.length + ' 个模型')) + '✓';
      showToast(_t('toast.fetchedModels', { count: ids.length }) || ('已获取 ' + ids.length + ' 个模型'));
    } else {
      fillAiModels([], '');
      if (status) status.textContent = _t('toast.noModelsReturned') || 'API did not return available models';
      showToast(_t('toast.noModelsReturned') || 'API did not return available models');
    }
  } catch (e) {
    if (status) status.textContent = _t('toast.fetchFail') || 'Fetch failed';
    showToast((_t('toast.fetchModelsFail') || 'Failed to fetch models: ') + e.message);
  } finally {
    btn.disabled = false;
    btn.textContent = old;
  }
}

function toggleAiKey() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const inp = $('ai-key');
  inp.type = (inp.type === 'password') ? 'text' : 'password';
  $('ai-key-toggle').title = inp.type === 'password' ? (_t('ai.showOrHide') || '显示 / 隐藏') : (_t('ai.hideKey') || '隐藏');
}

function clearAiKey() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p) return;
  p.clear_key = true;
  p.has_key = false;
  $('ai-key').value = '';
  $('ai-conn-status').textContent = _t('ai.clearKeyOnSave') || 'Clear stored Key upon saving';
}

function resetAiUrl() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p) return;
  $('ai-base-url').value = p.base_url || '';
  const mode = p.mode || (p.format === 'anthropic' ? 'messages' : 'auto');
  $('ai-mode').value = (mode === 'anthropic') ? 'messages' : mode;
  showToast(_t('ai.resetUrlDone') || 'Preset URL restored');
}

function updateAiUsage() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const el = $('ai-usage');
  if (!el) return;
  const u = state.ai.usage;
  const s = state.ai.sessUsage;
  const fmt = n => (n == null ? 0 : n);
  const thisRound = _t('ai.thisRound') || '本次';
  const sessTotal = _t('ai.sessionTotal') || '会话累计';
  el.textContent = thisRound + ' ' + fmt(u && u.prompt_tokens) + '/' + fmt(u && u.completion_tokens) + '/' + fmt(u && u.total_tokens)
    + ' · ' + sessTotal + ' ' + fmt(s.prompt_tokens) + '/' + fmt(s.completion_tokens) + '/' + fmt(s.total_tokens);
}

async function runAi(action) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p) { showToast(_t('toast.selectProviderFirst') || 'Please select AI provider first'); return; }
  const keyVal = $('ai-key').value.trim();
  const local = p.name.indexOf('Ollama') >= 0;
  if (!local && !keyVal && !p.has_key && !p.key_source) { showToast(_t('toast.noApiKeyNotice') || 'API Key not configured: please open settings to complete connection'); return; }
  const { text, isSelection } = getAiTargetText();
  if (!text || !text.trim()) { showToast(_t('toast.noDocContentNotice') || 'No document content to process'); return; }
  const prompt = $('ai-prompt').value.trim();
  const isIncognito = $('ai-incognito').checked;
  const model = $('ai-model').value.trim() || (p.models || [''])[0] || '';
  const mode = $('ai-mode').value || 'auto';
  const baseUrl = $('ai-base-url').value.trim();
  const stream = $('ai-stream').checked;
  saveAiSelection();

  const tpl = currentAiTemplate();
  let sys = (tpl && tpl.system) || (_t('ai.system_' + action) || AI_SYSTEM[action] || '你是 ReadMD 的文档助手。');
  if (action === 'translate' && prompt && !(tpl && tpl.system)) {
    sys = _t('ai.system_translate_prompt', { lang: prompt }) || ('你是专业翻译。将用户给出的文档翻译成「' + prompt + '」，保留 Markdown 结构、表格与代码块，只输出译文。');
  }
  const docs = text.length > 120000 ? text.slice(0, 120000) + '\n\n' + (_t('ai.contentTruncated') || '[内容过长已截断，请分段处理]') : text;
  const fill = s => String(s || '').replace(/\{doc\}/g, docs).replace(/\{prompt\}/g, prompt || '');
  let userMsg;
  const docFollows = _t('ai.docFollows') || '文档如下：';
  if (tpl && tpl.user) {
    userMsg = fill(tpl.user);
  } else if (action === 'ask' && prompt) userMsg = docFollows + '\n\n' + docs + '\n\n' + (_t('ai.questionLabel') || '问题：') + prompt;
  else if (action === 'modify' && prompt) userMsg = docFollows + '\n\n' + docs + '\n\n' + (_t('ai.reqLabel') || '修改要求：') + prompt;
  else if (prompt) userMsg = docFollows + '\n\n' + docs + '\n\n' + (_t('ai.extraReqLabel') || '补充要求：') + prompt;
  else userMsg = docFollows + '\n\n' + docs;


  const msgs = (state.ai.messages || []).slice(-40);
  msgs.push({ role: 'user', content: userMsg, ephemeral: isIncognito });

  const out = $('ai-output');
  const userBubble = document.createElement('div');
  userBubble.className = 'ai-msg user';
  const uTag = document.createElement('div');
  uTag.className = 'ai-msg-tag';
  const userSeq = (state.ai.messages || []).filter(m => m.role === 'user').length + 1;
  const scopeText = isSelection ? (_t('ai.scopeSelection') || '（选中文字）') : (_t('ai.scopeFull') || '（全文）');
  uTag.textContent = (_t('ai.meTag', { seq: userSeq }) || ('我 · 提问 ' + userSeq)) + ' · ' + (_t('ai.action_' + action) || AI_ACTIONS[action] || action) + scopeText + ' · ' + model;
  const uBody = document.createElement('div');
  uBody.className = 'ai-msg-body';
  uBody.textContent = userMsg.length > 2000 ? userMsg.slice(0, 2000) + '\n' + (_t('ai.omittedLong') || '... (omitted)') : userMsg;
  userBubble.appendChild(uTag); userBubble.appendChild(uBody);
  out.appendChild(userBubble);

  const aiBubble = document.createElement('div');
  aiBubble.className = 'ai-msg ai';
  const aiTag = document.createElement('div');
  aiTag.className = 'ai-msg-tag';
  aiTag.textContent = _t('ai.generating') || 'AI generating...';
  const aiBody = document.createElement('div');
  aiBody.className = 'ai-msg-body';
  aiBubble.appendChild(aiTag); aiBubble.appendChild(aiBody);
  out.appendChild(aiBubble);
  out.scrollTop = out.scrollHeight;

  state.ai.raw = '';
  updateAiRawButtons();
  setAiBusy(true);
  const ctrl = new AbortController();
  state.ai.aborter = ctrl;
  let renderTimer = null;
  const render = () => {
    renderTimer = null;
    if (!state.ai.raw) return;
    const prot = protectMath(state.ai.raw);
    const html = marked.parse(prot.src, { gfm: true, breaks: false });
    aiBody.innerHTML = restoreMath(html, prot.saved);
    out.scrollTop = out.scrollHeight;
  };
  try {
    const r = await apiFetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        provider: p.id, model: model, api_key: keyVal || undefined,
        base_url: baseUrl || undefined, mode: mode, stream: stream,
        messages: [{ role: 'system', content: sys }].concat(msgs),
        temperature: 0.7,
      }),
      signal: ctrl.signal,
    });
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      throw new Error(d.error || ('HTTP ' + r.status));
    }
    const reader = r.body.getReader();
    const dec = new TextDecoder('utf-8');
    let buf = '';
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      let idx;
      while ((idx = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, idx).trim();
        buf = buf.slice(idx + 1);
        if (line.indexOf('data:') !== 0) continue;
        const data = line.slice(5).trim();
        if (!data) continue;
        let obj;
        try { obj = JSON.parse(data); } catch (e) { continue; }
        if (obj.error) throw new Error(obj.error);
        if (obj.done) break;
        if (obj.usage) {
          state.ai.usage = obj.usage;
          const s = state.ai.sessUsage;
          s.prompt_tokens += obj.usage.prompt_tokens || 0;
          s.completion_tokens += obj.usage.completion_tokens || 0;
          s.total_tokens += obj.usage.total_tokens || 0;
          updateAiUsage();
          continue;
        }
        if (obj.d === undefined) continue;
        state.ai.raw += obj.d;
        if (!renderTimer) renderTimer = setTimeout(render, state.ai.raw.length > 150000 ? 500 : 120);
      }
    }
    if (renderTimer) { clearTimeout(renderTimer); renderTimer = null; render(); }
    renderMath(aiBody);
    aiTag.textContent = (_t('ai.aiTag', { seq: userSeq }) || ('AI · 回答 ' + userSeq)) + ' · ' + model + fmtAiUsage(state.ai.usage);
    if (state.ai.raw) {
      aiTag.appendChild(aiAnswerCopyButton(state.ai.raw));
      const last = { role: 'assistant', content: state.ai.raw, ephemeral: isIncognito };
      if (state.ai.usage) last.usage = state.ai.usage;
      msgs.push(last);
      state.ai.messages = msgs;
      const saved = isIncognito ? false : await saveCurrentSession(true);
      updateAiRawButtons();
      showToast(isIncognito ? (_t('toast.aiIncognitoDone') || 'AI completed (incognito session unsaved)') : (saved ? (_t('toast.aiSavedDone') || 'AI completed and session saved automatically') : (_t('toast.aiSaveFailDone') || 'AI completed, but session save failed; retry from history')));
    } else {
      msgs.pop();
    }
  } catch (e) {
    if (e.name === 'AbortError') {
      aiTag.textContent = (_t('ai.aiTag', { seq: userSeq }) || ('AI · 回答 ' + userSeq)) + ' ' + (_t('ai.stoppedSuffix') || ' (stopped)');
      if (state.ai.raw) {
        const last = { role: 'assistant', content: state.ai.raw, ephemeral: isIncognito };
        if (state.ai.usage) last.usage = state.ai.usage;
        msgs.push(last);
        state.ai.messages = msgs;
      }
      showToast(_t('ai.stopped') || 'Stopped');
    } else {
      aiTag.textContent = _t('ai.aiError') || 'AI · Error';
      const hint = aiErrorHint(e);
      setAiConnectionState(hint.kind, hint.summary);
      showToast(hint.message);
      aiBody.innerHTML = '<p class="ai-err">' + String(e.message).replace(/[<>&]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c])) + '</p>';
    }
  } finally {
    setAiBusy(false);
    state.ai.aborter = null;
  }
}

async function testAiConnection() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p) return;
  const button = $('ai-test-connection'); const before = button.textContent;
  button.disabled = true; button.textContent = _t('toast.testingConn') || 'Testing...';
  setAiConnectionState('loading', _t('toast.testingConn') || '正在测试连接…');
  try {
    const q = new URLSearchParams({ provider: p.id || '', base_url: $('ai-base-url').value.trim(), key: $('ai-key').value.trim(), mode: $('ai-mode').value || 'auto' });
    const r = await apiFetch('/api/ai/models?' + q.toString());
    const data = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(data.error || 'HTTP ' + r.status);
    setAiConnectionState('ready', _t('toast.connReady', { name: p.name }) || (p.name + ' · 连接正常'));
    $('ai-conn-status').textContent = (_t('toast.connReady', { name: p.name }) || 'Connection OK') + (data.models && data.models.length ? (' · ' + data.models.length + ' ' + (_t('ai.modelsAvail') || ' models available')) : '');
    showToast(_t('toast.connTestPass') || 'Connection test passed');
  } catch (e) {
    const hint = aiErrorHint(e); setAiConnectionState(hint.kind, hint.summary); $('ai-conn-status').textContent = hint.message; showToast(hint.message);
  } finally { button.disabled = false; button.textContent = before; }
}

function aiConversationMarkdown(session) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const s = session || {};
  const title = String(s.title || (_t('ai.untitledSession') || 'Untitled Session')).replace(/[\r\n]/g, ' ').slice(0, 300);
  const lines = ['# ' + title, '', '> ' + (_t('ai.sourcePrefix') || '来源：') + 'ReadMD AI'];
  if (s.provider) lines.push('> ' + (_t('ai.providerPrefix') || '提供商：') + String(s.provider).slice(0, 120));
  if (s.model) lines.push('> ' + (_t('ai.modelPrefix') || '模型：') + String(s.model).slice(0, 160));
  if (s.updated || s.created) lines.push('> ' + (_t('ai.timePrefix') || '时间：') + fmtTime(s.updated || s.created));
  (s.messages || []).forEach(m => { if (m && (m.role === 'user' || m.role === 'assistant') && m.content) lines.push('', '## ' + (m.role === 'user' ? (_t('ai.userRole') || '用户') : (_t('ai.assistantRole') || 'AI 助手')), '', String(m.content)); });
  return lines.join('\n').trim() + '\n';
}

async function selectedConversationMarkdown() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let id = state.ai.sessionId || $('ai-session').value;
  if (!id && (state.ai.messages || []).length) return aiConversationMarkdown({ title: _t('ai.currentSession') || '当前会话', provider: $('ai-provider').value, model: $('ai-model').value, messages: state.ai.messages });
  if (!id) throw new Error(_t('toast.selectOrFinishSession') || '请先选择或完成一段会话');
  const r = await apiFetch('/api/ai/history?id=' + encodeURIComponent(id)); const d = await r.json();
  if (!r.ok || !d.session) throw new Error(d.error || (_t('toast.sessionNotExist') || 'Session does not exist'));
  return aiConversationMarkdown(d.session);
}

async function copyCurrentConversation() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try { await copyText(await selectedConversationMarkdown(), _t('toast.copiedConversation') || 'Copied entire conversation Markdown'); } catch (e) { showToast(e.message); }
}

async function exportCurrentConversation() {
  try { const md = await selectedConversationMarkdown(); await saveMarkdownText(md, 'readmd-conversation.md'); }
  catch (e) { showToast(e.message); }
}

async function saveMarkdownText(markdown, suggested) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (hasPy && py.save_as) {
    const result = await py.save_as(markdown, suggested || 'conversation.md');
    if (result) { showToast((_t('toast.savedPrefix') || 'Saved: ') + result); return; }
    showToast(_t('toast.notSaved') || 'Not saved'); return;
  }
  const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([markdown], { type: 'text/markdown;charset=utf-8' })); a.download = suggested || 'conversation.md'; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 1000); showToast(_t('toast.downloadStarted') || 'Download started');
}

/* ---------------- 安全对话导入 ---------------- */
let chatImportResult = null;
let aiModalReturnFocus = null;

function openAiModal(id, trigger) {
  aiModalReturnFocus = trigger || document.activeElement;
  const modal = $(id);
  if (modal) modal.classList.remove('hidden');
  const target = modal ? modal.querySelector('input, button, select, textarea') : null;
  if (target) setTimeout(() => target.focus(), 0);
}

function closeAiModal(id) {
  const modal = $(id);
  if (modal) modal.classList.add('hidden');
  if (aiModalReturnFocus && typeof aiModalReturnFocus.focus === 'function') {
    try { aiModalReturnFocus.focus(); } catch (e) {}
  }
}

function bindAiResize() {
  const handle = $('ai-resize-handle');
  if (!handle) return;
  let startX = 0, startWidth = 0;
  handle.addEventListener('pointerdown', e => {
    startX = e.clientX; startWidth = state.aiPanelWidth;
    handle.setPointerCapture(e.pointerId);
    document.body.classList.add('ai-resizing');
  });
  handle.addEventListener('pointermove', e => {
    if (!handle.hasPointerCapture(e.pointerId)) return;
    const max = Math.max(360, Math.floor(window.innerWidth * 0.94));
    state.aiPanelWidth = Math.max(360, Math.min(max, startWidth + startX - e.clientX));
    document.body.style.setProperty('--ai-panel-width', state.aiPanelWidth + 'px');
  });
  const finish = e => {
    if (!handle.hasPointerCapture(e.pointerId)) return;
    handle.releasePointerCapture(e.pointerId);
    document.body.classList.remove('ai-resizing');
    saveSettings();
  };
  handle.addEventListener('pointerup', finish);
  handle.addEventListener('pointercancel', finish);
}


function aiErrorHint(error) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const raw = String((error && error.message) || error || (_t('toast.unknownError') || '未知错误'));
  if (/401|403|auth|key|token|鉴权|密钥/i.test(raw)) return { kind: 'error', summary: _t('ai.authFailSummary') || '鉴权失败 · 检查 API Key', message: _t('ai.authFailMsg') || '鉴权失败：请在设置中检查 API Key 或权限' };
  if (/429|rate.?limit|限流|too many/i.test(raw)) return { kind: 'warn', summary: _t('ai.rateLimitSummary') || '请求受限 · 请稍后再试', message: _t('ai.rateLimitMsg') || '请求过于频繁：请稍后重试或更换模型' };
  if (/network|fetch|timeout|connect|网络|连接/i.test(raw)) return { kind: 'error', summary: _t('ai.netFailSummary') || '网络不可达 · 检查连接', message: _t('ai.netFailMsg') || '网络连接失败：请检查 Base URL、网络或代理' };
  return { kind: 'error', summary: _t('ai.reqFailSummary') || '请求失败 · 查看设置', message: (_t('ai.reqFailMsg') || 'AI 请求失败：') + raw };
}

function fmtAiUsage(u) {
  if (!u) return '';
  const t = u.total_tokens != null ? u.total_tokens : ((u.prompt_tokens || 0) + (u.completion_tokens || 0));
  return t ? ' · ' + t + ' tokens' : '';
}

async function copyAi() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.ai.raw) return;
  await copyText(state.ai.raw, _t('toast.copiedAnswer') || '已复制回答');
}

async function applyAi() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.ai.raw) return;
  const selOnly = $('ai-selection').checked;
  if (state.mode === 'file') {
    let next = state.ai.raw;
    if (selOnly) {
      const sel = ((window.getSelection && window.getSelection()) || {}).toString() || '';
      const cur = state.original || state.fixed || '';
      const i = sel ? cur.indexOf(sel) : -1;
      if (i >= 0) next = cur.slice(0, i) + state.ai.raw + cur.slice(i + sel.length);
      else { showToast(_t('toast.appliedSelectionFallback') || 'Could not locate selected text, applied to entire document'); }
    }
    state.original = next;
    state.fixed = next;
    exitEdit();
    await toggleEdit();
    showToast(_t('toast.appliedSavedNotice') || 'Applied. Please verify and press Ctrl+S to save (automatic backup on first save)');
  } else {
    state.fixed = state.ai.raw;
    state.original = state.ai.raw;
    renderContent(state.ai.raw, (state.sourceName || (_t('ai.aiResult') || 'AI 结果')) + ' · AI');
    updateStatus();
    showToast(_t('toast.appliedVirtualNotice') || 'Applied as virtual document. You can save as .md');
  }
}

async function saveAiAs() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.ai.raw) return;
  const base = (state.sourceName || state.file || 'document').replace(/[\\/]/g, '_');
  const suggested = base.replace(/\.[^.]+$/, '') + '.ai.md';
  if (hasPy) {
    const out = await py.save_as(state.ai.raw, suggested);
    if (out) showToast((_t('toast.savedPrefix') || 'Saved: ') + out);
  } else {
    const blob = new Blob([state.ai.raw], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
  }
}

