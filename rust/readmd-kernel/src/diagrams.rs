//! ReadMD 专业图表辅助模块 (Diagrams Helper) —— `src/readmd_modules/diagrams.py` 的 Rust 复刻。
//!
//! 支持图表体系（与 Python 侧一致）：
//! 1. Mermaid / WaveDrom / Bitfield / Viz / TikZ / Chart：浏览器端懒加载离线渲染（服务端只探测资产是否存在）；
//! 2. PlantUML：双通道 —— 本地 Java/plantuml 进程，或 `allow_remote` 显式授权后的公网 SVG 代理；
//! 3. Vega & Vega-Lite：随包的 Node 运行时离线渲染（spec 走 stdin，不进命令行）；
//! 4. wsd / d2 / ditaa：本版本明确不可用，避免误判为联网成功。
//!
//! Parity 契约（权威依据 `scratch/rust_parity/contract.json` + `readmd.py:1879`/`1942`）：
//! * 失败分两类，绝不混用：
//!   [`DiagramError::Expected`] 复刻 Python 的 `DiagramRenderError` → HTTP 422 + `error_code` = 稳定码；
//!   [`DiagramError::Fatal`] 复刻 Python 的其它异常（严格 UTF-8 解码失败等）→ HTTP 500 + `diagram_render_failed`。
//! * 任何需要联网的分支都由调用方（`parity_diagram`）的 `allow_remote is True` 门控制，本模块不出网。

use crate::paths;
use serde_json::map::Map;
use serde_json::{json, Value};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// PlantUML 官方 6-bit 自定义 Base64 字母表。
pub const PLANTUML_CHARS: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_";

/// Python `get_plantuml_svg_url` 的默认服务器。
pub const PLANTUML_SERVER_URL: &str = "https://www.plantuml.com/plantuml";

/// Python `render_plantuml_svg(timeout=15.0)` 默认值。
pub const PLANTUML_TIMEOUT: Duration = Duration::from_secs(15);
/// Python `render_vega_svg(timeout=12.0)` 默认值。
pub const VEGA_TIMEOUT: Duration = Duration::from_secs(12);

const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SVG_BYTES: usize = 8 * 1024 * 1024;

/// 图表渲染失败。变体决定 HTTP 状态码，见模块文档。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagramError {
    /// Python `DiagramRenderError(code)`：422 + `error_code = code`。
    Expected(String),
    /// Python 中未被捕获的异常：500 + `diagram_render_failed`。
    Fatal,
}

impl DiagramError {
    /// 稳定错误码（Python `DiagramRenderError.__init__` 的 `str(code or "diagram_render_failed")`）。
    pub fn code(&self) -> &str {
        match self {
            DiagramError::Expected(code) if !code.is_empty() => code,
            DiagramError::Expected(_) => "diagram_render_failed",
            DiagramError::Fatal => "diagram_render_failed",
        }
    }
}

impl std::fmt::Display for DiagramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl std::error::Error for DiagramError {}

fn expected(code: &str) -> DiagramError {
    DiagramError::Expected(code.to_string())
}

pub type DiagramResult<T> = Result<T, DiagramError>;

/// Python `str.strip()` 的空白集（比 Rust `trim()` 多 `\x1c-\x1f`）。
pub fn py_strip(text: &str) -> &str {
    let is_space = |c: char| {
        c.is_whitespace()
            || matches!(c, '\u{1c}'..='\u{1f}' | '\u{85}')
    };
    text.trim_matches(is_space)
}

// --------------------------------------------------------------- plantuml url

fn plantuml_encode_6bit(b: u8) -> char {
    PLANTUML_CHARS[(b & 0x3F) as usize] as char
}

/// `zlib.compress(data)[2:-4]`：Python 取 zlib 流的裸 deflate 载荷。
fn zlib_payload(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::write::ZlibEncoder;
    use std::io::Write;
    // Python `zlib.compress()` 不带 level 参数 => 默认 level 6。
    let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    encoder.write_all(data).ok()?;
    let stream = encoder.finish().ok()?;
    if stream.len() < 6 {
        return None;
    }
    Some(stream[2..stream.len() - 4].to_vec())
}

/// 裸 deflate 载荷 -> PlantUML 6-bit 字母表（含尾部残缺组的 Python 行为）。
fn encode_deflate(data: &[u8]) -> String {
    let mut encoded = String::new();
    let mut i = 0usize;
    while i < data.len() {
        let b1 = data[i];
        let b2 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b3 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let c1 = b1 >> 2;
        let c2 = ((b1 & 0x3) << 4) | (b2 >> 4);
        let c3 = ((b2 & 0xF) << 2) | (b3 >> 6);
        let c4 = b3 & 0x3F;

        encoded.push(plantuml_encode_6bit(c1));
        encoded.push(plantuml_encode_6bit(c2));
        if i + 1 < data.len() {
            encoded.push(plantuml_encode_6bit(c3));
        }
        if i + 2 < data.len() {
            encoded.push(plantuml_encode_6bit(c4));
        }
        i += 3;
    }
    encoded
}

/// 将 PlantUML 纯文本压缩并转换为官方标准 Web URL 编码（= Python `plantuml_encode`）。
pub fn plantuml_encode(text: &str) -> String {
    match zlib_payload(text.as_bytes()) {
        Some(payload) => encode_deflate(&payload),
        None => String::new(),
    }
}

/// `batch2::h_diagram_render` 旧调用点保留的别名，语义同 [`plantuml_encode`]。
pub(crate) fn encode_plantuml(text: &str) -> String {
    plantuml_encode(text)
}

/// 生成 PlantUML 在线 SVG 渲染 URL（= Python `get_plantuml_svg_url`）。
///
/// 官方默认格式没有 `~1` 之类前缀：加了前缀 plantuml.com 会回“bad URL”错误图。
pub fn get_plantuml_svg_url(plantuml_code: &str, server_url: &str) -> String {
    let mut code = py_strip(plantuml_code).to_string();
    if !code.starts_with("@start") {
        code = format!("@startuml\n{code}\n@enduml");
    }
    let encoded = plantuml_encode(&code);
    let base = server_url.trim_end_matches('/');
    format!("{base}/svg/{encoded}")
}

// ----------------------------------------------------------------- local puml

/// `shutil.which` 等价实现（Windows 下按 `PATHEXT` 逐个扩展名尝试）。
pub fn which(command: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        let exts: Vec<String> =
            pathext.split(';').filter(|e| !e.is_empty()).map(|e| e.to_string()).collect();
        if exts
            .iter()
            .any(|ext| command.to_lowercase().ends_with(&ext.to_lowercase()))
        {
            vec![command.to_string()]
        } else {
            exts.into_iter().map(|ext| format!("{command}{ext}")).collect()
        }
    } else {
        vec![command.to_string()]
    };
    let path_env = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_env) {
        for name in &names {
            let candidate = dir.join(name);
            if !candidate.is_file() {
                continue;
            }
            if !has_exec_bit(&candidate) {
                continue;
            }
            return Some(candidate);
        }
    }
    None
}

/// `os.access(candidate, os.X_OK)`：POSIX 上看任意一位可执行位。
#[cfg(unix)]
fn has_exec_bit(candidate: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match candidate.metadata() {
        Ok(meta) => meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// Windows 的 `os.access(..., os.X_OK)` 只判定文件是否存在，不看扩展名以外的权限。
#[cfg(not(unix))]
fn has_exec_bit(_candidate: &Path) -> bool {
    true
}

/// Python `env_text`：空串按未设置处理（`os.environ.get(k)` 的真值判断）。
fn env_text(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// 探测系统本地是否具备 Java 及 PlantUML 环境（= Python `has_local_plantuml`）。
///
/// 只做 PATH/文件探测，绝不启动进程：`plantuml --help` 之类的探测会把
/// “装了但跑不起来”误判成可用，也会在离线机器上引入不必要的进程。
pub fn has_local_plantuml() -> bool {
    if which("plantuml").is_some() {
        return true;
    }
    let java = which("java");
    let jar = env_text("PLANTUML_JAR");
    match (java, jar) {
        (Some(_), Some(jar)) => Path::new(&jar).is_file(),
        _ => false,
    }
}

/// 返回免 shell 的本地 PlantUML 命令（= Python `_plantuml_command`）。
fn plantuml_command() -> Option<Vec<OsString>> {
    if let Some(exe) = which("plantuml") {
        return Some(vec![
            exe.into_os_string(),
            OsString::from("-tsvg"),
            OsString::from("-pipe"),
        ]);
    }
    let java = which("java")?;
    let jar = env_text("PLANTUML_JAR")?;
    let path = Path::new(&jar);
    if !path.is_file() {
        return None;
    }
    let resolved = crate::paths::canonicalize_or_clean(path);
    Some(vec![
        java.into_os_string(),
        OsString::from("-jar"),
        resolved.into_os_string(),
        OsString::from("-tsvg"),
        OsString::from("-pipe"),
    ])
}

/// 本地 Java/plantuml 渲染 SVG（= Python `render_plantuml_svg`，无网络、无临时文件）。
pub fn render_plantuml_svg(plantuml_code: &str, timeout: Duration) -> DiagramResult<String> {
    let command = match plantuml_command() {
        Some(cmd) => cmd,
        None => return Err(expected("diagram_dependency_missing")),
    };
    let code = py_strip(plantuml_code).to_string();
    if code.is_empty() || code.as_bytes().len() > MAX_SOURCE_BYTES {
        return Err(expected("diagram_input_too_large"));
    }
    let code = if code.starts_with("@start") {
        code
    } else {
        format!("@startuml\n{code}\n@enduml")
    };
    let root = repo_root();
    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..]).current_dir(&root);
    let outcome = run_captured(&mut cmd, code.as_bytes(), clamp_timeout(timeout, 1.0, 30.0))
        .ok_or_else(|| expected("diagram_engine_timeout"))?;
    if !outcome.success {
        return Err(expected("diagram_render_failed"));
    }
    let svg = match String::from_utf8(outcome.stdout) {
        Ok(text) => text,
        Err(_) => return Err(DiagramError::Fatal),
    };
    let mut svg = py_strip(&svg).to_string();
    if let Some(pos) = svg.find("<svg") {
        svg = svg[pos..].to_string();
    }
    if !svg.starts_with("<svg") || svg.as_bytes().len() > MAX_SVG_BYTES {
        return Err(expected("diagram_render_failed"));
    }
    Ok(svg)
}

/// 从公网 PlantUML 服务器取回 SVG（= Python `fetch_plantuml_svg`）。
///
/// **调用方必须先通过 `allow_remote is True` 门**：本函数是模块内唯一会出网的函数，
/// 失败一律如实报错（`diagram_network_unavailable` / `diagram_render_failed`），绝不返回假图。
pub fn fetch_plantuml_svg(plantuml_code: &str, timeout: Duration) -> DiagramResult<String> {
    let url = get_plantuml_svg_url(plantuml_code, PLANTUML_SERVER_URL);
    let timeout = clamp_timeout(timeout, 5.0, 30.0);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout_write(timeout)
        .build();
    let response = match agent.get(&url).set("User-Agent", "ReadMD").call() {
        Ok(resp) => resp,
        // urllib 的 HTTPError（4xx/5xx）=> 渲染失败；连接/DNS/超时/协议问题 => 网络不可用。
        Err(ureq::Error::Status(_, _)) => return Err(expected("diagram_render_failed")),
        Err(_) => return Err(expected("diagram_network_unavailable")),
    };
    let mut data: Vec<u8> = Vec::new();
    match response
        .into_reader()
        .take((MAX_SVG_BYTES + 1) as u64)
        .read_to_end(&mut data)
    {
        Ok(_) => {}
        Err(_) => return Err(expected("diagram_network_unavailable")),
    }
    if data.len() > MAX_SVG_BYTES {
        return Err(expected("diagram_render_failed"));
    }
    // Python: data.decode('utf-8', errors='replace').strip()
    let mut svg = py_strip(&String::from_utf8_lossy(&data)).to_string();
    if let Some(pos) = svg.find("<svg") {
        svg = svg[pos..].to_string();
    }
    if !svg.starts_with("<svg") {
        return Err(expected("diagram_network_unavailable"));
    }
    // plantuml.com 解不出编码时会以 200 回一张错误图，不能当成渲染成功。
    if svg.contains("generated a bad URL") {
        return Err(expected("diagram_render_failed"));
    }
    Ok(svg)
}

// --------------------------------------------------------------------- tikz

/// 将 TikZ 代码片段包装为 TikZjax 标准 HTML 节点（= Python `format_tikz_html`）。
pub fn format_tikz_html(tikz_code: &str) -> String {
    // 用户内容里的字面 `</script` 会提前结束 script 元素，只转义斜杠保持 TeX 语义不变。
    let mut code = py_strip(tikz_code).replace("</script", "<\\/script");
    if !code.starts_with("\\begin{tikzpicture}") {
        code = format!("\\begin{{tikzpicture}}\n{code}\n\\end{{tikzpicture}}");
    }
    format!("<script type=\"text/tikz\">\n{code}\n</script>")
}

// --------------------------------------------------------------------- vega

/// 解析服务端渲染可用的 Node 运行时（= Python `_node_runtime`）。
///
/// 打包后的安装必须自带 node；开发检出（parity 差分所在）退回 `PATH` 上的 node，
/// 与 Python 的 `shutil.which("node")` 分支一致。
pub fn node_runtime(root: &Path) -> Option<PathBuf> {
    let executable = if cfg!(windows) { "node.exe" } else { "node" };
    let candidates = [
        root.join("assets").join("vendor").join("node").join(executable),
        root.join(executable),
    ];
    if let Some(hit) = candidates.iter().find(|path| path.is_file()) {
        return Some(hit.clone());
    }
    which("node")
}

/// Vega/Vega-Lite 离线渲染脚本：Python `render_vega_svg` 里 `node -e` 的常量脚本，逐字符复刻。
const VEGA_SCRIPT: &str = r#"
const fs = require('fs');
const vega = require(process.argv[1]);
const vegaLite = require(process.argv[2]);
const language = process.argv[3];
let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => raw += chunk);
process.stdin.on('end', async () => {
  try {
    const source = JSON.parse(raw);
    const compiled = language === 'vega-lite' ? vegaLite.compile(source).spec : source;
    const loader = (typeof vega.loader === 'function')
      ? vega.loader({ load: () => Promise.reject(new Error('external data loading blocked in offline mode')) })
      : undefined;
    const view = new vega.View(vega.parse(compiled), { renderer: 'none', loader });
    await view.runAsync();
    const svg = await view.toSVG();
    process.stdout.write(String(svg || ''));
  } catch (_) {
    process.exitCode = 2;
  }
});
"#;

fn vega_assets(root: &Path) -> (PathBuf, PathBuf) {
    let vendor = root.join("assets").join("vendor").join("diagrams");
    (
        vendor.join("vega").join("vega.min.js"),
        vendor.join("vega-lite").join("vega-lite.min.js"),
    )
}

/// 通过随包 Node 运行时渲染 Vega/Vega-Lite（= Python `render_vega_svg`）。
///
/// `spec_text` 已经是 Python `str(spec_text or "")` 之后的结果；语言非法、输入过大、
/// JSON 非法、依赖缺失分别对应各自的稳定码，全部 422。
pub fn render_vega_svg(spec_text: &str, language: &str, timeout: Duration) -> DiagramResult<String> {
    let normalized = py_strip(language).to_lowercase();
    if normalized != "vega" && normalized != "vega-lite" {
        return Err(expected("diagram_engine_invalid"));
    }
    let raw = py_strip(spec_text).to_string();
    if raw.is_empty() || raw.as_bytes().len() > MAX_SOURCE_BYTES {
        return Err(expected("diagram_input_too_large"));
    }
    let spec: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => return Err(expected("diagram_invalid_input")),
    };
    if !spec.is_object() {
        return Err(expected("diagram_invalid_input"));
    }

    let root = repo_root();
    let node = match node_runtime(&root) {
        Some(node) => node,
        None => return Err(expected("diagram_dependency_missing")),
    };
    let (vega_path, vega_lite_path) = vega_assets(&root);
    if !vega_path.is_file() || !vega_lite_path.is_file() {
        return Err(expected("diagram_dependency_missing"));
    }

    let mut cmd = Command::new(&node);
    cmd.arg("-e")
        .arg(VEGA_SCRIPT)
        .arg(&vega_path)
        .arg(&vega_lite_path)
        .arg(&normalized)
        .current_dir(&root);
    // spec 走 stdin，绝不拼进命令行；失败不回显任何主机细节。
    let input = match serde_json::to_vec(&spec) {
        Ok(bytes) => bytes,
        Err(_) => return Err(DiagramError::Fatal),
    };
    let outcome = run_captured(&mut cmd, &input, clamp_timeout(timeout, 1.0, 30.0))
        .ok_or_else(|| expected("diagram_engine_timeout"))?;
    if !outcome.success {
        return Err(expected("diagram_render_failed"));
    }
    let svg = match String::from_utf8(outcome.stdout) {
        Ok(text) => text,
        Err(_) => return Err(DiagramError::Fatal),
    };
    let svg = py_strip(&svg).to_string();
    if !svg.starts_with("<svg") || svg.as_bytes().len() > MAX_SVG_BYTES {
        return Err(expected("diagram_render_failed"));
    }
    Ok(svg)
}

// ------------------------------------------------------------- capabilities

/// 与 Python 相同的“仓库根 / 安装根”：`assets/` 的父目录。
pub fn repo_root() -> PathBuf {
    let assets = paths::assets_dir();
    if assets.ends_with("assets") {
        match assets.parent() {
            Some(root) if !root.as_os_str().is_empty() => return root.to_path_buf(),
            _ => {}
        }
    }
    assets
}

fn engine_entry(available: bool, renderer: &str, requires_network: bool) -> Value {
    json!({
        "available": available,
        "offline": available,
        "renderer": renderer,
        "requires_network": requires_network
    })
}

fn with_reason(entry: Value, reason: &str) -> Value {
    let mut obj = match entry {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    obj.insert("reason".into(), json!(reason));
    Value::Object(obj)
}

/// 只探测随包文件与可选本地进程，绝不联网、绝不执行用户图表源码
/// （= Python `get_diagram_capabilities`，返回 `{schema_version, offline, engines}`）。
pub fn diagram_capabilities(root: &Path) -> Value {
    let vendor = root.join("assets").join("vendor").join("diagrams");
    let browser_assets: [(&str, &str); 6] = [
        ("mermaid", "mermaid/mermaid.min.js"),
        ("wavedrom", "wavedrom/wavedrom.min.js"),
        ("bitfield", "bitfield/bitfield.min.js"),
        ("viz", "viz/viz-standalone.js"),
        ("tikz", "tikzjax/tikzjax.js"),
        ("chart", "chart/chart.umd.js"),
    ];
    let mut engines: Map<String, Value> = Map::new();
    for (engine, relative) in browser_assets {
        let available = join_exists(&vendor, relative);
        engines.insert(engine.into(), engine_entry(available, "browser", false));
    }
    // `chartjs` / `chart.js` 是 Markdown 分发器接受的别名，共用一条能力记录。
    if let Some(chart) = engines.get("chart").cloned() {
        engines.insert("chartjs".into(), chart.clone());
        engines.insert("chart.js".into(), chart);
    }

    let node_ok = node_runtime(root).is_some();
    let (vega_path, vega_lite_path) = vega_assets(root);
    let vega_ready = node_ok && vega_path.is_file() && vega_lite_path.is_file();
    for engine in ["vega", "vega-lite"] {
        let entry = engine_entry(vega_ready, "node", false);
        let reason = if vega_ready { "" } else { "diagram_dependency_missing" };
        engines.insert(engine.into(), with_reason(entry, reason));
    }

    let local_plantuml = has_local_plantuml();
    let plantuml = with_reason(
        engine_entry(local_plantuml, if local_plantuml { "java" } else { "remote" }, !local_plantuml),
        if local_plantuml { "" } else { "diagram_dependency_missing" },
    );
    let mut plantuml_obj = match plantuml {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    plantuml_obj.insert("remote_available".into(), json!(true));
    let plantuml = Value::Object(plantuml_obj);
    engines.insert("plantuml".into(), plantuml.clone());
    engines.insert("puml".into(), plantuml);

    // WebSequenceDiagrams 有意不映射到 PlantUML：两种语言不可互换，给明确的不可用项。
    for engine in ["wsd", "d2"] {
        let entry = engine_entry(false, "none", false);
        engines.insert(engine.into(), with_reason(entry, "diagram_engine_unavailable"));
    }

    let all_offline = engines
        .values()
        .filter(|entry| entry.get("available").and_then(Value::as_bool).unwrap_or(false))
        .all(|entry| entry.get("offline").and_then(Value::as_bool).unwrap_or(false));
    json!({ "schema_version": 1, "offline": all_offline, "engines": engines })
}

/// `/api/diagram/capabilities` 的完整响应体（Python 为 `{'ok': True, **capabilities}`）。
pub fn capabilities_payload() -> Value {
    let caps = diagram_capabilities(&repo_root());
    let mut payload = Map::new();
    payload.insert("ok".into(), json!(true));
    if let Value::Object(map) = caps {
        for (key, value) in map {
            payload.insert(key, value);
        }
    }
    Value::Object(payload)
}

fn join_exists(base: &Path, relative: &str) -> bool {
    let mut path = base.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    path.is_file()
}

// --------------------------------------------------------------- subprocess

/// Python 的 `max(lower, min(float(timeout), upper))`。
fn clamp_timeout(timeout: Duration, lower: f32, upper: f32) -> Duration {
    let secs = timeout.as_secs_f32().min(upper).max(lower);
    Duration::from_secs_f32(secs)
}

struct RunOutcome {
    success: bool,
    stdout: Vec<u8>,
}

/// `subprocess.run(input=..., stdout=PIPE, stderr=DEVNULL, timeout=...)` 的等价物。
///
/// 返回 `None` 表示 Python 会落到 `except (OSError, subprocess.TimeoutExpired)`：
/// 调用方统一映射为 `diagram_engine_timeout`。
fn run_captured(cmd: &mut Command, input: &[u8], timeout: Duration) -> Option<RunOutcome> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdin = child.stdin.take()?;
    let mut stdout: ChildStdout = child.stdout.take()?;

    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        // Python 的 subprocess.run 不截断 stdout：体积门限在解码之后按字符串判定。
        let ok = stdout.read_to_end(&mut buf).is_ok();
        let _ = tx.send((ok, buf));
    });
    let payload: Vec<u8> = input.to_vec();
    let writer = thread::spawn(move || {
        use std::io::Write;
        let mut stdin = stdin;
        let _ = stdin.write_all(&payload);
        let _ = stdin.flush();
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Python 超时路径：`process.kill()` 随后 `process.wait()`；
                    // `Child::kill` 已经是“终止并回收”的稳态等价物。
                    let _ = child.kill();
                    return None;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    };
    let _ = writer.join();
    let (read_ok, stdout_bytes) = rx.recv().ok()?;
    if reader.join().is_err() || !read_ok {
        return None;
    }
    Some(RunOutcome { success: status.success(), stdout: stdout_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_strip_matches_python_whitespace_set() {
        assert_eq!(py_strip("  \u{1c}\t x \n"), "x");
        assert_eq!(py_strip("a"), "a");
        assert_eq!(py_strip("   "), "");
    }

    #[test]
    fn plantuml_encode_is_url_alphabet_only() {
        for sample in ["", "hello", "@startuml\nAlice -> Bob\n@enduml"] {
            let encoded = plantuml_encode(sample);
            assert!(
                encoded.chars().all(|c| PLANTUML_CHARS.contains(&(c as u8))),
                "non alphabet char in {encoded:?}"
            );
        }
    }

    #[test]
    fn puml_url_has_no_prefix_and_wraps_bare_source() {
        let url = get_plantuml_svg_url("Alice -> Bob", PLANTUML_SERVER_URL);
        assert!(url.starts_with("https://www.plantuml.com/plantuml/svg/"), "{url}");
        assert!(!url.contains("~1"));
        // 裸源码与 @startuml 包裹后的源码必须得到同一个 URL。
        assert_eq!(url, get_plantuml_svg_url("@startuml\nAlice -> Bob\n@enduml", PLANTUML_SERVER_URL));
    }

    #[test]
    fn tikz_html_wraps_and_escapes_script_end() {
        assert_eq!(
            format_tikz_html("a</script>b"),
            "<script type=\"text/tikz\">\n\\begin{tikzpicture}\na<\\/script>b\n\\end{tikzpicture}\n</script>"
        );
    }

    #[test]
    fn vega_rejects_before_touching_the_runtime() {
        // 语言名非法：Python 在任何 I/O 之前就 diagram_engine_invalid。
        assert_eq!(
            render_vega_svg("{}", "vega-lite2", VEGA_TIMEOUT).unwrap_err(),
            expected("diagram_engine_invalid")
        );
        assert_eq!(
            render_vega_svg("   ", "vega", VEGA_TIMEOUT).unwrap_err(),
            expected("diagram_input_too_large")
        );
        assert_eq!(
            render_vega_svg("x".repeat(3 * 1024 * 1024).as_str(), "vega", VEGA_TIMEOUT).unwrap_err(),
            expected("diagram_input_too_large")
        );
        for bad in ["{", "[]", "3", "\"x\"", "null", "marks: []"] {
            assert_eq!(
                render_vega_svg(bad, "vega", VEGA_TIMEOUT).unwrap_err(),
                expected("diagram_invalid_input"),
                "input {bad:?}"
            );
        }
    }

    #[test]
    fn capabilities_reports_python_engine_set() {
        let payload = capabilities_payload();
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["schema_version"], json!(1));
        assert!(payload["offline"].is_boolean());
        let engines = payload["engines"].as_object().expect("engines object");
        let mut names: Vec<&str> = engines.keys().map(|s| s.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "bitfield",
                "chart",
                "chart.js",
                "chartjs",
                "d2",
                "mermaid",
                "plantuml",
                "puml",
                "tikz",
                "vega",
                "vega-lite",
                "viz",
                "wavedrom",
                "wsd",
            ]
        );
        // 离线不可用项必须给出与 Python 一致的 reason。
        for name in ["wsd", "d2"] {
            assert_eq!(engines[name]["available"], json!(false));
            assert_eq!(engines[name]["offline"], json!(false));
            assert_eq!(engines[name]["renderer"], json!("none"));
            assert_eq!(engines[name]["requires_network"], json!(false));
            assert_eq!(engines[name]["reason"], json!("diagram_engine_unavailable"));
        }
        let plantuml = &engines["plantuml"];
        assert_eq!(plantuml["remote_available"], json!(true));
        let local = has_local_plantuml();
        assert_eq!(plantuml["available"], json!(local));
        assert_eq!(plantuml["requires_network"], json!(!local));
        assert_eq!(
            plantuml["reason"],
            json!(if local { "" } else { "diagram_dependency_missing" })
        );
    }

    #[test]
    fn probe_only_reads_files() {
        // has_local_plantuml 只允许 PATH/文件探测：本机没有 plantuml 时必须为 false 且不起进程。
        assert_eq!(which("definitely-not-a-real-binary-xyz"), None);
        let _ = has_local_plantuml();
        let root = repo_root();
        assert!(root.join("assets").join("vendor").join("diagrams").is_dir(), "{root:?}");
    }
}
