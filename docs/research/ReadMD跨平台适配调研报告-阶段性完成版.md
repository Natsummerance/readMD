# ReadMD 跨平台适配调研报告（已完成部分）

> 调研日期：2026-09-04
> 调研人：产品经理
> 状态：阶段性完成（部分调研因技术限制未完成）

---

## 一、调研概述

### 1.1 调研目标

ReadMD 当前基于 **Python + pywebview + Windows Runtime OCR** 技术栈，仅支持 Windows 平台。本次调研目标是制定 ReadMD 在以下平台的全面适配方案：

- macOS
- Linux（主流发行版）
- 国产操作系统（统信UOS、银河麒麟、玲珑、深度Deepin、中标麒麟等）

### 1.2 调研完成情况

| 调研方向 | 状态 | 报告文件 | 大小 |
|---------|:----:|---------|:----:|
| 跨平台 OCR 方案 | ✅ 已完成 | `ReadMD-跨平台OCR方案调研报告.md` | 31K |
| Linux 平台适配 | ✅ 已完成 | 已整合到主报告 | 79K |
| 统信 UOS 适配 | ✅ 已完成 | `ReadMD-统信UOS适配方案调研报告.md` | 53K |
| 银河麒麟适配 | ✅ 已完成 | 已整合到主报告 | 79K |
| 玲珑平台适配 | ❌ 未完成 | — | — |
| 深度 Deepin 适配 | ❌ 未完成 | — | — |
| 中标麒麟适配 | ❌ 未完成 | — | — |
| 中科方德适配 | ❌ 未完成 | — | — |
| 普华操作系统适配 | ❌ 未完成 | — | — |
| 整体适配策略 | ❌ 未完成 | — | — |

### 1.3 未完成原因

部分调研因系统网络工具持续超时（`glm-4.7[1M] is temporarily unavailable`）无法完成。WebSearch 和 WebFetch 工具均不可用，无法获取最新的真实数据。

---

## 二、已完成调研内容

### 2.1 跨平台 OCR 方案（已完成）

> 详见：`ReadMD-跨平台OCR方案调研报告.md`

#### 核心结论

**推荐方案：RapidOCR（首选）+ PaddleOCR（备选）**

| OCR 引擎 | 中文准确率 | 包大小 | CPU速度 | 跨平台 | 推荐度 |
|----------|:---------:|:------:|:-------:|:------:|:------:|
| **RapidOCR** | 95-98% | ~50MB | 100-300ms | ✅ Win/Mac/Linux/ARM | ⭐⭐⭐⭐⭐ |
| PaddleOCR | 95-98% | ~1.5GB | 100-300ms(CPU) | ✅ Win/Mac/Linux | ⭐⭐⭐⭐ |
| Tesseract | 75-80% | ~50MB | 2-4s | ✅ 全平台 | ⭐⭐⭐ |
| EasyOCR | 88-92% | ~1GB | 1.5-3s | ✅ 全平台 | ⭐⭐⭐ |

#### 推荐理由

1. **RapidOCR** 基于 ONNX Runtime，复用 PaddleOCR PP-OCRv3 模型
2. 包体积仅 ~50MB（PaddleOCR 的 1/30）
3. 全架构支持：x86_64 / ARM64 / MIPS64
4. `pip install rapidocr-onnxruntime` 一行安装
5. 无需 PaddlePaddle 深度学习框架

#### 各平台 OCR 方案

| 平台 | 推荐引擎 | 备选引擎 | 说明 |
|------|---------|---------|------|
| **Windows** | Windows Runtime OCR | RapidOCR | 现有方案保持不变 |
| **macOS** | Vision.framework | RapidOCR | 系统内置，免费 |
| **Linux x86_64** | RapidOCR | PaddleOCR | 体积小，性能好 |
| **Linux ARM64** | RapidOCR | Tesseract | RapidOCR 有 aarch64 wheel |
| **Linux MIPS64** | Tesseract | — | 兜底方案 |
| **玲珑容器** | RapidOCR | Tesseract | 容器内依赖隔离 |

---

### 2.2 Linux 平台适配（已完成）

#### 2.2.1 UI 框架方案

**推荐：继续使用 pywebview（GTK + WebKit2 后端）**

| 方案 | 推荐度 | 说明 |
|------|:------:|------|
| **pywebview (GTK)** | ⭐⭐⭐⭐⭐ | 代码改动最小，保持跨平台一致性 |
| PyQt6 / PySide6 | ⭐⭐⭐ | 功能强大但需重写 UI 层 |
| Electron / Tauri | ⭐⭐ | 完全重构，成本最高 |

**系统依赖安装**：
```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.0-37 libgtk-3-0 gir1.2-gtk-3.0
pip install pywebview[gtk]
```

#### 2.2.2 需要修改的 Windows 专属代码

| Windows 代码 | Linux 替代方案 |
|-------------|---------------|
| `winreg`（注册表） | `~/.config/readmd/config.ini`（XDG 规范） |
| `winrt.windows.media.ocr` | `rapidocr_onnxruntime` 或 `pytesseract` |
| `APPDATA` 路径 | `~/.local/share/readmd`（XDG Data Home） |
| `os.startfile()` | `subprocess.Popen(['xdg-open', path])` |
| `ctypes.windll.user32.MessageBoxW` | `zenity` 或 `tkinter.messagebox` |
| Windows 文件关联 API | `xdg-mime` + `.desktop` 文件 |

#### 2.2.3 打包分发方案

| 格式 | 推荐度 | 适用场景 |
|------|:------:|---------|
| **AppImage** | ⭐⭐⭐⭐⭐ | 通用分发，无需安装，跨发行版 |
| **DEB 包** | ⭐⭐⭐⭐ | Ubuntu/Debian 原生安装 |
| **Flatpak** | ⭐⭐⭐⭐ | Flathub 分发，沙箱隔离 |
| **RPM 包** | ⭐⭐⭐ | Fedora/RHEL 用户 |

#### 2.2.4 工作量和风险

**开发周期**：4-6 周

| 阶段 | 内容 | 时间 |
|------|------|------|
| 阶段一 | UI 适配 + OCR 替换 + 配置重构 + 文件关联 | 2-3 周 |
| 阶段二 | 系统集成测试 + 性能优化 + 打包分发 | 1-2 周 |
| 阶段三 | 多发行版测试 + Bug 修复 | 1 周 |

**主要风险**：

| 风险 | 等级 | 应对措施 |
|------|:----:|---------|
| pywebview WebKitGTK 兼容性 | 🟡 中 | 锁定 4.0 版本，提供 X11 回退 |
| Tesseract 中文识别质量 | 🟡 中 | 图像预处理 + RapidOCR 可选后端 |
| Linux 桌面环境碎片化 | 🔴 高 | AppImage 隔离依赖，覆盖主流发行版测试 |
| Wayland 兼容性 | 🟡 中 | 测试 GNOME/KDE 原生 Wayland + XWayland |

---

### 2.3 统信 UOS 适配（已完成）

> 详见：`ReadMD-统信UOS适配方案调研报告.md`

#### 2.3.1 UOS 技术栈

```
统信 UOS 技术栈全景
┌─────────────────────────────────────────────┐
│              用户应用层                        │
│   ReadMD / WPS / 微信 / 自研应用              │
├─────────────────────────────────────────────┤
│              桌面环境 DDE                      │
│   Deepin Desktop Environment (Qt)            │
├─────────────────────────────────────────────┤
│              基础框架层                        │
│   Qt 5.15+ / GTK 3.24+ / DTK               │
├─────────────────────────────────────────────┤
│              系统服务层                        │
│   lastore / deepin-deb-installer             │
├─────────────────────────────────────────────┤
│              内核层                            │
│   Linux Kernel 5.10+ (LTS)                  │
│   支持: x86_64 / ARM64 / MIPS / LoongArch   │
├─────────────────────────────────────────────┤
│              基础系统                           │
│   Debian 10/11 (Buster/Bullseye)            │
│   APT 包管理器 / dpkg                         │
└─────────────────────────────────────────────┘
```

**关键结论**：
- UOS 本质是 **Debian 的衍生发行版**，APT/dpkg 完全兼容
- 桌面环境 DDE 是 Qt 构建，但完全支持 GTK 应用
- 主要目标架构：**x86_64** 和 **ARM64**（鲲鹏/飞腾等国产 CPU）

#### 2.3.2 UOS 版本矩阵

| 版本 | 基于 | 内核 | 支持架构 | 生命周期 |
|------|------|------|---------|---------|
| UOS 家庭版 | Debian 10 | 5.10 | x86_64, ARM64 | 持续更新 |
| UOS 专业版 | Debian 10 | 5.10 | x86_64, ARM64, MIPS | 至 2027 |
| UOS 1070 | Debian 11 | 5.15+ | x86_64, ARM64, LoongArch | 至 2029 |

#### 2.3.3 适配方案

**推荐技术栈**：
- **UI 框架**：pywebview（GTK + WebKit2 后端）
- **OCR 引擎**：RapidOCR（x86_64）/ Tesseract（ARM64/LoongArch 后备）
- **打包方式**：dpkg-deb + dpkg-sig 签名
- **分发渠道**：统信应用商店 + DEB 离线包

**DEB 打包关键配置**：
```
Package: readmd
Version: 1.0.0
Architecture: amd64
Depends: python3 (>= 3.8), python3-pywebview, tesseract-ocr, tesseract-ocr-chi-sim
Maintainer: ReadMD Team <support@readmd.com>
Description: Markdown reader with OCR support
```

#### 2.3.4 应用商店上架流程

```
1. 注册开发者账号 → https://developer.uniontech.com
2. 准备上架材料（DEB包、截图、描述、图标、隐私政策）
3. 提交审核（自动检测 1-2天 + 人工审核 3-5天）
4. 审核通过后上架发布
```

**审核要点**：
- 应用签名验证（必须）
- 不含恶意代码
- 不违反隐私规范
- 兼容 DDE 桌面环境
- 中文界面支持

#### 2.3.5 工作量评估

**总计：约 20 个工作日（4 周）**

| 阶段 | 内容 | 时间 |
|------|------|------|
| 环境搭建 | UOS 虚拟机 + 开发环境 | 3 天 |
| 代码适配 | UI/OCR/配置/文件关联 | 5 天 |
| 打包测试 | DEB 打包 + 多架构测试 | 4 天 |
| 商店上架 | 材料准备 + 审核跟进 | 5 天 |
| 回归测试 | 功能验证 + Bug 修复 | 3 天 |

---

### 2.4 银河麒麟适配（已完成）

#### 2.4.1 麒麟 V10 版本矩阵

| 版本 | 定位 | 基于 | 支持架构 |
|------|------|------|---------|
| 银河麒麟桌面版 V10 | 桌面办公 | Ubuntu/Debian | x86_64, ARM64, MIPS64 |
| 银河麒麟服务器版 V10 | 服务器 | CentOS/RHEL | x86_64, ARM64 |
| 开放麒麟 openKylin | 社区版 | 独立根社区 | x86_64, ARM64 |

#### 2.4.2 各架构适配情况

| 架构 | WebKit2GTK | pywebview | OCR 引擎 | 风险等级 |
|------|:----------:|:---------:|---------|:-------:|
| **x86_64** | ✅ 官方源直接安装 | ✅ 完全兼容 | RapidOCR | 🟢 低 |
| **ARM64** | ✅ 官方源可用 | ✅ 基本兼容 | RapidOCR | 🟡 中 |
| **MIPS64 (龙芯)** | ⚠️ 社区维护 | ⚠️ 需源码编译 | Tesseract | 🔴 高 |

#### 2.4.3 适配方案

**与统信 UOS 高度一致**：
- 桌面版基于 Ubuntu/Debian → DEB 包
- 服务器版基于 CentOS/RHEL → RPM 包
- 使用 RapidOCR + Tesseract 兜底

**麒麟特殊处理**：
```bash
# 龙芯 MIPS 软件渲染
export LIBGL_ALWAYS_SOFTWARE=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
```

#### 2.4.4 应用商店上架

- 注册开发者账号：https://developer.kylinos.cn
- 提交 DEB 包 + 材料
- 安全审查（5-10 工作日）
- 兼容性验证（麒麟 V10 真机测试）

#### 2.4.5 工作量评估

**总计：约 35 人天（7 周，含 MIPS64 适配）**

| 任务 | 优先级 | 工作量 |
|------|:------:|:------:|
| 麒麟环境搭建 | P0 | 2 天 |
| pywebview 适配测试 | P0 | 3 天 |
| OCR 引擎适配 | P0 | 3 天 |
| 平台适配层开发 | P0 | 3 天 |
| 文件关联集成 | P1 | 2 天 |
| DEB 打包 | P0 | 3 天 |
| RPM 打包 | P2 | 2 天 |
| ARM64 专项适配 | P1 | 3 天 |
| MIPS64/龙芯适配 | P2 | 5 天 |
| 兼容性测试 | P0 | 3 天 |
| 应用商店材料 | P1 | 2 天 |
| 审核跟进 | P1 | 2 天 |

---

## 三、平台抽象层设计（已完成）

### 3.1 架构设计

```
readmd/
├── core/              # 核心业务逻辑（跨平台）
│   ├── markdown_renderer.py
│   └── ocr_processor.py
├── platform/          # 平台适配层
│   ├── __init__.py
│   ├── base.py        # 抽象基类
│   ├── windows.py     # Windows 实现
│   ├── macos.py       # macOS 实现
│   ├── linux.py       # Linux 实现
│   ├── uos.py         # 统信 UOS 扩展
│   └── kylin.py       # 银河麒麟扩展
└── ui/                # UI 层（跨平台）
    └── webview_app.py
```

### 3.2 平台适配器代码

```python
# platform/base.py
from abc import ABC, abstractmethod
from pathlib import Path

class PlatformAdapter(ABC):
    """平台适配抽象基类"""
    
    @abstractmethod
    def ocr_image(self, image_path: str) -> str:
        """OCR 识别"""
        pass
    
    @abstractmethod
    def get_config_dir(self) -> Path:
        """获取配置目录"""
        pass
    
    @abstractmethod
    def set_default_app(self):
        """设置为默认应用"""
        pass
    
    @abstractmethod
    def show_message(self, title: str, message: str):
        """显示消息框"""
        pass
```

### 3.3 OCR 引擎工厂

```python
# ocr/engine_factory.py
import platform

def create_ocr_engine():
    """根据平台自动选择最佳 OCR 引擎"""
    system = platform.system()
    arch = platform.machine()
    
    if system == 'Windows':
        from .winrt_engine import WinRTOCREngine
        return WinRTOCREngine()
    
    elif system == 'Darwin':
        try:
            from .vision_engine import VisionOCREngine
            return VisionOCREngine()
        except ImportError:
            from .rapidocr_engine import RapidOCREngine
            return RapidOCREngine()
    
    elif system == 'Linux':
        if arch in ('x86_64', 'aarch64'):
            try:
                from .rapidocr_engine import RapidOCREngine
                return RapidOCREngine()
            except ImportError:
                pass
        from .tesseract_engine import TesseractEngine
        return TesseractEngine()
    
    else:
        raise RuntimeError(f"不支持的平台: {system}")
```

---

## 四、整体适配策略（待完成）

### 4.1 适配优先级排序（待调研确认）

| 优先级 | 平台 | 理由（待验证） |
|:------:|------|------|
| P0 | 统信 UOS | 信创市场份额最大，政企客户核心需求 |
| P0 | 银河麒麟 | 政府/军队/央企主要操作系统 |
| P1 | 深度 Deepin | 社区用户基数大，国际影响力 |
| P1 | 玲珑 | 跨发行版分发，一次打包多平台运行 |
| P2 | 中标麒麟 | 市场份额较小，与银河麒麟合并中 |
| P2 | 中科方德 | 特定行业用户 |
| P3 | 普华操作系统 | 市场份额最小 |

### 4.2 统一适配方案（待调研确认）

**核心策略**：基于 Linux 适配，复用 90% 代码

```
ReadMD 跨平台代码复用策略:

Windows 原始代码 (100%)
    │
    ├── Linux 适配层 (复用 90%)
    │   ├── 统信 UOS 扩展 (复用 95%)
    │   ├── 银河麒麟扩展 (复用 95%)
    │   ├── 深度 Deepin 扩展 (复用 95%)
    │   └── 玲珑容器适配 (复用 85%)
    │
    └── macOS 适配层 (复用 80%)
```

### 4.3 多架构支持方案（待调研确认）

| 架构 | 代表芯片 | 支持策略 |
|------|---------|---------|
| x86_64 | Intel/AMD | ✅ 主力支持 |
| ARM64 | 鲲鹏/飞腾 | ✅ 重点支持 |
| MIPS64 | 龙芯 | ⚠️ 有限支持 |
| LoongArch | 龙芯新版 | ⚠️ 待评估 |
| SW64 | 申威 | ❌ 暂不支持 |

---

## 五、后续调研方向

### 5.1 待完成调研清单

| 序号 | 调研方向 | 调研内容 | 优先级 |
|:----:|---------|---------|:------:|
| 1 | **玲珑平台适配** | 玲珑技术栈分析、pywebview 容器化适配、OCR 引擎容器内运行、玲珑包构建流程、应用商店上架 | P1 |
| 2 | **深度 Deepin 适配** | Deepin 技术栈（基于 Debian）、DDE 桌面兼容性、应用商店上架流程 | P1 |
| 3 | **中标麒麟适配** | 技术栈分析（基于 CentOS）、与银河麒麟的差异、RPM 打包方案 | P2 |
| 4 | **中科方德适配** | 技术栈分析、应用打包格式、适配难度评估 | P2 |
| 5 | **普华操作系统适配** | 技术栈分析、市场份额评估、是否值得适配 | P3 |
| 6 | **整体适配策略** | 市场分析、优先级确认、时间规划、测试策略、发布策略 | P0 |

### 5.2 各调研方向详细内容

#### 5.2.1 玲珑平台适配（待调研）

**需要调研的关键问题**：
1. 玲珑的技术架构（基于 OCI 容器？类似 Flatpak？）
2. pywebview 在玲珑容器中如何运行（显示穿透？X11/Wayland 适配？）
3. OCR 引擎在容器中的可用性（RapidOCR/Tesseract）
4. 玲珑包格式和构建工具（ll-builder？）
5. 玲珑应用商店上架流程
6. 玲珑与 DEB 包的关系（互补还是替代？）

#### 5.2.2 深度 Deepin 适配（待调研）

**需要调研的关键问题**：
1. Deepin 当前版本（V23？）基于哪个 Debian 版本
2. DDE 桌面环境对 GTK 应用的兼容性
3. Deepin 应用商店上架要求和流程
4. 与统信 UOS 的差异点

#### 5.2.3 中标麒麟适配（待调研）

**需要调研的关键问题**：
1. 中标麒麟与银河麒麟合并后的产品线
2. 基于 CentOS 还是其他发行版
3. RPM 打包方案
4. 应用商店情况

#### 5.2.4 整体适配策略（待调研）

**需要调研的关键问题**：
1. 各国产操作系统市场份额数据
2. 信创市场用户群体分析
3. 适配投入产出比分析
4. 多平台 CI/CD 方案
5. 多架构测试矩阵

### 5.3 调研方法建议

由于本次调研中网络工具不可用，建议后续调研采用以下方法：

1. **官方文档**：访问各操作系统官网获取技术文档
2. **开发者社区**：查阅开发者论坛和社区讨论
3. **实际测试**：在各操作系统虚拟机中进行实际适配测试
4. **开发者账号**：注册各应用商店开发者账号了解上架要求

---

## 六、总结

### 6.1 已完成成果

✅ 确定了跨平台 OCR 方案（RapidOCR 首选）
✅ 完成了 Linux 平台适配方案
✅ 完成了统信 UOS 适配方案（含 DEB 打包、应用商店上架）
✅ 完成了银河麒麟适配方案（含多架构支持）
✅ 设计了平台抽象层架构
✅ 估算了各平台工作量

### 6.2 待完成工作

❌ 玲珑平台适配方案
❌ 深度 Deepin 适配方案
❌ 中标麒麟适配方案
❌ 中科方德适配方案
❌ 普华操作系统适配方案
❌ 整体适配策略（市场分析、优先级确认、时间规划）

### 6.3 下一步行动

1. **等待系统工具恢复**后，继续完成剩余调研
2. **优先完成**：玲珑平台适配 + 整体适配策略
3. **其次完成**：深度 Deepin + 中标麒麟
4. **最后评估**：中科方德 + 普华（根据市场份额决定是否适配）

---

**报告完成时间**：2026-09-04
**报告状态**：阶段性完成
**下次更新**：待系统工具恢复后继续调研
