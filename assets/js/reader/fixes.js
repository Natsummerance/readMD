'use strict';
/* ============================================================
   ReadMD Reader - Document Fixes Modal
   ============================================================ */

/* ---------------- 修正详情 ---------------- */

function showFixModal() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const list = $('fix-list');
  list.innerHTML = '';
  const fixes = state.fixes || [];
  $('fix-count').textContent = fixes.length ? (_t('fixes.countTotal', { count: fixes.length }) || ('（共 ' + fixes.length + ' 处）')) : '';
  if (!fixes.length) {
    const li = document.createElement('li');
    li.className = 'empty';
    li.textContent = _t('fixes.noFixes') || '本篇文档未发现需要修正的内容';
    list.appendChild(li);
  } else {
    fixes.forEach(f => {
      const li = document.createElement('li');
      li.textContent = f;
      list.appendChild(li);
    });
  }
  $('fix-modal').classList.remove('hidden');
}

async function handleAiDocumentFix() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const rawContent = state.editing && window.cmView ? window.cmView.state.doc.toString() : (state.fixed || state.original || '');
  if (!rawContent || !rawContent.trim()) {
    showToast(_t('fixes.noDocContent') || '当前没有可修复的文档内容');
    return;
  }

  const fixModal = $('fix-modal');
  if (fixModal) fixModal.classList.add('hidden');
  showToast(_t('fixes.aiFixing') || '正在进行 AI 深度格式排版自愈...', 2500);

  try {
    const connection = typeof resolveSharedAiConnection === 'function'
      ? await resolveSharedAiConnection()
      : null;
    if (!connection) throw new Error(_t('toast.selectProviderFirst') || '请先选择 AI 提供商');
    if (!connection.local && !connection.has_key) {
      throw new Error(_t('toast.noApiKeyNotice') || '未配置 API Key：请打开设置完成连接');
    }
    const resp = await apiFetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        provider: connection.provider,
        credential_id: connection.credential_id,
        model: connection.model,
        base_url: connection.base_url,
        mode: connection.mode,
        endpoint_mode: connection.endpoint_mode,
        headers: connection.headers,
        skill_id: 'readmd-format-fix',
        skill_variables: {
          document: rawContent,
          selection: rawContent,
          request: '',
          language: (window.i18n && window.i18n.locale) || document.documentElement.lang || 'en',
          context: '',
          output_format: 'Markdown'
        },
        messages: [{ role: 'user', content: rawContent }],
        stream: false
      })
    });

    const data = await resp.json();
    if (!data || !data.ok || !data.content) {
      showToast((_t('fixes.aiFixFail') || 'AI 修复失败：') + ((data && data.error) || '未返回有效内容'));
      return;
    }

    let fixedMd = data.content.trim();
    // 剥离可能存在的 markdown 代码块包裹
    if (fixedMd.startsWith('```markdown') && fixedMd.endsWith('```')) {
      fixedMd = fixedMd.slice(11, -3).trim();
    } else if (fixedMd.startsWith('```md') && fixedMd.endsWith('```')) {
      fixedMd = fixedMd.slice(5, -3).trim();
    } else if (fixedMd.startsWith('```') && fixedMd.endsWith('```') && !rawContent.startsWith('```')) {
      fixedMd = fixedMd.slice(3, -3).trim();
    }

    if (state.editing && window.cmView) {
      window.cmView.dispatch({
        changes: { from: 0, to: window.cmView.state.doc.length, insert: fixedMd }
      });
      state.isDirty = true;
      if (typeof updateEditorPreview === 'function') updateEditorPreview();
    } else {
      state.fixed = fixedMd;
      state.original = fixedMd;
      render();
    }

    showToast(_t('fixes.aiFixed') || 'AI 深度排版修复完成', 1800);
  } catch (err) {
    showToast((_t('fixes.aiFixFail') || 'AI 修复失败：') + err.message);
  }
}

document.addEventListener('DOMContentLoaded', () => {
  const aiFixBtn = $('fix-ai-btn');
  if (aiFixBtn) {
    aiFixBtn.addEventListener('click', handleAiDocumentFix);
  }
});
