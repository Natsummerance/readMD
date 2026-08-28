# Claude Code 学术论文技能

一个系统化的框架，用于使用 Claude Code 规划和撰写哲学及跨学科学术论文。这些技能通过结构化工作流程和质量检查点，将你的研究想法转化为可投稿的稿件。

## 功能特点

- **端到端流程**：从最初的想法到完善的稿件
- **循证差距识别**：每个研究空白都有 3-5 篇引文支持
- **平台特定风格学习**：分析 8-10 篇样本论文以提取写作标准
- **审稿人模拟**：7 维度、35 分评估系统
- **质量保证**：3 个验证关卡 + 2 个 Python 验证脚本
- **预印本平台支持**：PhilArchive、arXiv、PhilSci-Archive、PsyArXiv 等

## 双技能工作流程

```
┌─────────────────────────────────────────────────────────────┐
│              ACADEMIC-PAPER-STRATEGIST（策略师）              │
│  阶段 1：平台分析 → 目标平台 + 风格指南                        │
│  阶段 2：理论框架 → 文献综述 + 差距分析                        │
│  阶段 3：大纲优化 → 审稿人评估的大纲                           │
└─────────────────────────┬───────────────────────────────────┘
                          ↓
                     [详细大纲]
                          ↓
┌─────────────────────────┴───────────────────────────────────┐
│               ACADEMIC-PAPER-COMPOSER（写作者）               │
│  阶段 1：基础设置 → 风格指南 + 章节规划                        │
│  阶段 2：系统写作 → 带质量检查的草稿                           │
│  阶段 3：润色完善 → 最终评估 + 投稿准备                        │
└─────────────────────────────────────────────────────────────┘
```

## 安装

### 前提条件
- 已安装并配置 [Claude Code](https://claude.ai/code)
- Python 3.8+（用于验证脚本）

### 设置步骤

1. 克隆此仓库：
```bash
git clone https://github.com/lishix520/academic-paper-skills.git
```

2. 将技能复制到 Claude Code 技能目录：
```bash
cp -r strategist ~/.claude/skills/academic-paper-strategist
cp -r composer ~/.claude/skills/academic-paper-composer
```

3. 重启 Claude Code 以加载技能。

## 快速开始

### 规划论文（策略师）

```
你：规划一篇关于死亡如何产生意识的论文

Claude：[激活 academic-paper-strategist]
        让我引导你完成三个阶段...
```

### 从大纲写作（写作者）

```
你：根据这个大纲写论文：[你的大纲]

Claude：[激活 academic-paper-composer]
        开始阶段 1：基础设置...
```

## 目录结构

```
academic-paper-skills/
├── strategist/
│   ├── SKILL.md                    # 主技能定义
│   ├── references/
│   │   ├── quality_standards.md    # 评估标准
│   │   └── search_strategy.md      # 文献搜索指南
│   └── scripts/
│       ├── evaluate_samples.py     # 样本论文分析器
│       └── gap_analysis.py         # 研究空白验证器
├── composer/
│   ├── SKILL.md                    # 主技能定义
│   ├── references/
│   │   ├── section_guides.md       # 章节指南
│   │   └── writing_standards.md    # 学术写作原则
│   └── scripts/
│       ├── chapter_quality_check.py    # 章节质量检查
│       └── final_evaluation.py         # 完整稿件评估
└── examples/
    ├── consciousness-paper.md      # 示例：心灵哲学
    └── ai-ethics-paper.md          # 示例：AI 伦理
```

## 质量标准

### 审稿人模拟（7 个维度）

| 维度 | 权重 | 标准 |
|------|------|------|
| 原创性 | 5 分 | 对领域的新贡献 |
| 论证 | 5 分 | 逻辑连贯性、证据支持 |
| 文献 | 5 分 | 全面、最新的覆盖 |
| 方法论 | 5 分 | 适当、严谨的方法 |
| 清晰度 | 5 分 | 易读、结构良好的写作 |
| 影响力 | 5 分 | 对领域的潜在影响 |
| 技术性 | 5 分 | 准确性、正确的引用 |

**阈值**：评分 ≥28/35 的大纲进入写作阶段。

## 使用场景

- **博士生**：将学位论文章节转化为可发表的论文
- **独立研究者**：无需机构支持的专业级论文规划
- **跨学科工作**：应对多个平台的要求
- **预印本投稿**：为 PhilArchive、arXiv 等准备稿件

## 贡献

欢迎贡献！请参阅 [CONTRIBUTING.md](CONTRIBUTING.md) 了解指南。

## 许可证

MIT 许可证 - 详见 [LICENSE](LICENSE)。

## 作者

**李世雄**
独立研究者
ORCID: [0009-0008-2001-2865](https://orcid.org/0009-0008-2001-2865)

---

*这些技能是作者在发表意识和死亡相关哲学论文过程中开发的。*
