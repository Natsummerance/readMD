# Instant Reader Optimization Design

## Outcome

Make ReadMD the strongest lightweight, instant-open Markdown reader while staying on version 2.3.x. The product leads with time-to-readable-content, trustworthy local files, and responsive navigation for very long documents. Conversion, AI, OCR, presentation, and collaboration features remain progressive capabilities behind a reliable reader.

## Council Direction

The cross-functional council converged on five initiatives:

1. Preserve reader trust with atomic saves, first-save backups, and external-change detection.
2. Keep long-document interaction responsive through semantic pagination and bounded rendering work.
3. Search and build TOC against the logical document without forcing every block into the active view.
4. Keep optional modules and update checks off the critical startup path.
5. Remove duplicated save/export/startup paths and lock behavior with focused regression tests.

## Architecture Rules

- The desktop bridge and browser HTTP API must call one shared file-service implementation.
- Rendering, indexing, and persistence modules own one concern each and expose explicit data contracts.
- Optional integrations remain lazy; reader startup must not import them eagerly.
- User-facing failures are actionable, preserve drafts, and never leave partial output files.
- Security boundaries stay local-first: path containment, command validation, timeout isolation, checksum verification, and no silent source mutation.

## Milestones

### M1: Safe Saves

Extract atomic text persistence into `src/readmd_core/file_writer.py`. Existing-file saves create one backup, write to a temporary sibling, preserve permissions where supported, flush, and atomically replace the target. Callers may pass the last-known mtime; if another process changed the file, ReadMD returns a conflict instead of overwriting it. Desktop bridge, HTTP handler, and editor UI use this contract.

Acceptance: unit tests cover creation, backup, encoding/newline preservation, conflict detection, matching mtime, and failed-write cleanup. Playwright covers save refresh without stale content.

### M2: Bounded Long-Document Reading

Introduce a document-layout controller that separates logical blocks, pagination strategy, and visible-window rendering. The controller exposes stable anchors for TOC and search, renders only the active window plus a small prefetch buffer, and cancels stale layout work when navigation changes. Continuous mode may remain opt-in, but default huge documents cannot block input after first paint.

Acceptance: synthetic 1k/10k/50k-line corpora keep first paint, page turn, search jump, and TOC jump within budget; anchors survive edits; memory does not grow linearly with hidden DOM nodes.

### M3: Startup Discipline

Group imports into reader-critical, conversion-critical, and integration tiers. Reader startup initializes server/window/state only; AI, OCR, web extraction, presentation, and update flows load lazily or in background workers. Update checks remain non-blocking and checksum-driven.

Acceptance: import-time benchmark proves fewer eager modules, selftest remains green, optional module failure degrades with actionable UI instead of blocking open.

### M4: UX Accessibility Completion

Centralize modal focus management with a stack, add tab arrow/Home/End navigation, announce async result counts, and align all themes to contrast targets. Every icon-only action has an accessible name and every destructive action has recovery.

Acceptance: keyboard-only tests pass for tabs, modals, editor exit, export, and search; static audits find no new P0 accessibility issues.

## Quality Gates

Every milestone runs tracked Python compilation, the full unittest suite, relevant Playwright suites, privacy scan, version synchronization (without changing 2.3.x), workflow YAML parsing, Node syntax checks, and `git diff --check`. Each completed milestone gets one local commit; commits are not pushed.

