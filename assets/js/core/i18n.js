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

  /** 初始化多语言模块 */
  async init() {
    try {
      const urls = ['/assets/i18n/meta.json', '/i18n/meta.json', 'i18n/meta.json'];
      for (const u of urls) {
        try {
          const resp = await fetch(u);
          if (resp.ok) {
            this.meta = await resp.json();
            break;
          }
        } catch (err) {}
      }
    } catch (e) {
      console.warn('[i18n] Load meta failed:', e);
    }

    // 首选语言决策：1. LocalStorage 缓存 2. 系统侦测
    let preferred = localStorage.getItem('readmd_language');
    if (!preferred) {
      preferred = await this.detectSystemLanguage();
    }

    await this.setLanguage(preferred || 'zh-CN', false);
  },

  /** 侦测宿主操作系统语言 */
  async detectSystemLanguage() {
    try {
      if (typeof hasPy !== 'undefined' && hasPy && py.get_system_language) {
        const pyLang = await py.get_system_language();
        if (pyLang && this.meta[pyLang]) return pyLang;
      }
    } catch (e) {
      // ignore
    }

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

    const urls = [
      `/assets/i18n/${langCode}.json`,
      `/i18n/${langCode}.json`,
      `i18n/${langCode}.json`
    ];

    let loaded = false;
    for (const u of urls) {
      try {
        const resp = await fetch(u);
        if (resp.ok) {
          this.dict = await resp.json();
          this.currentLang = langCode;
          loaded = true;
          break;
        }
      } catch (err) {}
    }

    if (!loaded) {
      console.warn(`[i18n] Failed to load ${langCode}, fallback to zh-CN`);
      this.currentLang = 'zh-CN';
    }

    if (save) {
      localStorage.setItem('readmd_language', this.currentLang);
    }

    // 设置书写方向与语言标识
    const langMeta = this.meta[this.currentLang] || {};
    const dir = langMeta.dir || 'ltr';
    document.documentElement.setAttribute('dir', dir);
    document.documentElement.setAttribute('lang', this.currentLang);

    // 动态更新 DOM 文本
    this.translateDOM();

    // 派发切换事件
    window.dispatchEvent(new CustomEvent('readmd:language-changed', { detail: { lang: this.currentLang } }));
  },

  /** 获取翻译词条，支持 {count}, {name} 占位符替换 */
  t(key, params = {}) {
    let str = this.dict[key] || this.fallbackDict[key] || key;
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

    root.querySelectorAll('[data-i18n-title]').forEach(el => {
      const key = el.getAttribute('data-i18n-title');
      if (key) el.setAttribute('title', this.t(key));
    });

    root.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
      const key = el.getAttribute('data-i18n-placeholder');
      if (key) el.setAttribute('placeholder', this.t(key));
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
      searchInput.focus();
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
      item.className = 'lang-item' + (code === this.currentLang ? ' active' : '');
      item.innerHTML = `
        <div class="lang-item-native">${native}</div>
        <div class="lang-item-sub">${name} (${code})</div>
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
