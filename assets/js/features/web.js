'use strict';
/* ============================================================
   ReadMD Features - Modern Web to Markdown Extractor
   ============================================================ */

const webRun = {
  running: false,
  taskId: '',
  cancelled: false,
  lastUrl: '',
  privateGrant: '',
};
window.webRun = webRun;

function normalizeWebUrl(url) {
  url = String(url || '').trim();
  if (url && !/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) url = 'https://' + url;
  return url;
}

function setWebStatus(text, kind) {

  const el = $('url-status');
  el.textContent = text || '';
  el.classList.toggle('error', kind === 'error');
  el.classList.toggle('success', kind === 'success');
}

function setWebProgress(percent, title, count) {
  const wrap = $('url-progress');
  wrap.classList.remove('hidden');
  wrap.setAttribute('aria-hidden', 'false');
  $('url-progress-bar').style.width = Math.max(0, Math.min(100, percent || 0)) + '%';
  $('url-progress-title').textContent = title || '处理中…';
  $('url-progress-count').textContent = count || '';
}

function setWebRunning(running) {
  webRun.running = running;
  if ($('url-go')) $('url-go').disabled = running;
  if ($('url-render')) $('url-render').disabled = running || !hasPy;
  if ($('url-full')) $('url-full').disabled = running;
  if ($('url-cancel')) $('url-cancel').classList.toggle('hidden', !running);
  if ($('url-input')) $('url-input').disabled = running;
  if ($('url-mode')) $('url-mode').disabled = running;
  if ($('url-crawl')) $('url-crawl').disabled = running;
  if ($('url-pages')) $('url-pages').disabled = running;
  if ($('url-images')) $('url-images').disabled = running;
  if ($('url-private')) $('url-private').disabled = running || !hasPy;
}

async function postWebExtract(payload) {
  const response = await apiFetch('/api/web/extract', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  let data = {};
  try { data = await response.json(); } catch (e) { data = { error: '服务器返回了无法解析的响应' }; }
  if (!response.ok) {
    const error = new Error(data.error || ('网页转换失败（HTTP ' + response.status + '）'));
    error.code = data.code || 'request_failed';
    throw error;
  }
  return data;
}

async function extractOneWebPage(url, options, forceRender) {
  const base = {
    task_id: webRun.taskId, url, mode: options.mode,
    download_images: options.downloadImages,
  };
  let data = null;
  let diagnostics = null;
  let renderSourceHtml = '';
  if (!forceRender && !options.privateGrant) {
    setWebProgress(options.progress || 12, '下载并分析静态页面…', options.count || '');
    data = await postWebExtract(base);
    if (data.ok) return data;
    diagnostics = {
      engine_chain: data.engine_chain || ['http'], attempts: data.attempts || 1,
      fallback_reason: data.fallback_reason || data.code || 'render_required',
    };
    renderSourceHtml = data.render_html || '';
    if (!data.render_required) {
      const error = new Error(data.error || '未能提取网页正文');
      error.code = data.code || 'extract_failed';
      throw error;
    }
  }
  if (!hasPy || !py.render_web_page) {
    const error = new Error(LAN_TOKEN
      ? '该页面需要 JavaScript。请在 ReadMD 桌面应用中使用动态渲染抓取。'
      : '当前环境不支持系统 WebView 动态渲染。');
    error.code = 'render_unavailable';
    throw error;
  }
  const runRender = async interactive => {
    setWebProgress(Math.max(options.progress || 12, interactive ? 28 : 24),
      interactive ? '请在临时窗口完成验证后点击“提取此页”…' : '使用系统浏览器内核渲染…',
      options.count || (interactive ? '最多等待 5 分钟' : '最长 25 秒'));
    const rendered = await py.render_web_page(
      url, webRun.taskId, interactive ? 300000 : 25000,
      interactive, options.privateGrant || '', renderSourceHtml);
    if (!rendered || !rendered.ok) return { rendered, data: null };
    setWebProgress(Math.max(options.progress || 12, 32), '使用 Defuddle / Readability 提取…', options.count || '');
    const renderNode = interactive ? 'interactive-webview' : 'system-webview';
    diagnostics = diagnostics || { engine_chain: [], attempts: 0, fallback_reason: forceRender ? 'forced_render' : 'render_required' };
    diagnostics.engine_chain = (diagnostics.engine_chain || []).concat([renderNode]);
    diagnostics.attempts = (diagnostics.attempts || 0) + 1;
    const extracted = await postWebExtract(Object.assign({}, base, {
      html: rendered.html || '', final_url: rendered.final_url || url,
      defuddle: rendered.defuddle || null,
      readability: rendered.readability || null,
      diagnostics,
    }));
    return { rendered, data: extracted };
  };
  let attempt = await runRender(false);
  if (attempt.data && attempt.data.ok) return attempt.data;
  if (attempt.rendered && attempt.rendered.code === 'cancelled') {
    const error = new Error(attempt.rendered.error || '已取消网页转换'); error.code = 'cancelled'; throw error;
  }
  attempt = await runRender(true);
  if (attempt.data && attempt.data.ok) return attempt.data;
  const response = attempt.data || attempt.rendered || {};
  const error = new Error(response.error || '交互式抓取后仍未识别到正文');
  error.code = response.code || 'extract_failed';
  throw error;
}

async function cancelWebTask() {
  if (!webRun.running) return;
  webRun.cancelled = true;
  setWebStatus('正在取消网页转换…');
  try {
    await apiFetch('/api/web/cancel', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ task_id: webRun.taskId }),
    });
  } catch (e) { /* local cancellation still applies */ }
  try { if (hasPy && py.cancel_web_render) await py.cancel_web_render(webRun.taskId); } catch (e) { /* ignore */ }
  try { if (hasPy && py.revoke_private_web) await py.revoke_private_web(webRun.taskId); } catch (e) { /* ignore */ }
}

async function webToMd(url, crawl, forceRender) {
  url = normalizeWebUrl(url);
  if (!url || webRun.running) return;
  if ($('url-input')) $('url-input').value = url;
  if (!(await ensureModule('web'))) return;
  webRun.taskId = 'web-' + Date.now() + '-' + Math.random().toString(16).slice(2);
  webRun.lastUrl = url;
  webRun.cancelled = false;
  webRun.privateGrant = '';
  setWebRunning(true);
  setWebStatus('正在准备网页转换…');
  const options = {
    mode: forceRender ? 'full' : ($('url-mode') ? $('url-mode').value : 'smart'),
    downloadImages: $('url-images') ? $('url-images').checked : false,
    pageLimit: Math.max(1, Math.min(30, Number($('url-pages') ? $('url-pages').value : 1) || 1)),
    privateGrant: '',
  };
  const sections = [], assets = [], warnings = [], failures = [];
  let first = null, batchTotal = 1;
  try {
    if ($('url-private') && $('url-private').checked && hasPy && py.authorize_private_web) {
      const authorization = await py.authorize_private_web(url, webRun.taskId);
      if (authorization && authorization.ok) {
        options.privateGrant = authorization.grant;
        webRun.privateGrant = authorization.grant;
      }
    }


    first = await extractOneWebPage(url, Object.assign({}, options, { progress: 10 }), !!forceRender);
    if (webRun.cancelled) throw Object.assign(new Error('已取消网页转换'), { code: 'cancelled' });
    sections.push(first.content);
    assets.push(...(first.assets || []));
    warnings.push(...(first.warnings || []));
    const links = crawl ? (first.links || []).slice(0, options.pageLimit - 1) : [];
    const total = 1 + links.length;
    batchTotal = total;
    for (let i = 0; i < links.length; i++) {
      if (webRun.cancelled) throw Object.assign(new Error('已取消网页转换'), { code: 'cancelled' });
      const pageNo = i + 2;
      const progress = 35 + Math.round((i / Math.max(1, links.length)) * 55);
      setWebProgress(progress, '抓取同站页面…', pageNo + ' / ' + total);
      try {
        const result = await extractOneWebPage(links[i], Object.assign({}, options, {
          progress, count: pageNo + ' / ' + total,
        }), false);
        if (webRun.cancelled) throw Object.assign(new Error('已取消网页转换'), { code: 'cancelled' });
        sections.push(result.content.replace(/^# /, '## '));
        assets.push(...(result.assets || []));
        warnings.push(...(result.warnings || []));
      } catch (error) {
        if (error.code === 'cancelled' || webRun.cancelled) throw error;
        failures.push({ url: links[i], error: error.message });
      }
    }
    if (crawl) {
      const successCount = sections.length;
      sections.push('\n---\n\n## 抓取统计\n\n成功 ' + successCount + ' 页，跳过 0 页，失败 ' + failures.length + ' 页。' +
        (failures.length ? '\n\n' + failures.map(x => '- ' + x.url + '：' + x.error).join('\n') : ''));
    }
    const content = sections.join('\n\n---\n\n');
    setWebProgress(100, '网页转换完成', (crawl ? sections.length - 1 : sections.length) + ' 页');
    setWebStatus('提取成功' + (warnings.length ? '，有 ' + warnings.length + ' 条提示' : '') + '。', 'success');
    const title = (first.meta && first.meta.title) || url;
    await renderVirtual('url', title, first.asset_dir || '', content, [], { assets });
    if (warnings.length) showToast(warnings[0] + (warnings.length > 1 ? '（另有 ' + (warnings.length - 1) + ' 条）' : ''));
  } catch (error) {
    const cancelled = error.code === 'cancelled' || webRun.cancelled;
    if (cancelled && sections.length && first) {
      const skipped = Math.max(0, batchTotal - sections.length - failures.length);
      sections.push('\n---\n\n## 抓取统计\n\n成功 ' + sections.length + ' 页，跳过 ' + skipped + ' 页，失败 ' + failures.length + ' 页。');
      await renderVirtual('url', (first.meta && first.meta.title) || url,
        first.asset_dir || '', sections.join('\n\n---\n\n'), [], { assets });
      setWebStatus('网页转换已取消，已保留成功抓取的 ' + (sections.length - 1) + ' 页。', 'success');
    } else {
      setWebStatus(cancelled ? '网页转换已取消。' : (error.message || '网页转换失败'), cancelled ? '' : 'error');
    }
    setWebProgress(0, cancelled ? '已取消' : '转换未完成', '');
  } finally {
    try { if (hasPy && py.revoke_private_web) await py.revoke_private_web(webRun.taskId); } catch (e) { /* ignore */ }
    webRun.privateGrant = '';
    setWebRunning(false);
  }
}

/* ---------------- 网页对话框 ---------------- */

function openWebDialog() {
  if (moduleBlocked('web')) return;
  $('url-modal').classList.remove('hidden');
  $('url-render').disabled = !hasPy;
  $('url-private').disabled = !hasPy;
  $('url-progress').classList.add('hidden');
  $('url-progress').setAttribute('aria-hidden', 'true');
  setWebStatus(LAN_TOKEN
    ? '局域网页面支持增强静态抓取；动态渲染请使用桌面应用。'
    : '请输入公开的 HTTP/HTTPS 网页地址。');
  $('url-input').focus();
}

function closeWebDialog() {
  if (webRun.running) { cancelWebTask(); return; }
  $('url-modal').classList.add('hidden');
}

