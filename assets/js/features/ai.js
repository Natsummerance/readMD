'use strict';
/* ============================================================
   ReadMD Features - AI Assistant & Dialog Import
   ============================================================ */

/* ---------------- AI 助手 ---------------- */

const AI_ACTIONS = {
  quick_read: 'tpl.actionQuickRead',
  polish: 'tpl.actionPolish',
  proofread: 'tpl.actionProofread',
  translate_en: 'tpl.actionTranslateEn',
  translate_zh: 'tpl.actionTranslateZh',
  todo: 'tpl.actionTodo',
  continue: 'tpl.actionContinue',
  ask: 'tpl.actionAsk',
  modify: 'tpl.actionModify',
  expand: 'tpl.actionExpand',
};

const AI_SKILLS = {
  quick_read: 'readmd-quick-read', polish: 'readmd-polish', proofread: 'readmd-proofread',
  translate_en: 'readmd-translate', translate_zh: 'readmd-translate', todo: 'readmd-todo',
  continue: 'readmd-continue', ask: 'readmd-ask', summary: 'readmd-summary',
  outline: 'readmd-outline', weekly: 'readmd-weekly', code_review: 'readmd-code-review',
  modify: 'readmd-format-fix', expand: 'readmd-polish', translate: 'readmd-translate',
  code_to_doc: 'readmd-code-to-doc', code_analysis: 'readmd-code-analysis'
};

function toggleAiPanel() {
  if (moduleBlocked('ai')) return;
  const p = $('ai-panel');
  p.classList.toggle('hidden');
  if (!p.classList.contains('hidden')) {
    updateAiUsage();
    if (!state.ai.config) loadAiOnDemand();
    else { loadAiPrompts(); loadAiSessions(); }
    if (!state.ai.messages || state.ai.messages.length === 0) {
      renderAiEmptyState();
    }
    setTimeout(() => $('ai-prompt') && $('ai-prompt').focus(), 0);
  }
}

function handleTopAiButtonClick() {
  const isEditing = !!state.editing;
  const isPreviewOpen = isEditing && state.pvLayout && state.pvLayout !== 'none';
  if (isEditing && isPreviewOpen) {
    if (typeof openEditAiBar === 'function') {
      openEditAiBar();
    }
  } else {
    toggleAiPanel();
  }
}
window.handleTopAiButtonClick = handleTopAiButtonClick;

function openAiPanelWithPrompt(act, promptText, targetText) {
  const p = $('ai-panel');
  if (!p) return;
  if (p.classList.contains('hidden')) {
    toggleAiPanel();
  }
  const promptInput = $('ai-prompt');
  if (promptInput && promptText) {
    promptInput.value = promptText;
  }
  // Code/document actions can supply an ephemeral target without embedding a
  // second instruction prompt in the UI bundle.  It is consumed by runAi and
  // never persisted to history as provider configuration.
  state.ai.targetOverride = typeof targetText === 'string' ? targetText : null;
  if (act) {
    setTimeout(() => runAi(act), 50);
  }
}
window.openAiPanelWithPrompt = openAiPanelWithPrompt;

function toggleAiFullscreen() {
  const p = $('ai-panel');
  if (!p) return;
  const isFull = p.classList.toggle('fullscreen');
  const icExpand = p.querySelector('.ai-ic-expand');
  const icCompress = p.querySelector('.ai-ic-compress');
  if (icExpand) icExpand.classList.toggle('hidden', isFull);
  if (icCompress) icCompress.classList.toggle('hidden', !isFull);
}

function renderAiEmptyState() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const out = $('ai-output');
  if (!out) return;
  if (state.ai.messages && state.ai.messages.length > 0) return;
  out.innerHTML = `
    <div class="ai-empty-state">
      <svg class="ai-empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
        <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
        <circle cx="12" cy="12" r="3.5"/>
      </svg>
      <div class="ai-empty-title">${_t('ai.emptyTitle') || '随时向 AI 提问或处理文档'}</div>
      <div class="ai-empty-desc">${_t('ai.emptyDesc') || '点击上方快捷指令进行全文总结、专业润色或学术翻译，也可在下方直接输入要求。'}</div>
    </div>
  `;
}

async function loadAiOnDemand() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  setAiConnectionState('loading', _t('ai.statusReading') || '正在读取连接…');
  try { await apiFetch('/api/modules/load', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: 'ai' }) }); } catch (e) { /* old servers use the normal poll path */ }
  const cfg = await loadAiConfig();
  if (!cfg) setAiConnectionState('warn', _t('ai.statusOffline') || '未连接');
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
  if (summary) summary.textContent = (model && model.value) || (_t('ai.noModel') || '未选择模型');
  if (!p) return setAiConnectionState('warn', _t('ai.noProvider') || '请选择连接');
  const local = isLocalAiProvider(p);
  if (local || p.has_key || p.key_source) setAiConnectionState('ready', p.name + ' · ' + (_t('ai.statusReady') || '已就绪'));
  else setAiConnectionState('warn', p.name + ' · ' + (_t('ai.needKey') || '需要 API Key'));
}
/* ---------------- Prompt 模板 ---------------- */

async function loadAiPrompts() {
  try {
    const [promptResponse, skillResponse] = await Promise.all([
      apiFetch('/api/ai/prompts'), apiFetch('/api/skills'),
    ]);
    const legacy = promptResponse.ok ? ((await promptResponse.json()).templates || []) : [];
    const skills = skillResponse.ok ? ((await skillResponse.json()).skills || []) : [];
    const legacyBySkill = new Map(legacy.filter(t => t && t.skill_id).map(t => [t.skill_id, t]));
    // The workbench is Skill-first: every selectable action resolves to the
    // shared registry, while legacy template metadata is retained only for a
    // one-version action/name compatibility layer.
    state.ai.templates = skills.map(s => {
      const old = legacyBySkill.get(s.id) || {};
      return {
        id: old.id || s.id, skill_id: s.id, name: old.name || s.name,
        action: old.action || 'custom', user: old.user || '',
        system: s.instructions || '', builtin: s.scope === 'builtin',
        scope: s.scope, metadata: s.metadata || {}, variables: s.variables || [],
        description: s.description || '', provenance: s.provenance || {},
        license: s.license || '', source_files: s.source_files || [],
        adaptation_notes: s.adaptation_notes || '',
      };
    });
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
  none.textContent = _t('ai.defaultAction') || '默认动作（不使用模板）';
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
  if (t && (t.action === 'translate' || t.action === 'translate_en' || t.action === 'translate_zh')) $('ai-prompt').placeholder = _t('ai.promptTranslatePlaceholder') || '翻译：目标语言（如：英语 / 日语）';
  else $('ai-prompt').placeholder = _t('ai.promptDefaultPlaceholder') || '补充要求 / 提问内容 / 翻译目标语言（可选）';
}

function openTplModal() {
  $('tpl-modal').classList.remove('hidden');
  if (!state.ai.templates.length) loadAiPrompts();
  renderTplList();
  const cur = state.ai.templateId || (state.ai.templates[0] && state.ai.templates[0].id) || null;
  selectTpl(cur);
}

function renderTplList() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const list = $('tpl-list');
  if (!list) return;
  const q = ($('tpl-search') && $('tpl-search').value || '').trim().toLowerCase();
  list.innerHTML = '';
  const filtered = (state.ai.templates || []).filter(t => !q || (t.name || '').toLowerCase().includes(q) || (t.system || '').toLowerCase().includes(q));
  if (!filtered.length) {
    const empty = document.createElement('li');
    empty.className = 'ai-history-empty';
    empty.textContent = _t('tpl.noTemplates') || '无匹配模板';
    list.appendChild(empty);
    return;
  }
  filtered.forEach(t => {
    const li = document.createElement('li');
    li.textContent = (t.builtin ? '◆ ' : '◇ ') + t.name;
    li.dataset.id = t.id;
    li.setAttribute('role', 'option');
    li.tabIndex = 0;
    li.setAttribute('aria-selected', 'false');
    li.title = t.name + (t.user ? (' · ' + (_t('ai.hasUserTpl') || '含用户消息模板')) : '');
    li.addEventListener('click', () => selectTpl(t.id));
    li.addEventListener('keydown', e => {
      if (e.key !== 'Enter' && e.key !== ' ') return;
      e.preventDefault();
      selectTpl(t.id);
    });
    list.appendChild(li);
  });
}

function skillLicenseText(value) {
  if (!value) return '';
  if (typeof value === 'string') return value;
  return value.spdx || value.id || value.name || value.license || '';
}

function skillRevisionText(value) {
  if (!value || typeof value !== 'object') return '';
  return value.revision || value.commit || value.ref || value.source_revision || '';
}

function appendSkillFact(host, text, prefix) {
  if (!text) return;
  const fact = document.createElement('span');
  fact.className = 'tpl-skill-fact';
  fact.textContent = (prefix || '') + text;
  host.appendChild(fact);
}

function renderSkillOverview(t) {
  const host = $('tpl-skill-overview');
  if (!host) return;
  host.replaceChildren();
  if (!t) return;
  const title = document.createElement('h4');
  title.textContent = t.name || t.skill_id || t.id || '';
  host.appendChild(title);
  if (t.description) {
    const description = document.createElement('p');
    description.textContent = t.description;
    host.appendChild(description);
  }
  const facts = document.createElement('div');
  facts.className = 'tpl-skill-facts';
  appendSkillFact(facts, t.scope || (t.builtin ? 'builtin' : 'user'), '◫ ');
  appendSkillFact(facts, skillLicenseText(t.license), '© ');
  appendSkillFact(facts, skillRevisionText(t.provenance), '@ ');
  (t.variables || []).forEach(variable => appendSkillFact(facts, variable, '{ } '));
  if (facts.childElementCount) host.appendChild(facts);

  const sourceLines = (Array.isArray(t.source_files) ? t.source_files : [])
    .map(item => {
      if (typeof item === 'string') return item;
      const file = item && (item.path || item.file || item.name) || '';
      const hash = item && (item.sha256 || item.hash) || '';
      return [file, hash ? hash.slice(0, 16) : ''].filter(Boolean).join('  ·  ');
    }).filter(Boolean);
  if (sourceLines.length) {
    const sources = document.createElement('pre');
    sources.className = 'tpl-skill-source';
    sources.textContent = sourceLines.join('\n');
    host.appendChild(sources);
  }
  if (t.adaptation_notes) {
    const note = document.createElement('p');
    note.textContent = t.adaptation_notes;
    host.appendChild(note);
  }
  if (t.system) {
    const instructions = document.createElement('pre');
    instructions.className = 'tpl-skill-instructions';
    instructions.textContent = t.system;
    host.appendChild(instructions);
  }
}

function setTplEditing(editing) {
  const selected = (state.ai.templates || []).find(x => x.id === $('tpl-id').value) || null;
  const builtin = !!(selected && selected.builtin);
  $('tpl-skill-overview') && $('tpl-skill-overview').classList.toggle('hidden', !!editing);
  $('tpl-editor-fields') && $('tpl-editor-fields').classList.toggle('hidden', !editing);
  if ($('tpl-edit')) {
    $('tpl-edit').classList.toggle('active', !!editing);
    $('tpl-edit').setAttribute('aria-pressed', editing ? 'true' : 'false');
  }
  if ($('tpl-save')) $('tpl-save').disabled = !editing || builtin;
  if ($('tpl-publish')) $('tpl-publish').disabled = !editing || !selected || builtin;
  if (editing && $('tpl-name')) $('tpl-name').focus();
}

function editCurrentSkill() {
  if (!$('tpl-id').value) return setTplEditing(true);
  setTplEditing(true);
}

function selectTpl(id, editing) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const t = (state.ai.templates || []).find(x => x.id === id) || null;
  const builtin = !!(t && t.builtin);
  document.querySelectorAll('#tpl-list li').forEach(li => {
    const selected = li.dataset.id === id;
    li.classList.toggle('active', selected);
    li.setAttribute('aria-selected', selected ? 'true' : 'false');
  });
  $('tpl-id').value = t ? t.id : '';
  $('tpl-name').value = t ? t.name : '';
  if ($('tpl-action')) $('tpl-action').value = (t && t.action) || 'custom';
  $('tpl-system').value = t ? (t.system || '') : '';
  $('tpl-user').value = t ? (t.user || '') : '';
  $('tpl-name').readOnly = builtin;
  $('tpl-system').readOnly = builtin;
  $('tpl-user').readOnly = builtin;
  $('tpl-del').disabled = !t || builtin;
  const status = $('tpl-draft-status');
  if (status) status.textContent = _t('tpl.hint');
  renderSkillOverview(t);
  setTplEditing(editing === true || !t);
}

function copyCurrentSkill() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const current = (state.ai.templates || []).find(x => x.id === $('tpl-id').value);
  if (!current) return;
  const base = String(current.skill_id || current.id || 'skill').replace(/[^a-z0-9-]/gi, '-').toLowerCase().replace(/^-+|-+$/g, '') || 'skill';
  const copyId = (base + '-custom').slice(0, 64);
  selectTpl(null, true);
  $('tpl-id').value = copyId;
  $('tpl-name').value = (current.name || copyId) + ' (Copy)';
  $('tpl-system').value = current.system || '';
  $('tpl-user').value = current.user || '';
  $('tpl-name').readOnly = $('tpl-system').readOnly = $('tpl-user').readOnly = false;
  if ($('tpl-save')) $('tpl-save').disabled = false;
  if ($('tpl-publish')) $('tpl-publish').disabled = false;
  if ($('tpl-draft-status')) $('tpl-draft-status').textContent = _t('tpl.hint');
  $('tpl-name').focus(); $('tpl-name').select();
}

async function generateSkillDraft() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p) { showToast(_t('toast.selectProviderFirst') || '请先选择 AI 提供商'); return; }
  if (!p.credential_id) {
    if (!(await saveAiSelection(true))) return;
  }
  const active = currentAiProvider() || p;
  if (!active.credential_id && !isLocalAiProvider(active)) {
    showToast(_t('toast.noApiKeyNotice') || '请先在连接设置中配置凭据'); return;
  }
  const request = window.prompt(_t('ai.promptPlaceholder'), '');
  if (!request || !request.trim()) return;
  const button = $('tpl-ai-generate');
  if (button) { button.disabled = true; button.textContent = _t('ai.generating'); }
  try {
    const r = await apiFetch('/api/skills', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'generate', provider: active.id, credential_id: active.credential_id,
        model: $('ai-model').value || (active.models || [])[0] || '', request, document: getAiTargetText().text,
        language: (window.i18n && window.i18n.locale) || document.documentElement.lang || 'en' }),
    });
    const d = await r.json().catch(() => ({}));
    if (!r.ok || !d.ok || !d.draft) throw new Error(d.error || _t('ai.aiError'));
    const draft = d.draft;
    const description = draft.description || _t('tpl.hint');
    const content = `---\nname: ${draft.id}\ndescription: ${description}\n---\n\n${draft.instructions || ''}`;
    state.ai.skillDraft = { id: draft.id, content, metadata: Object.assign({}, draft.metadata, { enabled: false }) };
    selectTpl(null);
    $('tpl-id').value = draft.id;
    $('tpl-name').value = draft.name || draft.id;
    $('tpl-system').value = content;
    $('tpl-user').value = '';
    if ($('tpl-publish')) $('tpl-publish').disabled = false;
    if ($('tpl-draft-status')) $('tpl-draft-status').textContent = _t('tpl.hint');
    showToast(_t('ai.generating'));
  } catch (e) {
    showToast(_t('ai.aiError') + ': ' + e.message);
  } finally {
    if (button) { button.disabled = false; button.textContent = _t('exportai.generateBtn'); }
  }
}

async function publishCurrentSkill() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const id = String($('tpl-id').value || '').trim();
  const content = String($('tpl-system').value || '').trim();
  if (!id || !content) { showToast(_t('ai.aiError')); return; }
  let evaluationToken = '';
  try {
    const evaluation = await apiFetch('/api/skills', { method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'evaluate', id, content, metadata: { id, source: 'skill-workbench', enabled: false, scripts_allowed: false },
        variables: { document: getAiTargetText().text, selection: '', request: '', language: (window.i18n && window.i18n.locale) || 'en', context: 'ReadMD Skill workbench', output_format: 'Markdown' } }) });
    const evaluated = await evaluation.json().catch(() => ({}));
    if (!evaluation.ok || !evaluated.ok || !evaluated.evaluation_token) throw new Error(evaluated.error || _t('ai.aiError'));
    evaluationToken = evaluated.evaluation_token;
  } catch (e) {
    showToast(_t('ai.aiError'));
    return;
  }
  if (!(await confirmAction({ title: _t('tpl.title') || _t('ai.aiError'), message: _t('tpl.hint'), confirmText: _t('exportai.generateBtn'), cancelText: _t('dialog.cancel'), danger: true }))) return;
  try {
    const r = await apiFetch('/api/skills', { method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'publish', confirm: true, evaluation_token: evaluationToken, id, content, metadata: { id, source: 'skill-workbench', enabled: true, scripts_allowed: false } }) });
    const d = await r.json().catch(() => ({}));
    if (!r.ok || !d.ok) throw new Error(d.error || _t('ai.aiError'));
    state.ai.skillDraft = null;
    await loadAiPrompts();
    showToast(_t('toast.saved') || _t('tpl.hint'));
  } catch (e) { showToast(_t('ai.aiError')); }
}

function parseMarkdownTemplate(content, filename) {
  const text = String(content || '').trim();
  let name = (filename || '未命名模板').replace(/\.(md|markdown|json|txt)$/i, '');
  let action = 'custom';
  let system = '';
  let user = '';

  // 1. YAML frontmatter 格式解析
  if (text.startsWith('---')) {
    const endIdx = text.indexOf('\n---', 3);
    if (endIdx > 0) {
      const fm = text.slice(3, endIdx).trim();
      const body = text.slice(endIdx + 4).trim();
      fm.split('\n').forEach(line => {
        const colon = line.indexOf(':');
        if (colon > 0) {
          const k = line.slice(0, colon).trim().toLowerCase();
          const v = line.slice(colon + 1).trim().replace(/^["']|["']$/g, '');
          if (k === 'name' || k === 'title') name = v;
          else if (k === 'action') action = v;
          else if (k === 'system' || k === 'prompt') system = v;
          else if (k === 'user' || k === 'template') user = v;
        }
      });
      if (!system && body) {
        system = body;
      } else if (system && body && !user) {
        user = body;
      }
      return { name, action, system, user };
    }
  }

  // 2. Markdown 标题格式解析
  const lines = text.split('\n');
  let bodyLines = [];
  let currentSection = null;
  let sectionBuffers = { system: [], user: [] };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const h1Match = line.match(/^#\s+(.+)$/);
    const h2Match = line.match(/^##\s+(.+)$/);

    if (h1Match && !system && !bodyLines.length) {
      name = h1Match[1].trim();
      continue;
    }
    if (h2Match) {
      const h2Text = h2Match[1].trim().toLowerCase();
      if (/system|系统|角色/.test(h2Text)) {
        currentSection = 'system';
        continue;
      } else if (/user|用户|模板|template/.test(h2Text)) {
        currentSection = 'user';
        continue;
      }
    }
    if (currentSection) {
      sectionBuffers[currentSection].push(line);
    } else {
      bodyLines.push(line);
    }
  }

  if (sectionBuffers.system.length || sectionBuffers.user.length) {
    system = sectionBuffers.system.join('\n').trim();
    user = sectionBuffers.user.join('\n').trim();
  } else {
    system = bodyLines.join('\n').trim() || text;
  }

  return { name, action, system, user };
}

async function importTemplatesFromFile(file) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!file) return;
  const isJson = /\.json$/i.test(file.name);
  const reader = new FileReader();
  reader.onload = async () => {
    try {
      const content = reader.result;
      let templates = [];
      if (isJson) {
        const parsed = JSON.parse(content);
        if (Array.isArray(parsed)) {
          templates = parsed;
        } else if (parsed && Array.isArray(parsed.templates)) {
          templates = parsed.templates;
        } else if (parsed && (parsed.system || parsed.name)) {
          templates = [parsed];
        }
      } else {
        const tpl = parseMarkdownTemplate(content, file.name);
        if (tpl && (tpl.system || tpl.name)) {
          templates = [tpl];
        }
      }

      if (!templates.length) {
        showToast(_t('toast.noValidTemplates') || '未能解析到有效模板');
        return;
      }

      const r = await apiFetch('/api/ai/prompts', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'batch_save', templates }),
      });
      const d = await r.json();
      if (!r.ok || !d.ok) throw new Error(d.error || '导入失败');

      await loadAiPrompts();
      renderTplList();
      showToast((_t('toast.importedTemplates', { count: templates.length }) || `成功导入 ${templates.length} 个模板`));
    } catch (e) {
      showToast((_t('toast.importFailed') || '导入失败：') + e.message);
    }
  };
  reader.readAsText(file, 'UTF-8');
}

function toggleSkillImportMenu(force) {
  const menu = $('tpl-import-menu');
  const trigger = $('tpl-import-btn');
  if (!menu) return;
  const open = typeof force === 'boolean' ? force : menu.classList.contains('hidden');
  menu.classList.toggle('hidden', !open);
  if (trigger) trigger.setAttribute('aria-expanded', open ? 'true' : 'false');
  if (open) selectSkillImportSource('github', false);
}

async function selectSkillImportSource(source, openPicker = true) {
  const allowed = new Set(['github', 'folder', 'zip']);
  const selected = allowed.has(source) ? source : 'github';
  allowed.forEach(name => {
    const button = $(`tpl-import-source-${name}`);
    if (!button) return;
    const active = name === selected;
    button.classList.toggle('active', active);
    button.setAttribute('aria-selected', active ? 'true' : 'false');
  });
  const githubPanel = $('tpl-import-panel-github');
  if (githubPanel) githubPanel.classList.toggle('hidden', selected !== 'github');
  if (!openPicker) return;
  if (selected === 'github') {
    $('tpl-github-url') && $('tpl-github-url').focus();
    return;
  }
  if (typeof hasPy !== 'undefined' && hasPy && py && py.choose_skill_source) {
    const sourcePath = await py.choose_skill_source(selected === 'folder' ? 'directory' : 'zip');
    if (sourcePath) await previewSkillImportSource({
      source_type: selected === 'folder' ? 'directory' : 'zip',
      source: sourcePath,
    });
    return;
  }
  if (selected === 'folder' && $('tpl-folder-input')) $('tpl-folder-input').click();
  if (selected === 'zip' && $('tpl-zip-input')) $('tpl-zip-input').click();
}

function revealGithubCredential() {
  const wrap = $('tpl-github-credential-wrap');
  const input = $('tpl-github-credential');
  if (wrap) wrap.classList.remove('hidden');
  if (input) requestAnimationFrame(() => input.focus());
}

async function previewSkillImportSource(payload) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    const r = await apiFetch('/api/skill-imports/preview', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    const d = await r.json().catch(() => ({}));
    if (!r.ok || !d.ok || !d.preview) throw d;
    const preview = d.preview;
    preview.credential_id = d.credential_id || payload.credential_id || '';
    renderGithubSkillPreview(preview);
    return preview;
  } catch (e) {
    if (String(e && e.error_code || '') === 'github_auth_failed') revealGithubCredential();
    showToast(githubImportErrorText(e, _t));
    return null;
  }
}

async function previewBrowserZipSkill(file) {
  if (!file) return;
  const path = typeof uploadFile === 'function' ? await uploadFile(file) : null;
  if (path) await previewSkillImportSource({ source_type: 'zip', source: path });
}

function githubImportErrorText(payload, _t) {
  const code = String(payload && payload.error_code || '');
  const known = {
    github_auth_failed: 'toast.enterApiKeyFirst',
    github_network_error: 'toast.unknownNetworkErr',
    github_not_found: 'toast.fileNotFound',
    github_url_required: 'toast.invalidUrl',
  };
  const key = known[code] || 'toast.importFailed';
  const text = _t(key) || _t('toast.importFailed') || '导入失败';
  return code ? `${text} (${code})` : text;
}

function renderGithubSkillPreview(preview) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const host = $('tpl-github-preview');
  if (!host) return;
  host.innerHTML = '';
  const skills = Array.isArray(preview && preview.skills) ? preview.skills : [];
  skills.forEach((skill, index) => {
    const row = document.createElement('label');
    row.className = 'tpl-github-item';
    row.dataset.index = String(index);
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox'; checkbox.checked = !!skill.valid; checkbox.disabled = !skill.valid;
    checkbox.dataset.skillIndex = String(index);
    checkbox.setAttribute('aria-label', skill.name || skill.id || skill.path || 'Skill');
    const details = document.createElement('span');
    const title = document.createElement('strong');
    title.textContent = skill.name || skill.id || skill.path || '';
    const meta = document.createElement('small');
    meta.textContent = [skill.path, skill.scripts_present ? '⚙' : ''].filter(Boolean).join(' · ');
    details.append(title, meta);
    if (skill.description) {
      const desc = document.createElement('small');
      desc.textContent = skill.description;
      details.appendChild(desc);
    }
    row.append(checkbox, details);
    host.appendChild(row);
  });
  const apply = document.createElement('button');
  apply.type = 'button'; apply.id = 'tpl-github-apply-btn'; apply.className = 'tb-btn accent tpl-github-apply-btn';
  apply.textContent = _t('tpl.importMd') || '导入模板 (.md)';
  apply.disabled = !skills.some(s => s.valid);
  apply.addEventListener('click', () => applyGithubSkillImport(preview));
  host.appendChild(apply);
}

async function previewGithubSkillImport() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const url = String($('tpl-github-url') && $('tpl-github-url').value || '').trim();
  const githubToken = String($('tpl-github-credential') && $('tpl-github-credential').value || '').trim();
  if (!url) { showToast(_t('toast.invalidUrl') || '请输入有效链接'); return; }
  const button = $('tpl-github-preview-btn');
  if (button) { button.disabled = true; button.classList.add('tpl-github-importing'); }
  try {
    await previewSkillImportSource({ source_type: 'github', source: url, github_token: githubToken });
    if ($('tpl-github-credential')) $('tpl-github-credential').value = '';
  } finally {
    if (button) { button.disabled = false; button.classList.remove('tpl-github-importing'); }
  }
}

async function applyGithubSkillImport(preview) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const host = $('tpl-github-preview');
  const selected = [...(host ? host.querySelectorAll('input[type="checkbox"]:checked') : [])]
    .map(input => preview.skills[Number(input.dataset.skillIndex)])
    .filter(Boolean)
    .map(skill => Object.assign({}, skill, { conflict_action: 'skip' }));
  if (!selected.length) return;
  const confirmed = await confirmAction({
    title: _t('tpl.title') || 'Skill 工作台',
    message: _t('tpl.hint') || '导入前请检查 Skill 内容和来源。',
    confirmText: _t('tpl.importMd') || '导入模板 (.md)',
    cancelText: _t('dialog.cancel') || '取消',
  });
  if (!confirmed) return;
  const button = $('tpl-github-apply-btn');
  if (button) { button.disabled = true; button.classList.add('tpl-github-importing'); }
  try {
    const r = await apiFetch('/api/skill-imports/apply', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ preview, selections: selected, credential_id: preview.credential_id || '', confirm: true }),
    });
    const d = await r.json().catch(() => ({}));
    if (!r.ok || !d.ok) throw d;
    await loadAiPrompts(); renderTplList();
    if (host) host.innerHTML = '';
    showToast(_t('toast.importedTemplates', { count: (d.skills || []).length }) || '已导入');
  } catch (e) {
    showToast(githubImportErrorText(e, _t));
  } finally {
    if (button) { button.disabled = false; button.classList.remove('tpl-github-importing'); }
  }
}

function exportTemplatesAsJson() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const data = {
    version: '1.0',
    exported_at: new Date().toISOString(),
    templates: state.ai.templates || [],
  };
  const jsonStr = JSON.stringify(data, null, 2);
  const blob = new Blob([jsonStr], { type: 'application/json;charset=utf-8' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'readmd_prompts_backup.json';
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 3000);
  showToast(_t('toast.exportedTemplates') || '已导出全部 Prompt 模板');
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
  if (!t.name) { showToast(_t('toast.tplNameReq') || '请输入模板名称'); return; }
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
    showToast(_t('toast.tplSaved') || '模板已保存');
  } catch (e) { showToast((_t('toast.saveFailed') || '保存失败：') + e.message); }
}

async function deleteCurrentTpl() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const id = $('tpl-id').value;
  if (!id) return;
  const cur = (state.ai.templates || []).find(x => x.id === id);
  const templateName = cur?.name || (_t('ai.untitledTemplate') || '未命名模板');
  const msg = cur && cur.builtin
    ? (_t('toast.tplResetConfirm', { name: templateName }) || '将重置为默认模板，确定吗？')
    : (_t('toast.tplDelConfirm', { name: templateName }) || '确定删除此模板吗？');
  if (!(await confirmAction({
    title: _t('dialog.destructiveTitle') || '请确认',
    message: msg,
    confirmText: _t('dialog.confirm') || '确认',
    cancelText: _t('dialog.cancel') || '取消',
    danger: true,
  }))) {
    return false;
  }
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
    showToast(_t('toast.tplDeleted', { name: templateName }) || '模板已删除');
  } catch (e) { showToast((_t('toast.deleteFailed') || '删除失败：') + e.message); }
  return true;
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
    empty.textContent = _t('ai.historyEmpty') || '暂无历史会话';
    list.appendChild(empty);
    return;
  }
  rows.forEach(s => {
    const row = document.createElement('div'); row.className = 'ai-history-item';
    const load = document.createElement('button'); load.className = 'tb-btn ai-history-load';
    load.innerHTML = '<strong></strong><small></small>';
    load.querySelector('strong').textContent = s.title || (_t('ai.untitledSession') || '未命名会话');
    load.querySelector('small').textContent = fmtTime(s.updated) + ' · ' + (s.msgCount || 0) + ' ' + (_t('ai.msgCountUnit') || '条');
    load.addEventListener('click', async () => { $('ai-session').value = s.id; await onAiSessionChange(); closeAiModal('ai-history-modal'); });
    const rename = document.createElement('button'); rename.className = 'tb-btn'; rename.textContent = _t('tabs.rename') || '改名'; rename.title = _t('ai.renameSession') || '重命名会话';
    rename.addEventListener('click', () => renameAiSession(s));
    const del = document.createElement('button'); del.className = 'tb-btn'; del.textContent = _t('tpl.delete') || '删'; del.title = _t('ai.deleteSession') || '删除会话';
    del.addEventListener('click', async () => {
      if (!window.confirm((_t('ai.confirmDeleteSession') || '确定删除会话：') + (s.title || (_t('ai.untitledSession') || '未命名会话')) + '？')) return;
      await deleteAiSessionById(s.id);
    });
    row.append(load, rename, del); list.appendChild(row);
  });
}

async function renameAiSession(summary) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const title = window.prompt(_t('ai.sessionName') || '会话名称', summary.title || '');
  if (title === null) return;
  const next = title.trim().slice(0, 80);
  if (!next) { showToast(_t('toast.sessionNameEmpty') || '会话名称不能为空'); return; }
  try {
    const r = await apiFetch('/api/ai/history?id=' + encodeURIComponent(summary.id));
    const d = await r.json(); if (!r.ok || !d.session) throw new Error(d.error || '会话不存在');
    d.session.title = next;
    const saved = await apiFetch('/api/ai/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'save', session: d.session }) });
    if (!saved.ok) throw new Error((await saved.json().catch(() => ({}))).error || '保存失败');
    await loadAiSessions(); showToast(_t('toast.sessionRenamed') || '会话已重命名');
  } catch (e) { showToast((_t('toast.renameFailed') || '重命名失败：') + e.message); }
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
    if (!r.ok) { showToast(_t('toast.loadSessionFail') || '加载会话失败'); return; }
    const s = (await r.json()).session;
    if (!s) { showToast(_t('toast.sessionNotExist') || '会话不存在'); return; }
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
    showToast(_t('toast.sessionLoaded') || '已加载会话');
  } catch (e) { showToast(_t('toast.loadSessionFail') || '加载会话失败'); }
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
      body.textContent = m.content.length > 3000 ? m.content.slice(0, 3000) + '\n' + (_t('ai.omittedLong') || '…（已省略）') : m.content;
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
      body.innerHTML = renderSafeMarkdown(m.content);
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
  if (!msgs.length) { if (!silent) showToast((state.ai.messages || []).length ? (_t('toast.incognitoNoSave') || '当前会话为无痕内容，不会保存') : (_t('toast.noConversationContent') || '当前没有对话内容')); return false; }
  const title = ($('ai-prompt').value.trim() || msgs[0].content || (_t('ai.untitledSession') || '未命名会话')).slice(0, 40).replace(/\s+/g, ' ');
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
    if (!silent) showToast(_t('toast.sessionSaved') || '会话已保存');
    return true;
  } catch (e) { if (!silent) showToast((_t('toast.saveFailed') || '保存失败：') + e.message); return false; }
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
    showToast(_t('toast.sessionDeleted') || '会话已删除');
  } catch (e) { showToast((_t('toast.deleteFailed') || '删除失败：') + e.message); }
}

function clearAiContext() {
  if (!(state.ai.messages || []).length) { showToast(_t('toast.noContext') || '当前没有上下文'); return; }
  state.ai.messages = [];
  state.ai.sessionId = null;
  state.ai.raw = '';
  state.ai.usage = null;
  state.ai.sessUsage = { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
  updateAiUsage();
  clearAiOutput();
  $('ai-session').value = '';
  showToast(_t('toast.contextCleared') || '已清空上下文，开始新一轮');
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
    state.ai.upstreamCatalog = Array.isArray(cfg.upstream_catalog) ? cfg.upstream_catalog : [];
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
  const customGroup = document.createElement('optgroup'); customGroup.label = _t('ai.customConnections') || '自定义连接';
  const presetGroup = document.createElement('optgroup'); presetGroup.label = _t('ai.officialPresets') || '官方预设';
  merged.forEach(p => {
    const o = document.createElement('option');
    o.value = p.id;
    o.textContent = p.name;
    (p.custom ? customGroup : presetGroup).appendChild(o);
  });
  if (customGroup.children.length) sel.appendChild(customGroup);
  if (presetGroup.children.length) sel.appendChild(presetGroup);
  if (curId) sel.value = curId;
  const cards = $('ai-provider-cards');
  const search = $('ai-provider-search');
  if (cards) {
    const query = String(search && search.value || '').trim().toLowerCase();
    const sourceEntries = (state.ai.upstreamCatalog || []).map(p => Object.assign({ source_only: true }, p));
    const cardProviders = merged.concat(sourceEntries);
    cards.innerHTML = '';
    cardProviders.filter(p => !query || [p.name, p.note, p.website, p.category, p.format].some(v => String(v || '').toLowerCase().includes(query)))
      .forEach(p => {
        const card = document.createElement('button');
        card.type = 'button'; card.className = 'ai-provider-card' + (p.id === curId ? ' active' : '') + (p.source_only ? ' source-only' : '');
        if (p.source_only) card.disabled = true;
        card.setAttribute('role', 'option'); card.setAttribute('aria-selected', p.id === curId ? 'true' : 'false');
        const title = document.createElement('strong'); title.textContent = p.name || p.id;
        const meta = document.createElement('span');
        const caps = p.capabilities && typeof p.capabilities === 'object' ? Object.keys(p.capabilities).filter(k => p.capabilities[k]).slice(0, 3) : [];
        const capLabels = { chat: _t('ai.aiResult') || 'Chat', models: _t('ai.model') || 'Models', vision: _t('ai.explain') || 'Vision', tools: _t('ai.advanced') || 'Tools' };
        const kind = p.source_only ? (_t('ai.sourcePrefix') || '离线来源目录') : (p.custom ? (_t('ai.customConnections') || 'Custom') : (_t('ai.officialPresets') || 'Presets'));
        meta.textContent = kind + (caps.length ? ' · ' + caps.map(k => capLabels[k] || k).join(' · ') : '');
        const status = document.createElement('i'); status.className = 'ai-provider-state'; status.setAttribute('aria-label', p.has_key ? (_t('ai.keyConfigured') || 'Key configured') : (_t('ai.noKeyConfigured') || 'No key configured')); status.textContent = p.has_key ? '●' : '○';
        card.append(title, meta, status);
        if (!p.source_only) card.addEventListener('click', () => { sel.value = p.id; onAiProviderChange(); fillAiProviders(merged, { provider_id: p.id, model: $('ai-model').value }); });
        cards.appendChild(card);
      });
  }
  onAiProviderChange();
}

function currentAiProvider() {
  const id = $('ai-provider').value;
  return (state.ai.providers || []).find(p => p.id === id || p.name === id) || null;
}

async function resolveSharedAiConnection() {
  if (!state.ai.config) await loadAiConfig();
  const current = (state.ai.config && state.ai.config.current) || {};
  const currentId = current.provider_id || current.provider || '';
  const selectedId = $('ai-provider') ? $('ai-provider').value : '';
  const providers = state.ai.providers || [];
  const provider = providers.find(p => p.id === selectedId || p.name === selectedId)
    || providers.find(p => p.id === currentId || p.name === currentId)
    || null;
  if (!provider) return null;
  const selectedModel = $('ai-model') ? $('ai-model').value : '';
  let headers = {};
  try { headers = readAiCustomHeaders(); } catch (e) { headers = provider.headers || {}; }
  return {
    provider: provider.id || provider.name || '',
    credential_id: provider.credential_id || undefined,
    model: selectedModel || current.model || (provider.models || [])[0] || '',
    base_url: provider.base_url || undefined,
    mode: provider.mode || (provider.format === 'anthropic' ? 'messages' : 'auto'),
    endpoint_mode: provider.endpoint_mode || 'prefix',
    headers,
    has_key: !!(provider.has_key || provider.key_source || provider.credential_id),
    local: isLocalAiProvider(provider),
  };
}

function isLocalAiProvider(provider) {
  if (!provider) return false;
  if (String(provider.category || '').toLowerCase() === 'local') return true;
  const url = String(provider.base_url || '').toLowerCase();
  return /(^|[/:])(?:localhost|127\.0\.0\.1|\[::1\])(?::|\/|$)/.test(url);
}

function aiPresetBase(p) {
  return (p && p.base_url) || '';
}

function readAiCustomHeaders() {
  const raw = $('ai-headers') ? $('ai-headers').value.trim() : '';
  if (!raw) return {};
  const parsed = JSON.parse(raw);
  if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') {
    throw new Error('请求头必须是 JSON 对象');
  }
  return parsed;
}

function fillAiModels(models, selected) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = $('ai-model');
  sel.innerHTML = '';
  const list = Array.isArray(models) ? models.filter(Boolean) : [];
  const placeholder = new Option(list.length ? (_t('ai.selectModel') || '选择模型') : (_t('ai.fetchModelsFirst') || '请先获取模型'), '');
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
  if ($('ai-endpoint-mode')) $('ai-endpoint-mode').value = p.endpoint_mode || 'prefix';
  const current = (state.ai.config && state.ai.config.current) || {};
  fillAiModels(p.models, (current.provider_id || current.provider) === p.id ? current.model : '');
  $('ai-provider-name').value = p.name || '';
  $('ai-provider-name').disabled = !p.custom;
  $('ai-provider-delete').disabled = !p.custom;
  const headers = $('ai-headers');
  if (headers) headers.value = p.headers && typeof p.headers === 'object' ? JSON.stringify(p.headers, null, 2) : '';
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
     : (isLocalAiProvider(p) ? (_t('ai.apiKeyOllama') || 'API Key（本地服务可留空）') : (_t('ai.apiKeyRequired') || 'API Key（必填）'));
  if (status) {
    status.textContent = p.has_key
      ? (p.key_source ? (_t('ai.keyReady', { source: p.key_source }) || ('Key 就绪（' + p.key_source + '）')) : (_t('ai.keyConfigured') || 'Key 已配置'))
       : (isLocalAiProvider(p) ? (_t('ai.localNoKey') || '本地模型无需 Key') : (_t('ai.noKeyConfigured') || '未配置 Key'));
  }
  updateAiConnectionSummary();
}

function aiAnswerCopyButton(content) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const button = document.createElement('button');
  button.className = 'tb-btn ai-msg-copy'; button.textContent = _t('toast.copiedAnswer') || '复制回答';
  button.addEventListener('click', () => copyText(String(content || ''), _t('toast.copiedAnswer') || '已复制回答'));
  return button;
}

async function copyText(value, success) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!value) return;
  try { await navigator.clipboard.writeText(value); showToast(success || (_t('toast.copied') || '已复制')); }
  catch (e) { const ta = document.createElement('textarea'); ta.value = value; document.body.appendChild(ta); ta.select(); try { document.execCommand('copy'); showToast(success || (_t('toast.copied') || '已复制')); } catch (e2) { showToast(_t('toast.copyFailed') || '复制失败'); } ta.remove(); }
}

async function deleteAiSessionById(id) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!id) return;
  try {
    const r = await apiFetch('/api/ai/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'delete', id }) });
    const d = await r.json(); if (!r.ok || !d.ok) throw new Error(d.error || (_t('toast.deleteFailed') || '删除失败'));
    if (state.ai.sessionId === id) clearAiContext();
    await loadAiSessions(); showToast(_t('toast.sessionDeleted') || '会话已删除');
  } catch (e) { showToast((_t('toast.deleteFailed') || '删除失败：') + e.message); }
}

async function clearAiSessions() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!(state.ai.sessions || []).length || !window.confirm(_t('toast.clearSessionsConfirm') || '清空全部会话历史？此操作无法撤销。')) return;
  try {
    const r = await apiFetch('/api/ai/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'clear' }) });
    if (!r.ok) throw new Error((await r.json().catch(() => ({}))).error || (_t('toast.clearFailed') || '清空失败'));
    state.ai.sessionId = null; state.ai.sessions = []; fillAiSessions(); showToast(_t('toast.sessionsCleared') || '会话历史已清空');
  } catch (e) { showToast((_t('toast.clearFailed') || '清空失败：') + e.message); }
}

function newAiProvider() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.ai.config) return;
  const custom = state.ai.config.custom || (state.ai.config.custom = []);
  let seq = custom.length + 1;
  const connPrefix = _t('ai.customConnections') || '自定义连接';
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
    showToast(_t('toast.customConnDeleted') || '已删除自定义连接');
  } catch (e) { showToast((_t('toast.deleteFailed') || '删除失败：') + e.message); }
}


async function saveAiSelection(silent) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let p = currentAiProvider();
  if (!p || !state.ai.config) return false;
  const custom = (state.ai.config.custom || []).map(c => Object.assign({}, c));
  const keyVal = $('ai-key').value.trim();
  const baseUrl = $('ai-base-url').value.trim();
  const mode = $('ai-mode').value || 'auto';
  const endpointMode = $('ai-endpoint-mode') ? ($('ai-endpoint-mode').value || 'prefix') : 'prefix';
  try {
    var customHeaders = readAiCustomHeaders();
  } catch (e) {
    showToast((_t('toast.saveFailedSimple') || '请求头 JSON 无效：') + ' ' + e.message);
    return false;
  }
  const requestedName = $('ai-provider-name').value.trim() || p.name;
  if (p.custom && requestedName !== p.name && custom.some(c => c.name === requestedName)) {
    showToast(_t('toast.customConnNameExists') || '自定义连接名称已存在'); return false;
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
  over.endpoint_mode = endpointMode;
  over.headers = customHeaders;
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
      if (status) status.textContent = _t('status.saved') || '已保存';
      if (!silent) showToast(_t('toast.connSettingsSaved') || '连接设置已保存');
      return true;
    } else {
      const d = await r.json().catch(() => ({}));
      throw new Error(d.error || 'HTTP ' + r.status);
    }
  } catch (e) {
    showToast((_t('toast.saveFailed') || '保存失败：') + e.message);
    return false;
  }
}

function getAiTargetText() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let sel = '';
  if ($('ai-selection').checked) {
    sel = ((window.getSelection && window.getSelection()) || {}).toString() || '';
    if (!sel) showToast(_t('toast.noTextSelectionNotice') || '未选中文字，将处理全文');
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
  $('ai-status').textContent = b ? (_t('ai.generating') || '生成中…') : '';
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
  const endpointMode = $('ai-endpoint-mode') ? ($('ai-endpoint-mode').value || 'prefix') : 'prefix';
  if (!baseUrl) { showToast(_t('toast.enterBaseUrlFirst') || '请先填写 Base URL'); return; }
  let p = currentAiProvider();
  const local = isLocalAiProvider(p);
  if (!local && !key && !(p && p.has_key)) { showToast(_t('toast.enterApiKeyFirst') || '请先填写 API Key'); return; }
  try {
    var requestHeaders = readAiCustomHeaders();
  } catch (e) {
    showToast((_t('toast.saveFailedSimple') || '请求头 JSON 无效：') + ' ' + e.message);
    return;
  }
  const btn = $('ai-models-btn');
  const old = btn.textContent;
  btn.disabled = true; btn.textContent = _t('toast.fetchingModels') || '获取中…';
  const status = $('ai-conn-status');
  try {
    const r = await apiFetch('/api/ai/models', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ provider: (p && p.id) || '', credential_id: (p && p.credential_id) || undefined, base_url: baseUrl, mode: mode, endpoint_mode: endpointMode, headers: requestHeaders })
    });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error || ('HTTP ' + r.status));
    const ids = d.models || [];
    if (ids.length) {
      p.models = ids;
      fillAiModels(ids, $('ai-model').value);
      await saveAiSelection(true);
      if (status) status.textContent = _t('toast.fetchedModels', { count: ids.length }) || ('已获取 ' + ids.length + ' 个模型');
      showToast(_t('toast.fetchedModels', { count: ids.length }) || ('已获取 ' + ids.length + ' 个模型'));
    } else {
      fillAiModels([], '');
      if (status) status.textContent = _t('toast.noModelsReturned') || '接口未返回可选模型';
      showToast(_t('toast.noModelsReturned') || '接口未返回可选模型');
    }
  } catch (e) {
    if (status) status.textContent = _t('toast.fetchFail') || '获取失败';
    showToast((_t('toast.fetchModelsFail') || '获取模型失败：') + e.message);
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
  $('ai-conn-status').textContent = _t('ai.clearKeyOnSave') || '保存后清除已存 Key';
}

function resetAiUrl() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = currentAiProvider();
  if (!p) return;
  $('ai-base-url').value = p.base_url || '';
  const mode = p.mode || (p.format === 'anthropic' ? 'messages' : 'auto');
  $('ai-mode').value = (mode === 'anthropic') ? 'messages' : mode;
  showToast(_t('ai.resetUrlDone') || '已恢复预设地址');
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
  if (!p) { showToast(_t('toast.selectProviderFirst') || '请先选择 AI 提供商'); return; }
  const keyVal = $('ai-key').value.trim();
  const local = isLocalAiProvider(p);
  if (!local && !keyVal && !p.has_key && !p.key_source) { showToast(_t('toast.noApiKeyNotice') || '未配置 API Key：请打开设置完成连接'); return; }
  const target = state.ai.targetOverride;
  const { text, isSelection } = target != null
    ? { text: target, isSelection: true }
    : getAiTargetText();
  state.ai.targetOverride = null;
  if (!text || !text.trim()) { showToast(_t('toast.noDocContentNotice') || '没有可处理的文档内容'); return; }
  const prompt = $('ai-prompt').value.trim();
  const isIncognito = $('ai-incognito').checked;
  const model = $('ai-model').value.trim() || (p.models || [''])[0] || '';
  const mode = $('ai-mode').value || 'auto';
  const baseUrl = $('ai-base-url').value.trim();
  const stream = $('ai-stream').checked;
  const endpointMode = $('ai-endpoint-mode') ? ($('ai-endpoint-mode').value || 'prefix') : 'prefix';
  // Persist a newly entered credential before starting the request. This
  // prevents the request from racing the config write and lets the server
  // resolve the opaque credential_id instead of receiving a raw key.
  if (!(await saveAiSelection(true))) return;
  const activeProvider = currentAiProvider() || p;
  let requestHeaders = {};
  try { requestHeaders = readAiCustomHeaders(); } catch (e) { return; }

  const tpl = currentAiTemplate();
  const skillId = (tpl && tpl.skill_id) || AI_SKILLS[action] || 'readmd-ask';
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
  const emptyState = out.querySelector('.ai-empty-state');
  if (emptyState) emptyState.remove();

  const userBubble = document.createElement('div');
  userBubble.className = 'ai-msg user';
  const uTag = document.createElement('div');
  uTag.className = 'ai-msg-tag';
  const userSeq = (state.ai.messages || []).filter(m => m.role === 'user').length + 1;
  const scopeText = isSelection ? (_t('ai.scopeSelection') || '（选中文字）') : (_t('ai.scopeFull') || '（全文）');
  uTag.textContent = (_t('ai.meTag', { seq: userSeq }) || ('我 · 提问 ' + userSeq)) + ' · ' + _t(AI_ACTIONS[action] || action) + scopeText + ' · ' + model;
  const uBody = document.createElement('div');
  uBody.className = 'ai-msg-body';
  uBody.textContent = userMsg.length > 2000 ? userMsg.slice(0, 2000) + '\n' + (_t('ai.omittedLong') || '…（已省略）') : userMsg;
  userBubble.appendChild(uTag); userBubble.appendChild(uBody);
  out.appendChild(userBubble);

  const aiBubble = document.createElement('div');
  aiBubble.className = 'ai-msg ai';
  const aiTag = document.createElement('div');
  aiTag.className = 'ai-msg-tag';
  aiTag.textContent = _t('ai.generating') || 'AI 生成中…';
  const aiBody = document.createElement('div');
  aiBody.className = 'ai-msg-body';
  aiBody.innerHTML = '<span class="streaming-cursor"></span>';
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
    aiBody.innerHTML = renderSafeMarkdown(state.ai.raw) + '<span class="streaming-cursor"></span>';
    out.scrollTop = out.scrollHeight;
  };
  try {
    const r = await apiFetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        provider: activeProvider.id, model: model,
        credential_id: activeProvider.credential_id || undefined,
        // One-version compatibility for a newly created, not-yet-persisted
        // connection. Once saved, subsequent calls use only credential_id.
        api_key: activeProvider.credential_id ? undefined : (keyVal || undefined),
        base_url: baseUrl || undefined, mode: mode, endpoint_mode: endpointMode, headers: requestHeaders, stream: stream,
        skill_id: skillId,
        skill_variables: { document: docs, selection: isSelection ? docs : '', request: prompt,
          language: (window.i18n && window.i18n.locale) || document.documentElement.lang || 'en',
          context: '', output_format: 'Markdown' },
        messages: msgs,
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
        if (obj.type === 'error' || obj.error) throw new Error(obj.error || 'AI stream error');
        if (obj.type === 'done' || obj.done) break;
        const usage = obj.usage || (obj.type === 'usage' ? obj.usage : null);
        if (usage) {
          state.ai.usage = usage;
          const s = state.ai.sessUsage;
          s.prompt_tokens += usage.prompt_tokens || 0;
          s.completion_tokens += usage.completion_tokens || 0;
          s.total_tokens += usage.total_tokens || 0;
          updateAiUsage();
          continue;
        }
        const delta = obj.type === 'delta' ? obj.delta : obj.d;
        if (delta === undefined) continue;
        state.ai.raw += delta;
        if (!renderTimer) renderTimer = setTimeout(render, state.ai.raw.length > 150000 ? 500 : 120);
      }
    }
    if (renderTimer) { clearTimeout(renderTimer); renderTimer = null; }
    aiBody.innerHTML = renderSafeMarkdown(state.ai.raw);
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
      showToast(isIncognito ? (_t('toast.aiIncognitoDone') || 'AI 完成（无痕会话未保存）') : (saved ? (_t('toast.aiSavedDone') || 'AI 完成，已自动保存会话') : (_t('toast.aiSaveFailDone') || 'AI 完成，但会话保存失败；可在历史中重试')));
    } else {
      msgs.pop();
    }
  } catch (e) {
    if (e.name === 'AbortError') {
      aiTag.textContent = (_t('ai.aiTag', { seq: userSeq }) || ('AI · 回答 ' + userSeq)) + ' ' + (_t('ai.stoppedSuffix') || '（已停止）');
      if (state.ai.raw) {
        aiBody.innerHTML = renderSafeMarkdown(state.ai.raw);
        const last = { role: 'assistant', content: state.ai.raw, ephemeral: isIncognito };
        if (state.ai.usage) last.usage = state.ai.usage;
        msgs.push(last);
        state.ai.messages = msgs;
      }
      showToast(_t('ai.stopped') || '已停止');
    } else {
      aiTag.textContent = _t('ai.aiError') || 'AI · 出错';
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
  button.disabled = true; button.textContent = _t('toast.testingConn') || '测试中…';
  setAiConnectionState('loading', _t('toast.testingConn') || '正在测试连接…');
  try {
    await saveAiSelection(true);
    const active = currentAiProvider() || p;
    const r = await apiFetch('/api/ai/models', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ provider: active.id || '', credential_id: active.credential_id || undefined,
        base_url: $('ai-base-url').value.trim(), mode: $('ai-mode').value || 'auto',
        endpoint_mode: $('ai-endpoint-mode') ? ($('ai-endpoint-mode').value || 'prefix') : 'prefix' })
    });
    const data = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(data.error || 'HTTP ' + r.status);
    setAiConnectionState('ready', _t('toast.connReady', { name: p.name }) || (p.name + ' · 连接正常'));
    $('ai-conn-status').textContent = (_t('toast.connReady', { name: p.name }) || '连接正常') + (data.models && data.models.length ? (' · ' + data.models.length + ' ' + (_t('ai.modelsAvail') || '个模型可用')) : '');
    showToast(_t('toast.connTestPass') || '连接测试通过');
  } catch (e) {
    const hint = aiErrorHint(e); setAiConnectionState(hint.kind, hint.summary); $('ai-conn-status').textContent = hint.message; showToast(hint.message);
  } finally { button.disabled = false; button.textContent = before; }
}

function aiConversationMarkdown(session) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const s = session || {};
  const title = String(s.title || (_t('ai.untitledSession') || '未命名会话')).replace(/[\r\n]/g, ' ').slice(0, 300);
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
  if (!r.ok || !d.session) throw new Error(d.error || (_t('toast.sessionNotExist') || '会话不存在'));
  return aiConversationMarkdown(d.session);
}

async function copyCurrentConversation() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try { await copyText(await selectedConversationMarkdown(), _t('toast.copiedConversation') || '已复制整段对话 Markdown'); } catch (e) { showToast(e.message); }
}

async function exportCurrentConversation() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    const md = await selectedConversationMarkdown();
    const id = state.ai.sessionId || $('ai-session').value || 'conversation';
    const s = (state.ai.sessions || []).find(x => x.id === id);
    const title = (s && s.title) || 'ai-dialog';
    const filename = sanitizeFilename(title) + '.md';
    if (hasPy && py.save_as) {
      const out = await py.save_as(md, filename);
      if (out) showToast((_t('toast.savedPrefix') || '已保存：') + out);
    } else {
      const blob = new Blob([md], { type: 'text/markdown;charset=utf-8' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = filename;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 3000);
      showToast(_t('toast.exportedConversation') || '已导出对话 Markdown');
    }
  } catch (e) { showToast((_t('toast.exportFailed') || '导出失败：') + e.message); }
}

function sanitizeFilename(name) {
  return String(name || 'dialog').trim().replace(/[\\/:*?"<>|]/g, '_').slice(0, 80) || 'dialog';
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

  // Restore saved width from localStorage if present
  try {
    const savedWidth = parseInt(localStorage.getItem('readmd_ai_panel_width'), 10);
    if (savedWidth && savedWidth >= 320 && savedWidth <= Math.floor(window.innerWidth * 0.94)) {
      state.aiPanelWidth = savedWidth;
      document.body.style.setProperty('--ai-panel-width', savedWidth + 'px');
    }
  } catch (e) {}

  let startX = 0, startWidth = 0;
  const maxWidth = () => Math.max(360, Math.floor(window.innerWidth * 0.94));
  const syncResizeState = () => {
    handle.setAttribute('aria-valuemax', String(maxWidth()));
    handle.setAttribute('aria-valuenow', String(state.aiPanelWidth || 440));
  };
  const setPanelWidth = width => {
    state.aiPanelWidth = Math.max(320, Math.min(maxWidth(), Math.round(width)));
    document.body.style.setProperty('--ai-panel-width', state.aiPanelWidth + 'px');
    try { localStorage.setItem('readmd_ai_panel_width', String(state.aiPanelWidth)); } catch (e) {}
    syncResizeState();
  };
  handle.addEventListener('keydown', e => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    const step = e.shiftKey ? 32 : 16;
    setPanelWidth(state.aiPanelWidth + (e.key === 'ArrowLeft' ? step : -step));
  });
  handle.addEventListener('keyup', e => {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') saveSettings();
  });
  handle.addEventListener('dblclick', () => {
    setPanelWidth(440);
    saveSettings();
  });
  window.addEventListener('resize', syncResizeState);
  handle.addEventListener('pointerdown', e => {
    startX = e.clientX; startWidth = state.aiPanelWidth || 440;
    handle.setPointerCapture(e.pointerId);
    document.body.classList.add('ai-resizing');
  });
  handle.addEventListener('pointermove', e => {
    if (!handle.hasPointerCapture(e.pointerId)) return;
    setPanelWidth(startWidth + startX - e.clientX);
  });
  const finish = e => {
    if (!handle.hasPointerCapture(e.pointerId)) return;
    handle.releasePointerCapture(e.pointerId);
    document.body.classList.remove('ai-resizing');
    saveSettings();
  };
  handle.addEventListener('pointerup', finish);
  handle.addEventListener('pointercancel', finish);
  syncResizeState();
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
      else { showToast(_t('toast.appliedSelectionFallback') || '未定位到选中文字，已改为全文应用'); }
    }
    state.original = next;
    state.fixed = next;
    exitEdit();
    await toggleEdit();
    showToast(_t('toast.appliedSavedNotice') || '已应用，请检查后 Ctrl+S 保存（首存自动备份）');
  } else {
    state.fixed = state.ai.raw;
    state.original = state.ai.raw;
    renderContent(state.ai.raw, (state.sourceName || (_t('ai.aiResult') || 'AI 结果')) + ' · AI');
    updateStatus();
    showToast(_t('toast.appliedVirtualNotice') || '已应用（虚拟文档），可另存为 .md');
  }
}

async function saveAiAs() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.ai.raw) return;
  const base = (state.sourceName || state.file || 'document').replace(/[\\/]/g, '_');
  const suggested = base.replace(/\.[^.]+$/, '') + '.ai.md';
  if (hasPy) {
    const out = await py.save_as(state.ai.raw, suggested);
    if (out) showToast((_t('toast.savedPrefix') || '已保存：') + out);
  } else {
    const blob = new Blob([state.ai.raw], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
  }
}

