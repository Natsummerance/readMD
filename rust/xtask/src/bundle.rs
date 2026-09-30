//! `assets/readmd.boot.js`: the classic-script sources joined with `\n;\n`,
//! byte for byte (no newline normalisation), plus one trailing `\n` when the
//! last source does not already end with a line break.

use std::fs;
use std::path::Path;

/// Load order matters: later files use globals defined by earlier ones.
pub const SOURCES: &[&str] = &[
    "vendor/marked.min.js",
    "js/core/modal.js",
    "js/core/task-feedback.js",
    "js/core/state.js",
    "js/core/i18n.js",
    "js/core/dialog.js",
    "js/core/settings.js",
    "js/core/modules.js",
    "js/core/tabs.js",
    "js/core/history.js",
    "js/core/dragdrop.js",
    "js/reader/formula.js",
    "js/reader/fixes.js",
    "js/reader/toc.js",
    "js/reader/search.js",
    "js/reader/folder.js",
    "js/reader/render.js",
    "js/editor/preview.js",
    "js/editor/image.js",
    "js/editor/md-transforms.js",
    "js/editor/editor.js",
    "js/features/ai.js",
    "js/features/share.js",
    "js/features/convert.js",
    "js/features/batch.js",
    "js/features/pet-batch.js",
    "js/features/pet-workbench.js",
    "js/features/pet-companion-actions.js",
    "js/features/ocr.js",
    "js/features/web.js",
    "js/features/clipboard.js",
    "js/features/export.js",
    "js/features/updater.js",
    "js/features/graph.js",
    "app.js",
];

pub fn build(root: &Path) -> Vec<u8> {
    let assets = root.join("assets");
    let chunks: Vec<Vec<u8>> = SOURCES
        .iter()
        .filter_map(|s| fs::read(assets.join(s)).ok())
        .collect();
    let mut body = chunks.join(&b"\n;\n"[..]);
    if !matches!(body.last(), Some(b'\n') | Some(b'\r')) {
        body.push(b'\n');
    }
    body
}

/// Write (or, with `check`, compare) the bundle.  `Ok(false)` = out of date.
pub fn bundle(root: &Path, check: bool) -> Result<bool, String> {
    let missing: Vec<&str> = SOURCES.iter().copied().filter(|s| !root.join("assets").join(s).is_file()).collect();
    if !missing.is_empty() {
        return Err(format!("missing boot sources: {}", missing.join(", ")));
    }
    let out = root.join("assets").join("readmd.boot.js");
    let body = build(root);
    if check {
        let current = fs::read(&out).unwrap_or_default();
        if current == body {
            println!("[OK] assets/readmd.boot.js is up to date");
            return Ok(true);
        }
        println!("[FAIL] assets/readmd.boot.js is stale; run `cargo xtask bundle-boot`");
        return Ok(false);
    }
    fs::write(&out, &body).map_err(|e| format!("write {}: {e}", out.display()))?;
    println!("[SYNC] Rebundled assets/readmd.boot.js ({} bytes)", body.len());
    Ok(true)
}
