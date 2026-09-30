//! Repository maintenance tasks, run as `cargo xtask <command>` from `rust/`.
//!
//! * `bundle-boot [--check]` — rebuild `assets/readmd.boot.js` from its sources.
//! * `sync-version [VERSION] [--check]` — propagate the version from `.env` /
//!   `VERSION` (or the argument) to every manifest, page and README, then
//!   rebundle.  `--check` only reports files that are out of sync.
//! * `release-asset-sync --assets-dir DIR --tag TAG --commit SHA [--repo R]` —
//!   stage-then-swap GitHub Release assets (port of `tools/release_asset_sync.py`).
//! * `pet-package [--platform P] [--arch A] [--skip-build] [--output DIR]` —
//!   stage and zip the native pet runtime (port of `build-package.py`).
//!
//! This is the Rust port of `tools/sync_version.py`; the replacement rules
//! are the same, and each file keeps its own line endings.

mod bundle;
mod pet_package;
mod release_sync;
mod version;

use std::path::PathBuf;
use std::process::ExitCode;

fn repo_root() -> PathBuf {
    // rust/xtask/ → repository root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn usage() -> ExitCode {
    eprintln!("usage: cargo xtask <bundle-boot [--check] | sync-version [VERSION] [--check] | release-asset-sync --assets-dir DIR --tag TAG --commit SHA [--repo R] | pet-package [--platform P] [--arch A] [--skip-build] [--output DIR]>");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repo_root();
    let check = args.iter().any(|a| a == "--check");
    let positional: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    match args.first().map(String::as_str) {
        Some("bundle-boot") => match bundle::bundle(&root, check) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(1),
            Err(e) => {
                eprintln!("[ERROR] {e}");
                ExitCode::from(2)
            }
        },
        Some("sync-version") => {
            let ver = positional
                .first()
                .map(|s| s.to_string())
                .unwrap_or_else(|| version::load_env_version(&root));
            match version::sync_all(&root, &ver, check) {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => ExitCode::from(1),
                Err(e) => {
                    eprintln!("[ERROR] {e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("release-asset-sync") => match release_sync::main(&root, &args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[ERROR] {e}");
                ExitCode::from(1)
            }
        },
        Some("pet-package") => match pet_package::main(&root, &args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[ERROR] {e}");
                ExitCode::from(1)
            }
        },
        _ => usage(),
    }
}
