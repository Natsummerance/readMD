# Workspace UI/UX update — 2026-09-12

Scope: Skill creation, AI conversation, knowledge graph, backlinks, then the companion workbench. Existing runtime/packaging changes from other work were preserved.

## Delivered

- Skill creation: compact two-column form, example, inline errors, output format, character count, session draft recovery, focus loop and Ctrl/Cmd+Enter.
- AI conversation: document context, prompt starters that do not submit automatically, growing composer, improved message/code/table layout, and streaming that respects reading earlier messages.
- Graph: three-dimensional coordinates and force layout, perspective/depth projection on Canvas, orbit/pan/pinch, keyboard navigation, 2D toggle, search, neighborhood filter, note list and explicit open action. Simulation stops when settled, hidden or over its bounded frame budget. Reduced motion is respected. This is a Canvas-based 3D view, not a WebGL renderer.
- Backlinks: current file, incoming/outgoing tabs with counts, search, keyboard-accessible note buttons, unresolved targets and honest loading/error states. Request sequence guards prevent stale results replacing the current document.
- Companion: visual character cards and separate character/behavior/runtime sections; persistent quiet mode; deliberate greetings and an AI conversation shortcut; local typing feedback with reduced-motion support. Existing sprite/Live2D runtime and import controls remain in use.
- Companion follow-up: searchable and localized character cards, always-visible enable control, rollback and inline errors when configuration fails, actual character greetings shown inside the panel, keyboard interaction, and quick chat/quiet buttons. Live2D choices consistently select the desktop runtime; status reads the adapter's process state instead of treating the enabled preference as proof of a running process.

## Companion references

These are interaction references; this change imports no code or character assets from them and does not reproduce their full feature sets.

| Repository | Design direction considered |
| --- | --- |
| [AIRI](https://github.com/moeru-ai/airi) | Companion-oriented configuration and conversation entry |
| [BongoCat](https://github.com/ayangweb/BongoCat) | Small, immediate input feedback |
| [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) | Character and conversation integration |
| [live2d-widget](https://github.com/stevenjoezhang/live2d-widget) | Lightweight character toolbar and messages |
| [RunCat365](https://github.com/runcat-dev/RunCat365) | Unobtrusive ambient companion |
| [VPet](https://github.com/LorisYounger/VPet) | Direct interaction and behavior controls |
| [clawd-on-desk](https://github.com/rullerzhou-afk/clawd-on-desk) | Activity feedback and quiet presence |
| [vscode-pets](https://github.com/tonybaloney/vscode-pets) | Discoverable character selection |
| [Mate-Engine](https://github.com/shinyflvre/Mate-Engine) | Character library and display controls |
| [LingChat](https://github.com/SlimeBoyOwO/LingChat) | Character-centric conversation entry |

Voice engines, global keyboard hooks, CPU monitoring, new third-party characters and VRM rendering are outside this UI/UX change. The Live2D card is explicitly an appearance preview; the desktop runtime handles animation.

Quiet mode and typing feedback apply to the companion inside the reader. Native desktop companion behavior is still owned by the existing adapter.

## Verification

- The first four panels passed 8 Playwright cases across desktop and mobile.
- The initial five-panel desktop/mobile run passed 10 cases (`ui-tests/workspace-ux-verified.log`). Existing Skill and AI-output sanitizer cases passed in the earlier combined run; that run exposed a missing runtime script manifest, subsequently fixed.
- Companion backend/API and resource gates: 57 tests passed (`build/pytest-pet-upgrade.log`); final resource/localization recheck: 16 passed (`build/pytest-pet-upgrade-final.log`).
- The startup bundle gzip budget was updated from 200 KB to 220 KB for the additional UI, with a new compression-ratio limit of 30% of the original size. This is a budget adjustment, not a size reduction.
- Existing widget, personality/FSM and queue JavaScript checks passed. New browser checks cover character selection, rollback, Live2D routing, greeting/keyboard behavior, quiet controls and stopped-process status.
- No screenshots or image-viewing tools were used. Geometry, interaction, DOM and runtime checks are the validation evidence.

Source files: `assets/workspace-ui.css`, `assets/js/features/{ai,graph,pet-batch,pet-workbench}.js`, `assets/index.html`; generated runtime: `assets/readmd.boot.js`.
