# ReadMD Launch Distribution Pack

This pack contains approved drafts for manual distribution. Do not paste the same text across platforms unchanged. Disclose author affiliation where expected, answer technical questions directly, and stop if a community prohibits self-promotion.

## Short Social Posts

### English X / Mastodon

```text
ReadMD keeps large Markdown workflows local. Semantic pagination handles documents near 8,000 lines or 500 KB, rendering repair stays in memory, and Office/PDF material can become Markdown without a cloud account. MIT source: https://github.com/Natsummerance/readMD
```

### Simplified Chinese Social Post

```text
超长 Markdown 不一定要搬到云端。ReadMD 在约 8,000 行或 500 KB 时启用语义分页，目录、搜索和公式继续联动；渲染修正不改原文件，还能把 Word、PDF、网页转成 Markdown。MIT 开源：https://github.com/Natsummerance/readMD
```

## Community Posts

### Reddit Title

```text
I built a local-first Markdown reader for very large documents and direct presentation
```

### Reddit Body

```markdown
I kept running into two problems: long notes became slow in browser-based editors, and turning notes into a talk meant copying everything into slides.

ReadMD is my MIT-licensed attempt to keep one Markdown file useful from reading through presentation. It activates semantic pagination for documents around 8,000 lines or 500 KB while keeping outline, search, formulas, and page navigation connected. Rendering repairs happen in memory; the original file is not rewritten unless you save an edit.

It also imports DOCX/PDF/HTML/LaTeX into Markdown, uses native OCR on supported platforms, and has a Reveal-based presentation mode for the same document you were editing.

Windows, macOS, Linux, UOS/Kylin builds and checksums are on GitHub Releases. I am the developer, so I would especially like feedback on very large documents, academic references, or presentation workflows.

Repository: https://github.com/Natsummerance/readMD
```

### Hacker News Submission And Comment

Submission title:

```text
Show HN: ReadMD - local-first Markdown reading, conversion, and presenting
```

First comment:

```text
I built ReadMD because I wanted one local Markdown file to survive from research notes to editing and then to a talk. Large documents switch to semantic pagination around 8,000 lines or 500 KB; outline, search, formulas, and page controls stay connected. Syntax recovery happens at the rendering layer, so the original file is untouched unless you explicitly edit and save.

There are also converters for DOCX/PDF/HTML/LaTeX, native OCR on supported platforms, BibTeX citation cards, and a Reveal-based presentation mode. It ships for Windows, macOS, Linux, and UOS/Kylin, with VS Code and MCP integrations.

Source and downloads are here: https://github.com/Natsummerance/readMD . Happy to answer questions about pagination, preserving source bytes, or presentation constraints.
```

## Developer Article Outline

1. Start with the failure mode: a useful note becomes unusable after it grows, then gets duplicated into slides.
2. Define the invariant: Markdown remains the canonical artifact until the user explicitly saves.
3. Explain semantic pagination and why outline, search, formulas, and page state must remain connected.
4. Show a conversion workflow from DOCX or PDF into editable Markdown.
5. Describe presenting the same source file with Reveal mode.
6. End with checksum verification, supported platforms, and requests for real long-document scenarios.

## Distribution Rules

- Verify every claim against `llms-full.txt` before posting.
- Do not invent star counts, user counts, download counts, speed rankings, or AI-citation guarantees.
- Use each community's own title and tone; do not cross-post identical bodies.
- Prefer screenshots already captured by the showcase pipeline over new marketing art.
- Track the platform, date, link, and engagement in your publication ledger.
