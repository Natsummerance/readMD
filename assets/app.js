'use strict';

function syncSelectAccessibleName(el) {
  const selected = el.selectedOptions && el.selectedOptions[0];
  if (selected) el.setAttribute('aria-label', selected.textContent.trim());
}
/* ==============================================================================================
   ReadMD v2 - Application Integration Bus & Bootstrap (主集成总线与生命周期调度器)
   ==============================================================================================

   【架构定位】
   本项目采用分级分类的模块化架构，assets/app.js 作为前端总调度中心，负责：
     1. 全局生命周期初始化 (DOMContentLoaded, pywebviewready, beforeunload)
     2. 顶层 UI 事件与全局快捷键集中绑定分发
     3. 跨领域模块间的流程调度与状态协同
     4. Python 桌面端 (pywebview) 与 Web 端运行环境的桥接调度

   ==============================================================================================
   【模块目录索引与职责映射表 (Architecture & Module Directory Index)】
   ==============================================================================================

    assets/js/core/ - 核心基础设施与全局状态
   ----------------------------------------------------------------------------------------------
     • state.js     : 全局状态单例 (window.state)、DOM选择器 ($)、网络请求 (apiFetch)、提示 (showToast/busy)、Python桥接 (bindPy)
     • settings.js  : 用户配置持久化 (loadSettings/saveSettings/applySettings)、主题切换 (toggleTheme)、全局缩放 (zoom)
     • modules.js   : 后端服务生命周期与动态轮询 (startModules/pollModules/ensureModule/moduleBlocked/updateModuleUi)
     • tabs.js      : 多标签系统 (createTab/switchTab/closeTab/closeOtherTabs/closeAllTabs/renderTabsBar/promptDirtyClose)
     • history.js   : 最近打开文件、主页深度重置 (goHome)、欢迎页卡片交互 (bindWelcomeEvents)、自动重载 (startAutoReload)
     • dragdrop.js  : 全局文件与图片拖拽放置识别 (bindGlobalDragAndDrop)

    assets/js/reader/ - 文档解析、渲染与阅读增强引擎
   ----------------------------------------------------------------------------------------------
     • render.js    : Markdown 核心解析、分块虚拟渲染 (renderMarkdown/renderVirtual/loadFile/loadFileDialog/saveAs)
     • formula.js   : LaTeX 数学公式保护/还原、公式自修复、公式选择器 (openFormulaModal/insertFormulaFromPicker)
     • fixes.js     : Markdown 格式自动修复统计与修复标记弹窗 (showFixModal)
     • toc.js       : 目录大纲提取、侧边栏层级生成与滚动追踪高亮 (buildToc/updateActiveToc)
     • search.js    : 正文全文关键词即时检索、多结果高亮与上下跳转 (toggleSearch/doSearch/jumpToMark/closeSearch)
     • folder.js    : 本地工作区文件夹侧边栏树形浏览与文件快速切换 (openFolder/showSide/toggleSide)

    assets/js/editor/ - Markdown 源码编辑器与图片工作台
   ----------------------------------------------------------------------------------------------
     • editor.js    : CodeMirror 6 编辑器实例、Markdown语法插入 (cmInsertSyntax)、命令面板 (openMdCommandPalette)、saveEdit/exitEdit
     • preview.js   : 四向分栏实时预览布局 (setPvLayout)、双向滚动同步 (pvSyncFromPreview)、分栏拖拽手柄 (bindPvSplitter)
     • image.js     : 轻量图片编辑器 (openImgModal/rotateImg/flipImg/applyRatio/undoImg/redoImg/exportAndInsertImg)

    assets/js/features/ - 高级业务扩展功能
   ----------------------------------------------------------------------------------------------
     • ai.js        : AI 侧边栏对话、多模型切换 (onAiProviderChange)、Prompt模板 (openTplModal)、流式推理 (runAi)、无痕会话
     • web.js       : 现代网页转 MD (openWebDialog/webToMd/cancelWebTask)、滚轮步进器、动态渲染与同站批量抓取
     • clipboard.js : 智能自适应剪贴板新建 (createFromClipboard) 联动 Turndown/OCR/Web/虚拟文档
     • convert.js   : 万物转 MD 批量转换工作台 (openConvertModal/pickConvertFiles/pickConvertFolder/convertFile)
     • ocr.js       : 离线 OCR 图片与扫描件文字识别 (ocrFile)
     • export.js    : 多格式导出 (PDF/DOCX/HTML/LaTeX) (openExportModal/renderExportSections/updateExportLivePreview/runExport)
     • share.js     : 局域网移动端二维码与热点分享 (openShareModal/startShare/stopShare)
     • updater.js   : 客户端内自动检查更新与静默升级 (checkUpdate/openUpdateModal/startUpdateDownload)

   ==============================================================================================
   【核心跨模块联动关系 (Core Inter-Module Collaborations)】
   ==============================================================================================
     1. 标签切换与状态联动 : tabs.js (switchTab) -> state.js (sync) -> reader/render.js (渲染) -> toc.js (生成大纲)
     2. 编辑与实时预览联动 : editor.js (CodeMirror输入) -> preview.js (防抖渲染) -> formula.js (公式渲染)
     3. 智能剪贴板多路分流 : clipboard.js -> OCR(图片) / Web(网址) / Turndown(富文本) / renderVirtual(纯文本/公式)
     4. 导出排版与公式渲染 : export.js (配置组装) -> render.js (正文提取) -> Python后端 (mdexport/latex渲染)
     5. 虚拟文档与另存持久化: convert.js/web.js/clipboard.js -> renderVirtual() -> tabs.js -> render.js (saveAs)
   ============================================================================================== */

/* ----------------------------------------------------------------------------------------------
   顶层 UI 事件集中绑定与调度器
   ---------------------------------------------------------------------------------------------- */
function closeMoreMenu(restoreFocus = false) {
  const menu = $('more-menu');
  const wasOpen = menu && menu.classList.contains('open');
  if (menu) menu.classList.remove('open');
  const button = $('btn-more');
  if (button) button.setAttribute('aria-expanded', 'false');
  if (restoreFocus && wasOpen && button && document.activeElement !== button && !button.contains(document.activeElement)) {
    button.focus({ preventScroll: true });
  }
}

function bindEvents() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;

  /* --- 1. 欢迎页与全局导航 (Welcome & Navigation) [联动: history.js, render.js, folder.js] --- */
  bindWelcomeEvents(); // 绑定欢迎页各个功能入口卡片事件
  if ($('btn-home')) $('btn-home').addEventListener('click', goHome); // 回到主页并深度清理状态 (history.js)
  $('btn-open').addEventListener('click', () => { loadFileDialog(); }); // 打开本地文件 (reader/render.js)
  $('btn-folder').addEventListener('click', openFolder); // 打开工作区文件夹 (reader/folder.js)
  syncDesktopControls();

  /* --- 2. 更多功能下拉菜单交互 (More Dropdown Menu) --- */
  const moreBtn = $('btn-more');
  const moreMenu = $('more-menu');
  if (moreBtn && moreMenu) {
    moreBtn.addEventListener('click', e => {
      e.stopPropagation();
      moreMenu.classList.toggle('open');
      moreBtn.setAttribute('aria-expanded', moreMenu.classList.contains('open') ? 'true' : 'false');
    });
    document.addEventListener('click', e => {
      if (moreMenu.classList.contains('open') && !moreMenu.contains(e.target) && e.target !== moreBtn) {
        closeMoreMenu();
      }
    });
  }


  /* --- 3. 万物转 MD 模态框与文件批量导入 [联动: features/convert.js] --- */
  $('btn-convert').addEventListener('click', openConvertModal);
  $('convert-files').addEventListener('click', pickConvertFiles);
  $('convert-folder').addEventListener('click', pickConvertFolder);
  $('convert-close').addEventListener('click', closeConvertModal);
  $('convert-open-dir').addEventListener('click', () => {
    if (convertLastDir && py.open_dir) py.open_dir(convertLastDir);
  });
  $('convert-modal').addEventListener('click', e => { if (e.target === $('convert-modal')) closeConvertModal(); });

  /* --- 4. 离线 OCR / 网页抓取 / 剪贴板新建 / 演示 / 样式定制 / 禅模式 [联动: convert.js, ocr.js, web.js, clipboard.js, render.js] --- */
  $('btn-ocr').addEventListener('click', () => chooseFile('ocr')); // 触发离线 OCR 识别
  $('btn-web').addEventListener('click', openWebDialog);           // 打开网页抓取弹窗
  if ($('btn-clipboard-new')) $('btn-clipboard-new').addEventListener('click', createFromClipboard); // 智能自适应全类型剪贴板新建
  if ($('btn-presentation-menu')) $('btn-presentation-menu').addEventListener('click', () => { closeMoreMenu(); launchPresentationMode(); });
  if ($('btn-run-all-chunks')) $('btn-run-all-chunks').addEventListener('click', () => { closeMoreMenu(); if (window.runAllCodeChunks) runAllCodeChunks(); });
  if ($('btn-style-custom')) $('btn-style-custom').addEventListener('click', () => { closeMoreMenu(); openStyleModal(); });
  if ($('btn-zen')) $('btn-zen').addEventListener('click', () => toggleZenMode()); // 顶栏常驻禅模式按钮
  if ($('btn-zen-menu')) $('btn-zen-menu').addEventListener('click', () => { closeMoreMenu(); toggleZenMode(); });

  // 更多菜单手风琴分组折叠切换
  document.querySelectorAll('.more-group-header').forEach(hdr => {
    hdr.addEventListener('click', e => {
      e.stopPropagation();
      const grp = hdr.closest('.more-group');
      if (grp) {
        grp.classList.toggle('open');
        hdr.setAttribute('aria-expanded', grp.classList.contains('open') ? 'true' : 'false');
      }
    });
  });

  // 全局快捷键与无障碍键盘导航 (Esc 统一关闭顶层弹窗 / F5 演说放映 / F11 禅模式)
  window.addEventListener('keydown', e => {
    if (e.key === 'F5' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      if (window.launchPresentationMode) launchPresentationMode();
    } else if (e.key === 'Escape') {
      const openModals = [
        ['code-chunk-modal', closeCodeChunkModal],
        ['diagram-modal', closeDiagramModal],
        ['doc-import-modal', closeDocImportModal],
        ['style-custom-modal', closeStyleModal],
        ['convert-modal', closeConvertModal],
        ['export-modal', closeExportModal],
        ['img-modal', closeImgModal],
        ['formula-modal', closeFormulaModal],
        ['url-modal', typeof closeWebDialog === 'function' ? closeWebDialog : () => $('url-modal').classList.add('hidden')],
        ['share-modal', typeof closeShareDialog === 'function' ? closeShareDialog : () => $('share-modal').classList.add('hidden')],
        ['history-modal', () => $('history-modal').classList.add('hidden')],
        ['fix-modal', () => $('fix-modal').classList.add('hidden')],
        ['tpl-modal', () => $('tpl-modal').classList.add('hidden')],
        ['presentation-modal', () => window.closePresentationMode?.()],
      ];
      for (const [id, closeFn] of openModals) {
        const el = $(id);
        if (el && !el.classList.contains('hidden')) {
          e.preventDefault();
          if (typeof closeFn === 'function') closeFn();
          break;
        }
      }
    }
  });

  // 交互式代码块模态框事件
  if ($('code-chunk-modal-close')) $('code-chunk-modal-close').addEventListener('click', closeCodeChunkModal);
  if ($('code-chunk-cancel')) $('code-chunk-cancel').addEventListener('click', closeCodeChunkModal);
  if ($('code-chunk-insert')) $('code-chunk-insert').addEventListener('click', insertCodeChunkFromModal);
  if ($('code-chunk-lang')) $('code-chunk-lang').addEventListener('change', e => {
    const codeArea = $('code-chunk-code');
    if (codeArea && typeof CODE_CHUNK_SAMPLES !== 'undefined') {
      codeArea.value = CODE_CHUNK_SAMPLES[e.target.value] || CODE_CHUNK_SAMPLES.python;
    }
  });
  if ($('code-chunk-modal')) $('code-chunk-modal').addEventListener('click', e => { if (e.target === $('code-chunk-modal')) closeCodeChunkModal(); });

  // 科学工程图表模态框事件
  if ($('diagram-modal-close')) $('diagram-modal-close').addEventListener('click', closeDiagramModal);
  if ($('diagram-cancel')) $('diagram-cancel').addEventListener('click', closeDiagramModal);
  if ($('diagram-insert')) $('diagram-insert').addEventListener('click', insertDiagramFromModal);
  if ($('diagram-type')) $('diagram-type').addEventListener('change', e => {
    const codeArea = $('diagram-code');
    if (codeArea && typeof DIAGRAM_SAMPLES !== 'undefined') {
      codeArea.value = DIAGRAM_SAMPLES[e.target.value] || DIAGRAM_SAMPLES.plantuml;
    }
  });
  if ($('diagram-modal')) $('diagram-modal').addEventListener('click', e => { if (e.target === $('diagram-modal')) closeDiagramModal(); });

  // 子文档引用模态框事件
  if ($('doc-import-modal-close')) $('doc-import-modal-close').addEventListener('click', closeDocImportModal);
  if ($('doc-import-cancel')) $('doc-import-cancel').addEventListener('click', closeDocImportModal);
  if ($('doc-import-insert')) $('doc-import-insert').addEventListener('click', insertDocImportFromModal);
  if ($('doc-import-modal')) $('doc-import-modal').addEventListener('click', e => { if (e.target === $('doc-import-modal')) closeDocImportModal(); });

  // Toast 提示点击（用于升级跳转等场景）
  $('toast').addEventListener('click', () => {
    if (!upgradeUrl) return;
    const url = upgradeUrl; upgradeUrl = null;
    if (py && py.open_external) { py.open_external(url); }
    else window.open(url, '_blank');
  });

  /* --- 5. 现代网页转 MD 弹窗控件 (Web Extractor Dialog) [联动: features/web.js] --- */
  // 抓取页数步进器 (增/减/滚轮调节)
  if ($('url-pages-dec')) {
    $('url-pages-dec').addEventListener('click', () => {
      const input = $('url-pages');
      if (input) input.value = Math.max(1, (parseInt(input.value, 10) || 1) - 1);
    });
  }
  if ($('url-pages-inc')) {
    $('url-pages-inc').addEventListener('click', () => {
      const input = $('url-pages');
      if (input) input.value = Math.min(30, (parseInt(input.value, 10) || 1) + 1);
    });
  }
  if ($('url-pages')) {
    $('url-pages').addEventListener('wheel', e => {
      e.preventDefault();
      const delta = e.deltaY < 0 ? 1 : -1;
      const input = $('url-pages');
      input.value = Math.max(1, Math.min(30, (parseInt(input.value, 10) || 1) + delta));
    }, { passive: false });
  }
  // 网页网址一键粘贴
  if ($('url-paste-btn')) {
    $('url-paste-btn').addEventListener('click', async () => {
      try {
        if (hasPy && py.read_clipboard) {
          const clip = await py.read_clipboard(true);
          if (clip && clip.text && /^https?:\/\//i.test(clip.text.trim())) {
            $('url-input').value = clip.text.trim();
            showToast(_t('toast.clipUrlPasted'));
            return;
          }
        }
        const text = await navigator.clipboard.readText();
        if (text && /^https?:\/\//i.test(text.trim())) {
          $('url-input').value = text.trim();
          showToast(_t('toast.clipUrlPasted'));
        } else {
          showToast(_t('toast.clipNoValidHttpUrl'));
        }
      } catch (err) {
        showToast(_t('toast.clipReadManual'));
      }
    });
  }
  // 抓取模式双操作卡：智能提取 与 完整渲染
  $('url-go').addEventListener('click', () => {
    const url = $('url-input').value.trim();
    const pages = parseInt($('url-pages') ? $('url-pages').value : '1', 10) || 1;
    webToMd(url, pages > 1, false);
  });
  $('url-render').addEventListener('click', () => {
    const url = $('url-input').value.trim();
    const pages = parseInt($('url-pages') ? $('url-pages').value : '1', 10) || 1;
    webToMd(url, pages > 1, true);
  });
  $('url-cancel').addEventListener('click', cancelWebTask);
  $('url-close').addEventListener('click', closeWebDialog);
  $('url-modal').addEventListener('click', e => { if (e.target === $('url-modal') && !webRun.running) closeWebDialog(); });
  $('url-input').addEventListener('keydown', e => { if (e.key === 'Enter') $('url-go').click(); });
  $('url-modal').addEventListener('keydown', e => { if (e.key === 'Escape') { e.preventDefault(); closeWebDialog(); } });

  /* --- 6. 编辑器与工具栏交互 (Editor Studio & Markdown Tools) [联动: editor/editor.js, formula.js] --- */
  $('btn-edit').addEventListener('click', toggleEdit);
  document.querySelectorAll('#md-tool [data-md]').forEach(b => b.addEventListener('click', () => {
    closeMdPopups(); if (b.dataset.md === 'image') openImgModal(); else cmInsertSyntax(b.dataset.md);
  }));
  document.querySelectorAll('#md-tool [data-menu]').forEach(b => b.addEventListener('click', e => {
    e.stopPropagation(); const menu = $(b.dataset.menu); const wasHidden = menu.classList.contains('hidden'); closeMdPopups(); if (wasHidden) menu.classList.remove('hidden');
  }));
  // LaTeX 公式选择器模态框 [联动: reader/formula.js]
  $('formula-open').addEventListener('click', () => openFormulaModal('inline'));
  $('formula-close').addEventListener('click', closeFormulaModal);
  $('formula-search').addEventListener('input', renderFormulaPicker);
  $('formula-modal').addEventListener('click', e => { if (e.target === $('formula-modal')) closeFormulaModal(); });

  // 编辑保存与退出
  $('edit-save').addEventListener('click', saveEdit);
  $('edit-area').addEventListener('input', () => {
    updateUnloadGuard();
    if (typeof syncActiveTabDirty === 'function') syncActiveTabDirty();
  });
  $('edit-cancel').addEventListener('click', confirmExitEdit);

  // 编辑撤销与重做 [联动: editor/editor.js]
  if ($('edit-undo')) $('edit-undo').addEventListener('click', cmUndo);
  if ($('edit-redo')) $('edit-redo').addEventListener('click', cmRedo);

  // 选中文本浮动快捷操作 [联动: editor/editor.js]
  if ($('cm-sel-copy')) $('cm-sel-copy').addEventListener('click', cmCopySelection);
  if ($('cm-sel-cut')) $('cm-sel-cut').addEventListener('click', cmCutSelection);
  if ($('cm-sel-paste')) $('cm-sel-paste').addEventListener('click', cmPasteSelection);
  if (typeof bindEditorAiEvents === 'function') bindEditorAiEvents();


  /* --- 7. 实时分栏预览与滚动同步控制 (Split Preview) [联动: editor/preview.js] --- */
  $('pv-trigger').addEventListener('click', e => {
    e.stopPropagation();
    const m = $('pv-menu');
    const show = m.classList.contains('hidden');
    closeMdPopups();
    m.classList.toggle('hidden', !show);
    $('pv-trigger').setAttribute('aria-expanded', show ? 'true' : 'false');
  });
  document.querySelectorAll('.pv-btn').forEach(b => b.addEventListener('click', () => { setPvLayout(b.dataset.pv); closeMdPopups(); }));
  const pvSyncEl = $('pv-sync');
  if (pvSyncEl) pvSyncEl.addEventListener('change', e => { state.pvSync = e.target.checked; saveSettings(); });
  const pvWrap = $('preview-wrap');
  if (pvWrap) pvWrap.addEventListener('scroll', pvSyncFromPreview);
  bindPvSplitter(); // 绑定分栏拖拽调整手柄

  /* --- 8. 轻量图片编辑器控制 (Image Studio) [联动: editor/image.js] --- */
  $('img-file').addEventListener('click', () => $('img-file-input').click());
  $('img-file-input').addEventListener('change', e => {
    const f = e.target.files && e.target.files[0];
    if (f) loadImgFromFile(f);
    e.target.value = '';
  });
  $('img-url-load').addEventListener('click', insertImgUrl);
  $('img-url-input').addEventListener('keydown', e => { if (e.key === 'Enter') insertImgUrl(); });
  $('img-rot-l').addEventListener('click', () => rotateImg(-90));
  $('img-rot-r').addEventListener('click', () => rotateImg(90));
  $('img-angle').addEventListener('pointerdown', pushImgHistory);
  $('img-angle').addEventListener('input', e => setImgAngle(e.target.value));
  $('img-angle-number').addEventListener('change', e => { pushImgHistory(); setImgAngle(e.target.value); });
  $('img-view-zoom').addEventListener('pointerdown', pushImgHistory);
  $('img-view-zoom').addEventListener('input', e => setImgZoom(e.target.value, false));
  $('img-flip-x').addEventListener('click', () => flipImg('x'));
  $('img-flip-y').addEventListener('click', () => flipImg('y'));
  $('img-ratio').addEventListener('change', e => { pushImgHistory(); imgState.ratio = e.target.value; imgState.outW=0; imgState.outH=0; applyRatio(); });
  $('img-size-lock').addEventListener('click', () => { imgState.sizeLock=!imgState.sizeLock; syncImgControls(); });
  $('img-out-w').addEventListener('change', e => { const oldW=imgState.outW||1, oldH=imgState.outH||1; imgState.outW=Math.max(1,Math.min(16000,+e.target.value||1)); if(imgState.sizeLock)imgState.outH=Math.max(1,Math.round(imgState.outW*oldH/oldW)); updateImgInfo(); });
  $('img-out-h').addEventListener('change', e => { const oldW=imgState.outW||1, oldH=imgState.outH||1; imgState.outH=Math.max(1,Math.min(16000,+e.target.value||1)); if(imgState.sizeLock)imgState.outW=Math.max(1,Math.round(imgState.outH*oldW/oldH)); updateImgInfo(); });
  $('img-undo').addEventListener('click', undoImg); $('img-redo').addEventListener('click', redoImg);
  $('img-reset').addEventListener('click', resetImgEditing);
  $('img-insert').addEventListener('click', exportAndInsertImg);
  $('img-close').addEventListener('click', closeImgModal);
  $('img-close-x').addEventListener('click', closeImgModal);
  $('img-modal').addEventListener('click', e => { if (e.target === $('img-modal')) closeImgModal(); });
  const stage = $('img-stage');
  stage.addEventListener('pointerdown', stagePointer);
  stage.addEventListener('pointermove', stagePointerMove);
  stage.addEventListener('pointerup', stagePointerUp);
  stage.addEventListener('pointercancel', stagePointerUp);
  stage.addEventListener('wheel', e => { if(!imgState.img)return; e.preventDefault(); setImgZoom(imgState.viewZoom*(e.deltaY>0?.9:1.1), true); }, {passive:false});
  stage.addEventListener('keydown', e => {
    if(e.key===' '){imgState.spaceDown=true;e.preventDefault();return;}
    if((e.key==='+'||e.key==='=')&&imgState.img){e.preventDefault();setImgZoom(imgState.viewZoom+10,true);return;}
    if(e.key==='-'&&imgState.img){e.preventDefault();setImgZoom(imgState.viewZoom-10,true);return;}
    if(!e.key.startsWith('Arrow')||!imgState.img)return;
    e.preventDefault();pushImgHistory();
    const n=e.shiftKey?10:1;
    if (imgState.spaceDown) {
      if(e.key==='ArrowLeft')imgState.panX-=n;if(e.key==='ArrowRight')imgState.panX+=n;
      if(e.key==='ArrowUp')imgState.panY-=n;if(e.key==='ArrowDown')imgState.panY+=n;
      drawImg();
      return;
    }
    if (e.altKey) {
      const handle = imgState.keyboardHandle || 'se';
      const dx=e.key==='ArrowLeft'?-n:e.key==='ArrowRight'?n:0;
      const dy=e.key==='ArrowUp'?-n:e.key==='ArrowDown'?n:0;
      resizeCropWithKeyboard(handle,dx,dy);
      return;
    }
    if(e.key==='ArrowLeft')imgState.crop.x-=n;if(e.key==='ArrowRight')imgState.crop.x+=n;if(e.key==='ArrowUp')imgState.crop.y-=n;if(e.key==='ArrowDown')imgState.crop.y+=n;
    clampCrop();updateCropUI();updateImgInfo();
  });
  stage.addEventListener('keyup', e => { if(e.key===' ')imgState.spaceDown=false; });
  stage.addEventListener('blur', () => { imgState.spaceDown=false; });
  imgState.keyboardHandle = 'se';
  document.querySelectorAll('.crop-handle').forEach(handle => {
    const setHandle = () => { imgState.keyboardHandle = handle.dataset.handle; };
    handle.addEventListener('focus', setHandle);
    handle.addEventListener('pointerdown', setHandle);
  });
  ['formula-mode', 'tpl-action', 'img-ratio'].forEach(id => {
    const select = $(id);
    if (!select) return;
    syncSelectAccessibleName(select);
    select.addEventListener('change', () => syncSelectAccessibleName(select));
  });

  /* --- 9. 文件另存、重命名与历史记录 [联动: reader/render.js, core/history.js, core/tabs.js] --- */
  $('btn-saveas').addEventListener('click', saveAs);
  $('file-title').addEventListener('click', openFileRename); // 标题栏就地重命名
  $('btn-recent').addEventListener('click', openHistoryModal);
  $('btn-reload').addEventListener('click', () => { if (state.file && state.mode === 'file') loadFile(state.file, { force: true }); });
  $('recent-clear').addEventListener('click', clearRecent);
  $('history-clear').addEventListener('click', clearRecent);
  $('history-close').addEventListener('click', () => $('history-modal').classList.add('hidden'));
  $('history-modal').addEventListener('click', e => { if (e.target === $('history-modal')) $('history-modal').classList.add('hidden'); });

  /* --- 10. 目录大纲与自动修正查看 [联动: reader/toc.js, reader/fixes.js] --- */
  $('btn-toc').addEventListener('click', () => toggleSide('toc'));
  $('btn-fix').addEventListener('click', showFixModal);
  if ($('fix-ai-btn')) $('fix-ai-btn').addEventListener('click', () => { if (typeof handleAiDocumentFix === 'function') handleAiDocumentFix(); });
  $('fix-close').addEventListener('click', () => $('fix-modal').classList.add('hidden'));
  $('fix-save').addEventListener('click', async () => {
    const content = state.fixed || '';
    if (!content) return;
    if (state.mode === 'virtual' || !state.file) { await saveAs(); return; }
    if (!hasPy) { showToast(_t('toast.browserUseSaveAs')); return; }
    const out = await py.save_fixed(state.file, content);
    showToast(out ? (_t('toast.savedPrefix') + out) : _t('toast.saveFailedSimple'));
  });
  $('fix-modal').addEventListener('click', e => { if (e.target === $('fix-modal')) $('fix-modal').classList.add('hidden'); });

  /* --- 11. 全文搜索、主题外观与字号缩放 [联动: reader/search.js, core/settings.js] --- */
  $('btn-search').addEventListener('click', toggleSearch);
  $('search-close').addEventListener('click', () => closeSearch({ restoreFocus: true }));
  $('search-next').addEventListener('click', () => jumpToMark(1));
  $('search-prev').addEventListener('click', () => jumpToMark(-1));
  let searchDebounce = null;
  let initialSearchFocused = false;
  let searchQueryNeedsInitialEnter = false;
  $('search-input').addEventListener('input', e => {
    // Treat every edit as a new query even when the debounce wins the race
    // before Enter.  The first Enter must establish the query's initial
    // result; the following Enter advances to the next match.
    initialSearchFocused = false;
    searchQueryNeedsInitialEnter = true;
    clearTimeout(searchDebounce);
    searchDebounce = setTimeout(() => doSearch(e.target.value, undefined, { jump: false }), 40);
  });

  $('search-input').addEventListener('keydown', e => {
    if (e.key === 'Enter') {
      e.preventDefault();
      clearTimeout(searchDebounce);
      if (searchQueryNeedsInitialEnter || globalSearchState.query !== e.target.value) {
        searchQueryNeedsInitialEnter = false;
        initialSearchFocused = false;
        // Enter must win against the input debounce and immediately reveal the result.
        doSearch(e.target.value, undefined, { jump: true });
        return;
      }
      if (!initialSearchFocused) {
        initialSearchFocused = true;
        consumeInitialSearchJump();
        return;
      } else {
        jumpToMark(e.shiftKey ? -1 : 1);
      }
    }
  });
  $('btn-theme').addEventListener('click', toggleTheme);
  $('btn-a').addEventListener('click', () => zoom(-10));
  $('btn-A').addEventListener('click', () => zoom(10));

  /* --- 11.5. 超长文档智能翻页控制器事件 [联动: reader/render.js] --- */
  if (typeof initPaginationEvents === 'function') initPaginationEvents();

  /* --- 12. 专业导出工作台与排版微缩预览 (Export Studio) [联动: features/export.js] --- */
  $('btn-print').addEventListener('click', openExportModal);
  $('export-close').addEventListener('click', closeExportModal);
  $('export-modal').addEventListener('click', e => { if (e.target === $('export-modal')) closeExportModal(); });
  if ($('export-preview-card')) {
    $('export-preview-card').addEventListener('click', () => {
      const modal = $('export-preview-modal');
      if (modal) {
        modal.classList.remove('hidden');
        updateExportLivePreview();
      }
    });
  }
  if ($('export-preview-close')) {
    $('export-preview-close').addEventListener('click', () => {
      const modal = $('export-preview-modal');
      if (modal) modal.classList.add('hidden');
    });
  }
  if ($('export-preview-modal')) {
    $('export-preview-modal').addEventListener('click', e => {
      if (e.target === $('export-preview-modal')) $('export-preview-modal').classList.add('hidden');
    });
  }
  const exportFormatTabs = Array.from(document.querySelectorAll('.exp-fmt'));
  const activateExportFormat = btn => {
    exportFormatTabs.forEach(b => { b.classList.toggle('active', b === btn); b.setAttribute('aria-selected', b === btn ? 'true' : 'false'); });
    $('export-opts').setAttribute('aria-labelledby', btn.id);
    state.export.fmt = btn.dataset.fmt;
    renderExportSections();
  };
  exportFormatTabs.forEach(btn => btn.addEventListener('click', () => activateExportFormat(btn)));
  const moveExportTabFocus = (current, offset) => {
    const target = exportFormatTabs[(current + offset + exportFormatTabs.length) % exportFormatTabs.length];
    target.focus();
    activateExportFormat(target);
  };
  exportFormatTabs.forEach((btn, index) => btn.addEventListener('keydown', e => {
    if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();
    const next = e.key === 'ArrowRight' ? index + 1
      : e.key === 'ArrowLeft' ? index - 1
      : e.key === 'Home' ? 0
      : exportFormatTabs.length - 1;
    const target = exportFormatTabs[(next + exportFormatTabs.length) % exportFormatTabs.length];
    target.focus();
    activateExportFormat(target);
    requestAnimationFrame(() => {
      if (document.activeElement !== target) target.focus({ preventScroll: true });
    });
  }));
  $('export-print').addEventListener('click', () => window.print());
  $('export-run').addEventListener('click', runExport);
  $('exp-save-preset').addEventListener('click', expSavePreset);
  $('exp-reset').addEventListener('click', () => {
    state.export.options = expDeepMerge(state.export.defaults, {});
    renderExportSections();
    const sel = $('exp-preset'); if (sel) sel.value = '__custom__';
  });
  $('export-box').addEventListener('keydown', e => {
    if (e.key === 'Escape' && !$('export-modal').classList.contains('hidden')) { e.stopPropagation(); closeExportModal(); }
  });

  /* --- 13. 系统级功能：文件关联与返回顶部 [联动: core/settings.js] --- */
  $('btn-assoc').addEventListener('click', installAssoc);
  $('top-btn').addEventListener('click', () => { $('content').scrollTo({ top: 0, behavior: 'smooth' }); });

  /* --- 14. AI 智能助手与会话管理 (AI Assistant) [联动: features/ai.js] --- */
  $('btn-ai').addEventListener('click', typeof handleTopAiButtonClick === 'function' ? handleTopAiButtonClick : toggleAiPanel);
  $('ai-close').addEventListener('click', () => { $('ai-panel').classList.add('hidden'); });
  $('ai-settings-open').addEventListener('click', () => openAiModal('ai-settings-modal', $('ai-settings-open')));
  $('ai-settings-close').addEventListener('click', () => closeAiModal('ai-settings-modal'));
  $('ai-settings-modal').addEventListener('click', e => { if (e.target === $('ai-settings-modal')) closeAiModal('ai-settings-modal'); });
  $('ai-history-open').addEventListener('click', () => { openAiModal('ai-history-modal', $('ai-history-open')); loadAiSessions(); });
  $('ai-history-close').addEventListener('click', () => closeAiModal('ai-history-modal'));
  $('ai-history-modal').addEventListener('click', e => { if (e.target === $('ai-history-modal')) closeAiModal('ai-history-modal'); });
  $('ai-history-search').addEventListener('input', renderAiSessionSelect);
  $('ai-history-copy').addEventListener('click', copyCurrentConversation);
  $('ai-history-export').addEventListener('click', exportCurrentConversation);
  $('ai-history-clear').addEventListener('click', clearAiSessions);
  $('ai-provider').addEventListener('change', onAiProviderChange);
  $('ai-provider-search') && $('ai-provider-search').addEventListener('input', () => {
    fillAiProviders(state.ai.providers || [], { provider_id: $('ai-provider').value, model: $('ai-model').value });
  });
  $('ai-provider-new').addEventListener('click', newAiProvider);
  $('ai-provider-delete').addEventListener('click', deleteAiProvider);
  $('ai-mode').addEventListener('change', () => { /* 协议变更由保存设置时生效 */ });
  $('ai-url-reset').addEventListener('click', resetAiUrl);
  $('ai-key-toggle').addEventListener('click', toggleAiKey);
  $('ai-key-clear').addEventListener('click', clearAiKey);
  $('ai-models-btn').addEventListener('click', loadAiModels);
  $('ai-model').addEventListener('change', updateAiConnectionSummary);
  $('ai-test-connection').addEventListener('click', testAiConnection);
  $('ai-save-key').addEventListener('click', saveAiSelection);
  document.querySelectorAll('.ai-act').forEach(b => b.addEventListener('click', () => runAi(b.dataset.act)));
  $('ai-run').addEventListener('click', () => runAi('ask'));
  $('ai-stop').addEventListener('click', () => { if (state.ai.aborter) state.ai.aborter.abort(); });
  $('ai-apply').addEventListener('click', applyAi);
  $('ai-copy').addEventListener('click', copyAi);
  $('ai-saveas').addEventListener('click', saveAiAs);
  $('ai-prompt').addEventListener('keydown', e => { if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); $('ai-run').click(); } });
  $('ai-template').addEventListener('change', onAiTemplateChange);
  $('ai-tpl-btn').addEventListener('click', openTplModal);
  $('tpl-new').addEventListener('click', () => selectTpl(null));
  $('tpl-copy') && $('tpl-copy').addEventListener('click', copyCurrentSkill);
  $('tpl-save').addEventListener('click', saveTplForm);
  $('tpl-ai-generate') && $('tpl-ai-generate').addEventListener('click', generateSkillDraft);
  $('tpl-publish') && $('tpl-publish').addEventListener('click', publishCurrentSkill);
  $('tpl-del').addEventListener('click', deleteCurrentTpl);
  $('tpl-close').addEventListener('click', () => $('tpl-modal').classList.add('hidden'));
  $('ai-session').addEventListener('change', onAiSessionChange);
  $('ai-save-session').addEventListener('click', saveCurrentSession);
  $('ai-del-session').addEventListener('click', deleteCurrentSession);
  $('ai-clear-ctx').addEventListener('click', clearAiContext);
  $('ai-expand-toggle') && $('ai-expand-toggle').addEventListener('click', toggleAiFullscreen);
  $('tpl-search') && $('tpl-search').addEventListener('input', renderTplList);
  $('tpl-import-btn') && $('tpl-import-btn').addEventListener('click', () => $('tpl-file-input') && $('tpl-file-input').click());
  $('tpl-file-input') && $('tpl-file-input').addEventListener('change', e => { if (e.target.files) Array.from(e.target.files).forEach(f => importTemplatesFromFile(f)); e.target.value = ''; });
  $('tpl-github-preview-btn') && $('tpl-github-preview-btn').addEventListener('click', previewGithubSkillImport);
  $('tpl-github-url') && $('tpl-github-url').addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); previewGithubSkillImport(); } });
  $('tpl-export-btn') && $('tpl-export-btn').addEventListener('click', exportTemplatesAsJson);
  $('tpl-close-btn') && $('tpl-close-btn').addEventListener('click', () => $('tpl-modal').classList.add('hidden'));
  bindAiResize(); // 绑定 AI 侧边栏宽度拖拽调节手柄

  /* --- 15. 局域网分享 (LAN Share) [联动: features/share.js] --- */
  $('btn-share').addEventListener('click', openShareModal);
  $('share-start').addEventListener('click', startShare);
  $('share-stop').addEventListener('click', stopShare);
  $('share-close').addEventListener('click', () => { $('share-modal').classList.add('hidden'); });
  $('share-modal').addEventListener('click', e => { if (e.target === $('share-modal')) $('share-modal').classList.add('hidden'); });

  /* --- 16. 侧边栏 Tab 切换与滚动监听 --- */
  $('tab-toc').addEventListener('click', () => showSide('toc'));
  $('tab-files').addEventListener('click', () => showSide('files'));
  $('content').addEventListener('scroll', () => {
    $('top-btn').classList.toggle('hidden', $('content').scrollTop < 600);
  });

  /* --- 17. 客户端内版本检查、语言切换、开机自启与自动升级 [联动: core/i18n.js, features/updater.js] --- */
  if ($('btn-lang')) $('btn-lang').addEventListener('click', () => { closeMoreMenu(); if (window.i18n) window.i18n.openModal(); });
  if ($('btn-autostart')) $('btn-autostart').addEventListener('click', () => { closeMoreMenu(); toggleAutostart(); });
  if ($('btn-check-update')) $('btn-check-update').addEventListener('click', () => { closeMoreMenu(); checkUpdate(false); });

  if ($('status-update-badge')) $('status-update-badge').addEventListener('click', openUpdateModal);
  if ($('update-close')) $('update-close').addEventListener('click', closeUpdateModal);
  if ($('btn-update-start')) $('btn-update-start').addEventListener('click', startUpdateDownload);
  if ($('btn-update-cancel')) $('btn-update-cancel').addEventListener('click', cancelUpdateDownload);
  if ($('btn-update-browser')) $('btn-update-browser').addEventListener('click', () => {
    if (updateInfo && updateInfo.html_url) openExternal(updateInfo.html_url);
    else if (upgradeUrl) openExternal(upgradeUrl);
  });

  /* --- 17.1 编辑器增强：禅模式与全功能插入向导 [联动: editor/editor.js] --- */
  if ($('btn-zen-mode')) $('btn-zen-mode').addEventListener('click', () => toggleZenMode());
  if ($('btn-insert-table')) $('btn-insert-table').addEventListener('click', () => openTableModal());
  if ($('btn-insert-code-chunk')) $('btn-insert-code-chunk').addEventListener('click', () => openCodeChunkModal());
  if ($('btn-insert-diagram')) $('btn-insert-diagram').addEventListener('click', () => openDiagramModal());
  if ($('btn-insert-doc-import')) $('btn-insert-doc-import').addEventListener('click', () => openDocImportModal());
  if ($('btn-insert-frontmatter')) $('btn-insert-frontmatter').addEventListener('click', () => openFrontmatterModal());

  // 交互式代码块弹窗事件
  if ($('code-chunk-modal-close')) $('code-chunk-modal-close').addEventListener('click', closeCodeChunkModal);
  if ($('code-chunk-cancel')) $('code-chunk-cancel').addEventListener('click', closeCodeChunkModal);
  if ($('code-chunk-insert')) $('code-chunk-insert').addEventListener('click', insertCodeChunkFromModal);
  if ($('code-chunk-modal')) $('code-chunk-modal').addEventListener('click', e => { if (e.target === $('code-chunk-modal')) closeCodeChunkModal(); });
  if ($('code-chunk-lang')) {
    $('code-chunk-lang').addEventListener('change', () => {
      if ($('code-chunk-code') && (typeof CODE_CHUNK_SAMPLES !== 'undefined')) {
        $('code-chunk-code').value = CODE_CHUNK_SAMPLES[$('code-chunk-lang').value] || CODE_CHUNK_SAMPLES.python;
      }
    });
  }

  // 科学图表弹窗事件
  if ($('diagram-modal-close')) $('diagram-modal-close').addEventListener('click', closeDiagramModal);
  if ($('diagram-cancel')) $('diagram-cancel').addEventListener('click', closeDiagramModal);
  if ($('diagram-insert')) $('diagram-insert').addEventListener('click', insertDiagramFromModal);
  if ($('diagram-modal')) $('diagram-modal').addEventListener('click', e => { if (e.target === $('diagram-modal')) closeDiagramModal(); });
  if ($('diagram-type')) {
    $('diagram-type').addEventListener('change', () => {
      if ($('diagram-code') && (typeof DIAGRAM_SAMPLES !== 'undefined')) {
        $('diagram-code').value = DIAGRAM_SAMPLES[$('diagram-type').value] || DIAGRAM_SAMPLES.plantuml;
      }
    });
  }

  // 子文档引用弹窗事件
  if ($('doc-import-modal-close')) $('doc-import-modal-close').addEventListener('click', closeDocImportModal);
  if ($('doc-import-cancel')) $('doc-import-cancel').addEventListener('click', closeDocImportModal);
  if ($('doc-import-insert')) $('doc-import-insert').addEventListener('click', insertDocImportFromModal);
  if ($('doc-import-modal')) $('doc-import-modal').addEventListener('click', e => { if (e.target === $('doc-import-modal')) closeDocImportModal(); });

  // 样式元数据 (Frontmatter) 弹窗事件
  if ($('frontmatter-modal-close')) $('frontmatter-modal-close').addEventListener('click', closeFrontmatterModal);
  if ($('fm-modal-cancel')) $('fm-modal-cancel').addEventListener('click', closeFrontmatterModal);
  if ($('fm-modal-insert')) $('fm-modal-insert').addEventListener('click', insertFrontmatterFromModal);
  if ($('frontmatter-modal')) $('frontmatter-modal').addEventListener('click', e => { if (e.target === $('frontmatter-modal')) closeFrontmatterModal(); });

  // 样式定制弹窗事件与预设模板
  if ($('style-modal-close')) $('style-modal-close').addEventListener('click', closeStyleModal);
  if ($('style-modal-cancel')) $('style-modal-cancel').addEventListener('click', closeStyleModal);
  if ($('style-modal-save')) $('style-modal-save').addEventListener('click', saveStyleModal);
  if ($('style-custom-modal')) $('style-custom-modal').addEventListener('click', e => { if (e.target === $('style-custom-modal')) closeStyleModal(); });

  const STYLE_PRESETS = {
    indent: '/* 中文段落首行缩进 2 字符 */\n.markdown-body p {\n  text-indent: 2em;\n  margin-bottom: 0.8em;\n}\n\n',
    table: '/* 现代化精美表格与圆角 */\n.markdown-body table {\n  border-collapse: separate;\n  border-spacing: 0;\n  border-radius: 8px;\n  overflow: hidden;\n  border: 1px solid var(--border);\n}\n.markdown-body th {\n  background: var(--bg2);\n  font-weight: 600;\n}\n.markdown-body tr:nth-child(even) {\n  background: rgba(127, 127, 127, 0.05);\n}\n\n',
    font: '/* 优化代码块与等宽字体 */\n.markdown-body code, .markdown-body pre {\n  font-family: "Fira Code", "Cascadia Code", Consolas, Monaco, monospace !important;\n  font-size: 13.5px;\n}\n\n',
    print: '/* 打印与 PDF 导出分页规则 */\n@media print {\n  h1, h2, h3 { page-break-after: avoid; }\n  pre, blockquote, table { page-break-inside: avoid; }\n}\n\n'
  };

  function insertStylePreset(type) {
    const textarea = $('style-custom-css');
    if (!textarea) return;
    const snippet = STYLE_PRESETS[type] || '';
    if (!snippet) return;
    if (textarea.value && !textarea.value.endsWith('\n')) textarea.value += '\n';
    textarea.value += snippet;
    textarea.focus();
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    showToast(_t('toast.stylePresetAdded') || '已添加排版模板', 1000);
  }

  if ($('btn-preset-indent')) $('btn-preset-indent').addEventListener('click', () => insertStylePreset('indent'));
  if ($('btn-preset-table')) $('btn-preset-table').addEventListener('click', () => insertStylePreset('table'));
  if ($('btn-preset-font')) $('btn-preset-font').addEventListener('click', () => insertStylePreset('font'));
  if ($('btn-preset-print')) $('btn-preset-print').addEventListener('click', () => insertStylePreset('print'));

  // 样式代码编辑框 Tab 缩进与快捷保存
  ['style-custom-css', 'style-custom-head'].forEach(id => {
    const el = $(id);
    if (!el) return;
    el.addEventListener('keydown', e => {
      if (e.key === 'Tab') {
        e.preventDefault();
        const start = el.selectionStart;
        const end = el.selectionEnd;
        el.value = el.value.substring(0, start) + '  ' + el.value.substring(end);
        el.selectionStart = el.selectionEnd = start + 2;
      } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        saveStyleModal();
      }
    });
  });

  // 禅模式顶部工具栏动态悬停唤出 (仿 Windows 隐藏任务栏逻辑)
  window.addEventListener('mousemove', e => {
    if (!document.body.classList.contains('zen-mode')) return;
    const toolbar = $('toolbar');
    if (!toolbar) return;
    if (e.clientY <= 10) {
      toolbar.classList.add('zen-toolbar-revealed');
    } else if (e.clientY > 54) {
      toolbar.classList.remove('zen-toolbar-revealed');
    }
  });

  if ($('lang-modal')) $('lang-modal').addEventListener('click', e => { if (e.target === $('lang-modal')) if (window.i18n) window.i18n.closeModal(); });
  if ($('table-modal')) $('table-modal').addEventListener('click', e => { if (e.target === $('table-modal')) closeTableModal(); });
  window.addEventListener('readmd:language-changed', e => {
    const lang = e.detail.lang;
    const labelEl = $('lang-current-label');
    if (labelEl && window.i18n && window.i18n.meta[lang]) {
      labelEl.textContent = window.i18n.meta[lang].native || lang;
    }
    updateStatus();
    updateDocStatistics();
    ['formula-mode', 'tpl-action', 'img-ratio'].forEach(id => {
      const select = $(id);
      if (select) syncSelectAccessibleName(select);
    });
  });

  /* --- 18. 多标签页、全局拖拽与窗口响应 [联动: core/tabs.js, core/dragdrop.js] --- */
  bindGlobalDragAndDrop();      // 全局拖拽支持
  bindTabOverflowEvents();      // 标签栏左右滚动溢出支持
  bindTabContextMenuEvents();   // 标签右键菜单
  window.addEventListener('resize', () => {
    renderTabsBar();
  });

  /* --- 19. 全局键盘快捷键矩阵 (Global Keyboard Shortcuts) --- */
  document.addEventListener('keydown', e => {
    const mod = e.ctrlKey || e.metaKey;

    // 模态弹窗 ESC 优先拦截与层级关闭
    if (e.key === 'Escape') {
      const allModalIds = [
        'close-confirm-modal',
        'confirm-modal',
        'code-chunk-modal', 'diagram-modal', 'doc-import-modal', 'frontmatter-modal',
        'table-modal', 'export-preview-modal', 'export-modal', 'convert-modal',
        'update-modal', 'style-custom-modal', 'lang-modal', 'ai-history-modal',
        'ai-settings-modal', 'formula-modal', 'presentation-modal'
      ];
      const activeModal = allModalIds.find(id => $(id) && !$(id).classList.contains('hidden'));
      if (activeModal) {
        e.preventDefault();
        e.stopPropagation();
        if (activeModal === 'close-confirm-modal' || activeModal === 'confirm-modal') return;
        if (activeModal === 'code-chunk-modal') closeCodeChunkModal();
        else if (activeModal === 'diagram-modal') closeDiagramModal();
        else if (activeModal === 'doc-import-modal') closeDocImportModal();
        else if (activeModal === 'frontmatter-modal') closeFrontmatterModal();
        else if (activeModal === 'table-modal') closeTableModal();
        else if (activeModal === 'export-preview-modal') $(activeModal).classList.add('hidden');
        else if (activeModal === 'export-modal') closeExportModal();
        else if (activeModal === 'convert-modal') $('convert-modal').classList.add('hidden');
        else if (activeModal === 'update-modal') {
          if (!isUpdateDownloading()) $('update-modal').classList.add('hidden');
        }
        else if (activeModal === 'style-custom-modal') closeStyleModal();
        else if (activeModal === 'lang-modal' && window.i18n) window.i18n.closeModal();
        else if (activeModal.startsWith('ai-')) closeAiModal(activeModal);
        else if (activeModal === 'presentation-modal') window.closePresentationMode?.();
        else $(activeModal).classList.add('hidden');
        return;
      }
      if (document.body.classList.contains('zen-mode')) {
        e.preventDefault();
        toggleZenMode(false);
        return;
      }
    }

    const presentationVisible = $('presentation-modal') && !$('presentation-modal').classList.contains('hidden');
    if (e.key === 'F11') {
      e.preventDefault();
      if (presentationVisible) {
        window.togglePresentationFullscreen?.($('presentation-modal'));
      } else {
        toggleZenMode();
      }
    }
    else if (e.key === 'F2') { e.preventDefault(); openFileRename(); } // F2: 文件重命名
    else if (mod && e.key.toLowerCase() === 'o') { e.preventDefault(); $('btn-open').click(); } // Ctrl+O: 打开文件
    else if (mod && e.key.toLowerCase() === 'f') { // Ctrl+F: 全文搜索
      e.preventDefault();
      toggleSearch();
    }
    else if (mod && e.key.toLowerCase() === 'u') { e.preventDefault(); openWebDialog(); } // Ctrl+U: 网页抓取
    else if (mod && e.key.toLowerCase() === 'e') { e.preventDefault(); if (!$('btn-edit').disabled) toggleEdit(); } // Ctrl+E: 编辑模式
    else if (mod && e.key.toLowerCase() === 's') { // Ctrl+S: 保存文档
      if (state.editing) { e.preventDefault(); saveEdit(); }
      else if (state.mode === 'virtual' || (getActiveTab() && getActiveTab().isVirtual)) {
        e.preventDefault();
        saveAs();
      }
    }
    else if (mod && !e.shiftKey && e.key.toLowerCase() === 'v') { // Ctrl+V: 智能剪贴板新建
      const t = e.target;
      const inField = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable || (t.closest && t.closest('.CodeMirror, .cm-editor, [contenteditable="true"]')));
      if (inField) return; // 输入框内放行浏览器原生粘贴
      e.preventDefault();
      createFromClipboard();
    }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 's') { // Ctrl+Shift+S / Cmd+Shift+S: 即时行对齐
      e.preventDefault();
      if (window.alignEditorAndPreview) window.alignEditorAndPreview();
    }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'f') { e.preventDefault(); toggleSide('toc'); } // Ctrl+Shift+F: 目录大纲
    else if (mod && e.key.toLowerCase() === 'd') { e.preventDefault(); toggleTheme(); } // Ctrl+D: 主题切换
    else if (mod && e.key.toLowerCase() === 'r') { e.preventDefault(); if (state.file && state.mode === 'file') loadFile(state.file, { force: true }); } // Ctrl+R: 强制重载文件
    else if (mod && !e.shiftKey && e.key.toLowerCase() === 'p') { e.preventDefault(); openExportModal(); } // Ctrl+P: 导出面板
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'a') { e.preventDefault(); if (typeof handleTopAiButtonClick === 'function') handleTopAiButtonClick(); else toggleAiPanel(); } // Ctrl+Shift+A: AI面板/AI编辑助手
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'p' && state.editing) { e.preventDefault(); openMdCommandPalette(); } // Ctrl+Shift+P: 命令面板
    else if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoom(10); } // Ctrl++: 放大字号
    else if (mod && e.key === '-') { e.preventDefault(); zoom(-10); } // Ctrl+-: 缩小字号
    else if (mod && e.key === 'ArrowLeft') { e.preventDefault(); historyBack(); } // Alt/Ctrl+Left: 历史后退
    else if (mod && e.key === 'ArrowRight') { e.preventDefault(); historyForward(); } // Alt/Ctrl+Right: 历史前进
    else if (e.key === 'Escape') {
      // 级联清理所有开启的浮层
      if ($('style-custom-modal') && !$('style-custom-modal').classList.contains('hidden')) { closeStyleModal(); return; }
      if ($('formula-modal') && !$('formula-modal').classList.contains('hidden')) { closeFormulaModal(); return; }
      if ($('img-modal') && !$('img-modal').classList.contains('hidden')) { closeImgModal(); return; }
      if ($('history-modal') && !$('history-modal').classList.contains('hidden')) { $('history-modal').classList.add('hidden'); return; }
      if ($('export-preview-modal') && !$('export-preview-modal').classList.contains('hidden')) { $('export-preview-modal').classList.add('hidden'); return; }
      if ($('tab-context-menu') && !$('tab-context-menu').classList.contains('hidden')) { closeTabContextMenu({ restoreFocus: true }); return; }
      closeMoreMenu(true);
      closeSearch({ restoreFocus: true });
      if ($('fix-modal')) $('fix-modal').classList.add('hidden');
      if (typeof closeWebDialog === 'function') closeWebDialog();
      if ($('ai-panel')) $('ai-panel').classList.add('hidden');
      if ($('share-modal')) $('share-modal').classList.add('hidden');
      if ($('tpl-modal')) $('tpl-modal').classList.add('hidden');
      if ($('convert-modal')) $('convert-modal').classList.add('hidden');
      if ($('lang-modal') && window.i18n) window.i18n.closeModal();
      if ($('side') && !$('side').classList.contains('hidden')) $('side').classList.add('hidden');
      if ($('table-modal')) closeTableModal();
      closeFormulaModal(); closeMdPopups();
      stopConvertPoll();
      if (document.body.classList.contains('zen-mode')) toggleZenMode(false);
      if (state.editing) confirmExitEdit();
    }
  });


  document.addEventListener('click', closeMdPopups);
  setupModalAccessibility();
}

function getModalRoots() {
  return [
    'close-confirm-modal', 'code-chunk-modal', 'diagram-modal', 'doc-import-modal',
    'frontmatter-modal', 'table-modal', 'export-preview-modal', 'export-modal',
    'convert-modal', 'update-modal', 'style-custom-modal', 'lang-modal',
    'ai-history-modal', 'ai-settings-modal', 'formula-modal', 'presentation-modal',
    'img-modal', 'history-modal', 'share-modal', 'tpl-modal', 'url-modal',
    'save-conflict-modal', 'fix-modal', 'continuous-modal', 'confirm-modal'
  ].map(id => $(id)).filter(Boolean);
}

function isVisibleModal(modal) {
  return modal && !modal.classList.contains('hidden');
}

function getModalFocusable(modal) {
  return Array.from(modal.querySelectorAll('a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'))
    .filter(el => !el.closest('.hidden'));
}

function setupModalAccessibility() {
  const modalOpeners = new Map();
  const openModalStack = [];
  document.addEventListener('keydown', event => {
    if (event.key !== 'Tab') return;
    const modal = [...openModalStack].reverse().find(isVisibleModal);
    if (!modal) return;
    const focusable = getModalFocusable(modal);
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!modal.contains(event.target)) {
      event.preventDefault();
      first.focus();
      return;
    }
    if (event.shiftKey && event.target === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && event.target === last) {
      event.preventDefault();
      first.focus();
    }
  }, true);

  const observer = new MutationObserver(mutations => {
    for (const mutation of mutations) {
      const modal = mutation.target;
      const visible = !modal.classList.contains('hidden');
      const stackIndex = openModalStack.indexOf(modal);
      if (visible) {
        if (stackIndex < 0) openModalStack.push(modal);
        if (!modalOpeners.has(modal)) {
          const opener = document.activeElement;
          modalOpeners.set(modal, modal.contains(opener) ? null : opener);
        }
        if (!modal.contains(document.activeElement)) {
          const focusable = getModalFocusable(modal);
          if (focusable.length) focusable[0].focus();
        }
      } else if (!visible) {
        if (stackIndex >= 0) openModalStack.splice(stackIndex, 1);
        if (modal.contains(document.activeElement)) {
          const opener = modalOpeners.get(modal);
          if (opener && opener.isConnected) opener.focus({ preventScroll: true });
        }
        modalOpeners.delete(modal);
      }
    }
  });
  getModalRoots().forEach(modal => observer.observe(modal, { attributes: true, attributeFilter: ['class'] }));
}

function syncDesktopControls() {
  const folderButton = $('btn-folder');
  if (!folderButton) return;
  folderButton.disabled = !hasPy;
  if (!hasPy) {
    const notice = window.i18n ? window.i18n.t('toast.openFolderBrowserNotice') : '';
    folderButton.title = notice || '浏览器模式下请使用“打开文件”';
    folderButton.setAttribute('aria-description', notice || '浏览器模式下请使用“打开文件”');
  } else {
    folderButton.removeAttribute('aria-description');
  }
}

/* ----------------------------------------------------------------------------------------------
   初始文件智能分流器 (Smart Initial File Dispatcher)
   ---------------------------------------------------------------------------------------------- */
async function openInitialFile(path) {
  if (!path) return;
  // Markdown 文档 -> 直接阅读器加载
  if (MD_RE.test(path)) { loadFile(path); return; }
  // 图片或 PDF 扫描件 -> 触发离线 OCR 识别
  if (IMG_RE.test(path) || /\.pdf$/i.test(path)) { ocrFile(path); return; }
  // Office/Epub/HTML/LaTeX 等 -> 触发万物转 MD 引擎
  convertFile(path);
}

/* ----------------------------------------------------------------------------------------------
  应用启动引导序列 (Application Bootstrap Lifecycle)
  ---------------------------------------------------------------------------------------------- */
let startupServicesStarted = false;

async function init() {
  // 1. Make the already-parsed welcome shell interactive before optional preferences arrive.
  bindPy();
  applySettings();
  syncDesktopControls();
  // 2. Cache the welcome skeleton, then attach input paths without waiting on preferences.
  if ($('content')) state.welcomeHtml = $('content').innerHTML;
  bindEvents();
  updateStatus();
  const params = new URLSearchParams(location.search);
  const file = params.get('file');
  if (file) {
    openInitialFile(file);
  } else {
    restoreLastFile();
  }
  startAutoReload();
  // 3. Report the readable shell now; localized preferences continue in the background.
  finishInit();

  await loadSettings();
  if (window.i18n) await window.i18n.init();
  syncBuildVersionLabels();
  refreshRecent();
  updateModuleUi();
}

/* ----------------------------------------------------------------------------------------------
   就绪通知与后台任务引导 (Post-Initialization Finish)
   ---------------------------------------------------------------------------------------------- */
function finishInit() {
  performance.mark('readmd-app-ready');
  window.__readmdAppReady = true;
  reportNativeReady();
  if (startupServicesStarted) return;
  startupServicesStarted = true;
  checkAutostart(); // 初始化开机自启状态
  startControlPoll(); // 启动后端单例 IPC 唤醒与文件打开指令轮询
  setTimeout(() => checkUpdate(true), 2500); // 延迟 2.5s 静默检查软件更新
}

function reportNativeReady() {
  if (hasPy) {
    if (py.report_ready) { try { py.report_ready(); } catch (e) { /* ignore */ } }
    window.__trayOpenFile = loadFileDialog;
  }
}


/* ----------------------------------------------------------------------------------------------
   恢复上次会话文件 (Restore Last Session State)
   ---------------------------------------------------------------------------------------------- */
async function restoreLastFile() {
  if (state.file) return;
  let last = null;
  try {
    if (hasPy) {
      const s = await py.get_settings();
      last = (s && s.last) || null;
    }
  } catch (e) { /* ignore */ }
  if (!last) last = localStorage.getItem('readmd-last');
  if (last && /\.(md|markdown|mdown|mkd|mdx|txt)$/i.test(last)) loadFile(last);
}

/* ----------------------------------------------------------------------------------------------
   自定义样式与 Head 模态框逻辑
   ---------------------------------------------------------------------------------------------- */
async function openStyleModal() {
  const modal = $('style-custom-modal');
  if (!modal) return;
  modal.classList.remove('hidden');
  try {
    let res;
    if (hasPy && py.get_custom_styles) {
      res = await py.get_custom_styles();
    } else {
      const r = await apiFetch('/api/style/get');
      res = await r.json();
    }
    if (res && res.ok && res.data) {
      if ($('style-custom-css')) $('style-custom-css').value = res.data.css || '';
      if ($('style-custom-head')) $('style-custom-head').value = res.data.head || '';
    }
  } catch (e) {}
}

async function saveStyleModal() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const css = $('style-custom-css') ? $('style-custom-css').value : '';
  const head = $('style-custom-head') ? $('style-custom-head').value : '';
  try {
    let res;
    if (hasPy && py.save_custom_styles) {
      res = await py.save_custom_styles(css, head);
    } else {
      const r = await apiFetch('/api/style/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ css: css, head: head })
      });
      res = await r.json();
    }
    if (res && res.ok) {
      showToast(_t('toast.savedSuccess'), 1500);
      let dynStyle = $('readmd-user-custom-style');
      if (!dynStyle) {
        dynStyle = document.createElement('style');
        dynStyle.id = 'readmd-user-custom-style';
        document.head.appendChild(dynStyle);
      }
      dynStyle.textContent = css;
      closeStyleModal();
    } else {
      showToast(_t('toast.saveFailedSimple'));
    }
  } catch (e) {
    showToast(_t('toast.saveFailed', { error: e.message }));
  }
}

function closeStyleModal() {
  const modal = $('style-custom-modal');
  if (modal) modal.classList.add('hidden');
}

/* ----------------------------------------------------------------------------------------------
   跨进程事件监听与卸载钩子 (Window & Process Hooks)
   ---------------------------------------------------------------------------------------------- */
window.addEventListener('pywebviewready', async () => {
  const upgraded = !hasPy && bindPy();
  if (upgraded) {
    await loadSettings();
    refreshRecent();
    syncDesktopControls();
    reportNativeReady();
  }
});
window.addEventListener('DOMContentLoaded', init);
window.addEventListener('beforeunload', () => {
  // 退出前记录当前文档的滚动位置
  if (state.file && $('content')) {
    state.scrollPos[normalizePath(state.file)] = $('content').scrollTop;
  }
});
function updateUnloadGuard() {
  const dirty = typeof hasUnsavedEditorChanges === 'function' && hasUnsavedEditorChanges();
  window.onbeforeunload = dirty ? event => {
    event.preventDefault();
    event.returnValue = '';
    return '';
  } : null;
}
