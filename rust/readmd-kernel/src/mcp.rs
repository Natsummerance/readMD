//! `readmd --mcp`: a Model Context Protocol server on stdio (JSON-RPC 2.0, one
//! message per line), replacing `packages/mcp-server/readmd_mcp_server.py`.
//!
//! Every tool runs the kernel's own code — the same converters, exporters,
//! OCR, Skill registry and AI client the desktop UI uses — either through a
//! library call or, for routes that carry gating/validation, through
//! [`server::call_in_process`].  stdout carries protocol messages only; logs
//! go to stderr.
//!
//! Side effects are explicit: exports, network fetches, code execution and
//! PDF rollback require a literal `confirm: true`, output paths must be
//! absolute, never symlinks, and an existing file is only replaced with
//! `overwrite: true`.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::{server, App};

pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_CONCURRENT_TOOLS: usize = 8;

/// Tools that write files, touch the network or run code.
const CONFIRM_REQUIRED: &[&str] = &[
    "readmd_web_to_markdown",
    "readmd_export_document",
    "readmd_export_presentation",
    "readmd_export_epub",
    "readmd_run_code_chunk",
    "readmd_pdf_rollback",
];

/// One-release aliases for the former workflow ids (`prompts/get`, `readmd_ai_assistant`).
const LEGACY_WORKFLOW_ALIASES: &[(&str, &str)] = &[
    ("quick_read", "readmd-quick-read"),
    ("polish", "readmd-polish"),
    ("modify", "readmd-format-fix"),
    ("expand", "readmd-polish"),
    ("continue", "readmd-continue"),
    ("translate", "readmd-translate"),
    ("ask", "readmd-ask"),
    ("summary", "readmd-summary"),
    ("outline", "readmd-outline"),
    ("weekly", "readmd-weekly"),
    ("to_english", "readmd-translate"),
    ("code_review", "readmd-code-review"),
];

fn resolve_skill_id(raw: &str) -> String {
    let t = raw.trim();
    LEGACY_WORKFLOW_ALIASES.iter().find(|(k, _)| *k == t).map(|(_, v)| v.to_string()).unwrap_or_else(|| t.to_string())
}

// ---------------------------------------------------------------- tool list

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn confirm_prop() -> Value {
    json!({ "type": "boolean", "const": true, "description": "Explicit confirmation for this side effect" })
}

fn overwrite_prop() -> Value {
    json!({ "type": "boolean", "default": false, "description": "Replace the target if it already exists" })
}

pub fn tools() -> Value {
    json!([
        { "name": "readmd_fix_markdown",
          "description": "Diagnose and repair Markdown syntax problems (broken formulas, misaligned tables, unclosed code fences, CJK/Latin spacing and punctuation).",
          "inputSchema": schema(json!({ "content": { "type": "string", "description": "Markdown to repair" } }), &["content"]) },
        { "name": "readmd_convert_to_markdown",
          "description": "Convert a local document (docx, doc, pdf, pptx, ppt, xlsx, xls, csv, epub, mobi, azw3, html, tex, txt, rtf, odt, ipynb, zip …) to Markdown.",
          "inputSchema": schema(json!({ "file_path": { "type": "string", "description": "Absolute path of the source document" } }), &["file_path"]) },
        { "name": "readmd_web_to_markdown",
          "description": "Fetch an http(s) URL and extract its main content as clean Markdown.",
          "inputSchema": schema(json!({ "url": { "type": "string" }, "confirm": confirm_prop() }), &["url", "confirm"]) },
        { "name": "readmd_ocr_to_markdown",
          "description": "Recognise text in a local image or scanned PDF with the system OCR engine and return Markdown.",
          "inputSchema": schema(json!({ "file_path": { "type": "string", "description": "Absolute path of the image or PDF" } }), &["file_path"]) },
        { "name": "readmd_export_document",
          "description": "Export Markdown to PDF (vector formulas, embedded fonts), Word .docx (native equations), HTML or LaTeX.",
          "inputSchema": schema(json!({
              "markdown_content": { "type": "string" },
              "output_path": { "type": "string", "description": "Absolute target path; the extension must match output_format" },
              "output_format": { "type": "string", "enum": ["pdf", "docx", "html", "tex"] },
              "style_preset": { "type": "string", "description": "Style preset (default minimal)" },
              "title": { "type": "string" },
              "base_dir": { "type": "string", "description": "Directory relative images resolve against (default: the output directory)" },
              "overwrite": overwrite_prop(),
              "confirm": confirm_prop() }), &["markdown_content", "output_path", "output_format", "confirm"]) },
        { "name": "readmd_latex_to_md",
          "description": "Convert LaTeX source (papers or formulas) to Markdown.",
          "inputSchema": schema(json!({ "latex_content": { "type": "string" } }), &["latex_content"]) },
        { "name": "readmd_md_to_latex",
          "description": "Compile Markdown into standalone LaTeX source for pdflatex / xelatex.",
          "inputSchema": schema(json!({ "markdown_content": { "type": "string" }, "doc_title": { "type": "string" } }), &["markdown_content"]) },
        { "name": "readmd_parse_bibtex",
          "description": "Parse a BibTeX file (or the .bib files beside a document) into structured entries.",
          "inputSchema": schema(json!({ "bib_file_path": { "type": "string", "description": ".bib file, or a document / directory to search" } }), &["bib_file_path"]) },
        { "name": "readmd_latex_to_omml",
          "description": "Convert a LaTeX formula to Office Math (OMML) XML for Word.",
          "inputSchema": schema(json!({ "latex_formula": { "type": "string" }, "display": { "type": "boolean", "default": false } }), &["latex_formula"]) },
        { "name": "readmd_ai_assistant",
          "description": "Render a ReadMD Skill into a ready-to-send system prompt for the given document.",
          "inputSchema": schema(json!({
              "skill_id": { "type": "string" },
              "workflow_id": { "type": "string", "description": "Legacy alias of skill_id" },
              "markdown_content": { "type": "string" },
              "request": { "type": "string" },
              "language": { "type": "string" } }), &[]) },
        { "name": "readmd_ai_providers",
          "description": "List the AI providers configured in ReadMD and their connection state. Never returns API keys.",
          "inputSchema": schema(json!({}), &[]) },
        { "name": "readmd_ai_chat",
          "description": "Run one AI document task through a ReadMD Skill using a saved credential (credential_id, never a raw key).",
          "inputSchema": schema(json!({
              "provider": { "type": "string" }, "credential_id": { "type": "string" }, "model": { "type": "string" },
              "skill_id": { "type": "string" }, "markdown_content": { "type": "string" },
              "request": { "type": "string" }, "language": { "type": "string" } }), &["provider", "model", "skill_id", "markdown_content"]) },
        { "name": "readmd_process_imports",
          "description": "Flatten @import directives (nested Markdown, CSV as tables, code line ranges).",
          "inputSchema": schema(json!({ "markdown_content": { "type": "string" }, "base_dir": { "type": "string" } }), &["markdown_content"]) },
        { "name": "readmd_generate_toc",
          "description": "Build a linked table of contents from the document's headings.",
          "inputSchema": schema(json!({
              "markdown_content": { "type": "string" },
              "depth_from": { "type": "integer", "default": 1 }, "depth_to": { "type": "integer", "default": 6 },
              "ordered_list": { "type": "boolean", "default": false } }), &["markdown_content"]) },
        { "name": "readmd_export_presentation",
          "description": "Export Markdown slides (--- or <!-- slide --> separated) to a single offline Reveal.js HTML file.",
          "inputSchema": schema(json!({
              "markdown_content": { "type": "string" }, "output_path": { "type": "string" }, "title": { "type": "string" },
              "theme": { "type": "string" }, "transition": { "type": "string" },
              "overwrite": overwrite_prop(), "confirm": confirm_prop() }), &["markdown_content", "output_path", "confirm"]) },
        { "name": "readmd_export_epub",
          "description": "Package Markdown as an EPUB 3 e-book (local images embedded).",
          "inputSchema": schema(json!({
              "markdown_content": { "type": "string" }, "output_path": { "type": "string" }, "title": { "type": "string" },
              "author": { "type": "string" }, "language": { "type": "string" }, "base_dir": { "type": "string" },
              "overwrite": overwrite_prop(), "confirm": confirm_prop() }), &["markdown_content", "output_path", "confirm"]) },
        { "name": "readmd_run_code_chunk",
          "description": "Run a code block (python, javascript, bash, powershell, r, rust …) with the locally installed runtime and capture its output.",
          "inputSchema": schema(json!({
              "code": { "type": "string" }, "language": { "type": "string", "default": "python" },
              "capture_plot": { "type": "boolean", "default": true }, "confirm": confirm_prop() }), &["code", "confirm"]) },
        { "name": "readmd_pdf_audit",
          "description": "Inspect a PDF: page count, page sizes, rotation, text layer, read-only flag and processes holding it open.",
          "inputSchema": schema(json!({ "pdf_path": { "type": "string" } }), &["pdf_path"]) },
        { "name": "readmd_pdf_rollback",
          "description": "Restore a PDF from its .bak backup written by a previous ReadMD edit.",
          "inputSchema": schema(json!({ "pdf_path": { "type": "string" }, "confirm": confirm_prop() }), &["pdf_path", "confirm"]) },
    ])
}

// ---------------------------------------------------------------- results

fn text_result(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }] })
}

fn json_result(v: &Value) -> Value {
    text_result(serde_json::to_string_pretty(v).unwrap_or_default())
}

fn error_result(code: &str, message: Option<&str>) -> Value {
    let mut body = json!({ "ok": false, "error_code": code });
    if let Some(m) = message {
        body["error"] = json!(m);
    }
    json!({ "isError": true, "content": [{ "type": "text", "text": body.to_string() }] })
}

fn arg_str(args: &Value, key: &str, fallback: &str) -> String {
    match args.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => fallback.to_string(),
        Some(v) => v.to_string(),
    }
}

fn arg_bool(args: &Value, key: &str, fallback: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

fn arg_i64(args: &Value, key: &str, fallback: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))).unwrap_or(fallback)
}

fn existing_file(raw: &str) -> Result<PathBuf, Value> {
    let p = Path::new(raw);
    if raw.is_empty() || !p.is_absolute() {
        return Err(error_result("path_must_be_absolute", None));
    }
    if !p.is_file() {
        return Err(error_result("file_not_found", Some(&format!("文件不存在: {raw}"))));
    }
    Ok(p.to_path_buf())
}

/// Validate an explicit output target before any write: absolute, the right
/// extension, an existing parent directory, no symlink anywhere on the path,
/// and no silent replacement.
pub fn output_target(raw: &str, suffix: &str, overwrite: bool) -> Result<PathBuf, &'static str> {
    if raw.is_empty() || raw.contains('\0') || !Path::new(raw).is_absolute() {
        return Err("output_path_must_be_absolute");
    }
    let path = PathBuf::from(raw);
    for anc in path.ancestors() {
        if anc.as_os_str().is_empty() {
            break;
        }
        if std::fs::symlink_metadata(anc).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            return Err("output_target_symlink_denied");
        }
    }
    if !raw.to_lowercase().ends_with(&suffix.to_lowercase()) {
        return Err("invalid_output_extension");
    }
    match path.parent() {
        Some(parent) if parent.is_dir() => {}
        _ => return Err("output_directory_not_found"),
    }
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if !meta.is_file() {
            return Err("output_target_not_regular");
        }
        if !overwrite {
            return Err("output_exists");
        }
    }
    Ok(path)
}

/// Exclusive create, or atomic replace with `overwrite`.
fn write_output(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), &'static str> {
    if !overwrite {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists { "output_exists" } else { "write_failed" }
        })?;
        return f.write_all(bytes).map_err(|_| "write_failed");
    }
    crate::content::write_bytes_atomic(path, bytes).map_err(|_| "write_failed")
}

/// Unwrap an in-process API answer: `Ok(body)` on 2xx with `ok != false`.
fn api(app: &Arc<App>, method: &str, target: &str, body: Option<&Value>) -> Result<Value, Value> {
    let (status, v) = server::call_in_process(app, method, target, body);
    if (200..300).contains(&status) && v.get("ok") != Some(&Value::Bool(false)) {
        return Ok(v);
    }
    let code = v.get("error_code").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| {
        if status == 404 { "not_found".into() } else { format!("http_{status}") }
    });
    let msg = v.get("error").and_then(Value::as_str).map(str::to_string).or_else(|| v.as_str().map(str::to_string));
    Err(error_result(&code, msg.as_deref()))
}

/// Read-only view of the AI provider config with every secret-like field removed.
fn scrub(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    let k = k.to_lowercase();
                    !matches!(k.as_str(), "api_key" | "apikey" | "key" | "secret" | "password" | "token" | "authorization")
                })
                .map(|(k, v)| (k.clone(), scrub(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(scrub).collect()),
        other => other.clone(),
    }
}

fn providers_view(app: &Arc<App>) -> Result<Value, Value> {
    let cfg = api(app, "GET", "/api/ai/config", None)?;
    let mut providers = Vec::new();
    // `providers` is the legacy alias of `presets` in this payload.
    for key in ["presets", "custom"] {
        if let Some(Value::Array(list)) = cfg.get(key) {
            providers.extend(list.iter().cloned());
        }
    }
    Ok(scrub(&json!({
        "schema_version": cfg.get("schema_version").cloned().unwrap_or(Value::Null),
        "providers": providers,
        "current": cfg.get("current").cloned().unwrap_or(json!({})),
    })))
}

fn skill_variables(args: &Value) -> Value {
    let doc = arg_str(args, "markdown_content", "");
    json!({
        "document": if doc.is_empty() { "(no document supplied)".to_string() } else { doc },
        "selection": "",
        "request": arg_str(args, "request", ""),
        "language": arg_str(args, "language", "en"),
        "context": "",
        "output_format": "Markdown",
    })
}

// ---------------------------------------------------------------- tools/call

pub fn call_tool(app: &Arc<App>, name: &str, args: &Value) -> Value {
    if CONFIRM_REQUIRED.contains(&name) && args.get("confirm") != Some(&Value::Bool(true)) {
        return error_result("confirmation_required", None);
    }
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch_tool(app, name, args)));
    match run {
        Ok(Ok(v)) | Ok(Err(v)) => v,
        Err(_) => error_result("internal_error", None),
    }
}

fn dispatch_tool(app: &Arc<App>, name: &str, args: &Value) -> Result<Value, Value> {
    match name {
        "readmd_fix_markdown" => {
            let res = crate::readmd_fix::fix_markdown(&arg_str(args, "content", ""));
            Ok(json_result(&json!({
                "ok": true, "repaired_content": res.text, "fixes_count": res.fixes.len(),
                "fixes_details": res.fixes, "stats": res.stats,
            })))
        }
        "readmd_convert_to_markdown" => {
            let path = existing_file(&arg_str(args, "file_path", ""))?;
            let t = crate::convert::convert_triple(&path.to_string_lossy(), true);
            if let Some(err) = t.error {
                return Err(error_result("conversion_failed", Some(&err)));
            }
            if t.text.trim().is_empty() {
                return Err(error_result("convert_no_text", Some("未提取到文字；如果是扫描件，可使用 readmd_ocr_to_markdown")));
            }
            Ok(text_result(t.text))
        }
        "readmd_web_to_markdown" => {
            let url = arg_str(args, "url", "").trim().to_string();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err(error_result("invalid_url", Some("URL 必须以 http:// 或 https:// 开头")));
            }
            let doc = crate::parity_web::fetch_document(&url, "smart", 30, "")
                .map_err(|e| error_result(&e.code, Some(&e.message)))?;
            if doc.get("ok") != Some(&Value::Bool(true)) {
                let code = doc.get("error_code").and_then(Value::as_str).unwrap_or("web_extract_failed");
                return Err(error_result(code, doc.get("error").and_then(Value::as_str)));
            }
            Ok(json_result(&json!({
                "ok": true,
                "title": doc.pointer("/meta/title").cloned().unwrap_or(json!("")),
                "markdown": doc.get("content").cloned().unwrap_or(json!("")),
                "url": url,
                "engine": doc.get("engine").cloned().unwrap_or(Value::Null),
                "warnings": doc.get("warnings").cloned().unwrap_or(json!([])),
            })))
        }
        "readmd_ocr_to_markdown" => {
            let path = existing_file(&arg_str(args, "file_path", ""))?;
            let text = crate::ocr::ocr_any(&path.to_string_lossy()).map_err(|e| {
                let code = serde_json::to_value(&e.error_code).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "ocr_failed".into());
                error_result(&code, Some(&e.code))
            })?;
            Ok(text_result(text))
        }
        "readmd_export_document" => export_document(app, args),
        "readmd_latex_to_md" => Ok(text_result(crate::texmd::latex_to_markdown(&arg_str(args, "latex_content", ""), ""))),
        "readmd_md_to_latex" => {
            let title = arg_str(args, "doc_title", "ReadMD Document");
            let tex = crate::texmd::md_to_latex(&arg_str(args, "markdown_content", ""), &title, "", true, &crate::texmd::OptVal::Null);
            Ok(text_result(tex))
        }
        "readmd_parse_bibtex" => {
            let raw = arg_str(args, "bib_file_path", "");
            let p = Path::new(&raw);
            if raw.is_empty() || !p.exists() {
                return Err(error_result("file_not_found", Some(&format!("文件不存在: {raw}"))));
            }
            let entries: Vec<Value> = if p.is_file() && raw.to_lowercase().ends_with(".bib") {
                let text = crate::content::read_text(p).map_err(|e| error_result("bibtex_failed", Some(&e.to_string())))?;
                server::parse_bibtex(&text)
            } else {
                let mut all = Vec::new();
                let dir = if p.is_dir() { p.to_path_buf() } else { p.parent().map(Path::to_path_buf).unwrap_or_default() };
                if let Ok(read) = std::fs::read_dir(&dir) {
                    let mut bibs: Vec<PathBuf> = read.flatten().map(|e| e.path())
                        .filter(|q| q.extension().map(|x| x.eq_ignore_ascii_case("bib")).unwrap_or(false)).collect();
                    bibs.sort();
                    for b in bibs {
                        if let Ok(text) = crate::content::read_text(&b) {
                            all.extend(server::parse_bibtex(&text));
                        }
                    }
                }
                all
            };
            Ok(json_result(&json!({ "ok": true, "count": entries.len(), "entries": entries })))
        }
        "readmd_latex_to_omml" => Ok(text_result(crate::latex2omml::latex_to_omml(
            &arg_str(args, "latex_formula", ""),
            arg_bool(args, "display", false),
        ))),
        "readmd_ai_assistant" => {
            let wf = arg_str(args, "workflow_id", "");
            let requested = [arg_str(args, "skill_id", ""), wf.clone()].into_iter().find(|s| !s.trim().is_empty()).unwrap_or_else(|| "readmd-quick-read".into());
            let id = resolve_skill_id(&requested);
            let vars = skill_variables(args);
            let prompt = server::skill_render(app, &id, &vars).map_err(|e| error_result("skill_not_found", Some(&e)))?;
            let desc = server::skill_list(app).into_iter().find(|s| s.id == id).map(|s| s.description).unwrap_or_default();
            Ok(json_result(&json!({
                "workflow_id": wf, "skill_id": id, "workflow_name": id, "description": desc,
                "system_prompt": prompt, "user_payload": vars["document"],
            })))
        }
        "readmd_ai_providers" => Ok(json_result(&providers_view(app)?)),
        "readmd_ai_chat" => {
            if args.get("api_key").is_some() || args.get("key").is_some() {
                return Err(error_result("raw_key_rejected", Some("AI Chat 只接受 credential_id，不接受 API Key")));
            }
            let (provider, skill_id, doc) = (arg_str(args, "provider", ""), arg_str(args, "skill_id", ""), arg_str(args, "markdown_content", ""));
            if provider.is_empty() || skill_id.is_empty() || doc.is_empty() {
                return Err(error_result("invalid_request", Some("provider、skill_id、markdown_content 均为必填")));
            }
            let body = json!({
                "provider": provider,
                "credential_id": arg_str(args, "credential_id", ""),
                "model": arg_str(args, "model", ""),
                "skill_id": resolve_skill_id(&skill_id),
                "skill_variables": skill_variables(args),
                "messages": [{ "role": "user", "content": doc }],
                "stream": false,
            });
            let v = api(app, "POST", "/api/ai/chat", Some(&body))?;
            Ok(text_result(json!({
                "ok": true,
                "content": v.get("content").cloned().unwrap_or(json!("")),
                "usage": v.get("usage").cloned().unwrap_or(Value::Null),
                "skill_id": skill_id,
            }).to_string()))
        }
        "readmd_process_imports" => {
            let base = arg_str(args, "base_dir", "");
            let base = if base.is_empty() { std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default() } else { base };
            let out = crate::import_processor::process_markdown_imports(&arg_str(args, "markdown_content", ""), &base, None, None, None)
                .map_err(|e| {
                    let code = serde_json::to_value(&e.error_code).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "import_failed".into());
                    error_result(&code, Some(&e.code))
                })?;
            Ok(text_result(out))
        }
        "readmd_generate_toc" => {
            let headings = crate::toc_engine::extract_headings(&arg_str(args, "markdown_content", ""));
            Ok(text_result(crate::toc_engine::generate_toc_markdown(
                &headings,
                arg_i64(args, "depth_from", 1),
                arg_i64(args, "depth_to", 6),
                arg_bool(args, "ordered_list", false),
                None,
            )))
        }
        "readmd_export_presentation" => {
            let overwrite = arg_bool(args, "overwrite", false);
            let out = output_target(&arg_str(args, "output_path", ""), ".html", overwrite).map_err(|c| error_result(c, None))?;
            let html = crate::mdexport::render_presentation_html(
                &arg_str(args, "markdown_content", ""),
                &arg_str(args, "title", "ReadMD Presentation"),
                &arg_str(args, "theme", "black"),
                &arg_str(args, "transition", "slide"),
                true,
                &app.paths.assets_dir,
            )
            .map_err(|e| error_result("presentation_export_failed", Some(&e)))?;
            write_output(&out, html.as_bytes(), overwrite).map_err(|c| error_result(c, None))?;
            Ok(json_result(&json!({ "ok": true, "output_path": out, "file_size": html.len() })))
        }
        "readmd_export_epub" => {
            let overwrite = arg_bool(args, "overwrite", false);
            let out = output_target(&arg_str(args, "output_path", ""), ".epub", overwrite).map_err(|c| error_result(c, None))?;
            let base = arg_str(args, "base_dir", &out.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
            let (bytes, warns) = crate::mdexport::epub_build_bytes(
                &json!(arg_str(args, "markdown_content", "")),
                &base,
                &json!({}),
                &arg_str(args, "title", "ReadMD 电子书"),
                &arg_str(args, "author", "ReadMD Author"),
                &arg_str(args, "language", "zh-CN"),
                &uuid::Uuid::new_v4().to_string(),
                &crate::mdexport::now_iso_z(),
            )
            .map_err(|e| error_result("export_failed", Some(&e)))?;
            write_output(&out, &bytes, overwrite).map_err(|c| error_result(c, None))?;
            Ok(json_result(&json!({ "ok": true, "output_path": out, "file_size": bytes.len(), "warnings": warns })))
        }
        "readmd_run_code_chunk" => {
            let body = json!({
                "code": arg_str(args, "code", ""),
                "lang": arg_str(args, "language", "python"),
                "capture_plot": arg_bool(args, "capture_plot", true),
                "confirm": true,
            });
            Ok(json_result(&api(app, "POST", "/api/code/run", Some(&body))?))
        }
        "readmd_pdf_audit" => {
            let path = existing_file(&arg_str(args, "pdf_path", ""))?;
            let v = crate::pdf_editor::audit(&path.to_string_lossy()).map_err(|e| error_result("pdf_audit_failed", Some(&e.message)))?;
            Ok(json_result(&v))
        }
        "readmd_pdf_rollback" => {
            let raw = arg_str(args, "pdf_path", "");
            if raw.is_empty() || !Path::new(&raw).is_absolute() {
                return Err(error_result("path_must_be_absolute", None));
            }
            let v = crate::pdf_editor::rollback(&raw).map_err(|e| error_result("pdf_rollback_failed", Some(&e.message)))?;
            Ok(json_result(&v))
        }
        _ => Err(error_result("unknown_tool", Some(&format!("未知的工具名称: {name}")))),
    }
}

fn export_document(app: &Arc<App>, args: &Value) -> Result<Value, Value> {
    let fmt = arg_str(args, "output_format", "pdf").to_lowercase();
    if !matches!(fmt.as_str(), "pdf" | "docx" | "html" | "tex") {
        return Err(error_result("unsupported_output_format", None));
    }
    let overwrite = arg_bool(args, "overwrite", false);
    let out = output_target(&arg_str(args, "output_path", ""), &format!(".{fmt}"), overwrite).map_err(|c| error_result(c, None))?;
    let base = arg_str(args, "base_dir", &out.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
    let preset = arg_str(args, "style_preset", "minimal");
    let options = crate::export_styles::preset_style(&preset);
    let title = arg_str(args, "title", "ReadMD Document");
    // Exported straight to the validated target: TeX/HTML exports place their
    // `<stem>.assets/` folder beside it and reference it by that name.
    let out_s = out.to_string_lossy().into_owned();
    let res = crate::mdexport::export_document(&fmt, &arg_str(args, "markdown_content", ""), &base, &out_s, &options, &title, &app.paths.assets_dir)
        .map_err(|e| error_result("export_failed", Some(&e)))?;
    if !res.ok {
        return Err(error_result("export_failed", res.error.as_deref()));
    }
    let warns = res.warns.unwrap_or_default();
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    Ok(json_result(&json!({
        "ok": true, "format": fmt, "output_path": out, "file_size": size,
        "warnings": warns, "warn_items": crate::api_codes::warn_items(&warns),
    })))
}

// ---------------------------------------------------------------- resources / prompts

fn resources(app: &Arc<App>) -> Value {
    let mut out: Vec<Value> = server::skill_list(app)
        .into_iter()
        .map(|s| json!({ "uri": format!("readmd://skills/{}", s.id), "name": s.id, "description": s.description, "mimeType": "text/markdown" }))
        .collect();
    out.push(json!({ "uri": "readmd://providers", "name": "ReadMD AI providers",
        "description": "Provider catalog and connection state; secrets are omitted.", "mimeType": "application/json" }));
    out.push(json!({ "uri": "readmd://sessions", "name": "ReadMD AI sessions",
        "description": "Local AI chat history (read-only).", "mimeType": "application/json" }));
    json!({ "resources": out })
}

fn read_resource(app: &Arc<App>, uri: &str) -> Result<Value, (i64, String)> {
    let one = |mime: &str, text: String| json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] });
    match uri {
        "readmd://providers" => {
            let v = providers_view(app).map_err(|_| (-32603, "provider config unavailable".to_string()))?;
            Ok(one("application/json", v.to_string()))
        }
        "readmd://sessions" => {
            let (_, v) = server::call_in_process(app, "GET", "/api/ai/history", None);
            Ok(one("application/json", scrub(&v).to_string()))
        }
        _ => {
            let id = uri.strip_prefix("readmd://skills/").unwrap_or("");
            if id.is_empty() || id.contains('/') || id.contains('\\') {
                return Err((-32602, format!("unknown resource: {uri}")));
            }
            let skill = server::skill_list(app).into_iter().find(|s| s.id == id).ok_or((-32602, format!("Skill not found: {id}")))?;
            Ok(one("text/markdown", skill.instructions))
        }
    }
}

fn prompts(app: &Arc<App>) -> Value {
    let args = json!([
        { "name": "markdown_content", "description": "Document text", "required": false },
        { "name": "request", "description": "User request", "required": false },
        { "name": "language", "description": "Output language", "required": false },
    ]);
    let list: Vec<Value> = server::skill_list(app)
        .into_iter()
        .map(|s| json!({ "name": s.id, "description": s.description, "arguments": args }))
        .collect();
    json!({ "prompts": list })
}

fn get_prompt(app: &Arc<App>, params: &Value) -> Result<Value, (i64, String)> {
    let id = resolve_skill_id(&arg_str(params, "name", "readmd-quick-read"));
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let text = server::skill_render(app, &id, &skill_variables(&args)).map_err(|e| (-32602, e))?;
    Ok(json!({ "description": id, "messages": [{ "role": "user", "content": { "type": "text", "text": text } }] }))
}

// ---------------------------------------------------------------- JSON-RPC loop

/// Serialised writer: responses from worker threads never interleave.
#[derive(Clone)]
pub struct Out(Arc<Mutex<Box<dyn Write + Send>>>);

impl Out {
    pub fn new(w: Box<dyn Write + Send>) -> Out {
        Out(Arc::new(Mutex::new(w)))
    }
    fn send(&self, v: &Value) {
        let mut w = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(w, "{v}");
        let _ = w.flush();
    }
}

fn reply(out: &Out, id: &Value, result: Value) {
    out.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
}

fn reply_err(out: &Out, id: &Value, code: i64, message: &str) {
    out.send(&json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }));
}

/// Handle one incoming line.  `tools/call` runs on a worker thread so the loop
/// keeps reading (`ping`, further calls) while a long export or OCR runs.
pub fn handle_line(app: &Arc<App>, out: &Out, busy: &Arc<AtomicUsize>, cancelled: &Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return reply_err(out, &Value::Null, -32700, "Parse error"),
    };
    let Some(obj) = msg.as_object() else {
        return reply_err(out, &Value::Null, -32600, "Invalid Request");
    };
    let method = obj.get("method").and_then(Value::as_str).unwrap_or("");
    let params = obj.get("params").cloned().unwrap_or(json!({}));
    let Some(id) = obj.get("id").cloned() else {
        // Notification: never answered.
        if method == "notifications/cancelled" {
            if let Some(rid) = params.get("requestId") {
                if let Some(flag) = cancelled.lock().unwrap_or_else(|e| e.into_inner()).get(&rid.to_string()) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
        return;
    };
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSIONS[0] };
            reply(out, &id, json!({
                "protocolVersion": version,
                "serverInfo": { "name": "readmd", "title": "ReadMD", "version": server::VERSION },
                "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                "instructions": "ReadMD converts documents to Markdown, repairs Markdown, and exports PDF/DOCX/HTML/LaTeX/EPUB/slides. Tools that write files, fetch URLs or run code need confirm: true.",
            }));
        }
        "ping" => reply(out, &id, json!({})),
        "tools/list" => reply(out, &id, json!({ "tools": tools() })),
        "resources/list" => reply(out, &id, resources(app)),
        "resources/read" => match read_resource(app, &arg_str(&params, "uri", "")) {
            Ok(v) => reply(out, &id, v),
            Err((c, m)) => reply_err(out, &id, c, &m),
        },
        "prompts/list" => reply(out, &id, prompts(app)),
        "prompts/get" => match get_prompt(app, &params) {
            Ok(v) => reply(out, &id, v),
            Err((c, m)) => reply_err(out, &id, c, &m),
        },
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str).map(str::to_string) else {
                return reply_err(out, &id, -32602, "Invalid params");
            };
            let args = match params.get("arguments") {
                None | Some(Value::Null) => json!({}),
                Some(v @ Value::Object(_)) => v.clone(),
                Some(_) => return reply_err(out, &id, -32602, "Invalid params"),
            };
            if busy.fetch_add(1, Ordering::SeqCst) >= MAX_CONCURRENT_TOOLS {
                busy.fetch_sub(1, Ordering::SeqCst);
                return reply_err(out, &id, -32001, "server_busy");
            }
            let flag = Arc::new(AtomicBool::new(false));
            cancelled.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), flag.clone());
            let (app, out, busy, cancelled) = (app.clone(), out.clone(), busy.clone(), cancelled.clone());
            std::thread::Builder::new()
                .name("mcp-tool".into())
                .spawn(move || {
                    let result = call_tool(&app, &name, &args);
                    // A cancelled request gets no response (MCP spec).
                    if !flag.load(Ordering::SeqCst) {
                        reply(&out, &id, result);
                    }
                    cancelled.lock().unwrap_or_else(|e| e.into_inner()).remove(&id.to_string());
                    busy.fetch_sub(1, Ordering::SeqCst);
                })
                .ok();
        }
        _ => reply_err(out, &id, -32601, &format!("Method not found: {method}")),
    }
}

/// Serve MCP on stdin/stdout until stdin closes; waits for running tools.
pub fn run_stdio(app: Arc<App>) {
    let out = Out::new(Box::new(std::io::stdout()));
    let busy = Arc::new(AtomicUsize::new(0));
    let cancelled = Arc::new(Mutex::new(HashMap::new()));
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        handle_line(&app, &out, &busy, &cancelled, &line);
    }
    while busy.load(Ordering::SeqCst) > 0 {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct Chan(mpsc::Sender<String>, Vec<u8>);
    impl Write for Chan {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.1.extend_from_slice(b);
            while let Some(i) = self.1.iter().position(|&c| c == b'\n') {
                let line: Vec<u8> = self.1.drain(..=i).collect();
                let _ = self.0.send(String::from_utf8_lossy(&line).trim().to_string());
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn harness(tag: &str) -> (Arc<App>, Out, mpsc::Receiver<String>, Arc<AtomicUsize>, Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>) {
        let dir = std::env::temp_dir().join(format!("readmd-mcp-app-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let paths = crate::paths::AppPaths::with_dirs(&dir.join("data"), &dir, &dir.join("assets"));
        let app = Arc::new(App::bootstrap(paths).unwrap());
        let (tx, rx) = mpsc::channel();
        (app, Out::new(Box::new(Chan(tx, Vec::new()))), rx, Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(HashMap::new())))
    }

    fn roundtrip(tag: &str, lines: &[Value]) -> Vec<Value> {
        let (app, out, rx, busy, cancelled) = harness(tag);
        for l in lines {
            handle_line(&app, &out, &busy, &cancelled, &l.to_string());
        }
        let want = lines.iter().filter(|l| l.get("id").is_some()).count();
        (0..want).map(|_| serde_json::from_str(&rx.recv_timeout(std::time::Duration::from_secs(30)).unwrap()).unwrap()).collect()
    }

    fn by_id(res: &[Value], id: i64) -> Value {
        res.iter().find(|r| r["id"] == json!(id)).cloned().unwrap_or(Value::Null)
    }

    #[test]
    fn handshake_lists_tools_and_answers_ping() {
        let r = roundtrip("hs", &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"ping"}),
            json!({"jsonrpc":"2.0","id":4,"method":"nope"}),
        ]);
        assert_eq!(by_id(&r, 1)["result"]["protocolVersion"], "2024-11-05");
        let listed = by_id(&r, 2);
        let names: Vec<&str> = listed["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"readmd_convert_to_markdown") && names.contains(&"readmd_export_document"));
        assert_eq!(by_id(&r, 3)["result"], json!({}));
        assert_eq!(by_id(&r, 4)["error"]["code"], -32601);
    }

    #[test]
    fn tools_run_and_side_effects_need_confirmation() {
        let dir = std::env::temp_dir().join(format!("readmd-mcp-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("a.html");
        let r = roundtrip("tools", &[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"readmd_fix_markdown","arguments":{"content":"# t\n\n**x"}}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"readmd_export_document","arguments":{"markdown_content":"# t","output_path":out,"output_format":"html"}}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"readmd_generate_toc","arguments":{"markdown_content":"# A\n## B\n"}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"readmd_export_document","arguments":{"markdown_content":"x","output_path":"rel.pdf","output_format":"pdf","confirm":true}}}),
        ]);
        let fixed: Value = serde_json::from_str(by_id(&r, 1)["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(fixed["ok"], true);
        assert_eq!(by_id(&r, 2)["result"]["isError"], true);
        assert!(by_id(&r, 2)["result"]["content"][0]["text"].as_str().unwrap().contains("confirmation_required"));
        assert!(by_id(&r, 3)["result"]["content"][0]["text"].as_str().unwrap().contains("[B](#b)"));
        assert!(by_id(&r, 4)["result"]["content"][0]["text"].as_str().unwrap().contains("output_path_must_be_absolute"));
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_never_replaces_without_overwrite() {
        let dir = std::env::temp_dir().join(format!("readmd-mcp-ow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("doc.tex");
        std::fs::write(&out, "keep").unwrap();
        let call = |id: i64, ow: bool| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"readmd_export_document","arguments":{"markdown_content":"# Hi","output_path":out,"output_format":"tex","overwrite":ow,"confirm":true}}});
        let r = roundtrip("ow", &[call(1, false)]);
        assert!(by_id(&r, 1)["result"]["content"][0]["text"].as_str().unwrap().contains("output_exists"), "{r:?}");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
        let r = roundtrip("ow2", &[call(2, true)]);
        assert_eq!(by_id(&r, 2)["result"].get("isError"), None, "{r:?}");
        assert!(std::fs::read_to_string(&out).unwrap().contains("Hi"));
        let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".mcp-")).collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_errors_and_bad_params_are_reported() {
        let (app, out, rx, busy, cancelled) = harness("bad");
        handle_line(&app, &out, &busy, &cancelled, "{not json");
        let v: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32700);
        handle_line(&app, &out, &busy, &cancelled, r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"x","arguments":[1]}}"#);
        let v: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32602);
    }
}
