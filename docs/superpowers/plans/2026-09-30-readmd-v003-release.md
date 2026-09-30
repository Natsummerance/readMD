# ReadMD V0.0.3 Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the completed UI upgrade worktrees (`upgrade/shell` and `upgrade/reader`) into `main`, verify all static gates and test suites, update release notes for V0.0.3, tag `V0.0.3`, safely push to GitHub, and clean up worktrees.

**Architecture:** ReadMD uses a pure-Rust kernel (`readmd-kernel`) serving a local Webview frontend. Frontend assets are bundled into `assets/readmd.boot.js` via `cargo xtask bundle-boot`. Releases are driven by GitHub Actions triggering on `V*` tags, which build native installers for Windows, macOS, and Linux.

**Tech Stack:** Rust (1.85 edition 2021, Cargo workspace with `readmd-kernel` and `xtask`), Node.js (test runner, verification linters for styles, i18n, wiring, privacy, zero-python contract), HTML5/CSS3/ESM, GitHub Actions.

## Global Constraints

- Remote URL for `rust-ReadMD` contains an embedded Personal Access Token (`x-access-token:gho_...`). **NEVER print the remote URL or raw push output to the user**.
- Rust compilation target must use `CARGO_TARGET_DIR=Z:/readmd-target/main` because `T:` drive is low on storage.
- CRLF conversion by Windows git must not corrupt line endings in generated/bundled files (`assets/readmd.boot.js`).
- Tag naming convention follows `V0.0.1`, `V0.0.2` -> must use capital `V0.0.3`.
- `upgrade/ai` and `upgrade/pet` worktrees were canceled due to quota limits; keep existing scaffolding without blocking release.

---

### Task 1: Commit and finalize the `upgrade/shell` worktree

**Files:**
- Worktree: `.claude/worktrees/shell`
- Branch: `upgrade/shell`
- Modified/Untracked:
  - `assets/css/shell.css`
  - `assets/css/shell-deferred.css`
  - `assets/css/tokens.css`
  - `assets/i18n/*.json` (46 locales + meta.json)
  - `assets/index.html`
  - `assets/js/shell/command-palette.js`
  - `assets/js/shell/shell.js`
  - `assets/js/shell/shell-deferred.js`
  - `assets/readmd.boot.js`
  - `tools/styles-baseline.json`
  - `ui-tests/shell-upgrade.spec.js`

- [ ] **Step 1: Check git status in shell worktree**
  Verify all 57 modified and untracked files are ready.
  Command: `git -C .claude/worktrees/shell status`

- [ ] **Step 2: Run static verification on shell worktree**
  Run style, wiring, and i18n checks inside the shell worktree.
  Command:
  ```powershell
  node .claude/worktrees/shell/tools/check-styles.mjs
  node .claude/worktrees/shell/tools/check-wiring.mjs
  node .claude/worktrees/shell/tools/check-i18n.mjs
  ```
  Expected: All pass `ok`.

- [ ] **Step 3: Commit all changes in `upgrade/shell`**
  Stage and commit with descriptive message.
  Command:
  ```powershell
  git -C .claude/worktrees/shell add -A
  git -C .claude/worktrees/shell commit -m "feat(shell): Command Palette, design tokens v2, shortcuts modal and responsive shell"
  ```

---

### Task 2: Merge `upgrade/reader` into `main`

**Files:**
- Branch: `main` (workspace root `.`)
- Merge source: `upgrade/reader` (Commit `1d88640`)

- [ ] **Step 1: Perform git merge on `main`**
  Merge the clean, verified `upgrade/reader` branch into `main`.
  Command:
  ```powershell
  git merge upgrade/reader -m "Merge upgrade/reader: premium reading surface, callouts, outline scroll-spy, reading prefs"
  ```
  Expected: Fast-forward or clean recursive merge with zero conflicts.

- [ ] **Step 2: Verify git status on `main`**
  Confirm merge is clean.
  Command: `git status`

---

### Task 3: Merge `upgrade/shell` into `main` and reconcile assets

**Files:**
- Branch: `main`
- Merge source: `upgrade/shell`
- Potential conflict files: `assets/css/tokens.css`, `assets/i18n/*.json`, `assets/index.html`, `assets/readmd.boot.js`, `tools/styles-baseline.json`

- [ ] **Step 1: Merge `upgrade/shell` into `main`**
  Execute git merge:
  ```powershell
  git merge upgrade/shell -m "Merge upgrade/shell: command palette, tokens v2, shortcuts cheat sheet, responsive shell"
  ```

- [ ] **Step 2: Resolve any merge conflicts if they arise**
  Both reader and shell added keys to `i18n/*.json` and updated `tokens.css` and `index.html`. Merge cleanly, ensuring both reader and shell hooks, modals, and tokens coexist.

- [ ] **Step 3: Re-bundle boot file with xtask**
  Rebuild `assets/readmd.boot.js` from composite sources using xtask.
  Command:
  ```powershell
  $env:CARGO_TARGET_DIR="Z:/readmd-target/main"
  cargo xtask bundle-boot
  ```
  (Run from `rust/` directory)
  Expected: `assets/readmd.boot.js` updated and up to date.

- [ ] **Step 4: Update styles baseline if needed**
  Run `node tools/check-styles.mjs` and update baseline if sizes expanded legitimately due to new CSS.

---

### Task 4: Run full verification gate suite on `main`

**Files:**
- `tools/check-styles.mjs`
- `tools/check-wiring.mjs`
- `tools/check-i18n.mjs`
- `tools/check-no-python.mjs`
- `tools/check-assets.mjs`
- `tools/privacy-scan.mjs`
- `tests/frontend/*.test.mjs`
- `tools/test/*.test.mjs`
- `rust/xtask` and `rust/readmd-kernel`

- [ ] **Step 1: Run static JS linters and gates**
  Command:
  ```powershell
  node tools/check-styles.mjs
  node tools/check-wiring.mjs
  node tools/check-i18n.mjs
  node tools/check-no-python.mjs
  node tools/check-assets.mjs
  node tools/privacy-scan.mjs
  ```
  Expected: All 6 gates output `ok` / `PASSED`.

- [ ] **Step 2: Run Node.js unit tests**
  Command:
  ```powershell
  node --test tests/frontend/*.test.mjs tools/test/*.test.mjs
  ```
  Expected: All 36+ tests pass.

- [ ] **Step 3: Run Rust xtask and kernel sanity tests**
  Command:
  ```powershell
  $env:CARGO_TARGET_DIR="Z:/readmd-target/main"
  cargo test --offline -p xtask
  cargo test --offline -p readmd-kernel --lib server::tests::unknown_route_is_404_and_known_route_reaches_handler
  ```
  Expected: All pass without errors.

---

### Task 5: Bump version, update release notes, and commit

**Files:**
- `RELEASE_NOTES.md`

- [ ] **Step 1: Update `RELEASE_NOTES.md` for V0.0.3**
  Document the triple UI overhaul released in V0.0.3:
  1. Editor Upgrade: document-grade Markdown editor with slash menu (`/`), smart list/table editing, format toolbar, split live preview.
  2. Reader Upgrade: Obsidian callouts, GitHub alerts, code block headers with copy buttons, scroll-spy outline, reading preferences popover, sticky table headers.
  3. Shell Upgrade: Command Palette (`Ctrl+K`), design tokens v2, shortcuts cheat sheet modal, responsive mobile drawer.

- [ ] **Step 2: Commit release notes on `main`**
  Command:
  ```powershell
  git add RELEASE_NOTES.md assets/readmd.boot.js tools/styles-baseline.json
  git commit -m "docs: release notes for V0.0.3 — complete UI/UX overhaul (Editor, Reader, Shell)"
  ```

---

### Task 6: Create Git Tag `V0.0.3` and safely push to GitHub

- [ ] **Step 1: Create annotated tag `V0.0.3`**
  Command:
  ```powershell
  git tag -a V0.0.3 -m "ReadMD V0.0.3 - Complete UI/UX Overhaul (Editor, Reader, Shell)"
  ```

- [ ] **Step 2: Safely push `main` and `V0.0.3` to remote `rust-ReadMD`**
  **CRITICAL SECURITY RULE:** Remote URL has an embedded PAT token. Do NOT print the remote URL. Redirect/pipe stdout/stderr or suppress secrets.
  Command:
  ```powershell
  git push rust-ReadMD main tag V0.0.3 2>&1 | ForEach-Object { $_ -replace 'gho_[A-Za-z0-9_]+', '[REDACTED_TOKEN]' }
  ```
  Expected: Push successful, GitHub Actions CI release workflow triggered.

---

### Task 7: Clean up git worktrees

- [ ] **Step 1: Remove temporary worktrees**
  Command:
  ```powershell
  git worktree remove .claude/worktrees/shell
  git worktree remove .claude/worktrees/reader
  git worktree remove .claude/worktrees/editor
  git worktree remove .claude/worktrees/ai
  git worktree remove .claude/worktrees/pet
  ```

- [ ] **Step 2: Prune worktrees and verify clean repo**
  Command:
  ```powershell
  git worktree prune
  git status
  ```
  Expected: Clean working tree on `main`.
