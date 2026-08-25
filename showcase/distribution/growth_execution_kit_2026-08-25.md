# ReadMD Growth Execution Kit

## Purpose

This kit converts completed website clusters into a repeatable manual distribution program. It is written for the maintainer and trusted collaborators. It does not automate third-party posting, buy attention, or promise a star count.

Baseline on 2026-08-25: GitHub reported **18 stars** and **2 forks**. Star movement is a lagging signal; each publication should first be judged by relevant replies and qualified visits.

## Operating Rules

1. Publish one channel per day, then spend more time replying than broadcasting.
2. Adapt each title and first paragraph to the community. Never paste one body everywhere.
3. Disclose developer affiliation where the community expects it.
4. Lead with a task the audience already has, not with a request for stars.
5. Stop if a community prohibits self-promotion or the thread becomes promotion-only spam.
6. Record every published link, reply, and outcome in `growth_publication_ledger.csv`.
7. Escalate only reproducible bugs to GitHub Issues. Keep feature debates in discussions.
8. Never invent user counts, download counts, revenue, security certification, AI-citation placement, or star growth.

## Core Positioning

ReadMD keeps Markdown as the canonical artifact across reading, editing, conversion, OCR, citation work, and presentation. The useful contrast is not "another editor"; it is fewer disconnected tools and less silent rewriting of the source file.

| Audience | Existing pain | Entry workflow | Proof link |
|---|---|---|---|
| Researchers | Scans, BibTeX keys, and references live apart | Scan to Markdown, then citations | [/scan-to-markdown/](https://readmd.asia/scan-to-markdown/) and [/bibtex-citations/](https://readmd.asia/bibtex-citations/) |
| Technical writers | Office material must be retyped | Convert inherited documents | [/convert-to-markdown/](https://readmd.asia/convert-to-markdown/) |
| Teachers and speakers | Drafts and decks diverge | Present one Markdown source | [/markdown-to-slides/](https://readmd.asia/markdown-to-slides/) |
| Note keepers | Large files freeze browser tools | Local semantic pagination | [/large-markdown-files/](https://readmd.asia/large-markdown-files/) |
| Privacy-sensitive users | Online converters require uploads | Local workflows and checksums | [Download](https://readmd.asia/download/) |

## Approved Facts

- Current public prerelease: `2.3.7-beta.3`; license: MIT.
- Semantic pagination becomes relevant around 8,000 lines or 500 KB; behavior varies by structure and device.
- Rendering repair is display-only unless the user explicitly saves an edit.
- Conversion accepts DOCX, PPTX, XLSX, PDF, HTML, and LaTeX; complex layouts still need review.
- OCR is local and qualified to Windows WinRT and macOS Apple Vision on supported devices.
- BibTeX support scans a neighboring `.bib` file, renders citation badges, exposes available metadata, and assembles cited references.
- Presentation mode is Reveal-based and uses the same Markdown source.
- Core document work does not require a cloud account; web extraction, AI requests, and LAN sharing are explicit actions.
- Desktop packages use GitHub Releases and `SHA256SUMS.txt`. HarmonyOS is an ArkTS source project; no prebuilt HAP is provided.

## Message Architecture

Primary hook: one Markdown file can remain the source of truth from research through delivery.

Supporting claims:

1. Long documents stay navigable instead of forcing every node into one render pass.
2. Inherited office documents can become editable Markdown locally.
3. Qualified scans can become text without uploading them to a web form.
4. Citation keys connect to neighboring BibTeX metadata and assembled references.
5. A rehearsal edit returns to the same file that will be presented.

Calls to action:

- Ask for a specific workflow: "Which document size or conversion breaks your current tool?"
- Offer the download page only after the post establishes relevance.
- Mention checksums for technical communities.
- Invite issues with platform, file size, and workflow details.
- Use "Star if useful" sparingly; never make it the headline.

## Channel Playbook

### GitHub

Objective: turn existing visitors into users and contributors.

Actions:

1. Keep the pinned discussion focused on feedback rather than a star request.
2. Add one Discussions entry when a workflow page ships; link the page and one concrete question.
3. Reply to actionable issues within 48 hours during launch weeks.
4. Label recurring requests so release notes can show responsiveness.

Draft:

```text
Five new workflow guides are live: long-file reading, direct Markdown presentation, document conversion, scan-to-Markdown OCR, and BibTeX citations. Each guide states the supported boundary and the human-review step that remains. If you have a real file that slows down your current workflow, tell us its platform, size, and format.
```

### X or Mastodon

Objective: reach developers and writers with one concrete task per post.

English post:

```text
A 9,000-line Markdown draft should not freeze because the UI rendered everything at once.

ReadMD segments around 8,000 lines or 500 KB while keeping outline, search, formulas, and navigation connected. Source bytes stay untouched until you save.

https://readmd.asia/large-markdown-files/
```

Simplified Chinese post:

```text
论文引用不必在写作工具和文献管理器之间反复搬运。

ReadMD 可扫描同目录 .bib 文件，把 [@key] 或 @key 渲染成引用徽章，并汇总文末参考文献。提交前仍要按目标期刊复核格式。

https://readmd.asia/zh-cn/bibtex-citations/
```

Thread outline:

1. Name the broken handoff: writing tool, converter, slide deck, citation manager.
2. Show one Markdown file moving through two workflows.
3. State what stays local and what requires explicit action.
4. Link the matching answer page, not only the homepage.
5. Ask for the reader's current file size or format.

### LinkedIn

Objective: reach technical communicators, educators, analysts, and researchers.

Draft:

```text
Most documentation does not fail at writing; it fails at handoffs.

A report starts in Word, gets copied into Markdown, becomes a slide deck, and loses its source along the way. ReadMD keeps one Markdown file connected to those steps: convert inherited documents, edit beside a preview, cite from a neighboring BibTeX library, and present the same source.

Local work remains the default; uploads and AI actions are explicit. MIT source and checksums are on GitHub.

Which handoff costs you the most time?
```

### Reddit

Objective: help a community solve a named task; accept that a post may bring zero stars.

Preparation checklist:

1. Read the subreddit rules and recent moderator notes.
2. Confirm self-introduced projects are allowed.
3. Choose a community where the pain already appears in public questions.
4. Answer unrelated questions before posting if the account is inactive.

Candidates to research before use: r/markdown, r/ObsidianMD, r/productivity, r/PhD, r/academicwriting, and relevant language-specific communities. Do not post until the rule check passes.

Title pattern:

```text
How do you keep long research notes, citations, and presentations in one Markdown source?
```

Body pattern:

```markdown
I maintain ReadMD, an MIT-licensed Markdown workspace. It grew from a specific problem: notes became slow after thousands of lines, while citations and presentations lived in separate tools.

For very large files it uses semantic pagination around 8,000 lines or 500 KB and keeps search and the outline connected. For academic drafts it reads a neighboring .bib file, shows citation cards, and assembles cited references. Presentation mode reuses the same Markdown rather than copying content into slides.

Conversion and OCR are local on supported platforms; complex layouts and recognition results still need review.

I am the developer and am looking for failure cases: What file size, citation workflow, or conversion breaks your current setup?
```

### Hacker News

Objective: invite technical scrutiny once release notes and issue triage are ready.

Submission title:

```text
Show HN: ReadMD - local-first Markdown reading, conversion, and presenting
```

First comment:

```text
I built ReadMD around one constraint: Markdown remains the canonical artifact unless the user explicitly saves an edit.

Large documents switch to semantic pagination near 8,000 lines or 500 KB while outline, search, formulas, and page controls stay linked. Rendering repairs happen in memory. There are also local converters for office and web formats, qualified native OCR paths on Windows and macOS, BibTeX citation cards, and Reveal-based presentation mode.

Windows, macOS, Linux, and UOS/Kylin packages are on GitHub Releases with SHA256SUMS.txt. HarmonyOS is currently a source project, not a prebuilt binary.

Happy to answer questions about pagination thresholds, preserving source bytes, conversion limits, or why presentation reuses the same file.
```

Rules:

- Submit Tuesday through Thursday morning in the target timezone, unless a major news cycle will hide the launch.
- Have two hours available for replies.
- Acknowledge valid criticism and convert missing requirements into labeled issues.
- Do not ask for votes.

### V2EX And Zhihu

Objective: reach Simplified Chinese technical readers without cross-posting identical text.

V2EX title:

```text
ReadMD：把长文档、转换、OCR、BibTeX 和放映放进同一个本地 Markdown 工作流
```

V2EX body:

```text
我在维护一个 MIT 开源项目 ReadMD。它把几个容易断开的步骤放进同一条本地工作流：超长 Markdown 语义分页，Word、PDF、网页等来源转 Markdown，Windows 和 macOS 的本机 OCR，BibTeX 引用卡片，以及直接用同一份 Markdown 放映。

核心边界是：渲染修正只在显示层，除非主动保存；本地转换和 OCR 不需要上传到网页表单；复杂版式和识别结果仍要人工复核。

下载包和 SHA256SUMS 在 GitHub Releases。HarmonyOS 目前是 ArkTS 源码工程，没有提供预编译 HAP。欢迎分享你遇到过的文件规模、格式或引用流程问题。
```

Zhihu answer outline:

1. Restate the question in one sentence; do not start with the product name.
2. Describe the smallest workflow that solves it.
3. Name limits and human-review steps.
4. Link only the matching localized answer page.
5. Offer to test the reader's file type if disclosure is appropriate.

### Xiaohongshu

Objective: show a concrete before-and-after workflow with existing product captures.

Title options:

```text
长论文写作，我终于不用来回切五个工具了
```

```text
Markdown 写完直接讲，不用重做幻灯片
```

Body:

```text
整理研究资料最麻烦的不是写，而是中间那些搬运：扫描件先转文字，文献另存一遍，最后还要复制成幻灯片。

我用 ReadMD 试了一条更短的路径：扫描页在本机提取文字，BibTeX 放在同一个目录，正文用 [@key] 引用，写完直接进入放映模式。源文件仍然是 Markdown，修改也会回到同一份稿子。

它是 MIT 开源项目。复杂表格和识别结果还是要自己复核；提交论文前也要按学校或期刊格式再检查一遍。

#ReadMD #Markdown #学术写作 #效率工具 #开源项目
```

### WeChat Official Account

Use `wechat_five_workflows_minimal.html` as the paste-ready artifact. Publish at most one WeChat article per week. Keep the title below 64 characters and avoid promising that a tool can replace journal review or reference managers.

## Seven Day Sprint

| Day | Action | Completion evidence |
|---|---|---|
| 1 | Verify all links, screenshots, release assets, and current version facts. | Checklist marked in the ledger. |
| 2 | Update GitHub Discussion and repository pinned resources with one workflow question. | Published link recorded. |
| 3 | Post the English long-document item on X or Mastodon; reply for one hour. | Published link and replies recorded. |
| 4 | Research two Reddit communities and answer unrelated questions. | Rule check and answers recorded. |
| 5 | Publish the Simplified Chinese BibTeX item on a suitable Chinese channel. | Published link and comments recorded. |
| 6 | Publish LinkedIn handoff post or adapt it for a newsletter. | Published link recorded. |
| 7 | Review outcomes, retire weak copy, and queue only channels whose rules passed. | Weekly retro notes recorded. |

Do not start Hacker News until Days 1-7 are complete and someone can monitor replies for two hours.

## Thirty Day Operating Cadence

- Week 1: Run the seven-day sprint and collect qualitative signals.
- Week 2: Answer every thread; publish no new broadcast if reply debt exists.
- Week 3: Turn repeated questions into website answers or issue labels.
- Week 4: Retire copy that produced irrelevant traffic; keep only channels with at least one substantive conversation.

## Measurement

Track these indicators in the ledger:

1. Qualified visits: visits from people matching an audience segment.
2. Substantive replies: technical questions, workflow descriptions, or criticism.
3. Release downloads: only the count reported by GitHub.
4. Issues and discussions: reproducible bugs, feature requests, and adoption blockers.
5. Stars: record weekly from GitHub; do not use it to judge a single post.

A channel earns another test when it produces at least one substantive conversation or reproducible feedback. A channel is paused when it produces only anonymous clicks, violates its rules, or requires claims outside the approved facts.

## Objection Handling

| Objection | Response |
|---|---|
| "Another Markdown editor?" | ReadMD connects reading, conversion, OCR, citations, and presentation while keeping Markdown canonical. |
| "Cloud apps already do this." | Core document work is local; web extraction, AI, and LAN sharing require explicit action. |
| "Can it convert perfectly?" | No converter guarantees identical proprietary layout. Review headings, tables, formulas, images, and scanned text. |
| "Is OCR private?" | The qualified paths run locally on Windows WinRT and macOS Apple Vision; availability depends on device capability. |
| "Why not just use Pandoc?" | Use the best tool for the job. ReadMD adds a GUI workspace around reading, editing, citations, and presenting after conversion. |
| "Is this production ready?" | The public build is clearly labeled `2.3.7-beta.3`; checksums and known boundaries are published. |

## Stop Conditions

Pause distribution when any of these occurs:

- A release cannot pass checksum verification.
- A security bug has no fix or disclosure plan.
- The website reports broken download links.
- A community moderator objects.
- Reply debt prevents honest participation.

## Method And Dependency Mapping

| Source | Application to this kit | Evidence at review time |
|---|---|---|
| [every-app/open-seo](https://github.com/every-app/open-seo/) | Channel selection starts with task intent, competitive context, and measurable follow-up instead of broad promotion. | 13,349 stars, MIT. |
| [AgriciDaniel/claude-seo](https://github.com/AgriciDaniel/claude-seo/) | Public copy links to crawler-accessible answer pages, entity-rich titles, and explicit product boundaries. | 15,012 stars, MIT. |
| [zubair-trabzada/geo-seo-claude](https://github.com/zubair-trabzada/geo-seo-claude/) | Each channel points to the matching localized workflow and citation-safe corpus rather than a generic homepage. | 9,455 stars, MIT. |
| [AgriciDaniel/claude-blog](https://github.com/AgriciDaniel/claude-blog/) | Release discipline gates every public claim behind reproducible facts and review. | 1,803 stars, MIT. |
| [tailwindlabs/tailwindcss](https://github.com/tailwindlabs/tailwindcss/) | Existing production styling system for the linked website; no new frontend dependency is introduced here. | 97,319 stars, MIT; ranked first in a live GitHub search for `tailwindcss`. |

The three requested SEO libraries exceed 9,000 stars, and Tailwind CSS is the selected implementation dependency above the requested 10,000-star threshold.

## Asset Inventory

- Website home: https://readmd.asia/
- Download and checksums: https://readmd.asia/download/
- AI index: https://readmd.asia/llms.txt
- Full AI corpus: https://readmd.asia/llms-full.txt
- Long files: https://readmd.asia/large-markdown-files/
- Presentation: https://readmd.asia/markdown-to-slides/
- Conversion: https://readmd.asia/convert-to-markdown/
- OCR: https://readmd.asia/scan-to-markdown/
- BibTeX: https://readmd.asia/bibtex-citations/
- Simplified Chinese entry: https://readmd.asia/zh-cn/
- Traditional Chinese entry: https://readmd.asia/zh-tw/
- Japanese entry: https://readmd.asia/ja/
