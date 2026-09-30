//! ReadMD native kernel: storage, documents, HTTP API and desktop host.
//!
//! The kernel owns the authorities the legacy Python server owned: filesystem
//! access to markdown sources, the SQLite metadata index, settings and the
//! loopback HTTP contract consumed by the web UI.

pub mod ai;
pub mod batch2;
pub mod bibtex;
pub mod code_chunk_runner;
pub mod codecs;
pub mod content;
pub mod convert;
pub mod convert_ext;
pub mod crypto;
pub mod diagrams;
pub mod headless_renderer;
pub mod import_processor;
pub mod latex2omml;
pub mod link_indexer;
pub mod mdexport;
/// Shared pulldown-cmark AST for the DOCX / LaTeX / PDF exporters.
pub mod md_ast;
/// AST-based Markdown → LaTeX writer used by the LaTeX export.
pub mod latex_writer;
/// AST-based Markdown → DOCX (WordprocessingML) writer.
pub mod docx_writer;
/// Rule-based syntax colouring for exported code blocks.
pub mod code_highlight;
/// Encoding detection and non-lossy encoding for the editor save path.
pub mod text_encoding;
/// TrueType subset embedding (CIDFontType2 / Identity-H) for the PDF writer.
pub mod pdf_fonts;
/// OMML tree reader + MathML writer shared by EPUB/PDF math.
pub mod omml;
/// OpenType MATH layout → vector glyph paths for PDF formulas.
pub mod math_layout;
/// AST-based EPUB chapter (XHTML) writer.
pub mod epub_writer;
/// In-process Win32 `IFileDialog` pickers (no PowerShell spawn).
pub mod win_dialogs;
/// Cross-platform dialogs (Win32 / osascript / zenity·kdialog) and open/reveal.
pub mod native_dialogs;
/// Excel 97-2003 (BIFF5/8) → Markdown tables.
pub mod xls_biff;
/// PowerPoint 97-2003 record stream → Markdown.
pub mod ppt_binary;
/// MOBI / AZW / PalmDOC → HTML → Markdown.
pub mod mobi;
pub mod ocr;
/// Windows.Media.Ocr / Windows.Data.Pdf native OCR (no-op elsewhere).
pub mod ocr_winrt;
pub mod parity_aichat;
pub mod parity_diagram;
pub mod parity_pets;
/// P10 parity layer for `/api/ocr`, `/api/transcribe`, `/api/url`,
/// `/api/web/extract` and `/api/web/cancel`.  Declared at the crate root (not
/// nested via `#[path]` the way `parity_code` is) and routed from
/// `server.rs::ROUTES`.
pub mod parity_web;
pub mod pet_host;
pub mod pet_paths;
pub mod pet_window_state;
pub mod plugin_manager;
pub mod readmd_fix;
pub mod server;
pub mod store;
pub mod transcribe;
pub mod validators;
pub mod version_spec;
pub mod lan_guard;
pub mod desktop_pet;
pub mod window_state;
pub mod ole2;
pub mod pet_queue;
pub mod texmd;
pub mod pdf_editor;
/// Legacy `.doc` (Word 6/95 WordDocument stream) text extraction, line-for-line
/// against `src/readmd_modules/convert.py::parse_doc_stream_to_md`.
pub mod doc_legacy;
/// `src/readmd_modules/updater.py` tiers 2 and 3 (302 redirect sniffing
/// and `SHA256SUMS.txt` asset re-assembly) plus `readmd.py::check_latest_release`.
pub mod updater;
pub mod toc_engine;
pub mod source_map;
pub mod epub_render;
pub mod native_system;

pub mod pet_launcher;
pub mod pet_probe;

/// `src/readmd_modules/mdexport/formula.py::repair_latex` plus the PNG sizing
/// helpers `png_size` / `_needs_cjk_font`, byte-for-byte against CPython
/// (117-row oracle table).
pub mod formula_repair;
/// `src/readmd_modules/mdexport/styles.py` -- the whole export style schema:
/// defaults, presets, `deep_merge`, `_clamp` (including its int/float taint),
/// `sanitize`, `page_dimensions`, `preset_style`, `_hex`/`_font`.
pub mod export_styles;
/// `src/readmd_modules/skill_import.py` — skill package import.
pub mod skill_import;
/// `src/readmd_modules/ai.py` provider transports (skill_messages/chat_anthropic).
pub mod ai_providers;
/// `src/readmd_modules/mdexport/pdf_render.py` native PDF renderer.
pub mod pdf_render;
/// Stable error / note / warning codes riding next to the legacy Chinese text.
pub mod api_codes;
pub mod cancel;
/// `readmd --mcp`: Model Context Protocol server on stdio.
pub mod mcp;

pub use crate::error::{ApiError, ApiResult, Error, Result};
pub use crate::process::silent_command;

/// `parity_code` is declared with `#[path]` **inside** `code_chunk_runner`
/// (`code_chunk_runner.rs`, `pub mod parity_code`), because its body reaches
/// `super::execute_code_chunk_dict` / `super::EXECUTION_TIMEOUT` /
/// `super::MAX_TIMEOUT_SECONDS` in that parent.  Declaring it here as well would
/// compile the 30 KB file a second time as a second module *and* fail to
/// type-check (`super::` has no parent at the crate root), so the crate-root
/// spelling `crate::parity_code::h_code_run` is provided by re-export instead.
/// `code_chunk_runner.rs:645 fn which` therefore needs no visibility change:
/// the module tree is unchanged and `super::super::which` still resolves.
pub use crate::code_chunk_runner::parity_code;

pub mod process {
    use std::process::Command;

    pub fn silent_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
        let mut cmd = Command::new(program);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        cmd
    }
}

use std::sync::Mutex;
use std::time::Instant;

pub mod error {
    use serde_json::{json, Value};
    use std::fmt;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("io: {0}")]
        Io(#[from] std::io::Error),
        #[error("sqlite: {0}")]
        Sql(#[from] rusqlite::Error),
        #[error("json: {0}")]
        Json(#[from] serde_json::Error),
        #[error("utf8: {0}")]
        Utf8(#[from] std::string::FromUtf8Error),
        #[error("crypto: {0}")]
        Crypto(String),
        #[error("{0}")]
        Msg(String),
    }

    pub type Result<T> = std::result::Result<T, Error>;

    /// How the failure becomes an HTTP body.
    ///
    /// `readmd.py` only ever emits three shapes, and the parity harness diffs
    /// the key set of each one, so a handler may not invent a fourth:
    ///
    /// * [`ErrorShape::ApiCode`] — `Handler._send_api_error(status, code, **extra)`
    ///   at `readmd.py:1421`: `{"ok": false, "error_code": "<code>", ...extra}`.
    /// * [`ErrorShape::LegacyError`] — `Handler._send_json(status, {'error': text})`,
    ///   the pre-`error_code` envelope the reader/save/control routes still use.
    /// * [`ErrorShape::PlainText`] — `Handler._send(status, 'text/plain; charset=utf-8', body)`,
    ///   used by the dispatcher itself (`missing p`, `not found`, `forbidden`).
    #[derive(Debug, Clone)]
    pub enum ErrorShape {
        ApiCode { extras: Vec<(String, Value)> },
        LegacyError { text: String },
        PlainText { body: String },
    }

    /// HTTP facing failure. `code` is the stable machine string the UI localizes.
    #[derive(Debug, Clone)]
    pub struct ApiError {
        pub status: u16,
        pub code: String,
        /// Diagnostics only: never part of a response body, matching the legacy
        /// server's rule that exception text must not leave the process.
        pub detail: Option<String>,
        pub shape: ErrorShape,
    }

    impl ApiError {
        fn api_code(status: u16, code: String) -> ApiError {
            ApiError {
                status,
                code,
                detail: None,
                shape: ErrorShape::ApiCode { extras: Vec::new() },
            }
        }

        pub fn new<S: Into<String>>(status: u16, code: S) -> ApiError {
            ApiError::api_code(status, code.into())
        }
        pub fn with<S: Into<String>, D: Into<String>>(status: u16, code: S, detail: D) -> ApiError {
            ApiError::api_code(status, code.into()).noted("detail", detail)
        }

        /// Attach one key of the Python `**extra` kwargs.
        ///
        /// `detail` is the one key `readmd.py` never sends, so it is routed to
        /// the internal diagnostics string instead of the response body.
        pub fn noted<D: Into<String>>(mut self, key: &str, value: D) -> ApiError {
            let value = value.into();
            if key == "detail" {
                self.detail = Some(match &self.detail {
                    Some(prev) if !prev.is_empty() => format!("{prev}; {value}"),
                    _ => value,
                });
                return self;
            }
            match &mut self.shape {
                ErrorShape::ApiCode { extras } => {
                    extras.retain(|(k, _)| k != key);
                    extras.push((key.to_string(), json!(value)));
                }
                _ => {
                    self.detail = Some(match &self.detail {
                        Some(prev) if !prev.is_empty() => format!("{prev}; {key}: {value}"),
                        _ => format!("{key}: {value}"),
                    });
                }
            }
            self
        }

        /// Same as [`ApiError::noted`] but keeps a JSON value (Python passes
        /// lists and numbers through `_send_api_error` as-is).
        pub fn noted_json(mut self, key: &str, value: Value) -> ApiError {
            if let ErrorShape::ApiCode { extras } = &mut self.shape {
                extras.retain(|(k, _)| k != key);
                extras.push((key.to_string(), value));
            }
            self
        }

        pub fn bad_request<S: Into<String>>(code: S) -> ApiError {
            ApiError::api_code(400, code.into())
        }
        pub fn forbidden<S: Into<String>>(code: S) -> ApiError {
            ApiError::api_code(403, code.into())
        }
        pub fn not_found<S: Into<String>>(code: S) -> ApiError {
            ApiError::api_code(404, code.into())
        }
        pub fn internal<S: Into<String>>(code: S) -> ApiError {
            ApiError::api_code(500, code.into())
        }
        /// Capability the Rust kernel does not provide yet. Never silent: the UI
        /// can localize the code and the parity report can count it.
        pub fn pending<S: Into<String>>(feature: S) -> ApiError {
            ApiError::api_code(501, "rust_kernel_pending".to_string()).noted("feature", feature.into())
        }
        pub fn with_status(mut self, status: u16) -> ApiError {
            self.status = status;
            self
        }

        /// `readmd.py: _send_json(status, {'error': <text>})`.
        pub fn legacy_error<S: Into<String>>(status: u16, text: S) -> ApiError {
            let text = text.into();
            ApiError {
                status,
                code: text.clone(),
                detail: None,
                shape: ErrorShape::LegacyError { text },
            }
        }

        /// `readmd.py: _send(status, 'text/plain; charset=utf-8', <body>)`.
        pub fn plain_text<S: Into<String>>(status: u16, body: S) -> ApiError {
            let body = body.into();
            ApiError {
                status,
                code: body.clone(),
                detail: None,
                shape: ErrorShape::PlainText { body },
            }
        }

        pub fn is_plain_text(&self) -> bool {
            matches!(self.shape, ErrorShape::PlainText { .. })
        }

        pub fn payload(&self) -> Value {
            match &self.shape {
                ErrorShape::ApiCode { extras } => {
                    let mut out = json!({ "ok": false, "error_code": self.code });
                    if let Some(map) = out.as_object_mut() {
                        for (k, v) in extras {
                            map.insert(k.clone(), v.clone());
                        }
                    }
                    out
                }
                ErrorShape::LegacyError { text } => json!({ "error": text }),
                ErrorShape::PlainText { body } => Value::String(body.clone()),
            }
        }
    }

    impl From<std::io::Error> for ApiError {
        fn from(e: std::io::Error) -> ApiError {
            ApiError::internal("io_error").noted("detail", e.to_string())
        }
    }
    impl fmt::Display for ApiError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match &self.detail {
                Some(d) => write!(f, "{} ({}: {})", self.status, self.code, d),
                None => write!(f, "{} ({})", self.status, self.code),
            }
        }
    }

    impl std::error::Error for ApiError {}

    impl From<Error> for ApiError {
        fn from(e: Error) -> ApiError {
            let text = e.to_string();
            if text.contains("denied") || text.contains("forbidden") {
                ApiError::with(403, "forbidden", text)
            } else if text.contains("not found") || text.contains("cannot resolve") {
                ApiError::with(404, "not_found", text)
            } else if text.contains("empty path") {
                ApiError::with(400, "missing_path", text)
            } else {
                ApiError::with(500, "internal_error", text)
            }
        }
    }

    pub type ApiResult<T> = std::result::Result<T, ApiError>;
}

pub mod paths {
    use super::error::{Error, Result};
    use std::ffi::OsString;
    use std::path::{Component, Path, PathBuf};

    #[derive(Debug, Clone)]
    pub struct AppPaths {
        pub data_dir: PathBuf,
        pub db_path: PathBuf,
        pub assets_dir: PathBuf,
        pub workspace: PathBuf,
    }

    fn env_text(key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|v| !v.trim().is_empty())
    }

    pub fn home_dir() -> PathBuf {
        match env_text("USERPROFILE").or_else(|| env_text("HOME")) {
            Some(h) => PathBuf::from(h),
            None => PathBuf::from("."),
        }
    }

    fn config_base() -> PathBuf {
        if cfg!(windows) {
            return match env_text("APPDATA") {
                Some(a) => PathBuf::from(a),
                None => home_dir().join("AppData").join("Roaming"),
            };
        }
        if cfg!(target_os = "macos") {
            return home_dir().join("Library").join("Application Support");
        }
        match env_text("XDG_DATA_HOME") {
            Some(x) => PathBuf::from(x),
            None => home_dir().join(".local").join("share"),
        }
    }

    /// Honours `READMD_DATA_DIR` so the Rust kernel and the legacy app can point
    /// at one library instead of needing a copy step.
    pub fn data_dir() -> PathBuf {
        match env_text("READMD_DATA_DIR") {
            Some(p) => PathBuf::from(p),
            None => config_base().join("readmd"),
        }
    }

    /// Ordered places the frontend may live: the env override, beside the exe,
    /// the Linux FHS layout (`bin/../share/readmd/assets`), the macOS bundle
    /// (`MacOS/../Resources/assets`), dev-tree ancestors, cwd, the source tree.
    pub fn assets_candidates(exe_dir: &Path, env_override: Option<&str>, cwd: &Path) -> Vec<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = env_override {
            candidates.push(PathBuf::from(p));
        }
        candidates.push(exe_dir.join("assets"));
        candidates.push(exe_dir.join("..").join("share").join("readmd").join("assets"));
        candidates.push(exe_dir.join("..").join("Resources").join("assets"));
        for up in 1..=5usize {
            let mut p = exe_dir.to_path_buf();
            for _ in 0..up {
                p = p.join("..");
            }
            candidates.push(p.join("assets"));
        }
        candidates.push(cwd.join("assets"));
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
        candidates
    }

    pub fn assets_dir() -> PathBuf {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let exe_dir = exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let env = env_text("READMD_ASSETS_DIR");
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        match assets_candidates(&exe_dir, env.as_deref(), &cwd).into_iter().find(|p| p.is_dir()) {
            Some(p) => p,
            None => exe_dir.join("assets"),
        }
    }

    /// Windows canonicalize returns verbatim `\\?\` paths; the legacy API and the
    /// shipped UI compare and display plain drive paths.
    pub fn strip_verbatim(path: PathBuf) -> PathBuf {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!("\\\\{}", rest));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
        path
    }

    pub fn canonicalize_or_clean(path: &Path) -> PathBuf {
        match std::fs::canonicalize(path) {
            Ok(c) => strip_verbatim(c),
            Err(_) => {
                let mut out = PathBuf::new();
                for c in path.components() {
                    match c {
                        Component::ParentDir => {
                            out.pop();
                        }
                        Component::CurDir => {}
                        other => out.push(other.as_os_str()),
                    }
                }
                out
            }
        }
    }

    pub fn path_starts_with(path: &Path, prefix: &Path) -> bool {
        if path.starts_with(prefix) {
            return true;
        }
        #[cfg(windows)]
        {
            let p_str = path.to_string_lossy().to_ascii_lowercase();
            let pre_str = prefix.to_string_lossy().to_ascii_lowercase();
            let pre_clean = pre_str.trim_end_matches(['\\', '/']);
            if p_str == pre_clean {
                return true;
            }
            if p_str.starts_with(pre_clean) {
                let next_char = p_str[pre_clean.len()..].chars().next();
                if next_char == Some('\\') || next_char == Some('/') {
                    return true;
                }
            }
        }
        false
    }

    /// Canonicalize a path that may not exist yet by walking up to the nearest
    /// existing ancestor. Needed so create and rename targets can be authorized.
    pub fn canonical_existing(path: &Path) -> Result<PathBuf> {
        let mut stack: Vec<OsString> = Vec::new();
        let mut cur = path;
        loop {
            if let Ok(c) = std::fs::canonicalize(cur) {
                let mut out = strip_verbatim(c);
                for seg in stack.iter().rev() {
                    out.push(seg);
                }
                return Ok(out);
            }
            match cur.file_name() {
                Some(name) => {
                    stack.push(name.to_os_string());
                    match cur.parent() {
                        Some(p) if !p.as_os_str().is_empty() => cur = p,
                        _ => {
                            return Err(Error::Msg(format!(
                                "cannot resolve {}",
                                path.display()
                            )))
                        }
                    }
                }
                None => {
                    return Err(Error::Msg(format!(
                        "cannot resolve {}",
                        path.display()
                    )))
                }
            }
        }
    }

    impl AppPaths {
        pub fn resolve() -> Result<AppPaths> {
            let data = data_dir();
            std::fs::create_dir_all(&data)?;
            let workspace = match env_text("READMD_WORKSPACE") {
                Some(p) => PathBuf::from(p),
                None => data.join("workspace"),
            };
            std::fs::create_dir_all(&workspace)?;
            Ok(AppPaths {
                assets_dir: assets_dir(),
                db_path: data.join("readmd.db"),
                data_dir: data,
                workspace,
            })
        }

        pub fn with_dirs(data_dir: &Path, workspace: &Path, assets_dir: &Path) -> AppPaths {
            AppPaths {
                db_path: data_dir.join("readmd.db"),
                assets_dir: assets_dir.to_path_buf(),
                data_dir: data_dir.to_path_buf(),
                workspace: workspace.to_path_buf(),
            }
        }

        pub fn doc_roots(&self) -> Vec<PathBuf> {
            let mut v = vec![self.workspace.clone(), self.uploads_dir()];
            v.extend(Self::extra_roots());
            v
        }

        pub fn check_doc_allowed(&self, canonical: &Path) -> Result<()> {
            for root in self.doc_roots() {
                let clean_root = canonicalize_or_clean(&root);
                if path_starts_with(canonical, &clean_root) {
                    return Ok(());
                }
            }
            Err(Error::Msg(format!(
                "path denied: outside allowed roots: {}",
                canonical.display()
            )))
        }

        pub fn extra_roots() -> Vec<PathBuf> {
            match env_text("READMD_EXTRA_ROOTS") {
                Some(list) => list
                    .split(|c| c == ';' || c == ':')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect(),
                None => Vec::new(),
            }
        }

        pub fn roots(&self) -> Vec<PathBuf> {
            let mut v = vec![self.workspace.clone(), self.data_dir.clone()];
            v.extend(Self::extra_roots());
            v
        }

        pub fn check_allowed(&self, canonical: &Path) -> Result<()> {
            for root in self.roots() {
                let clean_root = canonicalize_or_clean(&root);
                if path_starts_with(canonical, &clean_root) {
                    return Ok(());
                }
            }
            Err(Error::Msg(format!("path denied: outside allowed roots: {}", canonical.display())))
        }

        /// Accepts absolute or root-relative paths, percent-encoded or plain.
        pub fn resolve_doc(&self, raw: &str) -> Result<PathBuf> {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(Error::Msg("empty path".into()));
            }
            let decoded = percent_encoding::percent_decode_str(trimmed)
                .decode_utf8_lossy()
                .into_owned();
            let mut s = decoded.strip_prefix("file://").unwrap_or(&decoded).trim();
            // Handle file:///C:/path (strip extra leading slash before Windows drive letter)
            if (s.starts_with('/') || s.starts_with('\\')) && s.len() >= 3 {
                let bytes = s.as_bytes();
                if bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
                    s = &s[1..];
                }
            }
            let candidate = PathBuf::from(s.replace('/', std::path::MAIN_SEPARATOR_STR));
            let is_abs = candidate.is_absolute();
            let joined = if is_abs {
                candidate
            } else {
                self.workspace.join(&candidate)
            };
            let canonical = canonical_existing(&joined)?;
            if !is_abs {
                self.check_doc_allowed(&canonical)?;
            }
            Ok(canonical)
        }

        /// Stable UI-facing path: relative to the nearest allowed root, `/` separated.
        pub fn display_path(&self, canonical: &Path) -> String {
            for root in self.roots() {
                let rc = canonicalize_or_clean(&root);
                if let Ok(rel) = canonical.strip_prefix(&rc) {
                    let s = rel.to_string_lossy().replace('\\', "/");
                    if !s.is_empty() {
                        return s;
                    }
                    return ".".into();
                }
            }
            canonical.to_string_lossy().replace('\\', "/")
        }

        pub fn uploads_dir(&self) -> PathBuf {
            self.data_dir.join("uploads")
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn tempdir(tag: &str) -> PathBuf {
            let mut p = std::env::temp_dir().join(format!("readmd-k-{}-{}", tag, std::process::id()));
            p.push(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos().to_string())
                .unwrap_or_default());
            std::fs::create_dir_all(&p).unwrap();
            p
        }

        #[test]
        fn rejects_paths_outside_roots() {
            let dir = tempdir("roots");
            let paths = AppPaths::with_dirs(&dir, &dir.join("ws"), &dir.join("assets"));
            std::fs::create_dir_all(paths.workspace.join("sub")).unwrap();
            let inside = paths.resolve_doc("sub/a.md").unwrap();
            assert!(inside.ends_with("a.md"));
            assert!(paths.resolve_doc("../escape.md").is_err());
            assert!(paths.resolve_doc("").is_err());
        }

        #[test]
        fn display_path_is_relative() {
            let dir = tempdir("display");
            let paths = AppPaths::with_dirs(&dir, &dir.join("ws"), &dir.join("assets"));
            std::fs::create_dir_all(paths.workspace.join("notes")).unwrap();
            let f = paths.workspace.join("notes").join("x.md");
            std::fs::write(&f, b"# x").unwrap();
            assert_eq!(paths.display_path(&paths.canonicalize_or_clean_test(&f)), "notes/x.md");
        }
    }

    impl AppPaths {
        #[doc(hidden)]
        pub fn canonicalize_or_clean_test(&self, path: &Path) -> PathBuf {
            canonicalize_or_clean(path)
        }
    }
}

pub mod settings {
    use serde_json::{Map, Value};
    use std::path::PathBuf;

    #[derive(Debug, Clone)]
    pub struct Settings {
        path: PathBuf,
        value: Value,
    }

    pub fn defaults() -> Value {
        let mut m = Map::new();
        m.insert("theme".into(), Value::String("system".into()));
        m.insert("language".into(), Value::String("auto".into()));
        m.insert("recent".into(), Value::Array(Vec::new()));
        m.insert("autostart".into(), Value::Bool(false));
        m.insert("engine".into(), Value::String("rust".into()));
        Value::Object(m)
    }

    impl Settings {
        pub fn load(path: &PathBuf) -> Settings {
            let value = std::fs::read_to_string(path)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .unwrap_or_else(defaults);
            Settings { path: path.clone(), value }
        }

        fn object(&self) -> Map<String, Value> {
            match self.value.as_object() {
                Some(o) => o.clone(),
                None => Map::new(),
            }
        }

        fn save(&mut self) -> std::io::Result<()> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let text = serde_json::to_string_pretty(&self.value).unwrap_or_else(|_| "{}".into());
            let tmp = self.path.with_extension("json.tmp");
            std::fs::write(&tmp, text.as_bytes())?;
            match std::fs::rename(&tmp, &self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    std::fs::copy(&tmp, &self.path)?;
                    let _ = std::fs::remove_file(&tmp);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }

        pub fn all(&self) -> Value {
            self.value.clone()
        }

        pub fn get(&self, key: &str) -> Value {
            self.value.get(key).cloned().unwrap_or(Value::Null)
        }

        /// Shallow merge; an explicit null removes the key, matching the legacy
        /// settings writer used by the UI.
        pub fn merge(&mut self, patch: &Value) -> Value {
            let mut obj = self.object();
            if let Some(src) = patch.as_object() {
                for (k, v) in src {
                    if v.is_null() {
                        obj.remove(k);
                    } else {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
            self.value = Value::Object(obj);
            let _ = self.save();
            self.value.clone()
        }

        pub fn set(&mut self, key: &str, value: Value) -> Value {
            let mut patch = Map::new();
            patch.insert(key.into(), value);
            self.merge(&Value::Object(patch))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn merge_roundtrips_and_removes() {
            let dir = std::env::temp_dir().join(format!("readmd-settings-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("settings.json");
            let mut s = Settings::load(&file);
            assert_eq!(s.get("engine"), Value::String("rust".into()));
            s.merge(&serde_json::json!({ "theme": "dark", "recent": null }));
            let again = Settings::load(&file);
            assert_eq!(again.get("theme"), Value::String("dark".into()));
            assert_eq!(again.get("recent"), Value::Null);
        }
    }
}

/// Shared mutable state behind the HTTP surface.
pub struct App {
    pub paths: paths::AppPaths,
    pub store: store::Store,
    pub settings: Mutex<settings::Settings>,
    pub control: Mutex<ControlQueue>,
    pub started_at: Instant,
    pub app_token: String,
    /// `Handler._api_file` adds every document it serves and `_do_save` refuses
    /// anything outside that set (`readmd.py:3015`, `readmd.py:3554`).
    pub authorized_save_paths: Mutex<std::collections::HashSet<String>>,
    /// Mirror of `src.readmd_modules`' `_status` / `_error` maps.
    pub modules: Mutex<modules::Registry>,
}

/// Mirrors `readmd.py`'s `_CONTROL` dictionary: every entry is a queue that a
/// polling reader pops one element from, never a single-slot mailbox.
#[derive(Debug, Default)]
pub struct ControlQueue {
    /// `_CONTROL['queue']` — `push_control()` / `pop_control()`.
    pub open_queue: std::collections::VecDeque<String>,
    /// `_CONTROL['pet_batches']` — `push_pet_batch()` / `pop_pet_batch()`.
    pub pet_batches: std::collections::VecDeque<Vec<String>>,
    /// `_CONTROL['pet_menus']` — `push_pet_menu()` / `pop_pet_menu()`.
    pub pet_menus: u32,
}

/// Workspace documents indexed before the first request is served.
const BOOT_INDEX_LIMIT: usize = 4000;

/// On-demand feature module registry.
///
/// The legacy server imports each optional module the first time an endpoint
/// needs it and reports the state through `/api/modules`.  The Rust kernel
/// compiles every engine in, so the registry exists to keep the *contract*
/// identical: `idle` until a route has been proven served, then `ready`,
/// `error` or `disabled`.
pub mod modules {
    use serde_json::Value;
    use std::collections::HashMap;

    /// `src.readmd_modules.MODULES`, in the legacy declaration order.
    pub const MODULES: &[&str] =
        &["convert", "ocr", "web", "ai", "transcribe", "pdf_editor"];

    #[derive(Debug, Default, Clone)]
    pub struct Registry {
        status: HashMap<String, String>,
        error: HashMap<String, String>,
    }

    impl Registry {
        /// `_status = {name: 'idle' for name in MODULES}`
        pub fn fresh() -> Registry {
            let mut status = HashMap::new();
            for name in MODULES {
                status.insert((*name).to_string(), "idle".to_string());
            }
            Registry { status, error: HashMap::new() }
        }

        pub fn is_known(name: &str) -> bool {
            MODULES.contains(&name)
        }

        pub fn set(&mut self, name: &str, state: &str) {
            if !Self::is_known(name) {
                return;
            }
            self.status.insert(name.to_string(), state.to_string());
        }

        pub fn set_error(&mut self, name: &str, message: &str) {
            if !Self::is_known(name) {
                return;
            }
            self.status.insert(name.to_string(), "error".to_string());
            self.error.insert(name.to_string(), message.to_string());
        }

        pub fn state(&self, name: &str) -> Option<String> {
            self.status.get(name).cloned()
        }

        pub fn is_ready(&self, name: &str) -> bool {
            self.status.get(name).map(|s| s == "ready").unwrap_or(false)
        }

        /// `RM.status()` -> `(statuses, errors)` snapshots.
        pub fn snapshot(&self) -> (Value, Value) {
            let mut st = serde_json::Map::new();
            for name in MODULES {
                if let Some(state) = self.status.get(*name) {
                    st.insert((*name).to_string(), Value::String(state.clone()));
                }
            }
            let mut err = serde_json::Map::new();
            for (k, v) in &self.error {
                err.insert(k.clone(), Value::String(v.clone()));
            }
            (
                Value::Object(st),
                Value::Object(err),
            )
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn registry_starts_idle_for_the_legacy_module_whitelist() {
            let reg = Registry::fresh();
            let (st, err) = reg.snapshot();
            assert_eq!(st.as_object().map(|m| m.len()), Some(MODULES.len()));
            assert_eq!(st["convert"], Value::String("idle".into()));
            assert_eq!(st["pdf_editor"], Value::String("idle".into()));
            assert_eq!(err, Value::Object(Default::default()));
            assert!(!reg.is_ready("ai"));
        }
    }
}

impl App {
    pub fn bootstrap(paths: paths::AppPaths) -> Result<App> {
        let store = store::Store::open(&paths.db_path)?;
        let settings = settings::Settings::load(&paths.data_dir.join("settings.json"));
        let app = App {
            control: Mutex::new(ControlQueue::default()),
            started_at: Instant::now(),
            store,
            settings: Mutex::new(settings),
            app_token: session_token(&paths.data_dir),
            authorized_save_paths: Mutex::new(std::collections::HashSet::new()),
            modules: Mutex::new(modules::Registry::fresh()),
            paths,
        };
        let _ = content::reindex_workspace(&app, BOOT_INDEX_LIMIT);
        Ok(app)
    }

    pub fn setting(&self, key: &str) -> serde_json::Value {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).get(key)
    }

    pub fn settings_all(&self) -> serde_json::Value {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).all()
    }

    pub fn update_settings(&self, patch: &serde_json::Value) -> serde_json::Value {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .merge(patch)
    }
}

fn session_token(seed: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    h.update(seed.to_string_lossy().as_bytes());
    h.update(std::process::id().to_string().as_bytes());
    h.update(nanos.to_string().as_bytes());
    let digest = h.finalize();
    let mut out = String::new();
    for b in digest.iter().take(12) {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn assets_candidates_keep_env_and_portable_first_then_packaged_layouts() {
        let exe = std::path::Path::new("/opt/readmd/bin");
        let cwd = std::path::Path::new("/work");
        let c = paths::assets_candidates(exe, Some("/custom/assets"), cwd);
        assert_eq!(c[0], PathBuf::from("/custom/assets"));
        assert_eq!(c[1], exe.join("assets"));
        assert_eq!(c[2], exe.join("..").join("share").join("readmd").join("assets"));
        assert_eq!(c[3], exe.join("..").join("Resources").join("assets"));
        assert!(c.contains(&cwd.join("assets")));
        let no_env = paths::assets_candidates(exe, None, cwd);
        assert_eq!(no_env[0], exe.join("assets"));
    }

    #[test]
    fn session_token_is_stable_length_and_unique() {
        let a = session_token(std::path::Path::new("/tmp/x"));
        let b = session_token(std::path::Path::new("/tmp/y"));
        assert_eq!(a.len(), 24);
        assert_ne!(a, b);
    }

    #[test]
    fn api_error_maps_kernel_errors() {
        let e: ApiError = Error::Msg("path denied: nope".into()).into();
        assert_eq!(e.status, 403);
        assert_eq!(e.code, "forbidden");
        let pending: ApiError = ApiError::pending("convert.docx");
        assert_eq!(pending.status, 501);
        let body = pending.payload();
        assert_eq!(body["ok"], false);
        assert_eq!(body["error_code"], "rust_kernel_pending");
        assert_eq!(body["feature"], "convert.docx");
        // Diagnostics must never leak into the body.
        let leak = ApiError::with(400, "bad", "boom");
        assert_eq!(leak.payload().as_object().unwrap().len(), 2);
    }
}
