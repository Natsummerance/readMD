'use strict';
/* ============================================================
   ReadMD Features - Smart Adaptive Clipboard Creation
   ============================================================ */

async function createFromClipboard() {
  try {
    if (!hasPy || !py) throw new Error('桌面版才能读取剪贴板；请用浏览器复制文件导入');
    let clip;
    if (py.authorize_clipboard_read) {
      const permit = await py.authorize_clipboard_read();
      if (!permit || !permit.ok || !permit.token) throw new Error((permit && permit.error) || '剪贴板授权失败，请重试');
      clip = await py.read_clipboard(permit.token);
    } else {
      try { clip = await py.read_clipboard({ user_intent_token: true }); } catch (e) { clip = await py.read_clipboard(true); }
    }
    if (!clip || clip.error || (!clip.text && !clip.html && !clip.image_path)) {
      throw new Error((clip && clip.error) || '剪贴板为空或不包含可识别内容');
    }

    // 1. 如果剪贴板包含复制的图像/截图临时文件 -> 触发 OCR 提取
    if (clip.image_path) {
      showToast('检测到剪贴板截图，正在进行离线 OCR 识别…', 3000);
      await convertOrOcr(clip.image_path, 'ocr');
      return;
    }

    // 2. 如果剪贴板是富文本 HTML -> 优先通过 TurndownService 转换为 Markdown
    if (clip.html && typeof TurndownService !== 'undefined') {
      try {
        const td = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced' });
        const mdFromHtml = td.turndown(clip.html);
        if (mdFromHtml && mdFromHtml.trim().length > (clip.text || '').trim().length) {
          const name = '剪贴板-' + new Date().toISOString().slice(0, 10) + '.md';
          renderVirtual('clipboard', name, '', mdFromHtml, []);
          showToast('已从剪贴板富文本转换为 Markdown（Ctrl+S 可保存）');
          return;
        }
      } catch (err) {
        // Fallback to text
      }
    }

    const text = (clip.text || '').trim();
    if (!text) throw new Error('剪贴板文本为空');

    // 3. 如果剪贴板内容是单个 HTTP/HTTPS URL -> 调起网页转 MD
    if (/^https?:\/\/[^\s]+$/i.test(text)) {
      openWebDialog();
      const input = $('url-input');
      if (input) {
        input.value = text;
        input.focus();
      }
      showToast('已将剪贴板网址填入，点击「⚡ 智能提取」即可抓取', 4000);
      return;
    }

    // 4. 纯文本 / LaTeX 数学公式 / Markdown -> 直接新建虚拟标签页
    const name = '剪贴板-' + new Date().toISOString().slice(0, 10) + '.md';
    renderVirtual('clipboard', name, '', text, []);
    showToast('已从剪贴板新建文档（Ctrl+S 可保存）');
  } catch (e) {
    showToast(e.message || '读取剪贴板失败');
  }
}

