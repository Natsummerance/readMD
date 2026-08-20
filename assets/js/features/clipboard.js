'use strict';
/* ============================================================
   ReadMD Features - Smart Adaptive Clipboard Creation
   ============================================================ */

async function createFromClipboard() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    let clip = null;
    if (typeof hasPy !== 'undefined' && hasPy && py) {
      if (py.authorize_clipboard_read) {
        const permit = await py.authorize_clipboard_read();
        if (!permit || !permit.ok || !permit.token) throw new Error((permit && permit.error) || (_t('toast.clipAuthFail') || '剪贴板授权失败，请重试'));
        clip = await py.read_clipboard(permit.token);
      } else {
        try { clip = await py.read_clipboard({ user_intent_token: true }); } catch (e) { clip = await py.read_clipboard(true); }
      }
    }

    // Web / 浏览器端兜底处理
    if (!clip || clip.error || (!clip.text && !clip.html && !clip.image && !clip.image_path && !clip.files)) {
      if (navigator.clipboard && navigator.clipboard.readText) {
        try {
          const webText = await navigator.clipboard.readText();
          if (webText && webText.trim()) {
            clip = { text: webText.trim(), source_type: 'text' };
          }
        } catch (err) {
          // fallback
        }
      }
    }

    if (!clip || clip.error || (!clip.text && !clip.html && !clip.image && !clip.image_path && !(clip.files && clip.files.length))) {
      throw new Error((clip && clip.error) || (_t('toast.clipEmptyOrUnrec') || '剪贴板为空或未包含可识别内容'));
    }

    // 1. 如果剪贴板包含复制的文件路径列表 (如在文件管理器中复制了文件)
    if (clip.files && Array.isArray(clip.files) && clip.files.length > 0) {
      showToast(_t('toast.clipFilesDetected', { count: clip.files.length }) || `检测到剪贴板包含 ${clip.files.length} 个文件，正在打开…`, 2000);
      for (const f of clip.files) {
        const ext = (f.split('.').pop() || '').toLowerCase();
        if (['md', 'markdown', 'txt'].includes(ext)) {
          await loadFile(f);
        } else if (['png', 'jpg', 'jpeg', 'bmp', 'webp', 'tif', 'tiff', 'pdf'].includes(ext)) {
          await convertOrOcr(f, 'ocr');
        } else {
          await convertOrOcr(f, 'convert');
        }
      }
      return;
    }

    // 2. 如果剪贴板包含复制的图像/截图临时文件 -> 触发 OCR 提取
    const imgPath = clip.image || clip.image_path;
    if (imgPath) {
      showToast(_t('toast.clipImageOcr') || 'Detected screenshot in clipboard, performing offline OCR...', 3000);
      await convertOrOcr(imgPath, 'ocr');
      return;
    }

    const clipNamePrefix = _t('tabs.clipboard') || '剪贴板';

    // 3. 如果剪贴板是富文本 HTML -> 优先通过 TurndownService 转换为 Markdown
    if (clip.html && typeof TurndownService !== 'undefined') {
      try {
        const td = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced' });
        const mdFromHtml = td.turndown(clip.html);
        if (mdFromHtml && mdFromHtml.trim().length > (clip.text || '').trim().length) {
          const name = clipNamePrefix + '-' + new Date().toISOString().slice(0, 10) + '_' + String(Date.now()).slice(-4) + '.md';
          await renderVirtual('clipboard', name, '', mdFromHtml, []);
          showToast(_t('toast.clipHtmlConverted') || 'Converted rich text from clipboard to Markdown (Ctrl+S to save)');
          return;
        }
      } catch (err) {
        // Fallback to text
      }
    }

    const text = (clip.text || '').trim();
    if (!text) throw new Error(_t('toast.clipTextEmpty') || '剪贴板文本为空');

    // 4. 如果剪贴板内容是单个 HTTP/HTTPS URL -> 调起网页转 MD
    if (/^https?:\/\/[^\s]+$/i.test(text)) {
      openWebDialog();
      const input = $('url-input');
      if (input) {
        input.value = text;
        input.focus();
      }
      showToast(_t('toast.clipUrlPasted') || 'URL filled from clipboard, click Smart Extract to fetch', 4000);
      return;
    }

    // 5. 纯文本 / LaTeX 数学公式 / Markdown -> 直接新建虚拟标签页
    const name = clipNamePrefix + '-' + new Date().toISOString().slice(0, 10) + '_' + String(Date.now()).slice(-4) + '.md';
    await renderVirtual('clipboard', name, '', text, []);
    showToast(_t('toast.clipCreated') || 'Created document from clipboard (Ctrl+S to save)');
  } catch (e) {
    showToast(e.message || (_t('toast.clipReadFail') || 'Failed to read clipboard'));
  }
}



