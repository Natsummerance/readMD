//! 扫描 / 图片转 md：`src/readmd_modules/ocr.py` 的 Rust 对等实现。
//!
//! Python 侧的引擎阶梯是 WinRT → macOS Vision → RapidOCR 插件 → Tesseract CLI，
//! 全部依赖宿主二进制或 Python 扩展。Rust 内核既不回调 Python，也不得把识别
//! 任务外包给外部可执行文件，因此 `pick_engine()` 稳定返回 `None`——这等价于
//! Python 在“没有 tesseract、没有 rapidocr 插件、没有 WinRT”的机器上的状态：
//! `load()` 抛 `ocr-no-engine：无可用 OCR 引擎`，模块注册表进入 `error`，
//! `/api/ocr` 走 503 `module_unavailable`（见 `parity_web::h_ocr`）。
//!
//! PDF 的文字层是纯解析工作，不需要任何 OCR 引擎，所以 `ocr_pdf_to_md` 在 Rust
//! 内核里真正可用，并逐字复刻 Python 的分页、占位符与截断提示。

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// Python `_OCR_IMAGE_EXTS`。
pub const OCR_IMAGE_EXTS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".bmp", ".gif", ".tif", ".tiff", ".webp",
];
/// Python `OCR_PDF_EMPTY_PLACEHOLDER`。
pub const OCR_PDF_EMPTY_PLACEHOLDER: &str = "> （PDF 未提取到文字，且 OCR 无结果）";
/// Python `ocr_pdf_to_md(path, max_pages=200)` 的默认页数上限。
pub const OCR_MAX_PAGES: usize = 200;

/// Python `_pick_engine()` 的引擎阶梯。Rust 内核里没有一项可用，保留枚举是为了
/// 让“未来接入原生引擎”这件事有一个明确的落点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrEngine {
    WinRt,
    MacVision,
    RapidOcr,
    Tesseract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OcrErrorCode {
    #[serde(rename = "ocr_dependency_missing")]
    OcrDependencyMissing,
    #[serde(rename = "ocr_image_not_found")]
    OcrImageNotFound,
    #[serde(rename = "ocr_image_format_unsupported")]
    OcrImageFormatUnsupported,
    #[serde(rename = "ocr_process_failed")]
    OcrProcessFailed,
    #[serde(rename = "ocr_invalid_image")]
    OcrInvalidImage,
    #[serde(rename = "ocr_no_engine")]
    OcrNoEngine,
}

/// `code` 承载 Python 异常文本，`error_code` 承载稳定 UI 代码。
#[derive(Debug, Clone)]
pub struct OcrError {
    pub code: String,
    pub error_code: OcrErrorCode,
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl OcrError {
    fn engine(message: &str) -> Self {
        OcrError {
            code: message.to_string(),
            error_code: OcrErrorCode::OcrNoEngine,
        }
    }
}

struct EngineCache {
    engine: Option<Option<OcrEngine>>,
}

lazy_static! {
    /// Python `_engine_cache`：`None` 也会被缓存，避免每次请求重新探测。
    static ref CACHE: Mutex<EngineCache> = Mutex::new(EngineCache { engine: None });
}

/// Python `_pick_engine()`。Rust 内核不桥接宿主 OCR，恒为 `None`（并缓存）。
pub fn pick_engine() -> Option<OcrEngine> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(engine) = cache.engine.clone() {
        return engine;
    }
    let engine: Option<OcrEngine> = None;
    cache.engine = Some(engine.clone());
    engine
}

/// Python `ocr.load()`：`readmd.py` 用它决定 `ocr` 模块是 `ready` 还是 `error`。
pub fn load() -> Result<(), String> {
    if pick_engine().is_none() {
        return Err("ocr-no-engine：无可用 OCR 引擎".to_string());
    }
    Ok(())
}

/// Python `_ocr_bytes()`：没有引擎时抛错，而不是静默返回空串。
pub fn ocr_bytes(_data: &[u8], _language: Option<&str>) -> Result<String, OcrError> {
    Err(OcrError::engine(
        "ocr-no-engine：无可用 OCR 引擎。Windows 需要 WinRT，macOS 需要 PyObjC，其他平台需要 RapidOCR 插件或 Tesseract。",
    ))
}

/// Python `_ocr_cascade()`：每一层都用 try/except 包住，失败即降级为空串。
pub fn ocr_cascade(data: &[u8], _fallback_path: Option<&str>) -> String {
    ocr_bytes(data, None).unwrap_or_default()
}

// -------------------------------------------------------------- table to md
//
// Python authority: `src/readmd_modules/ocr.py:292-372` — `_matrix_to_md_table`,
// `_html_table_to_md` and `extract_table_to_md`.
//
// `extract_table_to_md` has two steps upstream.  Step 1 mounts the `rapid_table`
// *Python* plugin (`from rapid_table import RapidTable`), i.e. it needs the
// interpreter that the pure-Rust kernel deliberately does not embed, so the
// kernel takes the same branch Python takes when the plugin is absent or
// disabled: the `try:` body yields nothing and control falls through to step 2.
// Step 2 is pure string work over `_ocr_cascade()` output and is ported in full
// below — including on a machine with no OCR engine at all, where it correctly
// ends at `''` because `_ocr_cascade()` is already `''` there.

/// `str.strip()` — Py_UNICODE_ISSPACE, which is Unicode `White_Space` *plus*
/// `\x1c`-`\x1f`.  Rust's `str::trim` stops at `char::is_whitespace` and so
/// would leave a `\x1c` glued to the text.
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `re.split(r'\s{2,}', line)` on a str pattern: one separator is a run of two
/// or more `Py_UNICODE_ISSPACE` characters.  Measured on CPython 3.11:
/// `'a \t b'`, `'a\x1c\x1db'`, `'a\xa0\xa0b'` and `'a\u2028\u2028b'` all split,
/// `'a b'` and a lone `'a\tb'` do not.  A trailing separator yields a final
/// empty field, exactly like `re.split`.
fn split_on_wide_gaps(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if !py_isspace(chars[i]) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < chars.len() && py_isspace(chars[j]) {
            j += 1;
        }
        if j - i >= 2 {
            out.push(chars[start..i].iter().collect());
            start = j;
        }
        i = j;
    }
    out.push(chars[start..].iter().collect());
    out
}

fn ci_char_eq(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

fn ci_at(hay: &[char], pos: usize, needle: &str) -> bool {
    let n: Vec<char> = needle.chars().collect();
    pos + n.len() <= hay.len() && n.iter().enumerate().all(|(k, c)| ci_char_eq(hay[pos + k], *c))
}

/// `str.find` for a case-insensitive literal.
fn ci_find(hay: &[char], needle: &str, from: usize) -> Option<usize> {
    let width = needle.chars().count();
    let mut i = from;
    while i + width <= hay.len() {
        if ci_at(hay, i, needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `<tr[^>]*>(.*?)</tr>` with `re.IGNORECASE | re.DOTALL`: the capture is
/// everything between the first `>` after `<tr` and the first `</tr>`.  A
/// candidate whose closer is missing is skipped, because the engine then simply
/// retries from the next `<tr` — which is how `re.findall` produced the single
/// row of `'<table><tr><td>a</td><td>b</tr><tr><td>c</tr></table>'`.
fn tr_rows(hay: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut scan = 0usize;
    while let Some(open) = ci_find(hay, "<tr", scan) {
        let after = open + 3;
        scan = after;
        let gt = match hay[after..].iter().position(|c| *c == '>') {
            Some(offset) => after + offset + 1,
            None => continue,
        };
        let close = match ci_find(hay, "</tr>", gt) {
            Some(index) => index,
            None => continue,
        };
        out.push((gt, close));
        scan = close + "</tr>".chars().count();
    }
    out
}

/// `<t[dh][^>]*>` — returns the opener start and the index just past its close.
fn cell_open(hay: &[char], from: usize) -> Option<(usize, usize)> {
    let mut k = from;
    while k + 3 <= hay.len() {
        if hay[k] == '<' && ci_char_eq(hay[k + 1], 't') && (ci_char_eq(hay[k + 2], 'd') || ci_char_eq(hay[k + 2], 'h'))
        {
            if let Some(offset) = hay[k + 3..].iter().position(|c| *c == '>') {
                return Some((k, k + 3 + offset + 1));
            }
        }
        k += 1;
    }
    None
}

/// `</t[dh]>` — a fixed five-code-unit literal, so `(index, index + 5)`.
fn cell_close(hay: &[char], from: usize) -> Option<(usize, usize)> {
    let mut k = from;
    while k + 5 <= hay.len() {
        if hay[k] == '<'
            && hay[k + 1] == '/'
            && ci_char_eq(hay[k + 2], 't')
            && (ci_char_eq(hay[k + 3], 'd') || ci_char_eq(hay[k + 3], 'h'))
            && hay[k + 4] == '>'
        {
            return Some((k, k + 5));
        }
        k += 1;
    }
    None
}

/// `<t[dh][^>]*>(.*?)</t[dh]>` over `hay`, non-overlapping left to right,
/// returning the capture ranges.
fn td_cells(hay: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut scan = 0usize;
    while let Some((start, gt)) = cell_open(hay, scan) {
        match cell_close(hay, gt) {
            Some((close, next)) => {
                out.push((gt, close));
                scan = next;
            }
            // No closer after this opener: the engine retries from the next
            // starting position, which is `start + 1` for a literal `<`.
            None => scan = start + 1,
        }
    }
    out
}

/// `re.sub(r'<[^>]+>', '', text)` — drops `<` + at least one non-`>` + `>`.
fn strip_html_tags(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '<' {
            if let Some(offset) = chars[i + 1..].iter().position(|c| *c == '>') {
                if offset >= 1 {
                    i = i + 1 + offset + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Python `_matrix_to_md_table(rows)` (`ocr.py:292-304`).
pub fn matrix_to_md_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols == 0 {
        return String::new();
    }
    let row_line = |cells: &Vec<String>| {
        let mut padded: Vec<&str> = cells.iter().map(|c| c.as_str()).collect();
        padded.resize(max_cols, "");
        format!("| {} |", padded.join(" | "))
    };
    let mut parts = vec![row_line(&rows[0])];
    parts.push(format!("| {} |", vec!["---"; max_cols].join(" | ")));
    for row in &rows[1..] {
        parts.push(row_line(row));
    }
    parts.join("\n")
}

/// Python `_html_table_to_md(html_str)` (`ocr.py:306-320`).
pub fn html_table_to_md(html_str: &str) -> String {
    if html_str.is_empty() || !html_str.to_lowercase().contains("<table") {
        return String::new();
    }
    let hay: Vec<char> = html_str.chars().collect();
    let mut md_rows: Vec<Vec<String>> = Vec::new();
    for (from, to) in tr_rows(&hay) {
        // `td_cells` reports offsets relative to the slice it was given, so the
        // captures must be read back out of `row`, never out of `hay`.
        let row = &hay[from..to];
        let cells: Vec<String> = td_cells(row)
            .into_iter()
            .map(|(a, b)| row[a..b].iter().collect::<String>())
            .map(|raw| py_strip(&strip_html_tags(&raw)).replace('|', "\\|"))
            .collect();
        if !cells.is_empty() {
            md_rows.push(cells);
        }
    }
    matrix_to_md_table(&md_rows)
}

/// The step-2 heuristic of `extract_table_to_md` (`ocr.py:344-366`), split out
/// so it is testable without an OCR engine: tab-separated columns win, else two
/// or more consecutive whitespace characters separate them, and only rows with
/// at least two columns and at least two such rows make a table.
pub fn ocr_text_to_md_table(raw_text: &str) -> String {
    if raw_text.is_empty() {
        return String::new();
    }
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    for line in raw_text.split('\n') {
        let l = py_strip(line);
        if l.is_empty() {
            continue;
        }
        let cols: Vec<String> = if l.contains('\t') {
            l.split('\t').map(py_strip).filter(|c| !c.is_empty()).map(|c| c.to_string()).collect()
        } else if l.contains("  ") {
            split_on_wide_gaps(l)
                .iter()
                .map(|c| py_strip(c).to_string())
                .filter(|c| !c.is_empty())
                .collect()
        } else {
            vec![l.to_string()]
        };
        if cols.len() >= 2 {
            table_rows.push(cols);
        }
    }
    if table_rows.len() >= 2 {
        matrix_to_md_table(&table_rows)
    } else {
        String::new()
    }
}

/// Python `extract_table_to_md(image_path_or_bytes)` for the bytes branch.
pub fn extract_table_to_md(image: &[u8]) -> String {
    // Step 1 (`rapid_table`) needs the Python plugin sandbox — see the note
    // above; the kernel falls through exactly as Python does with the plugin
    // disabled, which `plugin_manager.is_plugin_enabled('rapid_table')` already
    // reports on a machine that never installed it.
    // Step 2: `raw_text = _ocr_cascade(data)`; `''` short-circuits to `''`.
    ocr_text_to_md_table(&ocr_cascade(image, None))
}

/// Python `extract_table_to_md(image_path_or_bytes)` for the `str` branch, where
/// the file is read first.  A missing/unreadable file is swallowed by the
/// upstream `except Exception: pass`, so it yields `''` rather than an error.
pub fn extract_table_to_md_path(path: &str) -> String {
    match fs::read(Path::new(path)) {
        Ok(data) => extract_table_to_md(&data),
        Err(_) => String::new(),
    }
}

/// Python `ocr_image(path, dpi=None)`。签名保留 `language` 形参以兼容既有调用点。
pub fn ocr_image(image_path: &str, _language: Option<&str>) -> Result<String, OcrError> {
    let path = Path::new(image_path);
    if !path.is_file() {
        return Err(OcrError {
            code: image_path.to_string(),
            error_code: OcrErrorCode::OcrImageNotFound,
        });
    }
    let data = fs::read(path).map_err(|_| OcrError {
        code: image_path.to_string(),
        error_code: OcrErrorCode::OcrImageNotFound,
    })?;
    Ok(ocr_cascade(&data, Some(image_path)))
}

/// Python `ocr_image_to_md(path)`。
pub fn ocr_image_to_md(path: &str) -> String {
    let text = ocr_image(path, None).unwrap_or_default();
    let body = if !text.is_empty() {
        let formatted = normalize_ocr_text(&text);
        if formatted.is_empty() { text } else { formatted }
    } else {
        "> （未识别出文字，仅保留原图）".to_string()
    };
    format!("![原图]({})\n\n{}", path, body)
}

/// `os.path.splitext(path)[1].lower()`，Windows 与 POSIX 分隔符都接受。
///
/// 逐句复刻 CPython `genericpath._splitext`（ntpath：`sep='\\'`、`altsep='/'`、
/// `extsep='.'`）：先取最后一个分隔符之后的文件名区间，再取整串最后一个 `.`；
/// 只有当这个点落在文件名里、且点在文件名内起点之后至少隔着一个非点字符时才算
/// 扩展名（源码里的 “skip all leading dots” 循环）。所以 `notes.TXT → .txt`、
/// `x.. → .`、`.env`/`..file`/`..` → 空。大小写折叠用 `to_lowercase()`，与
/// Python `str.lower()` 一样是全 Unicode 映射，而不是只折 ASCII。
pub fn splitext_lower(path: &str) -> String {
    let name_start = path
        .rfind(['/', '\\'])
        .map(|index| index + 1)
        .unwrap_or(0);
    let dot_index = match path.rfind('.') {
        Some(index) if index >= name_start => index,
        _ => return String::new(),
    };
    if path[name_start..dot_index]
        .bytes()
        .all(|byte| byte == b'.')
    {
        // 点之前的文件名部分全是点（或为空）→ 这是 `.foo` 式隐藏名，没有扩展名。
        return String::new();
    }
    path[dot_index..].to_lowercase()
}

/// Python `ocr_pdf_to_md(path, max_pages=200)` 的纯 Rust 实现。
///
/// 逐页取文字层；文字层为空的页在 Python 里会渲染后走 OCR 级联，Rust 内核没有
/// 栅格化与识别引擎，这些页同样得到空文本并被跳过——与 Python 在无引擎机器上
/// 的逐页结果一致。
pub fn ocr_pdf_to_md(path: &str, max_pages: usize) -> Result<String, OcrError> {
    let pages = pdf_page_texts(path)?;
    let total = pages.len();
    let used = if total > max_pages { max_pages } else { total };
    let mut parts: Vec<String> = Vec::new();
    for idx in 0..used {
        let text = pages[idx].trim().to_string();
        if text.is_empty() {
            continue;
        }
        let formatted = normalize_ocr_text(&text);
        let body = if formatted.is_empty() { text } else { formatted };
        parts.push(format!("## 第 {} 页\n\n{}", idx + 1, body));
    }
    if parts.is_empty() {
        return Ok(OCR_PDF_EMPTY_PLACEHOLDER.to_string());
    }
    if total > used {
        parts.push(format!(
            "> （注意：文档共 {} 页，本次仅处理前 {} 页，其余未转换）",
            total, used
        ));
    }
    Ok(parts.join("\n\n---\n\n"))
}

/// 逐页文字层：先 `pdf-extract`，失败再退回 `lopdf`，最后是一步只做“这份文档
/// 声明了 0 页”判定的结构扫描。三者都读不出时抛错，对应 Python `import fitz` /
/// `fitz.open` 抛异常 → `/api/ocr` 500 `ocr_failed`。
fn pdf_page_texts(path: &str) -> Result<Vec<String>, OcrError> {
    let read_error = || OcrError {
        code: format!("ocr-pdf-read-failed：{}", path),
        error_code: OcrErrorCode::OcrProcessFailed,
    };
    // pdf-extract 对畸形 PDF 可能 panic，内核是在请求线程里跑的，必须兜住。
    let owned = path.to_string();
    let grabbed = std::panic::catch_unwind(move || pdf_extract::extract_text_by_pages(&owned));
    if let Ok(Ok(pages)) = grabbed {
        return Ok(pages);
    }
    let owned = path.to_string();
    let grabbed = std::panic::catch_unwind(move || lopdf_page_texts(&owned));
    if let Ok(Ok(pages)) = grabbed {
        return Ok(pages);
    }
    // MuPDF 会自己重建缺失的 xref：页面树里一个页面都没有的文档照样能打开，
    // `page_count == 0` → Python 走 `if not parts: return OCR_PDF_EMPTY_PLACEHOLDER`。
    // 两个纯 Rust 读取器都要求可用的 xref，所以这里补一步判定，命中即“无文字层”。
    if pdf_declares_no_pages(path) {
        return Ok(Vec::new());
    }
    Err(read_error())
}

/// 名字 token 的分界符（PDF 3.2.3 / 3.2.4）：空白与 `()<>[]{}/%` 结束一个名字。
fn is_pdf_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | 0x00
            | 0x0c
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}

/// `bytes` 里是否存在整体匹配的名字 `name`。名字里不可能出现 `/`，所以每个 `/`
/// 都必然是一个新名字的起点——`<</Type/Pages>>` 里的 `/Pages` 前面就是 `/Type`
/// 的字母。区分 `/Page` 与 `/Pages`/`/PageLabel` 只看名字后面的那一个字节。
fn pdf_has_name_token(bytes: &[u8], name: &[u8]) -> bool {
    bytes.windows(name.len()).enumerate().any(|(index, window)| {
        if window != name {
            return false;
        }
        match bytes.get(index + name.len()) {
            Some(next) => is_pdf_delimiter(*next),
            None => true,
        }
    })
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | 0x00 | 0x0c)
}

/// `/Kids` 后面的数组是否为空（`[]` 或只含空白）。
fn pdf_kids_all_empty(bytes: &[u8]) -> bool {
    let name = b"/Kids";
    let mut from = 0usize;
    loop {
        let index = match bytes[from..]
            .windows(name.len())
            .position(|window| window == name)
        {
            Some(offset) => from + offset,
            None => return true,
        };
        from = index + name.len();
        let mut cursor = from;
        while matches!(bytes.get(cursor), Some(byte) if is_pdf_whitespace(*byte)) {
            cursor += 1;
        }
        if !matches!(bytes.get(cursor), Some(b'[')) {
            // 引用的是间接对象（`/Kids 5 0 R`），无法在此判定为空。
            return false;
        }
        cursor += 1;
        while matches!(bytes.get(cursor), Some(byte) if is_pdf_whitespace(*byte)) {
            cursor += 1;
        }
        if !matches!(bytes.get(cursor), Some(b']')) {
            return false;
        }
    }
}

/// 这份文档是否明确声明“没有任何页面”——即 MuPDF 能打开且 `page_count == 0` 的那一类。
/// 只在两个读取器都失败后调用，判据刻意保守：任何一处不确定都退回读取失败。
fn pdf_declares_no_pages(path: &str) -> bool {
    let bytes = match fs::read(path) {
        Ok(data) => data,
        Err(_) => return false,
    };
    // MuPDF 靠魔数认 pdf；认不出来就是 FileDataError，我们也照样报错。
    let head = &bytes[..bytes.len().min(1024)];
    if !head.windows(5).any(|window| window == b"%PDF-") {
        return false;
    }
    if !pdf_has_name_token(&bytes, b"/Pages") {
        return false;
    }
    if pdf_has_name_token(&bytes, b"/Page") {
        // 有页面对象却读不出文字层，不能谎报“没有文字层”。
        return false;
    }
    pdf_kids_all_empty(&bytes)
}

fn lopdf_page_texts(path: &str) -> Result<Vec<String>, String> {
    let doc = lopdf::Document::load(path).map_err(|e| e.to_string())?;
    let mut pages = Vec::new();
    for number in 1..=doc.get_pages().len() as u32 {
        let text = doc
            .extract_text(&[number])
            .map_err(|e| e.to_string())?;
        pages.push(text);
    }
    Ok(pages)
}

/// Python `ocr_any(path)`：按扩展名分发，其他类型抛 `ocr-unsupported-type`。
pub fn ocr_any(path: &str) -> Result<String, OcrError> {
    let ext = splitext_lower(path);
    if ext == ".pdf" {
        return ocr_pdf_to_md(path, OCR_MAX_PAGES);
    }
    if OCR_IMAGE_EXTS.contains(&ext.as_str()) {
        return Ok(ocr_image_to_md(path));
    }
    Err(OcrError {
        code: format!(
            "ocr-unsupported-type：{} 不是可识别的图片或 PDF 文件",
            if ext.is_empty() { "未知类型" } else { &ext }
        ),
        error_code: OcrErrorCode::OcrImageFormatUnsupported,
    })
}

// ---------------------------------------------------------------- 文本规范化
//
// 这里不引入正则引擎：Python `re.sub` / `re.match` 用到的五个模式全部手写为
// 逐字符扫描，语义逐条对齐（贪婪 + 回溯、前瞻不消费、非重叠续扫）。

/// Python `_CN_NUM`：中文数字集，标题/列表启发式共用。
const CN_NUM: &str = "一二三四五六七八九十百千万两";

const SENT_END: &str = "。！？!?…:：；;";

/// Python `cjk_char = r'[\u4e00-\u9fa5]'`，同时也是 `_CJK_CHAR`。
fn is_cjk_char(ch: char) -> bool {
    matches!(ch, '\u{4e00}'..='\u{9fa5}')
}

/// Python `cjk_punc`：中文标点类。
fn is_cjk_punc(ch: char) -> bool {
    matches!(
        ch,
        '\u{3002}'
            | '\u{ff01}'
            | '\u{ff1f}'
            | '\u{ff1b}'
            | '\u{ff0c}'
            | '\u{3001}'
            | '\u{ff1a}'
            | '\u{ff08}'
            | '\u{ff09}'
            | '\u{300a}'
            | '\u{300b}'
            | '\u{3010}'
            | '\u{3011}'
            | '\u{201c}'
            | '\u{201d}'
            | '\u{2018}'
            | '\u{2019}'
    )
}

/// Python `[^\S\n]`：除换行以外的空白（换行必须保住，否则段落会被粘成一坨）。
fn is_inline_space(ch: char) -> bool {
    ch.is_whitespace() && ch != '\n'
}

/// Python `\d`（Unicode str 模式）等价于 Unicode Nd 类。
fn is_digit(ch: char) -> bool {
    ch.is_numeric()
}

/// `[CN0-9]` 类，即 `[%s0-9] % _CN_NUM`。
fn is_cn_num_or_digit(ch: char) -> bool {
    CN_NUM.contains(ch) || ch.is_ascii_digit()
}

/// 从 `start` 起的最长字符游程结束下标。
fn run_of(chars: &[char], start: usize, pred: impl Fn(char) -> bool) -> usize {
    let mut end = start;
    while end < chars.len() && pred(chars[end]) {
        end += 1;
    }
    end
}

/// Python `re.sub(r'(HEAD)[^\\S\\n]+(?=TAIL)', r'\\1', src)`。
///
/// 单遍从左到右：命中时把 HEAD 之后那段同行空白整段删掉，TAIL 字符**不**被消费，
/// 因而它立刻成为下一轮的 HEAD 候选——链式的「这 是 一 个」才会在一次 `sub` 里
/// 全部塌缩，与 Python 的前瞻语义等价。空白游程后面接的不是 TAIL 时，空白原样输出、
/// 扫描点前进（游程里都是空白，不可能有别的匹配起点，故与逐位前进等价）。
fn strip_inline_gaps(src: &str, head: impl Fn(char) -> bool, tail: impl Fn(char) -> bool) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        out.push(ch);
        i += 1;
        if !head(ch) || i >= chars.len() || !is_inline_space(chars[i]) {
            continue;
        }
        let gap_end = run_of(&chars, i, is_inline_space);
        if gap_end < chars.len() && tail(chars[gap_end]) {
            i = gap_end;
        } else {
            out.extend_from_slice(&chars[i..gap_end]);
            i = gap_end;
        }
    }
    out.into_iter().collect()
}

/// Python `re.sub(r'([a-zA-Z]{2,})-\\n([a-zA-Z]{2,})', r'\\1\\2', src)`。
///
/// 第一组贪婪取最长字母游程：游程内部不可能出现 `-`，所以「最早起点 + 最长游程」
/// 就是 Python 的 most-leftmost 匹配；命中后 `-\n` 被删掉，且第二组字母随匹配一起
/// 消费掉（Python 的 `sub` 从匹配末尾继续扫描，不会把第二组再当下一轮的第一组）。
fn join_hyphen_breaks(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let is_alpha = |ch: char| ch.is_ascii_alphabetic();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0usize;
    while i < chars.len() {
        if !is_alpha(chars[i]) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let word_end = run_of(&chars, i, is_alpha);
        let break_ok = word_end + 1 < chars.len()
            && chars[word_end] == '-'
            && chars[word_end + 1] == '\n';
        let tail_end = run_of(&chars, word_end + 2, is_alpha);
        out.extend_from_slice(&chars[i..word_end]);
        if break_ok && word_end - i >= 2 && tail_end - (word_end + 2) >= 2 {
            // `\1\2`：只删掉中间的 `-\n`，第二组字母照常输出并随匹配一起消费。
            out.extend_from_slice(&chars[word_end + 2..tail_end]);
            i = tail_end;
        } else {
            i = word_end;
        }
    }
    out.into_iter().collect()
}

/// Python `_HEAD_PATTERN.match(...)`：
/// `^(第[CN0-9]+[章节回部篇卷]|[（(]?[CN0-9]{1,3}[）)、．.]|\d{1,3}\.\d|#{1,6}\s)`。
fn head_matches(s: &str) -> bool {
    let ch: Vec<char> = s.chars().collect();
    // 分支 1：`第` + 至少一个中文数字/数字 + 章节回部篇卷。
    if ch.first() == Some(&'第') {
        let end = run_of(&ch, 1, is_cn_num_or_digit);
        if end > 1 && matches!(ch.get(end), Some('章') | Some('节') | Some('回') | Some('部') | Some('篇') | Some('卷')) {
            return true;
        }
    }
    // 分支 2：`[（(]?` 贪婪先带括号，`{1,3}` 再由长到短回溯。
    let bases: &[usize] = if matches!(ch.first(), Some('（') | Some('(')) {
        &[1usize, 0]
    } else {
        &[0]
    };
    for &base in bases {
        let end = {
            let mut e = base;
            while e < ch.len() && e - base < 3 && is_cn_num_or_digit(ch[e]) {
                e += 1;
            }
            e
        };
        let mut n = end - base;
        while n >= 1 {
            if matches!(ch.get(base + n), Some('）') | Some(')') | Some('、') | Some('．') | Some('.')) {
                return true;
            }
            n -= 1;
        }
    }
    // 分支 3：`\d{1,3}\.\d`（游程超过 3 位时后面必然是数字，回溯无解）。
    let digits = run_of(&ch, 0, is_digit);
    if digits >= 1 && digits <= 3 && ch.get(digits) == Some(&'.') && ch.get(digits + 1).copied().map_or(false, is_digit) {
        return true;
    }
    // 分支 4：`#{1,6}\s`。
    let hashes = run_of(&ch, 0, |c| c == '#');
    if hashes >= 1 && hashes <= 6 && ch.get(hashes).map_or(false, |c| c.is_whitespace()) {
        return true;
    }
    false
}

/// Python `_LIST_PATTERN.match(...)`：
/// `^([ \t]*[•·◦▪●*\-+]|\d{1,3}[、．.]|[（(]\d{1,3}[）)])\s*`。
/// 结尾的 `\s*` 不影响 `re.match` 是否命中，故不实现。
fn list_matches(s: &str) -> bool {
    let ch: Vec<char> = s.chars().collect();
    // 分支 1：`[ \t]*` + 项目符号。
    let indent = run_of(&ch, 0, |c| c == ' ' || c == '\t');
    if matches!(
        ch.get(indent),
        Some('\u{2022}') | Some('\u{b7}') | Some('\u{25e6}') | Some('\u{25aa}') | Some('\u{25cf}') | Some('*') | Some('-') | Some('+')
    ) {
        return true;
    }
    // 分支 2：`\d{1,3}[、．.]`。
    let digits = run_of(&ch, 0, is_digit);
    if digits >= 1 && digits <= 3 && matches!(ch.get(digits), Some('、') | Some('\u{ff0e}') | Some('.')) {
        return true;
    }
    // 分支 3：`[（(]\d{1,3}[）)]`。`run_of` 返回的是游程结束下标，位数要自己减出来。
    if matches!(ch.first(), Some('（') | Some('(')) {
        let end = run_of(&ch, 1, is_digit);
        if end >= 2 && end <= 4 && matches!(ch.get(end), Some('）') | Some(')')) {
            return true;
        }
    }
    false
}

/// Python `normalize_ocr_text()` 的第 1~4 步（CJK 空格剔除、断词连字、行合并）。
///
/// 第 5 步在 Python 里调用 `txtmd.to_markdown()` 做标题/列表/表格结构化，且整段
/// 包在 `try/except` 中；Rust 内核没有 txtmd 的公开入口，因此走的是 Python 的
/// 兜底分支（返回合并后的纯文本），随后 `/api/ocr` 仍会统一过一遍
/// `readmd_fix::fix_markdown`，与 Python 的最终 content 保持同源修复链路。
pub fn normalize_ocr_text(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let mut src: String = text.replace("\r\n", "\n").replace('\r', "\n");
    src = src
        .chars()
        .map(|c| {
            if matches!(c, '\u{3000}' | '\u{a0}' | '\u{200b}' | '\u{feff}') {
                ' '
            } else {
                c
            }
        })
        .collect();

    // Python 把「CJK↔CJK」这条 `re.sub` 连写了两遍（其余两条各一遍），这里逐笔照搬，
    // 而不是循环到不动点——两者的残差行为不同。
    src = strip_inline_gaps(&src, is_cjk_char, is_cjk_char);
    src = strip_inline_gaps(&src, is_cjk_char, is_cjk_char);
    src = strip_inline_gaps(&src, is_cjk_char, is_cjk_punc);
    src = strip_inline_gaps(&src, is_cjk_punc, is_cjk_char);
    src = join_hyphen_breaks(&src);

    let lines: Vec<&str> = src.split('\n').map(|l| l.trim_end()).collect();
    let mut merged: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let stripped = line.trim();
        if stripped.is_empty() {
            merged.push(String::new());
            i += 1;
            continue;
        }
        if is_structured(stripped) {
            merged.push(line.to_string());
            i += 1;
            continue;
        }
        let mut curr = line.to_string();
        while i + 1 < lines.len() {
            let next_line = lines[i + 1];
            let next_stripped = next_line.trim();
            if next_stripped.is_empty() || is_structured(next_stripped) {
                break;
            }
            let trimmed = curr.trim_end().to_string();
            if let Some(last) = trimmed.chars().last() {
                if SENT_END.contains(last) {
                    break;
                }
            }
            if curr.trim().chars().count() <= 30
                && !curr.chars().any(|c| c == '，' || c == ',' || c == '。' || c == '；')
            {
                break;
            }
            let last_char = trimmed.chars().last();
            let next_first = next_stripped.chars().next();
            let cjk_boundary = matches!(
                (last_char, next_first),
                (Some(a), Some(b)) if is_cjk_char(a) && is_cjk_char(b)
            );
            let sep = if cjk_boundary { "" } else { " " };
            curr = format!("{}{}{}", trimmed, sep, next_stripped);
            i += 1;
        }
        merged.push(curr);
        i += 1;
    }
    merged.join("\n").trim().to_string()
}

fn is_structured(stripped: &str) -> bool {
    head_matches(stripped)
        || list_matches(stripped)
        || stripped.contains('\t')
        || stripped.starts_with('|')
}

// ------------------------------------------------------------------ 版面分析

/// RapidOCR/WinRT 识别框；XY-Cut 只依赖这些几何量。
#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub text: String,
    pub x_left: f64,
    pub y_top: f64,
    pub width: f64,
    pub height: f64,
}

/// Python `_xy_cut_lines()`：先按垂直空白切栏，再按水平空白切块，最后行内聚类。
pub fn xy_cut_lines(items: &[LayoutItem]) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    if items.len() == 1 {
        let text = items[0].text.trim();
        return if text.is_empty() { Vec::new() } else { vec![text.to_string()] };
    }
    let avg_h: f64 = items
        .iter()
        .map(|it| if it.height > 0.0 { it.height } else { 15.0 })
        .sum::<f64>()
        / items.len() as f64;

    // 1. X-Cut：垂直空白投影，先切多栏。
    let mut sorted_by_x: Vec<LayoutItem> = items.to_vec();
    sorted_by_x.sort_by(|a, b| a.x_left.partial_cmp(&b.x_left).unwrap_or(std::cmp::Ordering::Equal));
    let col_gap_threshold = f64::max(25.0, avg_h * 1.8);
    let mut x_cut_idx: isize = -1;
    let mut max_x_gap = 0.0f64;
    let mut curr_right =
        sorted_by_x[0].x_left + if sorted_by_x[0].width > 0.0 { sorted_by_x[0].width } else { 20.0 };
    for idx in 0..sorted_by_x.len() - 1 {
        let it = &sorted_by_x[idx];
        curr_right = curr_right.max(it.x_left + if it.width > 0.0 { it.width } else { 20.0 });
        let next_left = sorted_by_x[idx + 1].x_left;
        let gap = next_left - curr_right;
        if gap >= col_gap_threshold && gap > max_x_gap {
            max_x_gap = gap;
            x_cut_idx = idx as isize + 1;
        }
    }
    if x_cut_idx > 0 {
        let cut = x_cut_idx as usize;
        let mut left = xy_cut_lines(&sorted_by_x[..cut]);
        left.extend(xy_cut_lines(&sorted_by_x[cut..]));
        return left;
    }

    // 2. Y-Cut：水平空白投影，切段落块。
    let mut sorted_by_y: Vec<LayoutItem> = items.to_vec();
    sorted_by_y.sort_by(|a, b| a.y_top.partial_cmp(&b.y_top).unwrap_or(std::cmp::Ordering::Equal));
    let y_threshold = f64::max(8.0, avg_h * 0.8);
    let mut y_cut_idx: isize = -1;
    let mut max_y_gap = 0.0f64;
    let mut curr_bottom =
        sorted_by_y[0].y_top + if sorted_by_y[0].height > 0.0 { sorted_by_y[0].height } else { 15.0 };
    for idx in 0..sorted_by_y.len() - 1 {
        let it = &sorted_by_y[idx];
        curr_bottom = curr_bottom.max(it.y_top + if it.height > 0.0 { it.height } else { 15.0 });
        let gap = sorted_by_y[idx + 1].y_top - curr_bottom;
        if gap >= y_threshold && gap > max_y_gap {
            max_y_gap = gap;
            y_cut_idx = idx as isize + 1;
        }
    }
    if y_cut_idx > 0 {
        let cut = y_cut_idx as usize;
        let mut top = xy_cut_lines(&sorted_by_y[..cut]);
        top.extend(xy_cut_lines(&sorted_by_y[cut..]));
        return top;
    }

    // 3. 叶子块：行聚类后自左向右拼接。
    let line_thresh = f64::max(6.0, avg_h * 0.5);
    let mut rows: Vec<(f64, Vec<&LayoutItem>)> = Vec::new();
    for it in sorted_by_y.iter() {
        let mut matched = None;
        for (y_ref, items_in) in rows.iter_mut() {
            if (it.y_top - *y_ref).abs() <= line_thresh {
                items_in.push(it);
                *y_ref = items_in.iter().map(|x| x.y_top).sum::<f64>() / items_in.len() as f64;
                matched = Some(true);
                break;
            }
        }
        if matched.is_none() {
            rows.push((it.y_top, vec![it]));
        }
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut lines = Vec::new();
    for (_, row) in rows {
        let mut row = row.clone();
        row.sort_by(|a, b| a.x_left.partial_cmp(&b.x_left).unwrap_or(std::cmp::Ordering::Equal));
        let line = row
            .iter()
            .filter(|it| !it.text.is_empty())
            .map(|it| it.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// 兼容旧调用点的入口名（等价于 `xy_cut_lines`）。
pub fn xy_cut_layout(items: &[LayoutItem]) -> Vec<String> {
    xy_cut_lines(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_to_md_table_pads_short_rows_like_python() {
        // Every expectation is live CPython output for
        // `ocr._matrix_to_md_table(rows)`.
        assert_eq!(matrix_to_md_table(&[]), "");
        assert_eq!(matrix_to_md_table(&[vec![]]), "");
        assert_eq!(matrix_to_md_table(&[vec![], vec![]]), "");
        assert_eq!(matrix_to_md_table(&[vec!["a".into()]]), "| a |\n| --- |");
        assert_eq!(
            matrix_to_md_table(&[vec!["a".into(), "b".into()], vec!["c".into()]]),
            "| a | b |\n| --- | --- |\n| c |  |"
        );
        // `matrix_to_md_table` does not strip or escape — that happens upstream.
        assert_eq!(
            matrix_to_md_table(&[vec!["a|b".into(), "c".into()]]),
            "| a|b | c |\n| --- | --- |"
        );
        assert_eq!(matrix_to_md_table(&[vec!["  ".into()]]), "|    |\n| --- |");
    }

    #[test]
    fn html_table_to_md_matches_python_regex_behaviour() {
        assert_eq!(html_table_to_md(""), "");
        assert_eq!(html_table_to_md("no table here"), "");
        // case-insensitive tag match, `[^>]*` tolerates attributes and a tab
        assert_eq!(
            html_table_to_md("<TABLE><TR><TD>A</TD><TD>B</TD></TR></TABLE>"),
            "| A | B |\n| --- | --- |"
        );
        assert_eq!(
            html_table_to_md("<table><tr\t><td>a</td></tr></table>"),
            "| a |\n| --- |"
        );
        // nested markup is stripped, the cell is stripped, `|` is escaped
        assert_eq!(
            html_table_to_md("<table><tr><td><b>bold</b> tail</td><td> x </td></tr></table>"),
            "| bold tail | x |\n| --- | --- |"
        );
        assert_eq!(
            html_table_to_md("<table><tr><td>a|b</td><td>c</td></tr></table>"),
            "| a\\|b | c |\n| --- | --- |"
        );
        // `colspan` is not honoured — the row simply comes out short and is padded
        assert_eq!(
            html_table_to_md("<table><tr><td colspan=\"2\">wide</td></tr><tr><td>a</td><td>b</td></tr></table>"),
            "| wide |  |\n| --- | --- |\n| a | b |"
        );
        // DOTALL: a newline stays inside the cell text
        assert_eq!(
            html_table_to_md("<table><tr>\n<td>multi\nline</td><td>x</td></tr></table>"),
            "| multi\nline | x |\n| --- | --- |"
        );
        // An empty row contributes nothing, and a `<td>` without its `</td>`
        // kills the rest of that row and the following row: both goldens are
        // live `re.findall` output.
        assert_eq!(
            html_table_to_md("<table><tr></tr><tr><td>a</td></tr></table>"),
            "| a |\n| --- |"
        );
        assert_eq!(
            html_table_to_md("<table><tr><td>a</td><td>b</tr><tr><td>c</tr></table>"),
            "| a |\n| --- |"
        );
        assert_eq!(html_table_to_md("<table><tr><td>a</td></tr>"), "| a |\n| --- |");
        assert_eq!(
            html_table_to_md(
                "<table><tr><td>a</td><td>b</td></tr></table><table><tr><td>c</td></tr></table>"
            ),
            "| a | b |\n| --- | --- |\n| c |  |"
        );
    }

    #[test]
    fn ocr_text_heuristic_builds_tables_only_from_multi_column_rows() {
        // Goldens are the live step-2 result of `ocr.extract_table_to_md`.
        assert_eq!(ocr_text_to_md_table(""), "");
        assert_eq!(
            ocr_text_to_md_table("Header A\tHeader B\nData 1\tData 2"),
            "| Header A | Header B |\n| --- | --- |\n| Data 1 | Data 2 |"
        );
        assert_eq!(ocr_text_to_md_table("solo line\nanother line"), "");
        assert_eq!(ocr_text_to_md_table("one\there"), "");
        assert_eq!(ocr_text_to_md_table("a  b\n\n   \nc  d"), "| a | b |\n| --- | --- |\n| c | d |");
        assert_eq!(
            ocr_text_to_md_table("  spaced  out  header  \n  spaced  out  body  "),
            "| spaced | out | header |\n| --- | --- | --- |\n| spaced | out | body |"
        );
        assert_eq!(
            ocr_text_to_md_table("a\tb\tc\nd\te"),
            "| a | b | c |\n| --- | --- | --- |\n| d | e |  |"
        );
        // The `elif '  ' in l` guard tests for two *ASCII* spaces, but once
        // inside it `re.split(r"\s{2,}")` treats U+00A0/U+2028 and the
        // `\x1c`-`\x1f` group as whitespace.  Both halves measured on CPython:
        assert_eq!(ocr_text_to_md_table("a\u{1c}\u{1d}b\nc\u{1c}\u{1d}d"), "");
        assert_eq!(
            ocr_text_to_md_table("a\u{a0}\u{a0}b  c\nd  e"),
            "| a | b | c |\n| --- | --- | --- |\n| d | e |  |"
        );
    }

    #[test]
    fn extract_table_to_md_falls_back_to_empty_without_an_engine() {
        // No WinRT / Vision / RapidOCR / Tesseract in the kernel, and no Python
        // `rapid_table` plugin either, so both branches of the upstream ladder
        // end at `''` — while the pure functions above stay engine-independent.
        assert_eq!(extract_table_to_md(b"fake_image_bytes"), "");
        assert_eq!(extract_table_to_md_path("Z:/definitely/not/here.png"), "");
    }


    #[test]
    fn engine_ladder_is_empty_so_load_reports_the_python_message() {
        assert!(pick_engine().is_none());
        assert_eq!(load().unwrap_err(), "ocr-no-engine：无可用 OCR 引擎");
        assert!(ocr_bytes(b"png", None).is_err());
        assert_eq!(ocr_cascade(b"png", None), "");
    }

    #[test]
    fn image_markdown_keeps_the_original_when_nothing_is_recognised() {
        let md = ocr_image_to_md("C:\\docs\\scan.png");
        assert!(md.starts_with("![原图](C:\\docs\\scan.png)\n\n"), "{md}");
        assert!(md.contains("> （未识别出文字，仅保留原图）"), "{md}");
    }

    #[test]
    fn ocr_any_dispatches_on_the_lowercase_extension() {
        let missing = Path::new(&std::env::temp_dir()).join("readmd-missing-x.pdf");
        assert!(ocr_any(missing.to_str().unwrap()).is_err());
        let err = ocr_any("C:\\a\\notes.docx").unwrap_err();
        assert_eq!(err.code, "ocr-unsupported-type：.docx 不是可识别的图片或 PDF 文件");
        assert!(matches!(err.error_code, OcrErrorCode::OcrImageFormatUnsupported));
        let unknown = ocr_any("C:\\a\\notes").unwrap_err();
        assert_eq!(unknown.code, "ocr-unsupported-type：未知类型 不是可识别的图片或 PDF 文件");
    }

    #[test]
    fn splitext_matches_python_on_paths_without_a_directory() {
        // 每个期望值都取自 `os.path.splitext(p)[1].lower()`（CPython 3.11, ntpath）。
        assert_eq!(splitext_lower("notes.TXT"), ".txt");
        assert_eq!(splitext_lower("C:\\a\\b.tar.gz"), ".gz");
        // `C:\a\dir.d`：点在文件名里，前面有非点字符 → 扩展名就是 `.d`。
        assert_eq!(splitext_lower("C:\\a\\dir.d"), ".d");
        assert_eq!(splitext_lower("C:\\a\\.env"), "");
        // “skip all leading dots”：整段前导点都算隐藏名，没有扩展名。
        assert_eq!(splitext_lower("C:\\a\\.dir"), "");
        assert_eq!(splitext_lower(".env"), "");
        assert_eq!(splitext_lower(".."), "");
        assert_eq!(splitext_lower("..file"), "");
        assert_eq!(splitext_lower("a.b/..c"), "");
        // 但前导点之后的点仍然照常切出扩展名。
        assert_eq!(splitext_lower("..env.txt"), ".txt");
        assert_eq!(splitext_lower("dir.foo/.foo"), "");
        // 名字里最后一个点后面什么都没有时，扩展名是那个点本身。
        assert_eq!(splitext_lower("notes."), ".");
        assert_eq!(splitext_lower("x.."), ".");
        // 点落在目录段里不算；空串与纯分隔符没有扩展名。
        assert_eq!(splitext_lower("C:\\a.d\\b"), "");
        assert_eq!(splitext_lower("C:\\a\\dir.d\\"), "");
        assert_eq!(splitext_lower(""), "");
    }

    #[test]
    fn normalize_strips_cjk_gaps_and_joins_broken_lines() {
        assert_eq!(normalize_ocr_text("这 是 一 个 示 例"), "这是一个示例");
        assert_eq!(normalize_ocr_text("hello infor-\nmation"), "hello information");
        assert_eq!(normalize_ocr_text("   "), "");
        // 句末标点后不再拼接下一行。
        assert_eq!(normalize_ocr_text("第一句话结束了。\n第二行文字内容若干"), "第一句话结束了。\n第二行文字内容若干");
    }

    #[test]
    fn normalize_keeps_structured_lines_untouched() {
        let src = "## 标题\n- 列表项一\n正文行";
        assert_eq!(normalize_ocr_text(src), src);
    }

    #[test]
    fn xy_cut_splits_columns_before_rows() {
        let items = vec![
            LayoutItem { text: "左栏甲".into(), x_left: 0.0, y_top: 0.0, width: 40.0, height: 12.0 },
            LayoutItem { text: "左栏乙".into(), x_left: 0.0, y_top: 14.0, width: 40.0, height: 12.0 },
            LayoutItem { text: "右栏甲".into(), x_left: 200.0, y_top: 0.0, width: 40.0, height: 12.0 },
            LayoutItem { text: "右栏乙".into(), x_left: 200.0, y_top: 14.0, width: 40.0, height: 12.0 },
        ];
        assert_eq!(xy_cut_lines(&items), vec!["左栏甲", "左栏乙", "右栏甲", "右栏乙"]);
    }

    #[test]
    fn xy_cut_single_item_drops_blank_text() {
        assert_eq!(xy_cut_lines(&[LayoutItem { text: " ".into(), ..item() }]), Vec::<String>::new());
        assert_eq!(xy_cut_lines(&[]), Vec::<String>::new());
    }

    fn item() -> LayoutItem {
        LayoutItem { text: String::new(), x_left: 0.0, y_top: 0.0, width: 10.0, height: 10.0 }
    }

    #[test]
    fn pdf_without_a_text_layer_yields_the_placeholder() {
        let dir = std::env::temp_dir().join(format!("readmd-ocr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("blank.pdf");
        // 一份最小 PDF：页面树里一个页都没有（也没有 xref，MuPDF 会自己重建），
        // `fitz.open` 成功且 `page_count == 0` → 逐页结果为空 → 占位符。
        std::fs::write(
            &file,
            b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
              2 0 obj<</Type/Pages/Kids[]/Count 0>>endobj\n\
              trailer<</Root 1 0 R>>\n%%EOF\n",
        )
        .unwrap();
        let text = ocr_any(file.to_str().unwrap()).unwrap();
        assert_eq!(text, OCR_PDF_EMPTY_PLACEHOLDER);

        // 反面对照：MuPDF 自己也打不开的那批输入仍然报错，恢复步不能把“读不出”
        // 说成“没有文字”。两者都实测过 `fitz.open` → FileDataError。
        let bare = dir.join("bare.pdf");
        std::fs::write(&bare, b"%PDF-1.4\n%%EOF\n").unwrap();
        assert!(ocr_any(bare.to_str().unwrap()).is_err());
        let garbage = dir.join("garbage.pdf");
        std::fs::write(&garbage, b"not a pdf, just text\n").unwrap();
        let err = ocr_any(garbage.to_str().unwrap()).unwrap_err();
        assert_eq!(err.code, format!("ocr-pdf-read-failed：{}", garbage.to_str().unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
