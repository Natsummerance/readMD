# ReadMD v2.3.8 Code Review Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all defects and smells identified in the independent Standards and Spec Code Reviews: restore module-level interface contracts, render clickable styled `[[wikilink]]` in the reader with document navigation, trim unrequested heavy plugins, add missing sample assets, and verify 100% test passing and showcase recording.

**Architecture:** 
- In Python backend (`readmd.py`), restore public symbol exports (`verify_model_bundle`, `foreground_fullscreen`) and clean up proxy delegation.
- In frontend reader (`assets/js/reader/render.js`, `assets/style.css`, `assets/app.js`), integrate a pre-pass/markdown-it rule transforming `[[target|alias]]` and `[[target#heading|alias]]` into interactive `.wikilink` DOM elements with click-to-load navigation.
- In showcase samples, generate `sample_doc.docx` and re-bundle scripts via `tools/sync_version.py`.

**Tech Stack:** Python 3.11, JavaScript (ES6+), CSS3 design tokens, marked / markdown-it AST, Playwright, SQLite WAL.

## Global Constraints
- Documented Repo Standards: Python PEP 8, zero-dependency pure Python fallback for core convert operations, non-executing metadata inspection at startup.
- Fowler Smell Baseline: 0 Middle Man, 0 Data Clumps, 0 Mysterious Names.
- Full offline capability: Zero remote network dependencies.
- Full test pass: All 838+ tests passing with 0 failures, 100% 46-language i18n coverage.

---

### Task 1: Restore Module-Level Interface Contracts & Remove Middle Man in readmd.py

**Files:**
- Modify: `readmd.py:53-65`, `readmd.py:4765-4775`
- Test: `tests/test_pet_api_contract.py`

**Interfaces:**
- Consumes: `src.readmd_modules.pet: (verify_model_bundle, foreground_fullscreen)`
- Produces: `readmd.verify_model_bundle`, `readmd.foreground_fullscreen`

- [ ] **Step 1: Write/run the failing test**
Verify `monkeypatch.setattr(readmd, "verify_model_bundle", ...)` works without AttributeError.

- [ ] **Step 2: Modify readmd.py**
Restore exports in `readmd.py`:
```python
from src.readmd_modules.pet import (
    HermesPetBridge,
    HermesPetLauncher,
    HermesPetPluginInstaller,
    PetBatchQueue,
    PetController,
    foreground_fullscreen,
    verify_model_bundle,
)
```
Replace `_SkillImportProxy` with direct import:
```python
import src.readmd_modules.skill_import as _skill_import
```
Remove unrequested thread pre-import in `main()` so startup flow is clean and deterministic.

- [ ] **Step 3: Run tests to verify pass**
```bash
pytest tests/test_pet_api_contract.py
```

---

### Task 2: Implement In-Reader `[[wikilink]]` Rendering & Click-to-Jump Navigation

**Files:**
- Modify: `assets/js/reader/render.js:15-60`, `assets/style.css`, `assets/app.js:140-160`
- Rebuild: Run `python tools/sync_version.py`
- Test: `tests/test_static_assets.py`, `ui-tests/plugins.spec.js`

**Interfaces:**
- Consumes: Wikilink patterns `\[\[(.*?)\]\]`
- Produces: HTML `<a class="wikilink" data-target="..." href="javascript:void(0)">...</a>`
- Interaction: Click event resolves relative/exact document path and calls `window.loadFile(targetPath)`.

- [ ] **Step 1: Add wikilink preprocessing/rendering in reader**
In `assets/js/reader/render.js`, preprocess Markdown before `marked.parse`:
Replace `\[\[([^\]]+)\]\]` outside code blocks with:
`<a class="wikilink" data-target="${target}" title="${target}">${displayText}</a>`
- [ ] **Step 2: Add styles for `.wikilink` in `assets/style.css`**
Style `.wikilink` with accent coloration, subtle border/dashed underline, and hover badge highlight for deadlinks and active documents.
- [ ] **Step 3: Wire click delegation**
In `assets/app.js`, add reader click listener for `.wikilink` elements to navigate to target document via `window.loadFile(targetPath)`.
- [ ] **Step 4: Recompile boot bundle**
```bash
python tools/sync_version.py
```

---

### Task 3: Prune Scope Creep & Add Missing Showcase Assets

**Files:**
- Modify: `src/readmd_modules/plugin_manager.py:40-60`
- Create: `showcase/v238_capabilities/samples/sample_doc.docx`
- Test: `tests/test_plugins_manager.py`

- [ ] **Step 1: Prune unrequested docling from PLUGIN_SPECS**
Remove `docling` from `PLUGIN_SPECS` in `src/readmd_modules/plugin_manager.py`, keeping focused, lightweight plugins: `rapidocr`, `rapid_table`, `pylatexenc`, `whisper`, `easyocr`.
- [ ] **Step 2: Generate sample_doc.docx**
Use `docx` module or pure python script to generate a rich Word document sample with headings, bold text, lists, and tables in `showcase/v238_capabilities/samples/sample_doc.docx`.
- [ ] **Step 3: Run plugin manager tests**
```bash
python -m unittest tests/test_plugins_manager.py
```

---

### Task 4: Re-record Full Showcase Videos & Regression Suite Gate

**Files:**
- Re-run: `node ui-tests/record_full_capabilities.js`
- Verify outputs:
  - `showcase/v238_capabilities/videos/01-05.mp4`
  - `showcase/v238_capabilities/snapshots/01-13.png` (verifying `04-wikilink-reader.png` shows styled links)
- Run full regression:
  - `python -m unittest discover -s tests -p "test_*.py"`
  - `pytest tests/test_pet_api_contract.py`
  - `python tests/test_i18n_coverage_test.py`
