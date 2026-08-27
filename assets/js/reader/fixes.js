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
    const promptText = `请对以下 Markdown 文档进行全面的格式排版与渲染自愈精修。
精细修复要求：
1. 表格自愈：对齐所有列数、补齐缺失表头分隔线（|---|---|）、正确转义单元格内的游离竖线；
2. 数学公式自愈：修复未闭合的 $ 与 $$ 公式，修复 LaTeX 矩阵与对齐环境，转义金额中的美元符号；
3. 代码块自愈：补齐未闭合的代码块围栏（\`\`\`），自动识别并标注代码语言；
4. 标题与列表规范：统一标题层级规范（确保 # 后面有空格）、修复嵌套列表缩进断层；
5. 纯净输出：严格保持原文所有语义和内容，禁止删减文字，禁止添加任何解释说明或前导后置客套话，全篇禁止任何 Emoji，直接输出修复后的 Markdown 源码：

${rawContent}`;

    const resp = await apiFetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        messages: [
          {
            role: 'system',
            content: '你是 ReadMD 专业 Markdown 排版自愈与格式精修引擎。严格保持原文所有内容和语义，禁止删减，禁止输出客套话，直接输出修复后的纯 Markdown 源码。'
          },
          {
            role: 'user',
            content: promptText
          }
        ],
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


