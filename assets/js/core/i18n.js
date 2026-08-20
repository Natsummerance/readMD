'use strict';
/* ============================================================
   ReadMD Core - i18n Internationalization Engine
   Supports 46+ Languages, Dynamic Translation, RTL Layout & System Detection
   ============================================================ */

window.i18n = {
  currentLang: 'zh-CN',
  meta: {
    'zh-CN': {'name': '简体中文', 'native': '简体中文', 'dir': 'ltr'},
    'zh-HK': {'name': '繁体中文（香港）', 'native': '繁體中文（香港）', 'dir': 'ltr'},
    'zh-TW': {'name': '繁体中文（台湾）', 'native': '繁體中文（台灣）', 'dir': 'ltr'},
    'en': {'name': '英语', 'native': 'English', 'dir': 'ltr'},
    'ja': {'name': '日语', 'native': '日本語', 'dir': 'ltr'},
    'ko': {'name': '韩语', 'native': '한국어', 'dir': 'ltr'},
    'fr': {'name': '法语', 'native': 'Français', 'dir': 'ltr'},
    'de': {'name': '德语', 'native': 'Deutsch', 'dir': 'ltr'},
    'es': {'name': '西班牙语', 'native': 'Español', 'dir': 'ltr'},
    'pt': {'name': '葡萄牙语', 'native': 'Português', 'dir': 'ltr'},
    'ru': {'name': '俄语', 'native': 'Русский', 'dir': 'ltr'},
    'it': {'name': '意大利语', 'native': 'Italiano', 'dir': 'ltr'},
    'ar': {'name': '阿拉伯语', 'native': 'العربية', 'dir': 'rtl'},
    'he': {'name': '希伯来语', 'native': 'עברית', 'dir': 'rtl'},
    'ug': {'name': '维吾尔语', 'native': 'ئۇيغۇرچە', 'dir': 'rtl'},
    'bo': {'name': '藏语', 'native': 'བོད་སྐད།', 'dir': 'ltr'},
    'mn': {'name': '蒙古语', 'native': 'Монгол хэл', 'dir': 'ltr'},
    'th': {'name': '泰语', 'native': 'ไทย', 'dir': 'ltr'},
    'vi': {'name': '越南语', 'native': 'Tiếng Việt', 'dir': 'ltr'},
    'id': {'name': '印尼语', 'native': 'Bahasa Indonesia', 'dir': 'ltr'},
    'hi': {'name': '印地语', 'native': 'हिन्दी', 'dir': 'ltr'},
    'bn': {'name': '孟加拉语', 'native': 'বাংলা', 'dir': 'ltr'},
    'my': {'name': '缅甸语', 'native': 'မြန်မာဘာသာ', 'dir': 'ltr'},
    'lo': {'name': '老挝语', 'native': 'ພາສາລາວ', 'dir': 'ltr'},
    'km': {'name': '高棉语', 'native': 'ភាសាខ្មែរ', 'dir': 'ltr'},
    'ms': {'name': '马来语', 'native': 'Bahasa Melayu', 'dir': 'ltr'},
    'ga': {'name': '爱尔兰语', 'native': 'Gaeilge', 'dir': 'ltr'},
    'da': {'name': '丹麦语', 'native': 'Dansk', 'dir': 'ltr'},
    'fi': {'name': '芬兰语', 'native': 'Suomi', 'dir': 'ltr'},
    'kg': {'name': '刚果语', 'native': 'Kikongo', 'dir': 'ltr'},
    'tl': {'name': '菲律宾语', 'native': 'Filipino', 'dir': 'ltr'},
    'no': {'name': '挪威语', 'native': 'Norsk', 'dir': 'ltr'},
    'sv': {'name': '瑞典语', 'native': 'Svenska', 'dir': 'ltr'},
    'kl': {'name': '格陵兰语', 'native': 'Kalaallisut', 'dir': 'ltr'},
    'nl': {'name': '荷兰语', 'native': 'Nederlands', 'dir': 'ltr'},
    'hr': {'name': '克罗地亚语', 'native': 'Hrvatski', 'dir': 'ltr'},
    'rw': {'name': '卢旺达语', 'native': 'Ikinyarwanda', 'dir': 'ltr'},
    'ro': {'name': '罗马尼亚语', 'native': 'Română', 'dir': 'ltr'},
    'mt': {'name': '马耳他语', 'native': 'Malti', 'dir': 'ltr'},
    'ne': {'name': '尼泊尔语', 'native': 'नेपाली', 'dir': 'ltr'},
    'eo': {'name': '世界语', 'native': 'Esperanto', 'dir': 'ltr'},
    'sl': {'name': '斯洛文尼亚语', 'native': 'Slovenščina', 'dir': 'ltr'},
    'tr': {'name': '土耳其语', 'native': 'Türkçe', 'dir': 'ltr'},
    'uk': {'name': '乌克兰语', 'native': 'Українська', 'dir': 'ltr'},
    'el': {'name': '希腊语', 'native': 'Ελληνικά', 'dir': 'ltr'},
    'hu': {'name': '匈牙利语', 'native': 'Magyar', 'dir': 'ltr'}
  },
  dict: {},
  fallbackDict: {},

  zhDefaults: {
    'app.disabled': '未开启',
    'app.enabled': '已开启',
    'app.loading': '加载中...',
    'app.success': '成功',
    'app.failed': '失败',
    'menu.checkUpdate': '检查更新',
    'menu.autoStart': '开机自启',
    'menu.assoc': '设为默认',
    'menu.clipboard': '剪贴板',
    'menu.saveAs': '另存为',
    'menu.share': '共享',
    'menu.fix': '修复详情',
    'menu.lang': '语言'
  },

  /** 初始化多语言模块（毫秒级极速初始化，零阻塞启动） */
  async init() {
    // 首选语言决策：1. LocalStorage 缓存 2. 系统侦测
    let preferred = null;
    try {
      preferred = localStorage.getItem('readmd_language');
    } catch (e) {}

    if (!preferred) {
      preferred = await this.detectSystemLanguage();
    }
    preferred = preferred || 'zh-CN';

    // 如果是默认简体中文，DOM 本身即为中文，直接初始化并载入词库
    if (preferred === 'zh-CN') {
      this.currentLang = 'zh-CN';
      document.documentElement.setAttribute('dir', 'ltr');
      document.documentElement.setAttribute('lang', 'zh-CN');
      const currentLabel = document.getElementById('lang-current-label');
      if (currentLabel) currentLabel.textContent = '简体中文';
      const d = await this.fetchDict('zh-CN');
      if (d) this.dict = d;
      this.loadFallback();
      return;
    }

    // 非中文环境快速载入目标语言并翻译
    await this.setLanguage(preferred, false);
    setTimeout(() => this.loadFallback(), 500);
  },

  /** 异步载入第一兜底英文词库 */
  async loadFallback() {
    if (Object.keys(this.fallbackDict).length > 0) return;
    try {
      const d = await this.fetchDict('en');
      if (d) this.fallbackDict = d;
    } catch (e) {}
  },

  /** 高效获取语言词库 JSON */
  async fetchDict(langCode) {
    try {
      const resp = await fetch(`/assets/i18n/${langCode}.json`);
      if (resp.ok) return await resp.json();
    } catch (e) {}
    try {
      const resp = await fetch(`assets/i18n/${langCode}.json`);
      if (resp.ok) return await resp.json();
    } catch (e) {}
    return null;
  },

  /** 侦测宿主操作系统语言 */
  async detectSystemLanguage() {
    try {
      if (typeof hasPy !== 'undefined' && hasPy && py.get_system_language) {
        const pyLang = await py.get_system_language();
        if (pyLang && this.meta[pyLang]) return pyLang;
      }
    } catch (e) {}

    const browserLangs = navigator.languages || [navigator.language || 'zh-CN'];
    for (const l of browserLangs) {
      if (!l) continue;
      if (this.meta[l]) return l;
      const lower = l.toLowerCase();
      if (lower.startsWith('zh-tw') || lower.startsWith('zh-hk') || lower.includes('hant')) {
        return lower.includes('hk') ? 'zh-HK' : 'zh-TW';
      }
      if (lower.startsWith('zh')) return 'zh-CN';
      const prefix = l.split('-')[0];
      if (this.meta[prefix]) return prefix;
    }
    return 'zh-CN';
  },

  /** 切换语言 */
  async setLanguage(langCode, save = true) {
    if (!langCode || (!this.meta[langCode] && langCode !== 'zh-CN' && langCode !== 'en')) {
      langCode = 'zh-CN';
    }

    const dict = await this.fetchDict(langCode);
    if (dict) {
      this.dict = dict;
      this.currentLang = langCode;
    } else if (langCode === 'zh-CN') {
      this.currentLang = 'zh-CN';
    }

    if (save) {
      try {
        localStorage.setItem('readmd_language', this.currentLang);
      } catch (e) {}
    }

    // 设置书写方向与语言标识
    const langMeta = this.meta[this.currentLang] || {};
    const dir = langMeta.dir || 'ltr';
    document.documentElement.setAttribute('dir', dir);
    document.documentElement.setAttribute('lang', this.currentLang);

    // 动态更新 DOM 文本
    this.translateDOM();

    // 更新菜单中的当前语言标签
    const currentLabel = document.getElementById('lang-current-label');
    if (currentLabel) {
      currentLabel.textContent = langMeta.native || langMeta.name || this.currentLang;
    }

    // 派发切换事件
    window.dispatchEvent(new CustomEvent('readmd:language-changed', { detail: { lang: this.currentLang } }));
  },


  /** 获取翻译词条，支持 {count}, {name} 占位符替换 */
  t(key, params = {}) {
    let str = this.dict ? this.dict[key] : undefined;
    if (str === undefined || str === null || str === '') {
      str = this.fallbackDict ? this.fallbackDict[key] : undefined;
    }
    if ((str === undefined || str === null || str === '') && this.zhDefaults && this.zhDefaults[key]) {
      str = this.zhDefaults[key];
    }
    if (str === undefined || str === null || str === '') {
      str = key;
    }
    if (typeof str === 'string' && params) {
      for (const [k, v] of Object.entries(params)) {
        str = str.replace(new RegExp(`\\{${k}\\}`, 'g'), v);
      }
    }
    return str;
  },

  /** 全局扫描并翻译带有 data-i18n 属性的 DOM */
  translateDOM(root = document) {
    root.querySelectorAll('[data-i18n]').forEach(el => {
      const key = el.getAttribute('data-i18n');
      if (key) el.textContent = this.t(key);
    });

    root.querySelectorAll('[data-i18n-html]').forEach(el => {
      const key = el.getAttribute('data-i18n-html');
      if (key) el.innerHTML = this.t(key);
    });

    root.querySelectorAll('[data-i18n-title]').forEach(el => {
      const key = el.getAttribute('data-i18n-title');
      if (key) el.setAttribute('title', this.t(key));
    });

    root.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
      const key = el.getAttribute('data-i18n-placeholder');
      if (key) el.setAttribute('placeholder', this.t(key));
    });

    root.querySelectorAll('[data-i18n-aria]').forEach(el => {
      const key = el.getAttribute('data-i18n-aria');
      if (key) el.setAttribute('aria-label', this.t(key));
    });
  },


  /** 打开语言选择对话框 */
  openModal() {
    const modal = document.getElementById('lang-modal');
    if (!modal) return;
    modal.classList.remove('hidden');

    const searchInput = document.getElementById('lang-search-input');
    if (searchInput) {
      searchInput.value = '';
      setTimeout(() => {
        try { searchInput.focus(); } catch (e) {}
      }, 50);
    }
    this.renderLanguageGrid('');
  },

  /** 关闭语言选择对话框 */
  closeModal() {
    const modal = document.getElementById('lang-modal');
    if (modal) modal.classList.add('hidden');
  },

  /** 渲染语言网格列表 */
  renderLanguageGrid(query = '') {
    const grid = document.getElementById('lang-grid');
    if (!grid) return;

    grid.innerHTML = '';
    const q = query.trim().toLowerCase();

    for (const [code, info] of Object.entries(this.meta)) {
      const name = info.name || code;
      const native = info.native || name;
      if (q && !code.toLowerCase().includes(q) && !name.toLowerCase().includes(q) && !native.toLowerCase().includes(q)) {
        continue;
      }

      const item = document.createElement('div');
      const isActive = code === this.currentLang;
      item.className = 'lang-item' + (isActive ? ' active' : '');
      item.innerHTML = `
        <div class="lang-item-content">
          <div class="lang-item-native">${native}</div>
          <div class="lang-item-sub">${name} (${code})</div>
        </div>
        ${isActive ? '<div class="lang-item-check"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg></div>' : ''}
      `;
      item.addEventListener('click', async () => {
        await this.setLanguage(code, true);
        this.closeModal();
        if (typeof showToast === 'function') {
          showToast(this.t('lang.autoDetected', { name: native }) || `Language switched to ${native}`);
        }
      });
      grid.appendChild(item);
    }
  }
};

