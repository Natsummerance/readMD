//! ReadMD 安全交互式多语言代码块执行器 (Polyglot Safe Code Chunk Runner)。
//!
//! 这是 `src/readmd_modules/code_chunk_runner.py` 的逐行为移植（Rust 原生，
//! 不依赖 Python 运行时）。语义以 Python 版为唯一权威：
//!
//! 1. Python (`python`, `py`)：调度系统上已存在的解释器执行用户代码块，
//!    并保留 Matplotlib 图像自动捕获与 Base64 回填；
//! 2. JavaScript (`javascript`, `js`, `node`)：Node.js 运行时；
//! 3. Shell (`bash`, `sh`, `shell`, `powershell`, `cmd`, `bat`)：系统终端；
//! 4. R (`r`, `rscript`)、Go (`go`, `golang`)、Rust (`rust`, `rs`)、
//!    C/C++ (`c`, `cpp`, `c++`)：已安装的编译器/脚本器；
//! 5. SQL (`sql`, `sqlite`, `sqlite3`)：内核内置 SQLite 内存库执行。
//!
//! 安全防线以 Python 版为唯一权威，逐条由本文件底部 `mod tests` 的 CPython
//! 差分金表把关（拒绝决策 93 例、SQL 渲染 29 例、语言归一化 8 例、cwd 闸门、
//! 输出捕获与超时）：
//! - `_SAFE_ENV_KEYS` 白名单环境（子进程不继承凭据/代理/API key）；
//! - 网络与路径逃逸静态正则拒绝（`network_not_allowed` / `path_access_not_allowed`）；
//! - `cwd` 必须落在 `READMD_DATA_DIR` 或系统临时目录内；
//! - 超时强杀（上限 10 秒，Windows 走进程树 `taskkill /T /F`）；
//! - 输出按字符截断到 200_000 并给出 `output_truncated` 警告。
//!
//! 与 Python 的已知实现差异全部记录在交付报告的「未解决」小节。

use std::env;
use std::ffi::OsString;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lazy_static::lazy_static;
use regex::Regex;
use serde_json::{Map, Value};
use tempfile::TempDir;

/// 与 Python 侧同名常量的权威取值。
pub const EXECUTION_TIMEOUT: u64 = 10;
pub const MAX_OUTPUT_CHARS: usize = 200_000;
pub const MAX_TIMEOUT_SECONDS: u64 = 10;
pub const MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_CHILD_PROCESSES: u32 = 32;
pub const MAX_SQL_ROWS: usize = 5000;
pub const MAX_CELL_CHARS: usize = 500;

/// 子进程只能拿到这些变量：API key、代理与任意用户变量被刻意排除。
const SAFE_ENV_KEYS: [&str; 12] = [
    "PATH", "PATHEXT", "SYSTEMROOT", "SYSTEMDRIVE", "COMSPEC", "TEMP", "TMP", "TMPDIR",
    "USERPROFILE", "HOME", "LANG", "LC_ALL",
];

/// Python 的 `\s`（编译自 `str` 模式）比 Rust 多 U+001C..U+001F。防线里的每个
/// `\s` 都要显式补齐，否则 `"/etc\x1cpasswd"`、`\x1c../win`、`require\x1c("http")`
/// 这类源码能在 Rust 侧绕过网络/路径防线。
const PY_S: &str = r"[\s\x1c-\x1f]";

lazy_static! {
    /// `_NETWORK_PATTERNS`：作用于「去掉字面量与注释之后」的源码。
    static ref NETWORK_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)\b(?:requests|httpx|urllib(?:\.request)?|socket|ftplib|aiohttp)\b").unwrap(),
        Regex::new(&format!(
            r#"(?ix)(?:require{S}*\({S}*['"](?:node:)?(?:http|https|net|tls|dns|dgram|undici)['"]|from{S}+['"](?:node:)?(?:http|https|net|tls|dns|dgram|undici)['"]|\bfetch{S}*\()"#,
            S = PY_S
        ))
        .unwrap(),
        Regex::new(r"(?i)\b(?:curl|wget|Invoke-WebRequest|Invoke-RestMethod|nc|netcat|ping|nslookup|dig|tracert|netsh)\b").unwrap(),
        Regex::new(r"(?i)\bhttps?://").unwrap(),
    ];

    /// `_PATH_ESCAPE_PATTERNS`：作用于**原始**源码（Python 亦然）。
    static ref PATH_ESCAPE_PATTERNS: Vec<Regex> = vec![
        // Python 用 (?<![A-Za-z0-9_])；regex crate 不支持回看，用等价的「^ 或非单词字符」消费式写法。
        // 权威里的 `[^\\s]` 是「非反斜杠且非 s」，不是「非空白」——照抄，不得自行修正。
        Regex::new(r"(?i)(?:(?:^|[^A-Za-z0-9_])[A-Za-z]:[\\/]|\\\\[^\\s]+)").unwrap(),
        Regex::new(&format!(
            r#"(?ix)(?:^|["'{S}])/(?:etc|home|root|tmp|var|usr|opt|workspace|mnt|proc|sys)(?:[/{S}"']|$)"#,
            S = PY_S
        ))
        .unwrap(),
        Regex::new(&format!(r#"(?i)(?:^|["'{S}])\.\.[\\/]"#, S = PY_S)).unwrap(),
        Regex::new(r"(?ix)\b(?:__import__|importlib|pathlib|open|io\.open|os\.(?:chdir|listdir|walk|scandir|remove|unlink|rename|replace|makedirs|mkdir|rmdir|system|popen|exec|spawn)|shutil\.|subprocess\.|ctypes\.|winreg\.|tempfile\.)").unwrap(),
        Regex::new(r"(?ix)\bos\.environ(?:\b|\[)").unwrap(),
        Regex::new(&format!(
            r#"(?ix)\b(?:require{S}*\({S}*["'](?:node:)?(?:fs|fs/promises|child_process|module)["']|from{S}+["'](?:node:)?(?:fs|fs/promises|child_process|module)["']|\bprocess\.(?:binding|dlopen|env|exec|spawn)|\b(?:Deno|Bun)\.)"#,
            S = PY_S
        ))
        .unwrap(),
        Regex::new(&format!(
            r"(?ix)\b(?:type|copy|xcopy|move|del|erase|dir|cat|cp|mv|rm|rmdir|find|grep|dd){S}+[^\n]*[/\\.]",
            S = PY_S
        ))
        .unwrap(),
    ];

    /// Matplotlib 图像回填标记。
    static ref PLOT_MARKER: Regex =
        Regex::new(r"__READMD_PLOT_BASE64__([A-Za-z0-9+/=]+)__END_READMD_PLOT__").unwrap();

    /// `_strip_literals_and_comments` 的非 Python（以及 tokenize 失败兜底）分支。
    static ref GENERIC_LITERALS: Regex = Regex::new(
        r#"/\*[\s\S]*?\*/|//[^\n]*|#[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`"#
    )
    .unwrap();
}

/// Matplotlib 图表捕获包装模板（与 Python 的 `MATPLOTLIB_WRAPPER` 逐字一致）。
const MATPLOTLIB_WRAPPER: &str = r#"
import sys
import io
import base64

# 用户源码开始
{user_code}
# 用户源码结束

try:
    if 'matplotlib.pyplot' in sys.modules or 'plt' in locals() or 'plt' in globals():
        import matplotlib.pyplot as plt
        figs = [plt.figure(n) for n in plt.get_fignums()]
        for idx, fig in enumerate(figs):
            buf = io.BytesIO()
            fig.savefig(buf, format='png', bbox_inches='tight', dpi=150)
            buf.seek(0)
            b64_img = base64.b64encode(buf.read()).decode('ascii')
            print(f"__READMD_PLOT_BASE64__{b64_img}__END_READMD_PLOT__")
            plt.close(fig)
except Exception as _e:
    pass
"#;

// ------------------------------------------------------------------ dict 辅助

#[derive(Default)]
struct Dict(Map<String, Value>);

impl Dict {
    fn set(mut self, key: &str, value: Value) -> Self {
        self.0.insert(key.to_string(), value);
        self
    }
    fn text(self, key: &str, value: &str) -> Self {
        self.set(key, Value::String(value.to_string()))
    }
    fn build(self) -> Map<String, Value> {
        self.0
    }
}

fn empty_images() -> Value {
    Value::Array(Vec::new())
}

/// 结果字典里 `ok=false` 的通用「异常」形状（对应 `_run_process` 的 except 分支）。
fn exception_dict(message: &str) -> Map<String, Value> {
    Dict::default()
        .set("ok", Value::Bool(false))
        .text("error", message)
        .text("stdout", "")
        .text("stderr", message)
        .set("images", empty_images())
        .set("exit_code", Value::from(-1i64))
        .build()
}

/// `{'ok': False, 'error': reason, 'stdout': '', 'stderr': reason, 'images': [],
/// 'exit_code': 1, 'lang': str(lang or 'python')}` —— 拒绝路径的公共形状。
fn denied_dict(reason: &str, raw_lang: &str) -> Map<String, Value> {
    Dict::default()
        .set("ok", Value::Bool(false))
        .text("error", reason)
        .text("stdout", "")
        .text("stderr", reason)
        .set("images", empty_images())
        .set("exit_code", Value::from(1i64))
        .text("lang", lang_or_python(raw_lang))
        .build()
}

/// `str(lang or "python")` —— 拒绝分支用的是原始字符串，不做规范化。
fn lang_or_python(lang: &str) -> &str {
    if lang.is_empty() {
        "python"
    } else {
        lang
    }
}

/// Python `str.strip()` 的空白集：Unicode White_Space **外加** U+001C..U+001F
/// （file/group/record/unit separator）。Rust 的 `char::is_whitespace` 用的是
/// White_Space 属性，不含这四个，直接 `.trim()` 会把 Python 认为空白的字符留下。
fn is_py_space(c: char) -> bool {
    matches!(c, '\u{1c}'..='\u{1f}') || c.is_whitespace()
}

fn py_trim(text: &str) -> String {
    text.trim_start_matches(is_py_space)
        .trim_end_matches(is_py_space)
        .to_string()
}

/// `_strip_literals_and_comments` 里的 `str(lang or "python").lower().strip().lstrip('.')`。
fn normalize_lang_or_python(lang: &str) -> String {
    py_trim(&lang_or_python(lang).to_lowercase())
        .trim_start_matches('.')
        .to_string()
}

/// `_execute_code_chunk` 的 `lang.lower().strip().lstrip('.')`（无 or-python 兜底）。
fn normalize_lang(lang: &str) -> String {
    py_trim(&lang.to_lowercase()).trim_start_matches('.').to_string()
}

fn clamp_timeout(timeout: u64) -> u64 {
    // max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS))
    let base = if timeout == 0 { EXECUTION_TIMEOUT } else { timeout };
    base.min(MAX_TIMEOUT_SECONDS).max(1)
}

// -------------------------------------------------------------- 源码净化

/// 对应 `_strip_literals_and_comments`。Python 走真分词器，失败（语法非法）时
/// 退回通用正则——这条兜底路径必须保留，否则拒绝集合会偏离 Python。
pub fn strip_literals_and_comments(source: &str, lang: &str) -> String {
    let norm = normalize_lang_or_python(lang);
    if norm == "python" || norm == "py" {
        if let Some(cleaned) = python_tokenize_join(source) {
            return cleaned;
        }
    }
    GENERIC_LITERALS
        .replace_all(source, |caps: &regex::Captures| {
            let hit = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            "\n".repeat(hit.matches('\n').count())
        })
        .into_owned()
}

/// Python `tokenize` 的等价净化：字符串（含前缀、三引号、续行）与注释被替换为
/// 其中包含的换行符，其余按原样保留，token 之间以单个空格拼接。
/// 返回 `None` 表示 Python 的 tokenize 会抛错（未闭合字符串等）→ 调用方退回正则。
fn python_tokenize_join(source: &str) -> Option<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut run = String::new();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];
        if c == '#' {
            parts.push(std::mem::take(&mut run));
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            parts.push(String::new()); // COMMENT token 自身不含换行
            continue;
        }
        if c == '"' || c == '\'' {
            let raw = take_string_prefix(&mut run);
            let (next, newlines) = scan_python_string(&chars, i, raw)?;
            parts.push(std::mem::take(&mut run));
            parts.push("\n".repeat(newlines));
            i = next;
            continue;
        }
        run.push(c);
        i += 1;
    }
    parts.push(run);
    Some(parts.join(" "))
}

/// 从当前代码片段尾部摘掉字符串前缀（r/b/u/f 的一到两个字母组合），
/// 返回是否为 raw 字符串。前缀之前必须是非单词字符，否则会误伤标识符。
fn take_string_prefix(run: &mut String) -> bool {
    let chars: Vec<char> = run.chars().collect();
    let mut end = chars.len();
    while end > 0 && (chars[end - 1] == 'r' || chars[end - 1] == 'R') {
        end -= 1;
    }
    while end > 0 && (chars[end - 1] == 'b' || chars[end - 1] == 'B' || chars[end - 1] == 'u' || chars[end - 1] == 'U' || chars[end - 1] == 'f' || chars[end - 1] == 'F') {
        end -= 1;
    }
    if end == chars.len() {
        return false;
    }
    // 至少留下一个引号之前的字符：前缀必须紧跟引号且前面不是单词字符。
    let prefix: String = chars[end..].iter().collect();
    let head = chars[..end].iter().collect::<String>();
    let boundary_ok = head.chars().next_back().map(|c| !(c.is_alphanumeric() || c == '_')).unwrap_or(true);
    if !boundary_ok || prefix.is_empty() || prefix.len() > 2 {
        return false;
    }
    *run = head;
    prefix.to_ascii_lowercase().contains('r')
}

/// 扫描一个字符串字面量，返回 (结束后的下标, 内部换行数)。
fn scan_python_string(chars: &[char], start: usize, raw: bool) -> Option<(usize, usize)> {
    let quote = chars[start];
    let triple = start + 2 < chars.len() && chars[start + 1] == quote && chars[start + 2] == quote;
    let delim_len = if triple { 3 } else { 1 };
    let mut i = start + delim_len;
    let mut newlines = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if !raw && c == '\\' {
            if i + 1 < chars.len() {
                if chars[i + 1] == '\n' {
                    newlines += 1;
                }
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if raw && c == '\\' && i + 1 < chars.len() {
            if chars[i + 1] == '\n' {
                newlines += 1;
            }
            i += 2;
            continue;
        }
        if c == quote {
            if triple {
                if i + 2 < chars.len() && chars[i + 1] == quote && chars[i + 2] == quote {
                    return Some((i + 3, newlines));
                }
                i += 1;
                continue;
            }
            return Some((i + 1, newlines));
        }
        if c == '\n' {
            if !triple {
                return None; // EOL while scanning string literal → tokenize 抛错
            }
            newlines += 1;
        }
        i += 1;
    }
    None // EOF in multi-line string / unterminated literal
}

// ------------------------------------------------------------- 子进程执行

/// `_run_process` 的有界输出收集器。
#[derive(Default)]
struct PipeSink {
    text: String,
    chars: usize,
    truncated: bool,
    done: bool,
}

fn sink_push(sink: &mut PipeSink, chunk: &str) {
    if chunk.is_empty() {
        return;
    }
    if sink.chars < MAX_OUTPUT_CHARS {
        let rem = MAX_OUTPUT_CHARS - sink.chars;
        let count = chunk.chars().count();
        sink.text.extend(chunk.chars().take(rem));
        sink.chars += count.min(rem);
        if count > rem {
            sink.truncated = true;
        }
    } else {
        sink.truncated = true;
    }
}

/// UTF-8 增量解码 + `errors='replace'`（每个非法子序列替换为一个 U+FFFD）。
#[derive(Default)]
struct Utf8Replacer {
    pending: Vec<u8>,
}

impl Utf8Replacer {
    fn feed(&mut self, input: &[u8], out: &mut String) {
        let mut data = std::mem::take(&mut self.pending);
        data.extend_from_slice(input);
        let mut pos = 0usize;
        loop {
            let rest = &data[pos..];
            match std::str::from_utf8(rest) {
                Ok(valid) => {
                    out.push_str(valid);
                    self.pending.clear();
                    return;
                }
                Err(err) => {
                    let valid_up_to = err.valid_up_to();
                    if valid_up_to > 0 {
                        out.push_str(std::str::from_utf8(&rest[..valid_up_to]).unwrap_or_default());
                    }
                    let bad = &rest[valid_up_to..];
                    match err.error_len() {
                        Some(n) => {
                            out.push('\u{fffd}');
                            pos += valid_up_to + n.min(bad.len());
                        }
                        None => {
                            self.pending.extend_from_slice(bad);
                            return;
                        }
                    }
                }
            }
        }
    }

    fn finish(&mut self, out: &mut String) {
        if !self.pending.is_empty() {
            out.push('\u{fffd}');
            self.pending.clear();
        }
    }
}

/// Python 文本管道的 universal newlines：`\r\n` 与孤立 `\r` 都归一为 `\n`。
#[derive(Default)]
struct NewlineTranslator {
    pending_cr: bool,
}

impl NewlineTranslator {
    fn feed(&mut self, chunk: &str, out: &mut String) {
        for c in chunk.chars() {
            if self.pending_cr {
                self.pending_cr = false;
                if c == '\n' {
                    continue;
                }
            }
            if c == '\r' {
                self.pending_cr = true;
                out.push('\n');
            } else {
                out.push(c);
            }
        }
    }
}

fn pump_pipe<R: Read + Send + 'static>(mut reader: R, sink: Arc<Mutex<PipeSink>>) {
    let mut buf = [0u8; 4096];
    let mut decoder = Utf8Replacer::default();
    let mut newlines = NewlineTranslator::default();
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut decoded = String::new();
                decoder.feed(&buf[..n], &mut decoded);
                let mut translated = String::new();
                newlines.feed(&decoded, &mut translated);
                if let Ok(mut guard) = sink.lock() {
                    sink_push(&mut guard, &translated);
                }
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            // Python 的 _bounded_reader 用 `except Exception: pass` 吞掉读错误。
            Err(_) => break,
        }
    }
    let mut tail = String::new();
    decoder.finish(&mut tail);
    if let Ok(mut guard) = sink.lock() {
        sink_push(&mut guard, &tail);
        guard.done = true;
    }
}

fn wait_for_sinks(sinks: &[&Arc<Mutex<PipeSink>>], budget: Duration) {
    let deadline = Instant::now() + budget;
    loop {
        let finished = sinks.iter().all(|s| s.lock().map(|g| g.done).unwrap_or(true));
        if finished || Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn take_sink(sink: &Arc<Mutex<PipeSink>>) -> PipeSink {
    match sink.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => PipeSink::default(),
    }
}

fn command(program: &OsString) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP（后者对应 Python 的进程组语义）
        cmd.creation_flags(0x0800_0000 | 0x0000_0200);
    }
    cmd
}

/// Python 的子进程只拿到白名单变量（Windows 上 `os.environ` 的键为大写）。
fn child_env(runtime: Option<&str>) -> Vec<(String, String)> {
    let live: Vec<(String, String)> = env::vars().collect();
    let mut out: Vec<(String, String)> = Vec::with_capacity(SAFE_ENV_KEYS.len() + 3);
    for key in SAFE_ENV_KEYS.iter() {
        if let Some((_, value)) = live
            .iter()
            .find(|(name, value)| !value.is_empty() && name.eq_ignore_ascii_case(key))
        {
            out.push((key.to_string(), value.clone()));
        }
    }
    out.push(("PYTHONIOENCODING".to_string(), "utf-8".to_string()));
    out.push(("PYTHONUTF8".to_string(), "1".to_string()));
    let node_options = if runtime == Some("node") {
        "--no-warnings --max-old-space-size=128"
    } else {
        "--no-warnings"
    };
    out.push(("NODE_OPTIONS".to_string(), node_options.to_string()));
    out
}

/// 超时后的进程树清理：Windows 先 `taskkill /T /F`，再兜底 kill 直接子进程。
fn terminate_tree(child: &mut Child) {
    if child.id() != 0 {
        #[cfg(target_os = "windows")]
        {
            let pid = child.id().to_string();
            let _ = crate::silent_command("taskkill")
                .args(["/T", "/F", "/PID", pid.as_str()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut killer| killer.wait());
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = child.kill();
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// 底层安全进程调用与 UTF-8 管道捕获（有界流式内存保护与全路径截断）。
fn run_process(cmd: &[OsString], cwd: Option<&Path>, timeout: u64, runtime: Option<&str>) -> Map<String, Value> {
    if cmd.is_empty() {
        return exception_dict("invalid_command");
    }
    let timeout = clamp_timeout(timeout);

    let mut spawned = command(&cmd[0]);
    spawned
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(child_env(runtime));
    if let Some(dir) = cwd {
        spawned.current_dir(dir);
    }

    let mut child = match spawned.spawn() {
        Ok(child) => child,
        Err(e) => return exception_dict(&format!("FileNotFoundError: {e}")),
    };

    let stdout_sink = Arc::new(Mutex::new(PipeSink::default()));
    let stderr_sink = Arc::new(Mutex::new(PipeSink::default()));
    if let Some(pipe) = child.stdout.take() {
        let sink = Arc::clone(&stdout_sink);
        let _ = std::thread::Builder::new()
            .name("readmd-code-stdout".into())
            .spawn(move || pump_pipe(pipe, sink));
    } else if let Ok(mut guard) = stdout_sink.lock() {
        guard.done = true;
    }
    if let Some(pipe) = child.stderr.take() {
        let sink = Arc::clone(&stderr_sink);
        let _ = std::thread::Builder::new()
            .name("readmd-code-stderr".into())
            .spawn(move || pump_pipe(pipe, sink));
    } else if let Ok(mut guard) = stderr_sink.lock() {
        guard.done = true;
    }

    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut timed_out = false;
    let mut exit_code: i64 = -1;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code().unwrap_or(-1) as i64;
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    terminate_tree(&mut child);
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                // Python: `except Exception: exit_code = -1`，仍然继续回收进程树。
                terminate_tree(&mut child);
                break;
            }
        }
    }

    wait_for_sinks(&[&stdout_sink, &stderr_sink], Duration::from_secs(1));
    let stdout = take_sink(&stdout_sink);
    let stderr = take_sink(&stderr_sink);

    if timed_out {
        return Dict::default()
            .set("ok", Value::Bool(false))
            .text("error_code", "execution_timeout")
            .text("error", &format!("代码执行超时 (超过 {timeout} 秒限制)"))
            .text("stdout", &bounded_strip(&stdout.text))
            .text("stderr", &bounded_strip(&stderr.text))
            .set("images", empty_images())
            .set("exit_code", Value::from(-1i64))
            .build();
    }

    let truncated = stdout.truncated
        || stderr.truncated
        || stdout.chars > MAX_OUTPUT_CHARS
        || stderr.chars > MAX_OUTPUT_CHARS;
    Dict::default()
        .set("ok", Value::Bool(exit_code == 0))
        .text("stdout", &bounded_strip(&stdout.text))
        .text("stderr", &bounded_strip(&stderr.text))
        .set("images", empty_images())
        .set("exit_code", Value::from(exit_code))
        .set(
            "warning",
            if truncated {
                Value::String("output_truncated".to_string())
            } else {
                Value::Null
            },
        )
        .build()
}

fn bounded_strip(text: &str) -> String {
    // Python: `text[:MAX_OUTPUT_CHARS].strip()` —— 先按字符截断，再按 Python 空白集剥边。
    py_trim(&text.chars().take(MAX_OUTPUT_CHARS).collect::<String>())
}

// ------------------------------------------------------------- 临时脚本

fn write_temp_script(suffix: &str, content: &str, cwd: Option<&Path>) -> std::io::Result<(PathBuf, TempDir)> {
    let base = cwd.map(|p| p.to_path_buf()).unwrap_or_else(env::temp_dir);
    let dir = tempfile::Builder::new()
        .prefix("readmd-script-")
        .tempdir_in(&base)
        .or_else(|_| tempfile::Builder::new().prefix("readmd-script-").tempdir())?;
    let path = dir.path().join(format!("main{suffix}"));
    fs_write(&path, content)?;
    Ok((path, dir))
}

fn fs_write(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content.as_bytes())
}

// ------------------------------------------------------------------ 调度

/// `shutil.which`：在进程 PATH 中按 PATHEXT 查找可执行文件。
fn which(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(target_os = "windows") {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_string())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn os(text: &str) -> OsString {
    OsString::from(text)
}

/// Python 侧用的是 `sys.executable`；Rust 内核没有宿主解释器，只能按 PATH 探测
/// 系统上已存在的解释器（这仍属于「调用已存在的解释器执行用户代码」）。
fn python_program() -> Option<OsString> {
    ["python", "python3", "py"]
        .iter()
        .find_map(|name| which(name))
        .map(|p| p.into_os_string())
}

fn set_lang(res: &mut Map<String, Value>, lang: &str) {
    res.insert("lang".to_string(), Value::String(lang.to_string()));
}

/// `execute_python_chunk`：捕获 Matplotlib 图像并回填 Base64。
pub fn execute_python_chunk(
    code: &str,
    capture_plot: bool,
    timeout: u64,
    cwd: Option<&Path>,
) -> Map<String, Value> {
    let wrapped = if capture_plot {
        MATPLOTLIB_WRAPPER.replace("{user_code}", code)
    } else {
        code.to_string()
    };

    let (script_path, _script_dir) = match write_temp_script(".py", &wrapped, cwd) {
        Ok(pair) => pair,
        Err(e) => return exception_dict(&format!("OSError: {e}")),
    };
    let interpreter = match python_program() {
        Some(program) => program,
        None => OsString::from("python"),
    };
    let cmd = vec![interpreter, script_path.into_os_string()];
    let mut res = run_process(&cmd, cwd, timeout, None);

    let errored = res
        .get("error")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if !is_truthy(&res) && errored {
        return res;
    }

    let mut images: Vec<Value> = Vec::new();
    let stdout = res.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let clean = PLOT_MARKER
        .replace_all(&stdout, |caps: &regex::Captures| {
            images.push(Value::String(format!(
                "data:image/png;base64,{}",
                caps.get(1).map(|m| m.as_str()).unwrap_or("")
            )));
            ""
        });
    let clean = py_trim(&clean);
    res.insert("stdout".to_string(), Value::String(clean));
    res.insert("images".to_string(), Value::Array(images));
    res
}

fn is_truthy(res: &Map<String, Value>) -> bool {
    match res.get("ok") {
        Some(Value::Bool(b)) => *b,
        Some(Value::Null) | None => false,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(other) => !other.is_null(),
    }
}

fn get_str(res: &Map<String, Value>, key: &str) -> String {
    res.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn insert_str(res: &mut Map<String, Value>, key: &str, value: &str) {
    res.insert(key.to_string(), Value::String(value.to_string()));
}

/// `_execute_sql_chunk` 的 Rust 原生实现：内置 SQLite 内存库，
/// 复刻 Python 的行/字符双重有界保护与 `format_cell` 渲染。
pub fn execute_sql_chunk(code: &str, _cwd: Option<&Path>, timeout: u64) -> Map<String, Value> {
    let timeout = clamp_timeout(timeout);
    let script = code.to_string();
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("readmd-sql".into())
        .spawn(move || {
            let _ = tx.send(run_sql_statements(&script));
        });

    let outcome = match rx.recv_timeout(Duration::from_secs(timeout)) {
        Ok(outcome) => outcome,
        Err(_) => {
            // 对应「子进程被强杀」：error_code 由 _run_process 给出，
            // _execute_sql_chunk 再把诊断文本换成 SQL 版本。
            let mut res = Dict::default()
                .set("ok", Value::Bool(false))
                .text("error_code", "execution_timeout")
                .text("error", &format!("SQL 执行超时 (超过 {timeout} 秒限制)"))
                .text("stdout", "")
                .text("stderr", "")
                .set("images", empty_images())
                .set("exit_code", Value::from(-1i64))
                .build();
            set_lang(&mut res, "sql");
            return res;
        }
    };

    let mut res = match outcome {
        Ok((out_text, any_truncated)) => {
            let stdout = bounded_strip(&out_text);
            let marker = if any_truncated || out_text.chars().count() > MAX_OUTPUT_CHARS {
                "__READMD_SQL_TRUNCATED__".to_string()
            } else {
                String::new()
            };
            Dict::default()
                .set("ok", Value::Bool(true))
                .text("stdout", &stdout)
                .text("stderr", &marker)
                .set("images", empty_images())
                .set("exit_code", Value::from(0i64))
                .set("warning", Value::Null)
                .build()
        }
        Err(message) => Dict::default()
            .set("ok", Value::Bool(false))
            .text("stdout", "")
            .text("stderr", &message)
            .set("images", empty_images())
            .set("exit_code", Value::from(1i64))
            .set("warning", Value::Null)
            .build(),
    };
    set_lang(&mut res, "sql");

    if !is_truthy(&res) {
        if get_str(&res, "error_code") == "execution_timeout" {
            insert_str(&mut res, "error", &format!("SQL 执行超时 (超过 {timeout} 秒限制)"));
        } else if get_str(&res, "error").is_empty() {
            let stderr = get_str(&res, "stderr");
            insert_str(
                &mut res,
                "error",
                &format!("SQL 执行错误: {}", if stderr.is_empty() { "Unknown error" } else { &stderr }),
            );
        }
        return res;
    }

    let stderr = get_str(&res, "stderr");
    let truncated = stderr.contains("__READMD_SQL_TRUNCATED__")
        || get_str(&res, "warning") == "output_truncated";
    if stderr.contains("__READMD_SQL_TRUNCATED__") {
        insert_str(&mut res, "stderr", &py_trim(&stderr.replace("__READMD_SQL_TRUNCATED__", "")));
    }
    if truncated {
        insert_str(&mut res, "warning", "output_truncated");
    }
    res
}

enum SqlStatementOutcome {
    Rows(String, bool),
    Affected(String),
}

/// 子进程版的 `run_sql()`：返回 (stdout 文本, 是否有行截断) 或错误消息。
fn run_sql_statements(code: &str) -> Result<(String, bool), String> {
    let conn = match rusqlite::Connection::open_in_memory() {
        Ok(conn) => conn,
        Err(e) => return Err(sqlite_error_text(&e)),
    };
    let statements = split_sql_statements(code);
    let mut results: Vec<String> = Vec::new();
    let mut any_truncated = false;

    for stmt in &statements {
        if py_trim(stmt).is_empty() {
            continue;
        }
        match run_single_statement(&conn, stmt) {
            Ok(SqlStatementOutcome::Rows(table, truncated)) => {
                any_truncated |= truncated;
                results.push(table);
            }
            Ok(SqlStatementOutcome::Affected(msg)) => results.push(msg),
            Err(e) => return Err(e),
        }
    }

    let out_text = results.join("\n\n");
    Ok((out_text, any_truncated))
}

fn run_single_statement(conn: &rusqlite::Connection, stmt: &str) -> Result<SqlStatementOutcome, String> {
    // SQLite 只在「输入里没有 SQL（空串或纯注释）」时把 prepare 的 pStmt 置 NULL，
    // 此时 CPython 的 `cur.execute("-- 注释")` 是一条都不执行的空操作：
    // `cur.description is None`、`cur.rowcount == -1`，子进程打印
    // "Query OK, -1 rows affected."（实测见 python 3.11.15 / sqlite 3.53.1）。
    // rusqlite 会带着空 RawStatement 返回 Ok，后续任何 sqlite3_* 调用都是
    // SQLITE_MISUSE（"bad parameter or other API misuse"），所以在这里短路。
    if sql_is_comment_only(stmt) {
        return Ok(SqlStatementOutcome::Affected(
            "Query OK, -1 rows affected.".to_string(),
        ));
    }

    let mut prepared = conn.prepare(stmt).map_err(|e| sqlite_error_text(&e))?;
    if prepared.column_count() == 0 {
        let changes = prepared.execute([]).map_err(|e| sqlite_error_text(&e))?;
        let rowcount = if is_dml(stmt) { changes as i64 } else { -1 };
        return Ok(SqlStatementOutcome::Affected(format!(
            "Query OK, {rowcount} rows affected."
        )));
    }

    let headers: Vec<String> = prepared.column_names().iter().map(|n| n.to_string()).collect();
    let mut rows_out: Vec<Vec<String>> = Vec::new();
    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    let mut total_row_chars = 0usize;
    let mut truncated_rows = false;

    let mut rows = prepared.query([]).map_err(|e| sqlite_error_text(&e))?;
    while let Some(row) = rows.next().map_err(|e| sqlite_error_text(&e))? {
        let mut formatted = Vec::with_capacity(headers.len());
        for idx in 0..headers.len() {
            let cell = match row.get_ref(idx) {
                Ok(value) => format_cell(&value),
                Err(_) => "NULL".to_string(),
            };
            if cell.chars().count() > col_widths[idx] {
                col_widths[idx] = cell.chars().count();
            }
            formatted.push(cell);
        }
        total_row_chars += formatted.iter().map(|c| c.chars().count()).sum::<usize>() + formatted.len() * 3;
        rows_out.push(formatted);
        if rows_out.len() >= MAX_SQL_ROWS || total_row_chars >= MAX_OUTPUT_CHARS {
            truncated_rows = true;
            break;
        }
    }

    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, h)| ljust(h, col_widths[i]))
        .collect::<Vec<_>>()
        .join(" | ");
    let sep_line = (0..headers.len())
        .map(|i| "-".repeat(col_widths[i]))
        .collect::<Vec<_>>()
        .join("-+-");
    let mut table = format!("{header_line}\n{sep_line}\n");
    table.push_str(
        &rows_out
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(i, v)| ljust(v, col_widths[i]))
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    if truncated_rows {
        table.push_str(&format!("\n[Output truncated at {} rows]", rows_out.len()));
    }
    Ok(SqlStatementOutcome::Rows(table, truncated_rows))
}

fn is_dml(stmt: &str) -> bool {
    let head = stmt
        .trim_start()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(head.as_str(), "INSERT" | "UPDATE" | "DELETE" | "REPLACE")
}

/// CPython 侧 `str(sqlite3.OperationalError)` 就是 sqlite3_errmsg 的原文
/// （例：`near "SELEC": syntax error`）。rusqlite 的 `Error::SqlInputError`
/// 会在 Display 时追加自己的 ` in <sql> at offset <n>`，那是 rusqlite 的
/// 诊断格式而不是 SQLite 的文本，必须取它的 `msg` 字段才能逐字对齐。
fn sqlite_error_text(err: &rusqlite::Error) -> String {
    match err {
        rusqlite::Error::SqlInputError { msg, .. } => msg.clone(),
        rusqlite::Error::SqliteFailure(_, Some(msg)) => msg.clone(),
        other => other.to_string(),
    }
}

/// 语句里是否只有空白与注释（SQLite 语义）。判定用的空白集与 SQLite 分词器的
/// `sqlite3Isspace` 一致（仅 ASCII），因为 CPython 侧能走到这里的语句都已经
/// `.strip()` 过，纯 Unicode 空白的语句根本不会成为一条语句。
fn sql_is_comment_only(stmt: &str) -> bool {
    let chars: Vec<char> = stmt.chars().collect();
    let (mut in_line, mut in_block) = (false, false);
    let mut i = 0usize;
    let n = chars.len();
    while i < n {
        let ch = chars[i];
        let has_nxt = i + 1 < n;
        let nxt = if has_nxt { chars[i + 1] } else { '\0' };
        if in_line {
            if ch == '\n' {
                in_line = false;
            }
            i += 1;
            continue;
        }
        if in_block {
            if ch == '*' && has_nxt && nxt == '/' {
                in_block = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if ch == '-' && has_nxt && nxt == '-' {
            in_line = true;
            i += 2;
            continue;
        }
        if ch == '/' && has_nxt && nxt == '*' {
            in_block = true;
            i += 2;
            continue;
        }
        // 引号与任何非空白字符都是可执行内容：单独一条字符串字面量在
        // CPython 里报 `near "'--x'": syntax error`，不能被当成注释。
        if !matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}') {
            return false;
        }
        i += 1;
    }
    true
}

fn ljust(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_string();
    }
    let mut out = text.to_string();
    out.extend(std::iter::repeat(' ').take(width - len));
    out
}

fn format_cell(value: &rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => py_str_f64(*f),
        ValueRef::Text(t) => truncate_cell(&String::from_utf8_lossy(t)),
        ValueRef::Blob(b) => format!("<BLOB {} bytes>", b.len()),
    }
}

fn truncate_cell(text: &str) -> String {
    if text.chars().count() > MAX_CELL_CHARS {
        let head: String = text.chars().take(MAX_CELL_CHARS - 3).collect();
        return format!("{head}...");
    }
    text.to_string()
}

/// `str(float)` 的近似复刻：整数值补 `.0`，超范围用 `e+NN` / `e-NN`。
fn py_str_f64(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let abs = value.abs();
    if value == value.trunc() && abs < 1e16 {
        return format!("{:.1}", value);
    }
    if abs != 0.0 && (abs >= 1e16 || abs < 1e-4) {
        let sci = format!("{:e}", value);
        if let Some((mantissa, exp)) = sci.split_once('e') {
            let trimmed: String = exp.chars().filter(|c| *c != '+').collect();
            if let Ok(parsed) = trimmed.parse::<i32>() {
                return format!("{}e{:+03}", mantissa, parsed);
            }
        }
    }
    format!("{}", value)
}

/// `_split_sql_statements`：尊重引号（含 SQL 的 `''` 转义）、反引号与两种注释。
pub fn split_sql_statements(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut statements: Vec<String> = Vec::new();
    let mut current = String::new();
    let (mut in_single, mut in_double, mut in_backtick, mut in_line, mut in_block) =
        (false, false, false, false, false);
    let mut i = 0usize;
    let n = chars.len();

    while i < n {
        let ch = chars[i];
        let nxt = if i + 1 < n { chars[i + 1] } else { '\0' };
        let has_nxt = i + 1 < n;

        if in_line {
            current.push(ch);
            if ch == '\n' {
                in_line = false;
            }
            i += 1;
            continue;
        }
        if in_block {
            current.push(ch);
            if ch == '*' && has_nxt && nxt == '/' {
                current.push(nxt);
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_single {
            current.push(ch);
            if ch == '\'' {
                if has_nxt && nxt == '\'' {
                    current.push(nxt);
                    i += 2;
                    continue;
                }
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            current.push(ch);
            if ch == '"' {
                if has_nxt && nxt == '"' {
                    current.push(nxt);
                    i += 2;
                    continue;
                }
                in_double = false;
            }
            i += 1;
            continue;
        }
        if in_backtick {
            current.push(ch);
            if ch == '`' {
                in_backtick = false;
            }
            i += 1;
            continue;
        }

        if ch == '-' && has_nxt && nxt == '-' {
            current.push(ch);
            current.push(nxt);
            in_line = true;
            i += 2;
        } else if ch == '/' && has_nxt && nxt == '*' {
            current.push(ch);
            current.push(nxt);
            in_block = true;
            i += 2;
        } else if ch == '\'' {
            current.push(ch);
            in_single = true;
            i += 1;
        } else if ch == '"' {
            current.push(ch);
            in_double = true;
            i += 1;
        } else if ch == '`' {
            current.push(ch);
            in_backtick = true;
            i += 1;
        } else if ch == ';' {
            let stmt = py_trim(&current);
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current = String::new();
            i += 1;
        } else {
            current.push(ch);
            i += 1;
        }
    }

    let last = py_trim(&current);
    if !last.is_empty() {
        statements.push(last);
    }
    statements
}

/// `_execute_code_chunk`：`cwd` 已经过沙箱闸门。
fn execute_code_chunk_internal(
    code: &str,
    lang: &str,
    capture_plot: bool,
    timeout: u64,
    cwd: Option<&Path>,
) -> Map<String, Value> {
    let normalized = normalize_lang(lang);

    if normalized == "python" || normalized == "py" {
        let mut res = execute_python_chunk(code, capture_plot, timeout, cwd);
        set_lang(&mut res, "python");
        return res;
    }

    if normalized == "javascript" || normalized == "js" || normalized == "node" {
        let node = which("node").map(|p| p.into_os_string());
        let Some(node) = node else {
            return runtime_missing(
                "本地未检测到 Node.js 运行环境 (请安装 Node.js 或将其加入 PATH)",
                "Node.js not found in PATH",
                &normalized,
            );
        };
        let (script_path, _dir) = match write_temp_script(".js", code, cwd) {
            Ok(pair) => pair,
            Err(e) => return exception_dict(&format!("OSError: {e}")),
        };
        let mut res = run_process(&[node, script_path.into_os_string()], cwd, timeout, Some("node"));
        set_lang(&mut res, &normalized);
        return res;
    }

    if matches!(normalized.as_str(), "bash" | "sh" | "shell" | "powershell" | "cmd" | "bat") {
        let cmd: Vec<OsString> = if cfg!(target_os = "windows") {
            if normalized == "powershell" {
                vec![os("powershell"), os("-Command"), os(code)]
            } else {
                vec![os("cmd"), os("/c"), os(code)]
            }
        } else if Path::new("/bin/bash").exists() {
            vec![os("/bin/bash"), os("-c"), os(code)]
        } else {
            vec![os("/bin/sh"), os("-c"), os(code)]
        };
        let mut res = run_process(&cmd, cwd, timeout, None);
        set_lang(&mut res, &normalized);
        return res;
    }

    if normalized == "r" || normalized == "rscript" {
        let Some(rscript) = which("Rscript").map(|p| p.into_os_string()) else {
            return runtime_missing(
                "本地未检测到 Rscript 环境 (请安装 R 并将其加入 PATH)",
                "Rscript not found in PATH",
                &normalized,
            );
        };
        let (script_path, _dir) = match write_temp_script(".R", code, cwd) {
            Ok(pair) => pair,
            Err(e) => return exception_dict(&format!("OSError: {e}")),
        };
        let mut res = run_process(&[rscript, script_path.into_os_string()], cwd, timeout, None);
        set_lang(&mut res, &normalized);
        return res;
    }

    if normalized == "sql" || normalized == "sqlite" || normalized == "sqlite3" {
        return execute_sql_chunk(code, cwd, timeout);
    }

    if normalized == "go" || normalized == "golang" {
        let Some(go) = which("go").map(|p| p.into_os_string()) else {
            return runtime_missing(
                "本地未检测到 Go 环境 (请安装 Go 并将其加入 PATH)",
                "go not found in PATH",
                &normalized,
            );
        };
        let source = if code.contains("package main") {
            code.to_string()
        } else {
            format!("package main\nimport \"fmt\"\nfunc main() {{\n{code}\n}}")
        };
        let (script_path, _dir) = match write_temp_script(".go", &source, cwd) {
            Ok(pair) => pair,
            Err(e) => return exception_dict(&format!("OSError: {e}")),
        };
        let mut res = run_process(
            &[go, os("run"), script_path.into_os_string()],
            cwd,
            timeout,
            None,
        );
        set_lang(&mut res, &normalized);
        return res;
    }

    if normalized == "rust" || normalized == "rs" {
        if let Some(rust_script) = which("rust-script").map(|p| p.into_os_string()) {
            let mut res = run_process(&[rust_script, os("-e"), os(code)], cwd, timeout, None);
            set_lang(&mut res, &normalized);
            return res;
        }
        let Some(rustc) = which("rustc").map(|p| p.into_os_string()) else {
            return runtime_missing(
                "本地未检测到 Rust 运行环境 (rustc 或 rust-script)",
                "rustc not found in PATH",
                &normalized,
            );
        };
        let source = if code.contains("fn main()") {
            code.to_string()
        } else {
            format!("fn main() {{\n{code}\n}}")
        };
        let (script_path, _dir) = match write_temp_script(".rs", &source, cwd) {
            Ok(pair) => pair,
            Err(e) => return exception_dict(&format!("OSError: {e}")),
        };
        let out_bin = binary_next_to(&script_path, ".rs");
        let mut compile = run_process(
            &[rustc, script_path.clone().into_os_string(), os("-o"), out_bin.clone().into_os_string()],
            cwd,
            timeout,
            None,
        );
        let compile_failed = !is_truthy(&compile)
            || compile
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1)
                != 0;
        if compile_failed {
            set_lang(&mut compile, &normalized);
            return compile;
        }
        let mut res = run_process(&[out_bin.into_os_string()], cwd, timeout, None);
        set_lang(&mut res, &normalized);
        return res;
    }

    if normalized == "c" || normalized == "cpp" || normalized == "c++" {
        let cpp = normalized == "cpp" || normalized == "c++";
        let compiler = if cpp {
            which("g++").or_else(|| which("clang++"))
        } else {
            which("gcc").or_else(|| which("clang"))
        }
        .map(|p| p.into_os_string());
        let Some(compiler) = compiler else {
            return runtime_missing(
                "本地未检测到 C/C++ 编译器 (gcc/g++/clang)",
                "compiler not found in PATH",
                &normalized,
            );
        };
        let suffix = if cpp { ".cpp" } else { ".c" };
        let source = if code.contains("main(") {
            code.to_string()
        } else {
            let header = if cpp {
                "#include <iostream>\nusing namespace std;\n"
            } else {
                "#include <stdio.h>\n"
            };
            format!("{header}int main() {{\n{code}\nreturn 0;\n}}")
        };
        let (script_path, _dir) = match write_temp_script(suffix, &source, cwd) {
            Ok(pair) => pair,
            Err(e) => return exception_dict(&format!("OSError: {e}")),
        };
        let out_bin = binary_next_to(&script_path, suffix);
        let mut compile = run_process(
            &[compiler, script_path.clone().into_os_string(), os("-o"), out_bin.clone().into_os_string()],
            cwd,
            timeout,
            None,
        );
        let compile_failed = !is_truthy(&compile)
            || compile
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1)
                != 0;
        if compile_failed {
            set_lang(&mut compile, &normalized);
            return compile;
        }
        let mut res = run_process(&[out_bin.into_os_string()], cwd, timeout, None);
        set_lang(&mut res, &normalized);
        return res;
    }

    Dict::default()
        .set("ok", Value::Bool(false))
        .text("error", &format!("暂不支持的代码语言: {lang}"))
        .text("stdout", "")
        .text("stderr", &format!("Unsupported language: {lang}"))
        .set("images", empty_images())
        .set("exit_code", Value::from(1i64))
        .text("lang", &normalized)
        .build()
}

fn runtime_missing(error: &str, stderr: &str, lang: &str) -> Map<String, Value> {
    Dict::default()
        .set("ok", Value::Bool(false))
        .text("error", error)
        .text("stdout", "")
        .text("stderr", stderr)
        .set("images", empty_images())
        .set("exit_code", Value::from(127i64))
        .text("lang", lang)
        .build()
}

fn binary_next_to(script: &Path, suffix: &str) -> PathBuf {
    let stem = script
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = stem.strip_suffix(suffix).unwrap_or(&stem);
    let name = if cfg!(target_os = "windows") {
        format!("{base}.exe")
    } else {
        base.to_string()
    };
    script.with_file_name(name)
}

/// `_allowed_cwd`：显式工作目录只能落在 ReadMD 数据根或系统临时目录内。
pub fn allowed_cwd(cwd: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let Some(candidate) = cwd else {
        return Ok(None);
    };
    if candidate.as_os_str().is_empty() {
        // Python 开头是 `if not cwd: return None`：空串等同「没传」，
        // 不得按进程 CWD 解析后再判定成员。
        return Ok(None);
    }
    let resolved = match resolve_realpath(candidate) {
        Some(path) => path,
        None => return Err("cwd_not_found".to_string()),
    };
    if !resolved.is_dir() {
        return Err("cwd_not_found".to_string());
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(configured) = env::var("READMD_DATA_DIR") {
        if !configured.is_empty() {
            if let Some(path) = resolve_realpath(Path::new(&configured)) {
                roots.push(path);
            }
        }
    }
    if let Some(temp) = resolve_realpath(&env::temp_dir()) {
        roots.push(temp);
    }
    roots.retain(|root| root.is_dir());
    if roots.is_empty() {
        return Err("cwd_not_allowed".to_string());
    }
    for root in &roots {
        if same_path(&resolved, root) || is_within(root, &resolved) {
            return Ok(Some(resolved));
        }
    }
    Err("cwd_not_allowed".to_string())
}

/// `os.path.realpath(os.path.abspath(str(cwd)))`：相对路径按进程 CWD 解析并消解符号链接。
fn resolve_realpath(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().ok()?.join(path)
    };
    let absolute = normalize_unc(&absolute);
    match std::fs::canonicalize(&absolute) {
        Ok(resolved) => Some(strip_verbatim(&resolved)),
        Err(_) => Some(absolute),
    }
}

fn normalize_unc(path: &Path) -> PathBuf {
    let text = path.to_string_lossy().replace('\\', std::path::MAIN_SEPARATOR_STR);
    PathBuf::from(text)
}

fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    #[cfg(target_os = "windows")]
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    #[cfg(target_os = "windows")]
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest.to_string());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = &text;
    }
    path.to_path_buf()
}

fn same_path(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

fn is_within(root: &Path, candidate: &Path) -> bool {
    let root = path_key(root);
    let candidate = path_key(candidate);
    // Python 逐字写作 `candidate.startswith(root + os.sep)`：盘根/文件系统根的
    // realpath 自带结尾分隔符，拼接后是两个分隔符，于是该根下的任何路径都不算成员。
    // 这里必须无条件追加，不得「已带分隔符就不再追加」。
    let mut root = root;
    root.push('/');
    candidate.starts_with(&root)
}

fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if cfg!(target_os = "windows") {
        text.to_ascii_lowercase()
    } else {
        text
    }
}

/// 公共入口。`Err(..)` 表示 Python 侧会抛出未捕获异常（HTTP 层应回 500）。
pub fn execute_code_chunk_dict(
    code: &str,
    lang: &str,
    capture_plot: bool,
    timeout: u64,
    cwd: Option<&Path>,
) -> Result<Map<String, Value>, String> {
    let source = code.to_string();
    let cleaned = strip_literals_and_comments(&source, lang);
    if NETWORK_PATTERNS.iter().any(|re| re.is_match(&cleaned)) {
        return Ok(denied_dict("network_not_allowed", lang));
    }
    if PATH_ESCAPE_PATTERNS.iter().any(|re| re.is_match(&source)) {
        return Ok(denied_dict("path_access_not_allowed", lang));
    }
    let explicit = match allowed_cwd(cwd) {
        Ok(dir) => dir,
        Err(reason) => return Ok(denied_dict(&reason, lang)),
    };

    let sandbox = match tempfile::Builder::new().prefix("readmd-code-").tempdir() {
        Ok(dir) => dir,
        Err(e) => return Err(format!("OSError: {e}")),
    };
    let run_cwd = explicit.clone().unwrap_or_else(|| sandbox.path().to_path_buf());
    let mut res = execute_code_chunk_internal(&source, lang, capture_plot, timeout, Some(&run_cwd));
    if !res.contains_key("lang") {
        set_lang(&mut res, lang_or_python(lang));
    }
    Ok(res)
}

/// 兼容旧签名的入口：把 `Err(..)`（Python 侧会向外抛、HTTP 层回 500）折成
/// `ok=false` 字典。线上路由 `/api/code/run` 走的是
/// `parity_code::h_code_run` → `execute_code_chunk_dict`；
/// `batch2::h_code_run` 没有登记进路由表，本入口目前只被测试使用。
pub fn execute_code_chunk(
    code: &str,
    lang: &str,
    capture_plot: bool,
    timeout: u64,
    cwd: Option<&Path>,
) -> Map<String, Value> {
    match execute_code_chunk_dict(code, lang, capture_plot, timeout, cwd) {
        Ok(res) => res,
        Err(e) => exception_dict(&e),
    }
}

#[path = "parity_code.rs"]
pub mod parity_code;

#[cfg(test)]
mod tests {
    use super::*;

    fn has(res: &Map<String, Value>, key: &str) -> bool {
        res.contains_key(key)
    }

    fn interpreter_ready() -> bool {
        python_program().is_some()
    }

    #[test]
    fn network_deny_shape_matches_python() {
        let res = execute_code_chunk("import requests", "python", false, 10, None);
        assert_eq!(res.get("ok"), Some(&Value::Bool(false)));
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("network_not_allowed"));
        assert_eq!(res.get("error_code"), None, "Python 的拒绝字典里没有 error_code 键");
        assert_eq!(res.get("exit_code").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("python"));
        assert!(!has(&res, "warning"));
    }

    #[test]
    fn deny_lang_field_keeps_raw_case() {
        let res = execute_code_chunk("import requests", "  .PY ", false, 10, None);
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("  .PY "));
    }

    #[test]
    fn path_escape_deny_covers_windows_drive_and_relative_up() {
        for snippet in [
            "open(r\"C:\\\\Windows\\\\win.ini\")",
            "print(\"../../etc/passwd\")",
        ] {
            let res = execute_code_chunk(snippet, "python", false, 10, None);
            assert_eq!(
                res.get("error").and_then(|v| v.as_str()),
                Some("path_access_not_allowed"),
                "{snippet} 必须命中路径逃逸防线"
            );
        }
    }

    #[test]
    fn hash_inside_string_is_not_a_comment() {
        // tokenize 会把整个字符串当一个 token，注释规则不得在字符串内部生效。
        let cleaned = strip_literals_and_comments("x = 'http://a#b'\n", "python");
        assert!(!cleaned.contains("http"), "字符串内容必须被剥离: {cleaned}");
    }

    #[test]
    fn unterminated_string_falls_back_like_python_tokenize_error() {
        // Python tokenize 抛错 → 退回通用正则 → 字符串内容留在净化结果里 → 拒绝。
        let cleaned = strip_literals_and_comments("x = 'requests\n", "python");
        assert!(cleaned.contains("requests"), "非法字符串应走正则兜底: {cleaned}");
        let res = execute_code_chunk("x = 'requests\n", "python", false, 10, None);
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("network_not_allowed"));
    }

    #[test]
    fn whitespace_before_absolute_system_path_is_denied() {
        let res = execute_code_chunk("print( \" /etc/passwd\" )", "python", false, 10, None);
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("path_access_not_allowed"));
    }

    #[test]
    fn urllib_without_request_is_denied_like_python() {
        let res = execute_code_chunk("import urllib", "python", false, 10, None);
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("network_not_allowed"));
    }

    #[test]
    fn unsupported_language_shape_and_normalized_lang() {
        let res = execute_code_chunk("x", " brainfuck ", false, 10, None);
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("brainfuck"));
        assert_eq!(
            res.get("stderr").and_then(|v| v.as_str()),
            Some("Unsupported language:  brainfuck ")
        );
        assert!(!has(&res, "warning"));
    }

    #[test]
    fn missing_cwd_is_reported_as_not_found() {
        let ghost = std::env::temp_dir().join("readmd-definitely-not-here-42");
        let res = execute_code_chunk("print(1)", "python", false, 10, Some(&ghost));
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("cwd_not_found"));
    }

    #[test]
    fn outside_cwd_is_not_allowed() {
        let outside = Path::new(if cfg!(target_os = "windows") { "C:\\Windows" } else { "/etc" });
        let res = execute_code_chunk("print(1)", "python", false, 10, Some(outside));
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("cwd_not_allowed"));
    }

    #[test]
    fn timeout_secs_clamps_to_python_bounds() {
        assert_eq!(clamp_timeout(0), 10);
        assert_eq!(clamp_timeout(999), 10);
        assert_eq!(clamp_timeout(3), 3);
    }

    #[test]
    fn sql_splitter_handles_escaped_quote() {
        let stmts = split_sql_statements("SELECT 'it''s; here'; SELECT 2;");
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("it''s"));
    }

    #[test]
    fn sql_runs_natively_without_python() {
        let res = execute_code_chunk(
            "CREATE TABLE t (id INTEGER, name TEXT);\n\
             INSERT INTO t VALUES (1, 'Alice'), (2, NULL);\n\
             SELECT * FROM t;",
            "sql",
            false,
            10,
            None,
        );
        assert_eq!(res.get("ok"), Some(&Value::Bool(true)), "{res:?}");
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("sql"));
        let stdout = res.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
        assert!(stdout.contains("Alice"), "{stdout}");
        assert!(stdout.contains("NULL"), "{stdout}");
        assert!(stdout.contains("Query OK, 2 rows affected."), "{stdout}");
        assert_eq!(res.get("stderr").and_then(|v| v.as_str()), Some(""));
    }

    #[test]
    fn sql_error_maps_to_failed_without_stdout() {
        let res = execute_code_chunk("SELEC 1;", "sql", false, 10, None);
        assert_eq!(res.get("ok"), Some(&Value::Bool(false)));
        assert_eq!(res.get("stdout").and_then(|v| v.as_str()), Some(""));
        assert_eq!(res.get("exit_code").and_then(|v| v.as_i64()), Some(1));
        let error = res.get("error").and_then(|v| v.as_str()).unwrap_or("");
        assert!(error.starts_with("SQL 执行错误: "), "{error}");
    }

    #[test]
    fn blob_and_real_cells_render_like_python() {
        let blob = rusqlite::types::Value::Blob(vec![7u8; 3]);
        let blob_ref: rusqlite::types::ValueRef = (&blob).into();
        assert_eq!(format_cell(&blob_ref), "<BLOB 3 bytes>");
        assert_eq!(py_str_f64(42.0), "42.0");
        assert_eq!(py_str_f64(1.5), "1.5");
        assert_eq!(py_str_f64(1e20), "1e+20");
        assert_eq!(py_str_f64(1e-5), "1e-05");
    }

    #[test]
    fn truncated_stream_counts_chars_not_bytes() {
        // 恰好铺满上限：Python 的 `len(text) > rem` 不成立，所以不算截断。
        let mut exact = PipeSink::default();
        sink_push(&mut exact, &"汉".repeat(MAX_OUTPUT_CHARS));
        assert!(!exact.truncated, "正好 200_000 字符不得报截断（Python 同）");
        assert_eq!(exact.text.chars().count(), MAX_OUTPUT_CHARS);

        // 多一个字符才翻截断标记，且捕获长度依旧钳在上限。
        let mut over = PipeSink::default();
        sink_push(&mut over, &"汉".repeat(MAX_OUTPUT_CHARS + 1));
        assert!(over.truncated);
        assert_eq!(over.text.chars().count(), MAX_OUTPUT_CHARS);
    }

    #[test]
    fn newline_translation_matches_universal_newlines() {
        let mut raw = Vec::new();
        raw.extend_from_slice(b"a\r");
        raw.extend_from_slice(b"\nb\r\n");
        let mut decoder = Utf8Replacer::default();
        let mut translated = String::new();
        let mut decoded = String::new();
        decoder.feed(&raw, &mut decoded);
        NewlineTranslator::default().feed(&decoded, &mut translated);
        assert_eq!(translated, "a\nb\n");
    }

    #[test]
    fn python_chunk_executes_when_interpreter_exists() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        let res = execute_code_chunk("print(6*7)", "python", false, 10, None);
        assert_eq!(res.get("ok"), Some(&Value::Bool(true)), "{res:?}");
        assert_eq!(res.get("stdout").and_then(|v| v.as_str()), Some("42"));
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("python"));
        assert!(has(&res, "warning"), "正常完成的运行必须带 warning 键");
        assert!(!has(&res, "error"));
        assert!(!has(&res, "error_code"));
    }

    #[test]
    fn python_chunk_timeout_reports_timeout() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        let started = Instant::now();
        let res = execute_code_chunk("import time\nwhile True:\n    time.sleep(0.1)\n", "python", false, 2, None);
        assert_eq!(res.get("error_code").and_then(|v| v.as_str()), Some("execution_timeout"));
        assert_eq!(res.get("exit_code").and_then(|v| v.as_i64()), Some(-1));
        assert!(!has(&res, "warning"), "超时字典不含 warning 键");
        assert!(started.elapsed() < Duration::from_secs(8), "超时闸门失效: {:?}", started.elapsed());
    }

    #[test]
    fn python_chunk_truncates_output_to_max_chars() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        let res = execute_code_chunk("print('x' * 250000)", "python", false, 10, None);
        let stdout = res.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(stdout.chars().count(), MAX_OUTPUT_CHARS);
        assert_eq!(res.get("warning").and_then(|v| v.as_str()), Some("output_truncated"));
    }

    #[test]
    fn shell_and_node_dispatchers_are_reachable() {
        // 与 Python 一样，运行时缺失时给 127 + 固定 stderr 文案。
        if which("node").is_none() {
            let res = execute_code_chunk("console.log(1)", "js", false, 10, None);
            assert_eq!(res.get("exit_code").and_then(|v| v.as_i64()), Some(127));
            assert_eq!(res.get("stderr").and_then(|v| v.as_str()), Some("Node.js not found in PATH"));
        }
        if cfg!(target_os = "windows") {
            let res = execute_code_chunk("@echo off & echo 42", "cmd", false, 10, None);
            assert_eq!(res.get("ok"), Some(&Value::Bool(true)), "{res:?}");
            assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("cmd"));
            assert_eq!(res.get("stdout").and_then(|v| v.as_str()), Some("42"));
        }
    }

    // ==================================================================
    // 差分金表：由 scratch/rust_parity/coderun_r2/gen_golden.py 从本机
    // CPython 3.11.15（.venv 同款解释器，即 sys.executable 权威）录制备份。
    // 表里的期望值一律是 Python 的实际输出，不是 Rust 的输出。
    // ==================================================================

    const NET_PATH_GOLDEN: &[(&str, bool, bool)] = &[
        ("print('http://example.com')", false, false),
        ("print(\"curl -s x\")", false, false),
        ("x = f'http://{host}/y'", false, false),
        ("x = f'{scheme}://a'", false, false),
        ("x = rf'\\..\\..\\etc'", false, false),
        ("x = b'http://x'", false, false),
        ("x = \"\"\"http://a\nb\"\"\"", false, false),
        ("x = '''a#b'''\nprint(x)", false, false),
        ("# requests.get(url)\nprint(1)", false, false),
        ("x = 1  # http://y\n", false, false),
        ("import requests", true, false),
        ("import urllib", true, false),
        ("from urllib.request import urlopen", true, false),
        ("x = 1", false, false),
        ("x = ('socket')", false, false),
        ("def f():\n    return 'ping'", false, false),
        ("x = 'unterminated\n", false, false),
        ("x = 1 ?\n", false, false),
        ("  x = 1\n", false, false),
        ("x = (1,\n", false, false),
        ("x = '''unterminated\n", false, false),
        ("\n\nimport os\n", false, false),
        ("import os\n    x=1\n", false, false),
        ("print('a') ; import requests", true, false),
        ("x = 'x' + requests\n", true, false),
        ("while True:\n    pass  # socket", false, false),
        ("x = 1_000  # http://z\n", false, false),
        ("async def f():\n    await fetch('http://a')", true, false),
        ("x = '''\nimport requests\n'''", false, false),
        ("if a:\n\tb=1\nelse:\n\tb=2  # wget", false, false),
        ("x = '\\\\'\nprint(1)", false, true),
        ("print(\"\"\"a\\\"\"\"b\"\"\")", false, false),
        ("x = r'\\n\\n'\nimport socket", true, false),
        ("s = 'a' 'b'  # concat\n", false, false),
        ("x = \\\n    1\nimport httpx", true, false),
        ("class A:\n    'http://docstring'\n    x=1", false, false),
        ("x = 中文requests", false, false),
        ("x = '中文' + requests", true, false),
        ("import requests as r  # noqa", true, false),
        ("print(f'{1+1}')  # fetch(", false, false),
        ("x = 'http://a\n", false, false),
        ("print('http://a\n", false, false),
        ("x = 'requests\n", true, false),
        ("x = (\n  'requests'\n", false, false),
        ("x = (\n  'http://a'\n", false, false),
        ("x = 1  # comment with http://u\n", false, false),
        ("s = \"a'sql\"\nimport requests", true, false),
        ("x = f'{1}'\nimport socket", true, false),
        ("x = 'a' 'http://b'\n", false, false),
        ("x = \\\n'http://a'\n", false, false),
        ("x = ''  # http://\n", false, false),
        ("x = \"\"\"http://a\"\"\"  # b\n", false, false),
        ("x = '\\'' + requests\n", true, false),
        ("x = \"\"\"'\"\"\" + requests\n", true, false),
        ("x = r\"\\\\\\\\\" + requests\n", true, true),
        ("x = b'''http://a'''\n", false, false),
        ("if 1:\n  pass\nelse:\n  pass  # fetch(", false, false),
        ("x = 'x\ny'  # requests\n", false, false),
        ("def f():\n  '''docstring with socket'''\n  return 1", false, false),
        ("x = \"unterminated double\nprint(requests)\n", true, false),
        ("x = 1\n\u{c}\nimport requests\n", true, false),
        ("import requests  # noqa", true, false),
        ("from urllib   .   request import x", true, false),
        ("require('http')", false, false),
        ("const x = require('node:fs')", false, true),
        ("import {fetch} from 'node:http'", false, false),
        ("x = 1\nfetch('http://a')\n", true, false),
        ("print('ping')", false, false),
        ("ping", true, false),
        ("中文requests", false, false),
        ("x=1;curl", true, false),
        ("x = F\"http://{a}\"", false, false),
        ("x = rf'{a}\nhttp://b'", false, false),
        ("import socket  # f'{x}'", true, false),
        ("x = f'{requests}'", false, false),
        ("x = f'a{b}c' + requests", true, false),
        ("print(f'\"http://x\"')", false, false),
        ("x = f'''\nhttp://{a}\n'''", false, false),
        // probe6：Python 的 `\s` 含 U+001C..U+001F，防线宽于 Rust 的 `\s`。
        ("x = \"/etc\u{1c}passwd\"", false, true),
        ("x = \"/etc\"", false, true),
        ("x = '/tmp\u{1d}sock'", false, true),
        ("x = \"/usr\u{1e}lib\"", false, true),
        ("x = \"\u{1c}../win\"", false, true),
        ("print(\"\\x1c../etc\")", false, false),
        ("x = \"/\u{1c}/../etc\"", false, false),
        ("x = \"dir\u{1c}C:\\\\a\"", false, true),
        ("x = \"cat\u{1e}/etc/passwd\"", false, true),
        ("require\u{1c}(\"http\")", false, false),
        ("require\u{1c}('node:fs')", false, true),
        ("from\u{1c}\"http\" import x", false, false),
        ("fetch\u{1c}(url)", true, false),
        // 权威里 UNC 分支的 `[^\\s]` 是「非反斜杠且非 s」：主机名以 s 开头时不拦。
        ("x = \"\\\\\\\\server\u{1c}share\"", false, false),
        ("x = \"\\\\\\\\HOST\\\\share\"", false, true),
    ];
    const NET_PATH_GOLDEN_COUNT: usize = 93;

    const SQL_GOLDEN: &[(&str, bool, i64, &str, &str)] = &[
        ("SELEC 1;", false, 1, "", "near \"SELEC\": syntax error"),
        ("CREATE TABLE t(a INTEGER UNIQUE); INSERT INTO t VALUES (1); INSERT INTO t VALUES (1);", false, 1, "", "UNIQUE constraint failed: t.a"),
        ("CREATE TABLE t (id INTEGER, name TEXT);\nINSERT INTO t VALUES (1, 'Alice'), (2, NULL);\nSELECT * FROM t;", true, 0, "Query OK, -1 rows affected.\n\nQuery OK, 2 rows affected.\n\nid | name \n---+------\n1  | Alice\n2  | NULL", ""),
        ("SELECT 1.0 AS a, 1e16 AS b, 1e-5 AS c, 0.1 AS d, 1.5e300 AS e, -0.0 AS f, 2/7.0 AS g;", true, 0, "a   | b     | c     | d   | e        | f    | g                 \n----+-------+-------+-----+----------+------+-------------------\n1.0 | 1e+16 | 1e-05 | 0.1 | 1.5e+300 | -0.0 | 0.2857142857142857", ""),
        ("CREATE TABLE b (x BLOB); INSERT INTO b VALUES (x'414243'); SELECT * FROM b;", true, 0, "Query OK, -1 rows affected.\n\nQuery OK, 1 rows affected.\n\nx             \n--------------\n<BLOB 3 bytes>", ""),
        ("SELECT cast(1 as int)/0 AS divzero;", true, 0, "divzero\n-------\nNULL", ""),
        ("SELECT 'aaaa' AS a; -- trailing comment\nSELECT 1||2;", true, 0, "a   \n----\naaaa\n\n1||2\n----\n12", ""),
        ("PRAGMA table_info(t);", true, 0, "cid | name | type | notnull | dflt_value | pk\n----+------+------+---------+------------+---", ""),
        ("SELECT 'it''s; here' AS s;", true, 0, "s         \n----------\nit's; here", ""),
        ("SELECT x'00ff' AS bin;", true, 0, "bin           \n--------------\n<BLOB 2 bytes>", ""),
        ("WITH RECURSIVE cnt(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM cnt LIMIT 3) SELECT * FROM cnt;", true, 0, "x\n-\n1\n2\n3", ""),
        ("", true, 0, "", ""),
        ("   ", true, 0, "", ""),
        ("CREATE TABLE t(a); INSERT INTO t VALUES(1),(2),(3); UPDATE t SET a=a+1; DELETE FROM t WHERE a>3; SELECT count(*) FROM t;", true, 0, "Query OK, -1 rows affected.\n\nQuery OK, 3 rows affected.\n\nQuery OK, 3 rows affected.\n\nQuery OK, 1 rows affected.\n\ncount(*)\n--------\n2", ""),
        ("SELECT 'ünïcödé' AS u, '中文' AS z, 1 AS n;", true, 0, "u       | z  | n\n--------+----+--\nünïcödé | 中文 | 1", ""),
        ("-- only a comment", true, 0, "Query OK, -1 rows affected.", ""),
        ("/* block */ SELECT 1", true, 0, "1\n-\n1", ""),
        ("SELECT 'ünïcödé' AS u", true, 0, "u      \n-------\nünïcödé", ""),
        ("SELECT '中' AS 列名, 'x' AS b", true, 0, "列名 | b\n---+--\n中  | x", ""),
        ("SELECT 'x' AS a LIMIT 0", true, 0, "a\n-", ""),
        ("CREATE TABLE t(x); INSERT INTO t VALUES('a'*501); SELECT * FROM t;", true, 0, "Query OK, -1 rows affected.\n\nQuery OK, 1 rows affected.\n\nx\n-\n0", ""),
        ("SELECT 9007199254740993 AS big, -9223372036854775808 AS neg;", true, 0, "big              | neg                 \n-----------------+---------------------\n9007199254740993 | -9223372036854775808", ""),
        ("SELECT 0.0 AS z, 1e15 AS f15, 1e-4 AS m4, 1e-3 AS m3, 3.0 AS three;", true, 0, "z   | f15                | m4     | m3    | three\n----+--------------------+--------+-------+------\n0.0 | 1000000000000000.0 | 0.0001 | 0.001 | 3.0", ""),
        ("SELECT x'' AS emptyblob, 'a\\n b' AS nl;", true, 0, "emptyblob      | nl   \n---------------+------\n<BLOB 0 bytes> | a\\n b", ""),
        ("SELECT 1 AS a, 1 AS a;", true, 0, "a | a\n--+--\n1 | 1", ""),
        ("INSERT INTO nope VALUES (1);", false, 1, "", "no such table: nope"),
        ("SELECT 1; SELECT 'a');", false, 1, "", "near \")\": syntax error"),
        ("  ;  ; SELECT 42 ;", true, 0, "42\n--\n42", ""),
        ("CREATE TABLE u(a TEXT COLLATE NOCASE); SELECT char(10) AS j, cast('1e400' AS REAL) AS ov;", true, 0, "Query OK, -1 rows affected.\n\nj | ov \n--+----\n\n | inf", ""),
    ];

    const CMD_GOLDEN: &[(&str, bool, &str)] = &[
        ("echo 42", true, "42"),
        ("echo \"hello world\"", true, "\\\"hello world\\\""),
        ("echo a\\", true, "a\\\\"),
        ("echo \"a b\" & echo c", true, "\\\"a b\\\" \nc"),
        ("echo x\"y", true, "x\\\"y"),
        ("echo a%%b", true, "a%%b"),
        ("echo one  two", true, "one  two"),
        // 末行含一次性临时目录名（cd 的输出），不参与逐字比较。
        ("echo \"path\" & cd", true, "\\\"path\\\" \nC:\\Users\\Natsumer\\AppData\\Local\\Temp\\readmd-code-_6t5bm18"),
    ];

    const NODE_GOLDEN: &[(&str, bool, &str)] = &[
        ("console.log(\"a b\");", true, "a b"),
        (
            "console.log(process.argv.slice(1))",
            true,
            "[\n  'C:\\\\Users\\\\Natsumer\\\\AppData\\\\Local\\\\Temp\\\\readmd-code-5ruq2pwi\\\\readmd-script-lo1dpreg\\\\main.js'\n]",
        ),
        ("console.log(1+1)", true, "2"),
    ];

    const LANG_GOLDEN: &[(&str, bool, &str, &str)] = &[
        ("py\u{1c}", true, "42", ""),
        ("python\u{1f}", true, "42", ""),
        ("  PY  ", true, "42", ""),
        (".py", true, "42", ""),
        ("py\t", true, "42", ""),
        ("py\u{a0}", true, "42", ""),
        ("python\u{0}", false, "", "暂不支持的代码语言: python\u{0}"),
        ("PyThOn", true, "42", ""),
    ];

    const X1C_GOLDEN: &[(&str, &str)] = &[("tail", "'ab'"), ("head", "'cd'")];

    /// probe4 `exit_codes`：非零退出的字典键集（排序后）必须是这 7 个键。
    const EXIT_CODE_GOLDEN: &[(&str, &str, bool, i64)] = &[
        ("exit 3", "cmd", false, 3),
        ("import sys;sys.exit(3)", "python", false, 3),
        ("process.exit(4)", "js", false, 4),
    ];

    /// Python 的拒绝判定只作用于净化后的文本，比较决策而不是净化字符串本身
    /// （Python 的 cleaned 里多一个 tokenize ENCODING token `"utf-8"`）。
    fn deny_decisions(src: &str) -> (bool, bool) {
        let cleaned = strip_literals_and_comments(src, "python");
        let net = NETWORK_PATTERNS.iter().any(|re| re.is_match(&cleaned));
        let path = PATH_ESCAPE_PATTERNS.iter().any(|re| re.is_match(src));
        (net, path)
    }

    #[test]
    fn golden_network_and_path_deny_decisions_match_cpython() {
        assert_eq!(NET_PATH_GOLDEN.len(), NET_PATH_GOLDEN_COUNT);
        let mut bad: Vec<String> = Vec::new();
        for (src, net, path) in NET_PATH_GOLDEN {
            let (got_net, got_path) = deny_decisions(src);
            if got_net != *net || got_path != *path {
                bad.push(format!("{src:?}: rust=({got_net},{got_path}) cpython=({net},{path})"));
            }
        }
        assert!(bad.is_empty(), "拒绝决策与 CPython 不一致 {} 条:\n{}", bad.len(), bad.join("\n"));
    }

    #[test]
    fn golden_sql_output_matches_cpython() {
        let mut bad: Vec<String> = Vec::new();
        for (sql, ok, exit, stdout, stderr) in SQL_GOLDEN {
            let res = execute_sql_chunk(sql, None, 10);
            let got_ok = res.get("ok").and_then(|v| v.as_bool());
            let got_exit = res.get("exit_code").and_then(|v| v.as_i64());
            let got_out = get_str(&res, "stdout");
            let got_err = get_str(&res, "stderr");
            if got_ok != Some(*ok)
                || got_exit != Some(*exit)
                || got_out != *stdout
                || got_err != *stderr
            {
                bad.push(format!(
                    "{sql:?}\n  cpython=({}, {exit}, {stdout:?}, {stderr:?})\n  rust   =({got_ok:?}, {got_exit:?}, {got_out:?}, {got_err:?})",
                    *ok
                ));
            }
            if !*ok {
                let want = format!("SQL 执行错误: {stderr}");
                if get_str(&res, "error") != want {
                    bad.push(format!("{sql:?}: error={:?} cpython={:?}", get_str(&res, "error"), want));
                }
            }
        }
        assert!(bad.is_empty(), "SQL 渲染与 CPython 不一致:\n{}", bad.join("\n"));
    }

    #[test]
    fn golden_lang_normalization_end_to_end_matches_cpython() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        let mut bad: Vec<String> = Vec::new();
        for (lang, ok, stdout, error) in LANG_GOLDEN {
            let res = execute_code_chunk("print(7*6)", lang, false, 5, None);
            let got_ok = res.get("ok").and_then(|v| v.as_bool()) == Some(*ok);
            let got_out = get_str(&res, "stdout") == *stdout;
            let got_err = get_str(&res, "error") == *error;
            if !(got_ok && got_out && got_err) {
                bad.push(format!(
                    "lang={lang:?}: rust(ok={:?},stdout={:?},error={:?}) cpython(ok={ok},stdout={stdout:?},error={error:?})",
                    res.get("ok"),
                    get_str(&res, "stdout"),
                    get_str(&res, "error")
                ));
            }
        }
        assert!(bad.is_empty(), "语言归一化与 CPython 不一致:\n{}", bad.join("\n"));
    }

    #[test]
    fn golden_u1c_u1f_stripped_like_python_strip() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        let cases = [
            ("import sys\nsys.stdout.write('ab\\x1c')\n", "tail"),
            ("import sys\nsys.stdout.write('\\x1ecd')\n", "head"),
        ];
        for (code, tag) in cases {
            let want = X1C_GOLDEN
                .iter()
                .find(|(t, _)| *t == tag)
                .map(|(_, v)| v.trim_matches('\'').to_string())
                .unwrap_or_default();
            let res = execute_code_chunk(code, "python", false, 5, None);
            assert_eq!(get_str(&res, "stdout"), want, "{tag}: Python 的 str.strip() 会剥掉 U+001C");
        }
    }

    #[test]
    fn golden_cwd_gates_match_cpython_semantics() {
        // Python `_allowed_cwd`：`if not cwd: return None` —— 空串等同没传。
        assert_eq!(allowed_cwd(Some(Path::new(""))).ok(), Some(None));
        assert_eq!(allowed_cwd(None).ok(), Some(None));
        // Python 的成员判定是 `candidate.startswith(root + os.sep)`，
        // 盘根/文件系统根的 realpath 自带分隔符，拼接后是两个分隔符，
        // 因此 root 自身之下的任何路径都不算成员。
        assert!(!is_within(&PathBuf::from("C:/"), &PathBuf::from("C:/Windows")));
        assert!(!is_within(&PathBuf::from("/"), &PathBuf::from("/etc")));
        // 普通根仍是逐字 `root + sep` 前缀语义。
        assert!(is_within(&PathBuf::from("C:/a"), &PathBuf::from("C:/a/b")));
        assert!(!is_within(&PathBuf::from("C:/a"), &PathBuf::from("C:/ab")));
        assert!(!is_within(&PathBuf::from("C:/a"), &PathBuf::from("C:/a")));
        // 但相等由 same_path 单独放行，与 Python 的 `candidate == root` 一致。
        assert!(same_path(&PathBuf::from("C:/a"), &PathBuf::from("C:\\a")));
    }

    #[test]
    fn golden_cmd_argv_assembly_matches_cpython() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let mut bad: Vec<String> = Vec::new();
        for (code, ok, stdout) in CMD_GOLDEN {
            if stdout.contains("readmd-code-") {
                // 含一次性临时目录名，只比较 ok。
                let res = execute_code_chunk(code, "cmd", false, 5, None);
                if res.get("ok").and_then(|v| v.as_bool()) != Some(*ok) {
                    bad.push(format!("{code:?}: ok={:?}", res.get("ok")));
                }
                continue;
            }
            let res = execute_code_chunk(code, "cmd", false, 5, None);
            if res.get("ok").and_then(|v| v.as_bool()) != Some(*ok)
                || get_str(&res, "stdout") != *stdout
            {
                bad.push(format!(
                    "{code:?}\n  cpython=({ok}, {stdout:?})\n  rust   =({:?}, {:?})",
                    res.get("ok"),
                    get_str(&res, "stdout")
                ));
            }
        }
        assert!(bad.is_empty(), "cmd 命令行拼装/输出与 CPython 不一致:\n{}", bad.join("\n"));
    }

    #[test]
    fn golden_node_argv_assembly_matches_cpython() {
        if which("node").is_none() {
            eprintln!("跳过：系统上没有 node");
            return;
        }
        for (code, ok, stdout) in NODE_GOLDEN {
            let res = execute_code_chunk(code, "js", false, 5, None);
            assert_eq!(res.get("ok").and_then(|v| v.as_bool()), Some(*ok), "{code:?}");
            if stdout.contains("readmd-script-") {
                // 临时目录名一次性：只验「脚本路径被当成一个 argv 元素」。
                let out = get_str(&res, "stdout");
                assert!(out.contains("main.js'"), "{out}");
                assert_eq!(out.matches('\'').count(), 2, "argv 必须只有一个元素: {out}");
            } else {
                assert_eq!(get_str(&res, "stdout"), *stdout, "{code:?}");
            }
        }
    }

    #[test]
    fn golden_nonzero_exit_envelope_matches_cpython() {
        const WANTED_KEYS: [&str; 7] =
            ["exit_code", "images", "lang", "ok", "stderr", "stdout", "warning"];
        for (code, lang, ok, exit) in EXIT_CODE_GOLDEN {
            if *lang == "js" && which("node").is_none() {
                continue;
            }
            if *lang == "python" && !interpreter_ready() {
                continue;
            }
            let res = execute_code_chunk(code, lang, false, 5, None);
            assert_eq!(res.get("ok").and_then(|v| v.as_bool()), Some(*ok), "{lang}:{code}");
            assert_eq!(res.get("exit_code").and_then(|v| v.as_i64()), Some(*exit), "{lang}:{code}");
            let mut keys: Vec<&str> = res.keys().map(|k| k.as_str()).collect();
            keys.sort_unstable();
            assert_eq!(keys, WANTED_KEYS, "{lang}:{code} 的键集必须与 CPython 一致");
        }
    }

    /// probe_authority.py 里真实录到的 CPython 子进程 env 键（本机快照，14 个）。
    const CPYTHON_CHILD_ENV_KEYS: [&str; 14] = [
        "COMSPEC",
        "HOME",
        "LANG",
        "LC_ALL",
        "NODE_OPTIONS",
        "PATH",
        "PATHEXT",
        "PYTHONIOENCODING",
        "PYTHONUTF8",
        "SYSTEMDRIVE",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "USERPROFILE",
    ];

    #[test]
    fn golden_child_env_whitelist_matches_cpython() {
        let injected = child_env(None);
        let keys: Vec<&str> = injected.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            keys.iter().all(|k| CPYTHON_CHILD_ENV_KEYS.contains(k)),
            "子进程拿到了白名单之外的变量: {keys:?}"
        );
        for (key, want) in [
            ("PYTHONIOENCODING", "utf-8"),
            ("PYTHONUTF8", "1"),
            ("NODE_OPTIONS", "--no-warnings"),
        ] {
            let got = injected.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
            assert_eq!(got, Some(want), "{key} 必须与 Python 注入值一致");
        }
        // Python 的规则是「存在且非空才透传」，逐键复现。
        for key in SAFE_ENV_KEYS.iter() {
            let present = env::var(key).ok().is_some_and(|v| !v.is_empty());
            assert_eq!(
                keys.contains(key),
                present,
                "{key} 的透传结果必须跟着 Python 的 presence 规则"
            );
        }
        // 密钥/代理类变量一律不得外泄。
        env::set_var("READMD_AUDIT_SECRET_KEY", "leak-me");
        let after = child_env(Some("node"));
        env::remove_var("READMD_AUDIT_SECRET_KEY");
        assert!(after.iter().all(|(k, _)| k != "READMD_AUDIT_SECRET_KEY"));
        assert_eq!(
            after.iter().find(|(k, _)| k == "NODE_OPTIONS").map(|(_, v)| v.as_str()),
            Some("--no-warnings --max-old-space-size=128"),
            "node 运行时的 NODE_OPTIONS 必须带内存上限"
        );
    }

    #[test]
    fn golden_capture_parity_newlines_and_replacement_chars() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        // 与 probe_authority.py 逐字相同的两个 chunk；期望值就是 CPython 的记录。
        let res = execute_code_chunk("import sys\nsys.stdout.write('a\\rb\\nc\\r\\nd')\n", "python", false, 5, None);
        assert_eq!(get_str(&res, "stdout"), "a\nb\nc\n\nd", "通用换行翻译必须与 text 模式一致");

        let res = execute_code_chunk(
            "import sys\nsys.stdout.buffer.write(b'\\xff\\xfeabc\\xc3\\x28a\\xc3\\x29')\n",
            "python",
            false,
            5,
            None,
        );
        assert_eq!(
            get_str(&res, "stdout"),
            "\u{fffd}\u{fffd}abc\u{fffd}(a\u{fffd})",
            "errors='replace' 的逐字节替换形状"
        );
    }

    #[test]
    fn utf8_replacer_handles_split_multibyte_sequences() {
        // 前导字节被切断在两个 read(4096) 之间：Python 的增量解码器保留半个序列，
        // 下一块补上续字节时必须拼回原字符，不得提前吐 U+FFFD。
        //
        // 期望值是实测的 CPython 3.11.15 输出（errors="replace" 增量解码）：
        //   d = codecs.getincrementaldecoder("utf-8")("replace")
        //   d.decode(b"a\xc3", False) + d.decode(b"\xbfz\xff", True) -> 'aÿz\ufffd'
        //   d.decode(b"a\xcf", False) + d.decode(b"\xbdz\xff", True) -> 'a\u03fdz\ufffd'
        let mut decoder = Utf8Replacer::default();
        let mut out = String::new();
        decoder.feed(b"a\xc3", &mut out);
        decoder.feed(b"\xbfz\xff", &mut out);
        decoder.finish(&mut out);
        assert_eq!(out, "a\u{ff}z\u{fffd}");

        let mut decoder = Utf8Replacer::default();
        let mut out = String::new();
        decoder.feed(b"a\xcf", &mut out);
        decoder.feed(b"\xbdz\xff", &mut out);
        decoder.finish(&mut out);
        assert_eq!(out, "a\u{3fd}z\u{fffd}");
    }

    #[test]
    fn golden_timeout_keeps_flushed_partial_output() {
        if !interpreter_ready() {
            eprintln!("跳过：系统上没有 python 解释器");
            return;
        }
        let started = Instant::now();
        let res = execute_code_chunk(
            "import sys,time\nprint('partial', flush=True)\nsys.stderr.write('oops\\n')\nwhile True:\n    time.sleep(0.05)",
            "python",
            false,
            1,
            None,
        );
        // Python 的超时字典同样回填已捕获的 stdout/stderr（`stdout[:MAX].strip()`）。
        assert_eq!(res.get("error_code").and_then(|v| v.as_str()), Some("execution_timeout"));
        assert_eq!(res.get("stdout").and_then(|v| v.as_str()), Some("partial"), "{res:?}");
        assert_eq!(res.get("stderr").and_then(|v| v.as_str()), Some("oops"), "{res:?}");
        assert_eq!(res.get("exit_code").and_then(|v| v.as_i64()), Some(-1));
        assert!(!has(&res, "warning"));
        assert!(started.elapsed() < Duration::from_secs(6), "超时闸门失效: {:?}", started.elapsed());
    }

    #[test]
    fn golden_gate_order_is_network_then_path_then_cwd() {
        // 三层防线都在任何调度/临时目录之前，且 lang 字段用原始的 `str(lang or "python")`。
        let both = "import requests\nopen(r\"C:\\\\Windows\\\\win.ini\")";
        let res = execute_code_chunk(both, "brainfuck", false, 5, Some(Path::new("C:\\Windows")));
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("network_not_allowed"));
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("brainfuck"));

        let res = execute_code_chunk("open(r\"C:\\\\Windows\\\\win.ini\")", "", false, 5, None);
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("path_access_not_allowed"));
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("python"), "空 lang 回落 python 字面量");

        let ghost = std::env::temp_dir().join("readmd-audit-no-such-dir-7");
        let res = execute_code_chunk("print(1)", "sql", false, 5, Some(&ghost));
        assert_eq!(res.get("error").and_then(|v| v.as_str()), Some("cwd_not_found"));
        assert_eq!(res.get("lang").and_then(|v| v.as_str()), Some("sql"));
        // 三条拒绝字典的键集合起来只有这 7 个，且都不带 error_code/warning。
        let mut keys: Vec<&str> = res.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["error", "exit_code", "images", "lang", "ok", "stderr", "stdout"]);
    }
}
