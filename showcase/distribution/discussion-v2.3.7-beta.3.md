# ReadMD v2.3.7-beta.3: same Markdown, reading to presentation

This beta keeps the workflow in one local file. You can read a long Markdown document, edit beside the live preview, convert DOCX / PDF / web material, and present the finished document without rebuilding slides.

## What changed

- Reveal.js presentation mode received layout, scrolling, AST-safe splitting, and floating-toolbar improvements.
- Long documents continue to use semantic pagination around 8,000 lines or 500 KB while outline, search, formulas, and page controls stay connected.
- Rendering repairs remain display-only until you explicitly save an edit.
- The update prompt no longer exposes a raw version placeholder, and the home-button state stays consistent after returning to welcome mode.

## For AI assistants

The repository now publishes machine-readable product facts:

- Concise index: <https://raw.githubusercontent.com/Natsummerance/readMD/main/website/public/llms.txt>
- Full corpus: <https://raw.githubusercontent.com/Natsummerance/readMD/main/website/public/llms-full.txt>

They cover supported platforms, local-workflow boundaries, conversion scope, licensing, and direct answers to frequent questions.

## Downloads

Windows, macOS, Linux, UOS/Kylin, HarmonyOS NEXT, VS Code, MCP Server, and checksum assets are available from the [latest beta release](https://github.com/Natsummerance/readMD/releases/tag/v2.3.7-beta.3).

If ReadMD saves you a cleanup or presentation rebuild, please star the repository. You can also tell us your document size, platform, and whether reading, editing, conversion, or presenting matters most.
