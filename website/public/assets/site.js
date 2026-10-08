'use strict';
(() => {
  const setupDownloadHub = () => {
    const mirrorEngines = {
      direct: (url) => url,
      ghfast: (url) => `https://ghfast.top/${url}`,
      ghproxy: (url) => `https://ghproxy.net/${url}`,
      gitmirror: (url) => `https://hub.gitmirror.com/${url}`,
      fastgit: (url) => `https://gh-proxy.com/${url}`,
    };

    let activeMirror = localStorage.getItem('readmd_download_mirror') || 'direct';

    const updateDownloadLinks = (mirrorKey, animate = true) => {
      activeMirror = mirrorKey;
      localStorage.setItem('readmd_download_mirror', mirrorKey);

      const targetElements = document.querySelectorAll('.platform-card a, .platform-card button');

      const applyLinks = () => {
        document.querySelectorAll('[data-mirror-url]').forEach((link) => {
          const original = link.dataset.mirrorUrl;
          const transform = mirrorEngines[mirrorKey] || mirrorEngines.direct;
          link.href = transform(original);
        });

        document.querySelectorAll('.mirror-tab').forEach((tab) => {
          tab.classList.toggle('is-active', tab.dataset.mirror === mirrorKey);
        });
      };

      if (animate && targetElements.length > 0) {
        targetElements.forEach((el) => {
          el.style.transition = 'opacity 0.18s cubic-bezier(0.25, 0.1, 0.25, 1), transform 0.18s cubic-bezier(0.25, 0.1, 0.25, 1)';
          el.style.opacity = '0.35';
          el.style.transform = 'scale(0.985)';
        });
        setTimeout(() => {
          applyLinks();
          targetElements.forEach((el) => {
            el.style.opacity = '1';
            el.style.transform = 'scale(1)';
          });
        }, 120);
      } else {
        applyLinks();
      }
    };

    document.querySelectorAll('.mirror-tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        updateDownloadLinks(tab.dataset.mirror, true);
      });
    });

    // Detect user platform
    const ua = navigator.userAgent.toLowerCase();
    let detectedOS = 'windows';
    if (ua.includes('mac') || ua.includes('darwin')) detectedOS = 'macos';
    else if (ua.includes('linux') || ua.includes('x11') || ua.includes('kylin') || ua.includes('uos')) detectedOS = 'linux';

    document.querySelectorAll(`[data-platform="${detectedOS}"]`).forEach((card) => {
      card.classList.add('is-recommended');
    });

    // Initialize links with data-mirror-url if present
    document.querySelectorAll('a[href^="https://github.com/Natsummerance/rust-ReadMD/releases/download/"]').forEach((link) => {
      if (!link.dataset.mirrorUrl) link.dataset.mirrorUrl = link.href;
    });

    updateDownloadLinks(activeMirror);

    // Dual-Layer Dynamic Version & Release Synchronizer
    const GITHUB_REPO = 'Natsummerance/rust-ReadMD';

    const applyVersionData = (versionTag, pureVersion) => {
      if (!versionTag) return;
      const cleanTag = /^[vV]/.test(versionTag) ? versionTag : `v${versionTag}`;
      const cleanPure = pureVersion || cleanTag.replace(/^[vV]/, '');

      document.querySelectorAll('.latest-version-badge, [data-version-slot]').forEach((el) => {
        el.textContent = cleanTag;
      });
      document.querySelectorAll('[data-pure-version]').forEach((el) => {
        el.textContent = cleanPure;
      });

      // Update SHA256SUMS.txt links
      document.querySelectorAll('a[href*="SHA256SUMS.txt"]').forEach((a) => {
        a.href = `https://github.com/${GITHUB_REPO}/releases/download/${cleanTag}/SHA256SUMS.txt`;
      });

      // Update all download URLs and mirror slots
      document.querySelectorAll('a[data-mirror-url]').forEach((link) => {
        let orig = link.dataset.mirrorUrl;
        if (orig.includes('/releases/download/')) {
          const oldTagMatch = orig.match(/\/releases\/download\/([^/]+)\//);
          if (oldTagMatch && oldTagMatch[1] && oldTagMatch[1] !== cleanTag) {
            const oldTag = oldTagMatch[1];
            const oldPure = oldTag.replace(/^[vV]/, '');
            let updated = orig.replace(new RegExp(`/releases/download/${oldTag}/`, 'g'), `/releases/download/${cleanTag}/`);
            updated = updated.replace(new RegExp(`-${oldPure}\\.`, 'g'), `-${cleanPure}.`);
            link.dataset.mirrorUrl = updated;
          }
        }
      });

      updateDownloadLinks(activeMirror);
    };

    // Published manifest is same-origin and independent of API rate limits.
    fetch('/version.json', { signal: AbortSignal.timeout(5000) })
      .then(response => response.ok ? response.json() : null)
      .then(data => {
        if (data && /^V\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(data.releaseTag))
          applyVersionData(data.releaseTag, data.version);
      }).catch(() => {});
  };

  /* Interactive MCP Configuration & 1-Click Multi-Harness Generator */
  const setupMcpGuide = () => {
    const mcpConfigs = {
      // --- IDEs & Desktop ---
      claude: {
        name: 'Claude Desktop',
        format: 'JSON',
        path: 'macOS: ~/Library/Application Support/Claude/claude_desktop_config.json | Windows: %APPDATA%\\Claude\\claude_desktop_config.json',
        rawPath: '~/Library/Application Support/Claude/claude_desktop_config.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      cursor: {
        name: 'Cursor IDE',
        format: 'JSON',
        path: '项目级: .cursor/mcp.json | 全局: Cursor Settings > Features > MCP',
        rawPath: '.cursor/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      trae: {
        name: 'Trae (字节跳动)',
        format: 'JSON',
        path: '~/.trae/mcp.json 或 Trae Settings > MCP',
        rawPath: '~/.trae/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      vscode: {
        name: 'VS Code (Cline/Roo)',
        format: 'JSON',
        path: 'VS Code Cline/Roo MCP Settings 或 .vscode/settings.json',
        rawPath: '.vscode/settings.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      zcode: {
        name: 'ZCode IDE',
        format: 'JSON',
        path: '~/.zcode/mcp.json 或 ZCode Settings > MCP Servers',
        rawPath: '~/.zcode/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      qoder: {
        name: 'Qoder (阿里/通用)',
        format: 'JSON',
        path: '~/.qoder/settings.json 或 qoder --mcp-config',
        rawPath: '~/.qoder/settings.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"],\n      "transport": "stdio"\n    }\n  }\n}`,
      },

      // --- Agent Harnesses & CLI ---
      codex: {
        name: 'Codex / ChatGPT',
        format: 'TOML',
        path: '~/.codex/config.toml (OpenAI Codex / ChatGPT Desktop)',
        rawPath: '~/.codex/config.toml',
        code: `[mcp_servers.readmd]\ncommand = "readmd"\nargs = ["--mcp"]`,
      },
      antigravity: {
        name: 'Antigravity / Gemini',
        format: 'JSON',
        path: '~/.gemini/antigravity/mcp/readmd.json',
        rawPath: '~/.gemini/antigravity/mcp/readmd.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      opencode: {
        name: 'OpenCode CLI',
        format: 'JSONC',
        path: '~/.config/opencode/opencode.json 或项目根目录 opencode.json',
        rawPath: '~/.config/opencode/opencode.json',
        code: `{\n  "mcp": {\n    "readmd": {\n      "type": "local",\n      "enabled": true,\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      hermes: {
        name: 'Nous Hermes',
        format: 'YAML',
        path: '~/.hermes/config.yaml (Nous Hermes Agent Harness)',
        rawPath: '~/.hermes/config.yaml',
        code: `mcp_servers:\n  readmd:\n    command: "readmd"\n    args:\n      - "--mcp"`,
      },
      deepseek: {
        name: 'DeepSeek Harness',
        format: 'YAML',
        path: '~/.dsh/settings.yaml (DeepSeek Coding Agent Harness)',
        rawPath: '~/.dsh/settings.yaml',
        code: `mcpServers:\n  readmd:\n    command: "readmd"\n    args:\n      - "--mcp"`,
      },
      openclaw: {
        name: 'OpenClaw',
        format: 'JSON5',
        path: '~/.openclaw/openclaw.json (OpenClaw Agent Gateway)',
        rawPath: '~/.openclaw/openclaw.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      workbuddy: {
        name: 'WorkBuddy (腾讯)',
        format: 'JSON',
        path: '~/.workbuddy/mcp.json (腾讯云代码助手 / WorkBuddy)',
        rawPath: '~/.workbuddy/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      doubao: {
        name: '豆包 / Coze',
        format: 'JSON',
        path: '~/.coze/mcp.json 或 扣子/豆包 Studio 插件连接器',
        rawPath: '~/.coze/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "readmd",\n      "args": ["--mcp"]\n    }\n  }\n}`,
      },
      cli: {
        name: 'Terminal',
        format: 'BASH',
        path: '终端命令行直接启动 stdio 服务',
        rawPath: 'readmd --mcp',
        code: `# ReadMD 可执行程序内置 MCP stdio 服务（无需 Python）\nreadmd --mcp`,
      },
    };

    let currentTarget = 'claude';

    const updateMcpView = (target) => {
      currentTarget = target;
      const data = mcpConfigs[target] || mcpConfigs.claude;
      document.querySelectorAll('.mcp-tab').forEach((tab) => {
        tab.classList.toggle('is-active', tab.dataset.mcpTarget === target);
      });
      document.querySelectorAll('.mcp-config-path').forEach((el) => {
        el.textContent = data.path;
      });
      document.querySelectorAll('.mcp-format-badge').forEach((el) => {
        el.textContent = data.format;
      });
      document.querySelectorAll('.macos-agent-name').forEach((el) => {
        el.textContent = data.name;
      });
      document.querySelectorAll('.mcp-code-block code').forEach((el) => {
        el.style.opacity = '0.3';
        el.textContent = data.code;
        setTimeout(() => {
          el.style.opacity = '1';
        }, 60);
      });
    };

    document.querySelectorAll('.mcp-tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        updateMcpView(tab.dataset.mcpTarget);
      });
    });

    // Apple Accordion Interactions
    document.querySelectorAll('.apple-accordion-item').forEach((item) => {
      const header = item.querySelector('.apple-accordion-header');
      if (!header) return;
      header.addEventListener('click', () => {
        const isOpen = item.classList.contains('is-open');
        const parentList = item.closest('.apple-accordion-list');
        if (parentList) {
          parentList.querySelectorAll('.apple-accordion-item').forEach((sibling) => {
            sibling.classList.remove('is-open');
          });
        }
        if (!isOpen) {
          item.classList.add('is-open');
        }
      });
    });

    const copyTextWithFallback = async (text, btn, successLabel) => {
      const textEl = btn.querySelector('.copy-text') || btn;
      const origText = textEl.textContent;
      try {
        await navigator.clipboard.writeText(text);
        textEl.textContent = successLabel || '✓ Copied!';
        btn.style.borderColor = 'var(--color-action)';
        setTimeout(() => {
          textEl.textContent = origText;
          btn.style.borderColor = '';
        }, 2000);
      } catch (e) {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
        textEl.textContent = successLabel || '✓ Copied!';
        setTimeout(() => {
          textEl.textContent = origText;
          btn.style.borderColor = '';
        }, 2000);
      }
    };

    document.querySelectorAll('.btn-copy-mcp').forEach((btn) => {
      btn.addEventListener('click', () => {
        const data = mcpConfigs[currentTarget] || mcpConfigs.claude;
        copyTextWithFallback(data.code, btn, '✓ Copied!');
      });
    });

    document.querySelectorAll('.btn-copy-path').forEach((btn) => {
      btn.addEventListener('click', () => {
        const data = mcpConfigs[currentTarget] || mcpConfigs.claude;
        copyTextWithFallback(data.rawPath || data.path, btn, '✓ Path Copied!');
      });
    });
  };


 const initialize = () => { setupDownloadHub(); setupMcpGuide(); };
 if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded',initialize,{once:true}); else initialize();
})();
