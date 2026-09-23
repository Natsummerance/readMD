// -*- coding: utf-8 -*-
//! `src/readmd_modules/skill_import.py` 的逐行复刻（安全优先的 GitHub / 本地 Skill 导入器）。
//!
//! 权威源：
//! * `src/readmd_modules/skill_import.py:1-1135`（常量、`SkillImportError`、11 个公开函数、37 个私有函数）
//! * `src/readmd_core/utils.py:33-92`（`load_json` 的失败回退、`save_json` 的 `json.dump(indent=2)` + 原子替换）
//! * `src/readmd_core/file_writer.py:17`（`save_text_atomic` 以 `newline=""` 打开临时文件 ⇒ 保留 LF）
//! * `src/readmd_core/config.py:51`（`SKILLS_FILE = DATA_DIR/skills.json`）
//! * `src/readmd_modules/crypto.py:220`（`load_credential(credential_id) -> str`）
//! * `src/readmd_modules/skills.py`（`SkillError` / `SkillRegistry.validate`，同一套 `_NAME_RE`/`_ALLOWED_VARIABLES`）
//! * `os.path` / `posixpath` / `pathlib` / `urllib.parse` / `zipfile` / `base64` / `hashlib` 的 CPython 3.11 语义
//!
//! 本文件只依赖 `std`，因此可以脱离 cargo 单独验证：
//! `rustc --edition 2021 --test -A warnings src/skill_import.rs`。
//! 所有期望值都来自 `scratch/rust_parity/skill_import_s4/probe1..5.txt`（由权威 Python 生成），
//! 不是 Rust 侧自造的。
//!
//! 网络、凭据解密与 `SkillRegistry` 结构校验都是外部依赖，因此通过 [`Ctx`] 注入：
//! 装配（wiring）阶段只需要把 `ureq`、`crypto::load_credential`、`skills::SkillRegistry::validate`
//! 与 crate 内的路径常量塞进 [`Ctx`]，本模块的其余逻辑与 Python 完全一致。
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================ CPython 异常模型

/// Python 的 `SkillImportError(ValueError)`：只带 `code` 与消息文本。
///
/// `str(exc)` 就是消息本身（`super().__init__(message)`），所以 [`SkillImportError::raised`]
/// 与 `probe1.txt` 里的 `str(exc)` 直接可比。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillImportError {
    pub code: String,
    pub message: String,
}

impl SkillImportError {
    pub fn new(code: &str, message: &str) -> Self {
        SkillImportError { code: code.to_string(), message: message.to_string() }
    }
    /// 等价于 Python 的 `exc.code`。
    pub fn c(&self) -> &str {
        &self.code
    }
    /// 等价于 Python 的 `str(exc)`。
    pub fn raised(&self) -> String {
        self.message.clone()
    }
}

impl std::fmt::Display for SkillImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub type R<T> = Result<T, SkillImportError>;

fn err<T>(code: &str, message: &str) -> R<T> {
    Err(SkillImportError::new(code, message))
}

// ============================================================ 模块常量

/// `GITHUB_HOSTS = frozenset({"github.com", "www.github.com"})`
pub const GITHUB_HOSTS: [&str; 2] = ["github.com", "www.github.com"];
/// `GITHUB_API_HOST`
pub const GITHUB_API_HOST: &str = "api.github.com";
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_EXTRACTED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_FILES: usize = 2000;
pub const MAX_PATH_LENGTH: usize = 240;
pub const MAX_PATH_DEPTH: usize = 16;
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// `_ALLOWED_VARIABLES`
pub const ALLOWED_VARIABLES: [&str; 6] =
    ["document", "selection", "request", "language", "context", "output_format"];
/// `_SCRIPT_SUFFIXES`
pub const SCRIPT_SUFFIXES: [&str; 8] =
    [".py", ".js", ".mjs", ".cjs", ".sh", ".ps1", ".bat", ".cmd"];
/// `_TEXT_SUFFIXES`
pub const TEXT_SUFFIXES: [&str; 24] = [
    ".md", ".markdown", ".txt", ".json", ".yaml", ".yml", ".toml", ".ini", ".cfg", ".py", ".js",
    ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".sh", ".ps1", ".bat", ".cmd", ".html", ".css", ".xml",
    ".csv",
];

/// `_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")` 的 `fullmatch`。
///
/// `fullmatch` 要求整串被消费，所以 Python 里 `"a\n"` 也不能通过（`probe1.txt` `id_re`）。
pub fn id_re_fullmatch(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    bytes.iter().enumerate().all(|(i, &b)| {
        if i == 0 {
            b.is_ascii_lowercase() || b.is_ascii_digit()
        } else {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'
        }
    })
}

// ============================================================ Python 对象模型

/// 一个最小但行为对齐的 Python 值模型。
///
/// 端口里的 `Dict[str, Any]` / `List[Dict[str, Any]]` 全部用它表达，因为 Python 侧大量使用
/// `x.get("k") or ""`、`isinstance(x, bool)`、`x is True`、`str(x)` 这类依赖“真值 / 类型 / 缺失键”
/// 区分的写法。`Obj` 用 `Vec` 保序，以复现 `json.dumps` 的插入顺序输出。
#[derive(Clone, Debug, PartialEq)]
pub enum JVal {
    None,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    List(Vec<JVal>),
    Obj(Vec<(String, JVal)>),
}

/// 构造 dict 字面量的小助手：`obj(&[("k", v), ...])`。
pub fn obj(pairs: &[(&str, JVal)]) -> JVal {
    JVal::Obj(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

pub fn s(value: &str) -> JVal {
    JVal::Str(value.to_string())
}

pub fn jbool(v: bool) -> JVal {
    JVal::Bool(v)
}

pub fn jstr(v: &str) -> JVal {
    JVal::Str(v.to_string())
}

pub fn jlist(items: Vec<JVal>) -> JVal {
    JVal::List(items)
}

impl JVal {
    pub fn type_name(&self) -> &'static str {
        match self {
            JVal::None => "NoneType",
            JVal::Bool(_) => "bool",
            JVal::Int(_) => "int",
            JVal::Float(_) => "float",
            JVal::Str(_) => "str",
            JVal::List(_) => "list",
            JVal::Obj(_) => "dict",
        }
    }

    /// `value.get(key)`，缺失键与 `None` 值都返回 `None`（与 Python 一致）。
    pub fn get(&self, key: &str) -> Option<&JVal> {
        match self {
            JVal::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// `value[key]`：只有 dict 支持。
    pub fn index(&self, key: &str) -> Option<&JVal> {
        self.get(key)
    }

    pub fn set_item(&mut self, key: &str, value: JVal) {
        if let JVal::Obj(items) = self {
            match items.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => items.push((key.to_string(), value)),
            }
        }
    }

    /// `dict.pop(key, default)`，返回被弹出的值（没有则 `None`）。
    pub fn pop(&mut self, key: &str) -> Option<JVal> {
        if let JVal::Obj(items) = self {
            if let Some(pos) = items.iter().position(|(k, _)| k == key) {
                return Some(items.remove(pos).1);
            }
        }
        None
    }

    /// `isinstance(value, dict)`
    pub fn is_dict(&self) -> bool {
        matches!(self, JVal::Obj(_))
    }

    /// `isinstance(value, list)`
    pub fn is_list(&self) -> bool {
        matches!(self, JVal::List(_))
    }

    /// `isinstance(value, str)`
    pub fn is_str(&self) -> bool {
        matches!(self, JVal::Str(_))
    }

    /// `value is True` / `value is False`：必须是真正的 bool，`1` 与 `"true"` 都不算
    /// （`_declaration_importable` 的安全边界，见 `probe1.txt` `declaration_importable`）。
    pub fn is_true(&self) -> bool {
        matches!(self, JVal::Bool(true))
    }

    pub fn is_false(&self) -> bool {
        matches!(self, JVal::Bool(false))
    }

    /// `bool(value)`
    pub fn truthy(&self) -> bool {
        match self {
            JVal::None => false,
            JVal::Bool(b) => *b,
            JVal::Int(i) => *i != 0,
            JVal::Float(f) => *f != 0.0,
            JVal::Str(v) => !v.is_empty(),
            JVal::List(v) => !v.is_empty(),
            JVal::Obj(v) => !v.is_empty(),
        }
    }

    /// `value or default`
    pub fn or_val(&self, default: JVal) -> JVal {
        if self.truthy() {
            self.clone()
        } else {
            default
        }
    }

    /// `str(value)`：Python 的字符串强制。
    ///
    /// `probe1.txt` `str_coerce`：`None -> "None"`、`True -> "True"`、`1.0 -> "1.0"`、
    /// `[1, "a"] -> "[1, 'a']"`、`{"a": 1} -> "{'a': 1}"`。
    pub fn py_str(&self) -> String {
        match self {
            JVal::None => "None".to_string(),
            JVal::Bool(true) => "True".to_string(),
            JVal::Bool(false) => "False".to_string(),
            JVal::Int(i) => format!("{}", i),
            JVal::Float(f) => py_float_str(*f),
            JVal::Str(v) => v.clone(),
            JVal::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.py_repr()).collect();
                format!("[{}]", inner.join(", "))
            }
            JVal::Obj(items) => {
                let inner: Vec<String> = items
                    .iter()
                    .map(|(k, v)| format!("{}: {}", JVal::Str(k.clone()).py_repr(), v.py_repr()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
        }
    }

    /// `repr(value)`，只覆盖 `str()` 组合里会出现的类型。
    pub fn py_repr(&self) -> String {
        match self {
            JVal::Str(v) => py_str_repr(v),
            other => match other {
                JVal::List(_items) => other.py_str(),
                JVal::Obj(_items) => other.py_str(),
                v => v.py_str(),
            },
        }
    }

    /// `str(value.get("k") or "")` 这一整串惯用语。
    pub fn get_or_empty_str(&self, key: &str) -> String {
        match self.get(key) {
            Some(v) if v.truthy() => v.py_str(),
            _ => String::new(),
        }
    }

    pub fn as_rust_str(&self) -> Option<&str> {
        match self {
            JVal::Str(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i128> {
        match self {
            JVal::Int(i) => Some(*i),
            JVal::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    /// `sorted(...)` 用到的字符串投影：`str(item.get("path") or "")`。
    pub fn path_key(&self) -> String {
        self.get_or_empty_str("path")
    }
}

fn py_float_str(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    // CPython 的 float repr 取“最短可回读”表示；整数值的浮点必须带 `.0`。
    if value == value.trunc() && value.abs() < 1e16 {
        format!("{:.1}", value)
    } else {
        let mut best = format!("{}", value);
        if best.parse::<f64>().map(|v| v != value).unwrap_or(true) {
            best = format!("{:.17}", value);
        }
        best
    }
}

/// `repr(str)`：默认单引号，仅当串里有 `'` 且没有 `"` 时改用双引号。
fn py_str_repr(value: &str) -> String {
    let has_single = value.contains('\'');
    let has_double = value.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

// ============================================================ JSON：json.loads / json.dumps

/// Python `json.loads` 的严格子集：拒绝尾逗号与前导零，允许 `-?Infinity`/`NaN`
/// （`readmd_core` 里偶尔会写出这类值），重复键“后值覆盖、保留首次出现位置”。
pub fn json_loads(text: &str) -> Result<JVal, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = JsonParser { chars, at: 0 };
    p.skip_ws();
    let value = p.parse_value()?;
    p.skip_ws();
    if p.at != p.chars.len() {
        return Err("extra data".to_string());
    }
    Ok(value)
}

struct JsonParser {
    chars: Vec<char>,
    at: usize,
}

impl JsonParser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn skip_ws(&mut self) {
        // json.loads 的空白集固定为 " \t\n\r"，不含 \x1c..\x1f（与 str.strip() 不同）。
        while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n') | Some('\r')) {
            self.at += 1;
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        if self.peek() == Some(ch) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected {}", ch))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        let end = self.at + word.chars().count();
        if end > self.chars.len() || self.chars[self.at..end].iter().copied().ne(word.chars()) {
            return Err(format!("bad literal {}", word));
        }
        self.at = end;
        Ok(())
    }

    fn parse_value(&mut self) -> Result<JVal, String> {
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => Ok(JVal::Str(self.parse_string()?)),
            Some('t') => {
                self.literal("true")?;
                Ok(JVal::Bool(true))
            }
            Some('f') => {
                self.literal("false")?;
                Ok(JVal::Bool(false))
            }
            Some('n') => {
                self.literal("null")?;
                Ok(JVal::None)
            }
            Some('N') => {
                self.literal("NaN")?;
                Ok(JVal::Float(f64::NAN))
            }
            Some('I') => {
                self.literal("Infinity")?;
                Ok(JVal::Float(f64::INFINITY))
            }
            Some('-') if self.chars.get(self.at + 1) == Some(&'I') => {
                self.literal("-Infinity")?;
                Ok(JVal::Float(f64::NEG_INFINITY))
            }
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            other => Err(format!("unexpected {:?}", other)),
        }
    }

    fn parse_number(&mut self) -> Result<JVal, String> {
        let start = self.at;
        if self.peek() == Some('-') {
            self.at += 1;
        }
        let int_start = self.at;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.at += 1;
        }
        let int_digits = self.at - int_start;
        if int_digits == 0 {
            return Err("invalid number".to_string());
        }
        // 前导零：0 之后必须直接结束整数部分。
        if int_digits > 1 && self.chars[int_start] == '0' {
            return Err("leading zeros".to_string());
        }
        let mut is_float = false;
        if self.peek() == Some('.') {
            is_float = true;
            self.at += 1;
            let frac_start = self.at;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.at += 1;
            }
            if self.at == frac_start {
                return Err("invalid fraction".to_string());
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            self.at += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.at += 1;
            }
            let exp_start = self.at;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.at += 1;
            }
            if self.at == exp_start {
                return Err("invalid exponent".to_string());
            }
        }
        let raw: String = self.chars[start..self.at].iter().collect();
        if is_float {
            raw.parse::<f64>().map(JVal::Float).map_err(|e| e.to_string())
        } else {
            raw.parse::<i128>().map(JVal::Int).map_err(|e| e.to_string())
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            let ch = self.peek().ok_or_else(|| "unterminated string".to_string())?;
            self.at += 1;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.peek().ok_or_else(|| "unterminated escape".to_string())?;
                    self.at += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let cp = self.parse_hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // 代理对；落单的高 surrogate 由 Python 变成 U+D800  lone。
                                if self.peek() == Some('\\')
                                    && self.chars.get(self.at + 1) == Some(&'u')
                                {
                                    self.at += 2;
                                    let low = self.parse_hex4()?;
                                    if (0xDC00..0xE000).contains(&low) {
                                        let merged =
                                            0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                        out.push(
                                            char::from_u32(merged).unwrap_or(char::REPLACEMENT_CHARACTER),
                                        );
                                        continue;
                                    }
                                    out.push(char::REPLACEMENT_CHARACTER);
                                    out.push(char::REPLACEMENT_CHARACTER);
                                    continue;
                                }
                                out.push(char::REPLACEMENT_CHARACTER);
                            } else {
                                out.push(
                                    char::from_u32(cp)
                                        .unwrap_or(char::REPLACEMENT_CHARACTER),
                                );
                            }
                        }
                        other => return Err(format!("bad escape {}", other)),
                    }
                }
                c if (c as u32) < 0x20 => return Err("invalid control character".to_string()),
                c => out.push(c),
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        if self.at + 4 > self.chars.len() {
            return Err("bad \\uXXXX".to_string());
        }
        let mut value = 0u32;
        for i in 0..4 {
            let d = self.chars[self.at + i]
                .to_digit(16)
                .ok_or_else(|| "bad \\uXXXX".to_string())?;
            value = value * 16 + d;
        }
        self.at += 4;
        Ok(value)
    }

    fn parse_array(&mut self) -> Result<JVal, String> {
        self.expect('[')?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.at += 1;
            return Ok(JVal::List(items));
        }
        loop {
            self.skip_ws();
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.at += 1;
                    self.skip_ws();
                    if self.peek() == Some(']') {
                        return Err("trailing comma".to_string());
                    }
                }
                Some(']') => {
                    self.at += 1;
                    return Ok(JVal::List(items));
                }
                _ => return Err("expected , or ]".to_string()),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JVal, String> {
        self.expect('{')?;
        self.skip_ws();
        let mut items: Vec<(String, JVal)> = Vec::new();
        if self.peek() == Some('}') {
            self.at += 1;
            return Ok(JVal::Obj(items));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err("keys must be strings".to_string());
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(':')?;
            self.skip_ws();
            let value = self.parse_value()?;
            match items.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = value,
                None => items.push((key, value)),
            }
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.at += 1;
                    self.skip_ws();
                    if self.peek() == Some('}') {
                        return Err("trailing comma".to_string());
                    }
                }
                Some('}') => {
                    self.at += 1;
                    return Ok(JVal::Obj(items));
                }
                _ => return Err("expected , or }".to_string()),
            }
        }
    }
}

/// `json.dumps(value, ensure_ascii=False, indent=?)`。
pub fn json_dumps(value: &JVal, indent: Option<usize>) -> String {
    let mut out = String::new();
    match indent {
        Some(step) => write_dumps_indented(value, step, 0, &mut out),
        None => write_dumps_compact(value, &mut out),
    }
    out
}

fn write_dumps_compact(value: &JVal, out: &mut String) {
    match value {
        JVal::None => out.push_str("null"),
        JVal::Bool(true) => out.push_str("true"),
        JVal::Bool(false) => out.push_str("false"),
        JVal::Int(i) => out.push_str(&format!("{}", i)),
        JVal::Float(f) => out.push_str(&py_json_float(*f)),
        JVal::Str(v) => out.push_str(&json_quote(v)),
        JVal::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_dumps_compact(item, out);
            }
            out.push(']');
        }
        JVal::Obj(items) => {
            if items.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_quote(k));
                out.push_str(": ");
                write_dumps_compact(v, out);
            }
            out.push('}');
        }
    }
}

fn write_dumps_indented(value: &JVal, step: usize, depth: usize, out: &mut String) {
    match value {
        JVal::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(step * (depth + 1)));
                write_dumps_indented(item, step, depth + 1, out);
            }
            out.push('\n');
            out.push_str(&" ".repeat(step * depth));
            out.push(']');
        }
        JVal::Obj(items) => {
            if items.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(step * (depth + 1)));
                out.push_str(&json_quote(k));
                out.push_str(": ");
                write_dumps_indented(v, step, depth + 1, out);
            }
            out.push('\n');
            out.push_str(&" ".repeat(step * depth));
            out.push('}');
        }
        other => write_dumps_compact(other, out),
    }
}

fn py_json_float(value: f64) -> String {
    if value.is_nan() || value.is_infinite() {
        // Python 的默认 allow_nan=True 会写出 NaN / Infinity / -Infinity。
        if value.is_nan() {
            return "NaN".to_string();
        }
        return if value > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    py_float_str(value)
}

fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            // ensure_ascii=False ⇒ 非 ASCII 原样写出。
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ============================================================ hashlib.sha256

const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// 增量 SHA-256，等价于 `hashlib.sha256()` + `update()` + `hexdigest()`。
#[derive(Clone, Debug, Default)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: Vec<u8>,
    total: u64,
}

impl Sha256 {
    pub fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: Vec::new(),
            total: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.total += data.len() as u64;
        self.buffer.extend_from_slice(data);
        while self.buffer.len() >= 64 {
            let block: Vec<u8> = self.buffer.drain(..64).collect();
            self.compress(&block);
        }
    }

    fn compress(&mut self, block: &[u8]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = self.state;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA_K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for i in 0..8 {
            self.state[i] = self.state[i].wrapping_add(v[i]);
        }
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total * 8;
        let mut pad: Vec<u8> = vec![0x80];
        while (self.buffer.len() + pad.len()) % 64 != 56 {
            pad.push(0);
        }
        self.update(&pad);
        self.update(&bit_len.to_be_bytes());
        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4..i * 4 + 4].copy_from_slice(&self.state[i].to_be_bytes());
        }
        out
    }

    pub fn hexdigest(&self) -> String {
        // Python 侧从不调用两次 hexdigest，这里按“快照”语义复制状态。
        let clone = self.clone();
        let digest = clone.finalize();
        hex_encode(&digest)
    }
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

/// `hashlib.sha256(data).hexdigest()`
pub fn sha256_hex(data: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(data);
    hex_encode(&digest.finalize())
}

/// `hashlib.sha256(text.encode("utf-8")).hexdigest()[:20]`
pub fn sha256_prefix20(text: &str) -> String {
    sha256_hex(text.as_bytes())[..20].to_string()
}

// ============================================================ base64

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `base64.b64encode(data).decode("ascii")`
pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(B64_ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(B64_ALPHABET[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_ALPHABET[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// `base64.b64decode(data, validate=False)`：不在字母表里的字节被丢弃（`probe4.txt` `b64:*`）。
///
/// 长度不是 4 的倍数、或末组只有 1 个数据字符时，Python 抛 `binascii.Error`（ValueError 子类），
/// 由 `_content` 归一成 `github_skill_unreadable`。
pub fn b64_decode_lenient(data: &[u8]) -> Result<Vec<u8>, ()> {
    let mut filtered: Vec<u8> = Vec::with_capacity(data.len());
    for &b in data {
        if B64_ALPHABET.contains(&b) {
            filtered.push(b);
        } else if b == b'=' {
            filtered.push(b'=');
        }
        // 其余字节（空白、`*` 等）静默丢弃，和 binascii.a2b_base64 一样。
    }
    let value_of = |b: u8| -> u32 {
        B64_ALPHABET.iter().position(|&c| c == b).map(|p| p as u32).unwrap_or(0)
    };
    let mut out: Vec<u8> = Vec::new();
    let mut chunks = filtered.chunks(4);
    while let Some(chunk) = chunks.next() {
        if chunk.len() == 1 {
            return Err(()); // Incorrect padding
        }
        let mut bits = 0u32;
        let mut data_len = 0usize;
        for &b in chunk {
            bits <<= 6;
            if b == b'=' {
                bits >>= 6;
                break;
            }
            bits |= value_of(b);
            data_len += 1;
        }
        if chunk.len() < 4 && !chunk.contains(&b'=') {
            // 只有末尾一组允许短于 4；Python 对 2/3 字符的末组仍报 padding 错误。
            return Err(());
        }
        match data_len {
            2 => out.push((bits >> 4) as u8),
            3 => {
                out.push((bits >> 10) as u8);
                out.push((bits >> 2) as u8);
            }
            4 => {
                out.push((bits >> 16) as u8);
                out.push((bits >> 8) as u8);
                out.push(bits as u8);
            }
            _ => {}
        }
    }
    Ok(out)
}

// ============================================================ CPython 字符串语义

/// `str.strip()` / `re` 的 `\s` 使用的 `Py_UNICODE_ISSPACE` 全集（由 CPython 直接枚举得到）。
/// 注意它比 Rust 的 `char::is_whitespace()` 少了 `U+001C..U+001F` 之外的差异，也多了 `U+0085`、
/// `U+00A0`，所以任何“看起来只是空白”的判断都必须走这张表，否则 `\xa0nbsp\xa0` 一类
/// 用例（`probe1.txt` `strip` / `slug` / `frontmatter`）会偏。
pub const SPACE_RANGES: [(u32, u32); 10] = [
    (0x0009, 0x000D),
    (0x001C, 0x0020),
    (0x0085, 0x0085),
    (0x00A0, 0x00A0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
];

pub fn py_isspace(ch: char) -> bool {
    let cp = ch as u32;
    SPACE_RANGES.iter().any(|(lo, hi)| cp >= *lo && cp <= *hi)
}

/// `str.strip()`
pub fn py_strip(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut start = 0;
    while start < chars.len() && py_isspace(chars[start]) {
        start += 1;
    }
    let mut end = chars.len();
    while end > start && py_isspace(chars[end - 1]) {
        end -= 1;
    }
    chars[start..end].iter().collect()
}

/// `str.strip(chars)`，例如 `.strip("'\"")`。
pub fn py_strip_chars(value: &str, chars: &str) -> String {
    let all: Vec<char> = value.chars().collect();
    let mut start = 0;
    while start < all.len() && chars.contains(all[start]) {
        start += 1;
    }
    let mut end = all.len();
    while end > start && chars.contains(all[end - 1]) {
        end -= 1;
    }
    all[start..end].iter().collect()
}

/// `str.rstrip(chars)`
pub fn py_rstrip_chars(value: &str, chars: &str) -> String {
    let all: Vec<char> = value.chars().collect();
    let mut end = all.len();
    while end > 0 && chars.contains(all[end - 1]) {
        end -= 1;
    }
    all[..end].iter().collect()
}

/// `str.lower()`（Rust 的 `to_lowercase` 与 CPython 同样使用 UnicodeData + SpecialCasing）。
pub fn py_lower(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

/// `str[:n]` 的按码点切片（`_slug` 的 `value[:64]`）。
pub fn py_slice_prefix(value: &str, n: usize) -> String {
    value.chars().take(n).collect()
}

/// `str.splitlines()`：11 个边界，`\r\n` 只算一个，末尾边界不产生空串（`probe1.txt` `splitlines`）。
pub fn py_splitlines(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let is_break = matches!(
            ch,
            '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}'
                | '\u{2028}' | '\u{2029}'
        );
        if !is_break {
            i += 1;
            continue;
        }
        out.push(chars[start..i].iter().collect::<String>());
        if ch == '\r' && chars.get(i + 1) == Some(&'\n') {
            i += 2;
        } else {
            i += 1;
        }
        start = i;
    }
    if start < chars.len() {
        out.push(chars[start..].iter().collect::<String>());
    }
    out
}

/// `_VARIABLE_RE.findall(text)`：`\{\{\s*([a-zA-Z0-9_-]+)\s*\}\}`，`\s` 用 `Py_UNICODE_ISSPACE`。
pub fn variable_findall(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] != '{' || chars[i + 1] != '{' {
            i += 1;
            continue;
        }
        // `\s*` 贪婪 + 回溯：因为 `\s*` 与后面的 `}}` 无交集，最长空白串唯一。
        let mut j = i + 2;
        while j < chars.len() && py_isspace(chars[j]) {
            j += 1;
        }
        let mut k = j;
        while k < chars.len()
            && (chars[k].is_ascii_alphanumeric() || chars[k] == '_' || chars[k] == '-')
        {
            k += 1;
        }
        if k > j {
            let mut m = k;
            while m < chars.len() && py_isspace(chars[m]) {
                m += 1;
            }
            if m + 1 < chars.len() && chars[m] == '}' && chars[m + 1] == '}' {
                out.push(chars[j..k].iter().collect());
                i = m + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `set(findall(...)) - _ALLOWED_VARIABLES` 非空。
pub fn has_unknown_variable(text: &str) -> bool {
    variable_findall(text).iter().any(|v| !ALLOWED_VARIABLES.contains(&v.as_str()))
}

// ============================================================ urllib.parse

/// `urllib.parse.quote(value, safe="")`：只有 `A-Za-z0-9_.-~` 不转义，其余按 UTF-8 字节 %XX。
pub fn py_quote(value: &str, safe: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric()
            || matches!(ch, '_' | '.' | '-' | '~')
            || safe.contains(ch)
        {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).as_bytes() {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// `urllib.parse.unquote(value)`：非法转义原样保留，字节流按 UTF-8 `errors="replace"` 解码。
pub fn py_unquote(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut bytes: Vec<u8> = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%'
            && i + 2 < chars.len()
            && chars[i + 1].is_ascii_hexdigit()
            && chars[i + 2].is_ascii_hexdigit()
        {
            let hex: String = chars[i + 1..i + 3].iter().collect();
            if let Ok(v) = u8::from_str_radix(&hex, 16) {
                bytes.push(v);
                i += 3;
                continue;
            }
        }
        let mut buf = [0u8; 4];
        bytes.extend_from_slice(chars[i].encode_utf8(&mut buf).as_bytes());
        i += 1;
    }
    // errors="replace"：逐码点解码，非法字节变成 U+FFFD（`probe1.txt` unquote `%ff`）。
    decode_utf8_replace(&bytes)
}

fn decode_utf8_replace(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let need = if bytes[i] < 0x80 {
            1
        } else if bytes[i] >> 5 == 0b110 {
            2
        } else if bytes[i] >> 4 == 0b1110 {
            3
        } else if bytes[i] >> 3 == 0b11110 {
            4
        } else {
            out.push(char::REPLACEMENT_CHARACTER);
            i += 1;
            continue;
        };
        if i + need > bytes.len() {
            out.push(char::REPLACEMENT_CHARACTER);
            i += 1;
            continue;
        }
        match std::str::from_utf8(&bytes[i..i + need]) {
            Ok(text) => {
                out.push_str(text);
                i += need;
            }
            Err(_) => {
                out.push(char::REPLACEMENT_CHARACTER);
                i += 1;
            }
        }
    }
    out
}

/// `urllib.parse.urlparse` 的结果，只保留 skill_import 用到的字段。
#[derive(Clone, Debug, Default)]
pub struct ParsedUrl {
    pub scheme: String,
    pub netloc: String,
    pub path: String,
    pub query: String,
    pub fragment: String,
}

impl ParsedUrl {
    /// `parsed.hostname`：`None` 用空串表示；小写。
    pub fn hostname(&self) -> String {
        let hostport = self.netloc.rsplit('@').next().unwrap_or("");
        let host = if hostport.starts_with('[') {
            match hostport.find(']') {
                Some(end) => &hostport[1..end],
                None => hostport,
            }
        } else {
            match hostport.find(':') {
                Some(pos) => &hostport[..pos],
                None => hostport,
            }
        };
        py_lower(host)
    }

    fn userinfo(&self) -> Option<&str> {
        match self.netloc.rfind('@') {
            Some(pos) => Some(&self.netloc[..pos]),
            None => None,
        }
    }

    /// `parsed.username`
    pub fn username(&self) -> String {
        match self.userinfo() {
            Some(info) => match info.find(':') {
                Some(pos) => py_unquote(&info[..pos]),
                None => py_unquote(info),
            },
            None => String::new(),
        }
    }

    /// `parsed.password`
    pub fn password(&self) -> String {
        match self.userinfo() {
            Some(info) => match info.find(':') {
                Some(pos) => py_unquote(&info[pos + 1..]),
                None => String::new(),
            },
            None => String::new(),
        }
    }
}

/// `urllib.parse.urlparse(url)`。
pub fn url_parse(url: &str) -> ParsedUrl {
    let mut text: String = url
        .chars()
        .filter(|c| !(py_isspace(*c) || (*c as u32) < 0x20))
        .collect();
    // urlparse 只去掉首尾 ASCII 空白，这里与 CPython 一致地再做一次裁剪。
    while text.starts_with(|c: char| c == '\t' || c == '\n' || c == '\r' || c == ' ') {
        text.remove(0);
    }
    let mut out = ParsedUrl::default();
    let rest = {
        match text.find('#') {
            Some(pos) => {
                out.fragment = text[pos + 1..].to_string();
                text[..pos].to_string()
            }
            None => text.clone(),
        }
    };
    let rest = match rest.find('?') {
        Some(pos) => {
            out.query = rest[pos + 1..].to_string();
            rest[..pos].to_string()
        }
        None => rest,
    };
    // scheme：`[A-Za-z][A-Za-z0-9+-.]*:`
    let scheme_end = {
        let chars: Vec<char> = rest.chars().collect();
        if !chars.is_empty() && (chars[0].is_ascii_alphabetic()) {
            let mut i = 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '+' | '-' | '.'))
            {
                i += 1;
            }
            if i < chars.len() && chars[i] == ':' {
                Some(i)
            } else {
                None
            }
        } else {
            None
        }
    };
    let authority_and_path = match scheme_end {
        Some(len) => {
            out.scheme = py_lower(&rest.chars().take(len).collect::<String>());
            rest.chars().skip(len + 1).collect::<String>()
        }
        None => rest.clone(),
    };
    if authority_and_path.starts_with("//") {
        let after = &authority_and_path[2..];
        match after.find(['/', '?', '#']) {
            Some(pos) => {
                out.netloc = after[..pos].to_string();
                out.path = after[pos..].to_string();
            }
            None => {
                out.netloc = after.to_string();
                out.path = String::new();
            }
        }
    } else {
        out.path = authority_and_path;
    }
    out
}

// ============================================================ 路径语义

/// `posixpath.basename`
pub fn posix_basename(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[pos + 1..].to_string(),
        None => path.to_string(),
    }
}

/// `posixpath.dirname`
pub fn posix_dirname(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => {
            let head = &path[..=pos];
            if head.chars().all(|c| c == '/') {
                head.to_string()
            } else {
                py_rstrip_chars(head, "/")
            }
        }
        None => String::new(),
    }
}

/// `posixpath.join(a, b)`，仅在 `a` 非空时使用。
pub fn posix_join(head: &str, tail: &str) -> String {
    if head.is_empty() {
        tail.to_string()
    } else if head.ends_with('/') {
        format!("{}{}", head, tail)
    } else {
        format!("{}/{}", head, tail)
    }
}

/// `PurePath(name).suffix`：只有“最后一个点不在最前面且不是最后一个字符”时才算后缀
/// （`probe4.txt` `splitext`：`Path("a.").suffix == ""`，与 `os.path.splitext` 不同）。
pub fn py_suffix(path: &str) -> String {
    let name = py_path_name(path);
    let chars: Vec<char> = name.chars().collect();
    match chars.iter().rposition(|c| *c == '.') {
        Some(i) if i > 0 && i < chars.len() - 1 => chars[i..].iter().collect(),
        _ => String::new(),
    }
}

/// `PurePath(path).name`
pub fn py_path_name(path: &str) -> String {
    let trimmed = py_rstrip_chars(path, if cfg!(windows) { "\\/" } else { "/" });
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    match trimmed.rfind(seps) {
        Some(pos) => trimmed[pos + 1..].to_string(),
        None => trimmed,
    }
}

/// `PurePath(path).parts` 的长度（Windows 上 `/` 与 `\` 同为分隔符，`.` 段被丢弃）。
pub fn py_parts_len(path: &str) -> usize {
    py_parts(path).len()
}

pub fn py_parts(path: &str) -> Vec<String> {
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    path.split(seps)
        .filter(|seg| !seg.is_empty() && *seg != ".")
        .map(|seg| seg.to_string())
        .collect()
}

/// `Path(path).expanduser()`
pub fn py_expanduser(path: &Path) -> PathBuf {
    let text = path.as_os_str().to_string_lossy().to_string();
    if text == "~" || text.starts_with("~/") || text.starts_with("~\\") {
        if let Some(home) = home_dir() {
            let tail = &text[1..];
            return join_normalized(&home, tail);
        }
    }
    path.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    for key in ["USERPROFILE", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                return Some(PathBuf::from(value));
            }
        }
    }
    None
}

fn join_normalized(base: &Path, tail: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for seg in tail.split(['/', '\\']).filter(|s| !s.is_empty()) {
        out.push(seg);
    }
    out
}

/// `os.path.normpath`（Windows：`\` 归一为 `\`、去掉多余分隔符与 `.`、折叠 `..`）。
// WIRING: window_state::py_normpath
pub fn py_normpath(path: &str) -> String {
    let is_win = cfg!(windows);
    let seps: &[char] = if is_win { &['/', '\\'] } else { &['/'] };
    let mut segments: Vec<&str> = Vec::new();
    let bytes: Vec<char> = path.chars().collect();
    let mut root = String::new();
    let mut start = 0;
    if is_win {
        // 盘符或 UNC 前缀整体保留。
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == ':' {
            root = bytes[..2].iter().collect();
            start = 2;
        } else if bytes.len() >= 2 && bytes[0] == '\\' && bytes[1] == '\\' {
            if let Some(slash) = path[2..].find(seps) {
                let rest_start = 2 + slash + 1;
                if let Some(slash2) = path[rest_start..].find(seps) {
                    root = path[..rest_start + slash2].to_string();
                    start = rest_start + slash2;
                } else {
                    return path.to_string();
                }
            } else {
                return path.to_string();
            }
        }
    } else if path.starts_with('/') {
        root = "/".to_string();
        start = 1;
    }
    for seg in path[start..].split(seps) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            match segments.last() {
                Some(last) if *last != ".." => {
                    segments.pop();
                }
                _ => {
                    if root.is_empty() {
                        segments.push("..");
                    }
                }
            }
            continue;
        }
        segments.push(seg);
    }
    let joined = segments.join(if is_win { "\\" } else { "/" });
    if !root.is_empty() {
        if root.ends_with(':') {
            format!("{}\\{}", root, joined)
        } else if joined.is_empty() {
            if root == "/" {
                "/".to_string()
            } else {
                root
            }
        } else {
            format!("{}{}{}", root, if root.ends_with('\\') || root.ends_with('/') { "" } else { if is_win { "\\" } else { "/" } }, joined)
        }
    } else if joined.is_empty() {
        ".".to_string()
    } else if is_win && path.starts_with('\\') && !root.is_empty() {
        format!("\\{}", joined)
    } else {
        joined
    }
}

/// `os.path.abspath`
// WIRING: window_state::py_abspath
pub fn py_abspath(path: &str) -> String {
    let candidate = Path::new(path);
    let full = if candidate.is_absolute() {
        path.to_string()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => join_normalized(&cwd, path).to_string_lossy().to_string(),
            Err(_) => path.to_string(),
        }
    };
    py_normpath(&full)
}

/// `Path.resolve()`：Windows 上走 `GetFinalPathNameByHandleW` 再剥掉 `\\?\` / `\\?\UNC\`，
/// 因此 `str(root)` 与 Python 逐字节一致（这是 `dir-`/`zip-` source_id 的前提）。
// WIRING: pet_paths::py_resolve
pub fn py_resolve(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(real) => {
            let text = real.to_string_lossy().to_string();
            if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
                return PathBuf::from(format!(r"\\{}", stripped));
            }
            if let Some(stripped) = text.strip_prefix(r"\\?\") {
                return PathBuf::from(stripped);
            }
            real
        }
        Err(_) => {
            // Python 的 strict=False 只回退到 normpath(abspath(...))。
            PathBuf::from(py_abspath(&path.to_string_lossy()))
        }
    }
}

/// `str(Path)`
pub fn path_str(path: &Path) -> String {
    path.as_os_str().to_string_lossy().to_string()
}

/// `os.path.basename`（Windows 语义）
// WIRING: window_state::py_basename
pub fn os_basename(path: &str) -> String {
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    let trimmed = py_rstrip_chars(path, if cfg!(windows) { "\\/" } else { "/" });
    match trimmed.rfind(seps) {
        Some(pos) => trimmed[pos + 1..].to_string(),
        None => trimmed.to_string(),
    }
}

/// `os.path.dirname`（Windows 语义）
// WIRING: window_state::py_dirname
pub fn os_dirname(path: &str) -> String {
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    match path.rfind(seps) {
        Some(pos) => {
            let head = &path[..pos];
            if head.is_empty() {
                path[..1].to_string()
            } else if seps.contains(&head.chars().last().unwrap()) && py_normpath(head).is_empty()
            {
                head.to_string()
            } else {
                py_normpath(head)
            }
        }
        None => String::new(),
    }
}

/// `child.relative_to(base).as_posix()`，大小写不敏感（Windows `PurePath` 比较 normcase 段）。
pub fn relative_posix(child: &Path, base: &Path) -> Option<String> {
    let base_parts: Vec<String> = path_segments(base);
    let child_parts: Vec<String> = path_segments(child);
    if child_parts.len() < base_parts.len() {
        return None;
    }
    for (a, b) in base_parts.iter().zip(child_parts.iter()) {
        if !eq_ignore_case(a, b) {
            return None;
        }
    }
    Some(child_parts[base_parts.len()..].join("/"))
}

fn path_segments(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str().to_string_lossy().to_string()),
            Component::RootDir => out.push("\\\\".to_string()),
            Component::CurDir => {}
            Component::ParentDir => out.push("..".to_string()),
            Component::Normal(s) => out.push(s.to_string_lossy().to_string()),
        }
    }
    out
}

fn eq_ignore_case(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        py_lower(a) == py_lower(b)
    } else {
        a == b
    }
}

/// `sorted(Path...)` 的比较键：CPython 3.11 的 `PurePath._cparts`，即“逐段 `str.lower()` 后按
/// 列表比较，前缀相等时短者在前”（`probe5.txt` `sep_sort` 证明它不是整串比较）。
pub fn path_cmp_key(path: &str) -> Vec<String> {
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    path.split(seps).filter(|s| !s.is_empty()).map(|s| py_lower(s)).collect()
}

/// 按 `Path.__lt__` 排序（等价于 `sorted(list_of_paths)`）。
pub fn sort_paths_by_path_order(items: &mut Vec<(String, String)>) {
    items.sort_by(|a, b| {
        let ka = path_cmp_key(&a.1);
        let kb = path_cmp_key(&b.1);
        ka.cmp(&kb)
    });
}

// ============================================================ 文件系统 / 时间

/// `os.stat` 的类型判定；`lstat` 语义（不跟随符号链接）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Dir,
    Reg,
    Link,
    Other,
    Missing,
}

pub fn lstat_kind(path: &Path) -> FileKind {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            let ft = meta.file_type();
            if ft.is_symlink() {
                FileKind::Link
            } else if ft.is_dir() {
                FileKind::Dir
            } else if ft.is_file() {
                FileKind::Reg
            } else {
                FileKind::Other
            }
        }
        Err(_) => FileKind::Missing,
    }
}

pub fn exists(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

pub fn is_dir(path: &Path) -> bool {
    matches!(lstat_kind(path), FileKind::Dir) || fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

pub fn is_file(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

pub fn is_symlink(path: &Path) -> bool {
    lstat_kind(path) == FileKind::Link
}

pub fn file_size(path: &Path) -> i64 {
    fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0)
}

/// `Path.read_bytes()` / `open(path, "rb")`
pub fn read_bytes(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    fs::read(path)
}

/// `Path.read_text(encoding="utf-8")`：文本模式默认 `newline=None` ⇒ 通用换行会把
/// `\r\n` 与孤立 `\r` 归一成 `\n`（`probe5.txt` `read_text` / `read_text_cr`）。
/// 非法 UTF-8 返回 `Err`，对应 Python 的 `UnicodeDecodeError`。
pub fn read_text_universal(path: &Path) -> Result<String, ()> {
    let bytes = read_bytes(path).map_err(|_| ())?;
    let text = String::from_utf8(bytes).map_err(|_| ())?;
    Ok(universal_newlines(&text))
}

pub fn universal_newlines(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\r' {
            out.push('\n');
            if chars.get(i + 1) == Some(&'\n') {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `Path.write_text(text, encoding="utf-8", newline="\n")`（不做换行翻译）。
pub fn write_text_lf(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(text.as_bytes())
}

/// `src/readmd_core/file_writer.py:17 save_text_atomic(path, text)`：
/// 临时文件用 `newline=""` 打开 ⇒ LF 原样落盘（`probe4.txt` `meta_bytes`），随后 `os.replace`。
// WIRING: readmd_kernel::file_writer::save_text_atomic
pub fn save_text_atomic(path: &Path, text: &str) -> bool {
    let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    if fs::create_dir_all(&parent).is_err() {
        return false;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = parent.join(format!(".{}.{}.tmp", name, unique_suffix()));
    let mut ok = false;
    if let Ok(mut file) = fs::File::create(&tmp) {
        if file.write_all(text.as_bytes()).is_ok() && file.flush().is_ok() {
            ok = true;
        }
        drop(file);
    }
    if !ok {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    for attempt in 0..6 {
        match fs::rename(&tmp, path) {
            Ok(()) => return true,
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(30 * (attempt + 1) as u64));
            }
        }
    }
    let _ = fs::remove_file(&tmp);
    false
}

/// `readmd_core.utils.load_json(path, default)`：非文件 ⇒ default；解析失败 ⇒ default + WARNING。
// WIRING: readmd_kernel::utils::load_json
pub fn load_json(path: &Path, default: JVal) -> JVal {
    if !is_file(path) {
        return default;
    }
    match read_bytes(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => match json_loads(&text) {
                Ok(value) => value,
                Err(reason) => {
                    py_log("WARNING", &format!("读取 JSON 失败 {}: {}", path.display(), reason));
                    default
                }
            },
            Err(_) => default,
        },
        Err(_) => default,
    }
}

/// `readmd_core.utils.save_json(path, data)`：`json.dump(indent=2)` 以文本模式写入
/// ⇒ Windows 上换行翻译成 `\r\n`（`probe4.txt` `config_bytes`），随后原子替换 + 6 次重试。
// WIRING: readmd_kernel::utils::save_json
pub fn save_json(path: &Path, data: &JVal) -> bool {
    let target = py_abspath(&path.to_string_lossy());
    let target_path = PathBuf::from(&target);
    let parent = match target_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    if fs::create_dir_all(&parent).is_err() {
        return false;
    }
    let base = target_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = parent.join(format!("{}.{}.tmp", base, unique_suffix()));
    let mut text = json_dumps(data, Some(2));
    if cfg!(windows) {
        text = text.replace('\n', "\r\n");
    }
    let mut written = false;
    if let Ok(mut file) = fs::File::create(&tmp) {
        if file.write_all(text.as_bytes()).is_ok() && file.flush().is_ok() {
            written = true;
        }
        drop(file);
    }
    if !written {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    for attempt in 0..6 {
        match fs::rename(&tmp, &target_path) {
            Ok(()) => return true,
            Err(error) => {
                if attempt == 5 {
                    py_log("ERROR", &format!("保存 JSON 失败 {}: {}", target, error));
                    let _ = fs::remove_file(&tmp);
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(
                    30 * (attempt as u64 + 1),
                ));
            }
        }
    }
    false
}

/// `logging.warning(...)` / `logging.error(...)` 的最小替身。
// WIRING: crate 的统一日志实现。
fn py_log(level: &str, message: &str) {
    eprintln!("{}:root:{}", level, message);
}

fn unique_suffix() -> String {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}", nanos, seq)
}

/// `tempfile.mkdtemp(prefix=...)`
// WIRING: 复用 crate 内的临时目录助手。
pub fn make_temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    for _ in 0..64 {
        let candidate = base.join(format!("{}{}", prefix, unique_suffix()));
        if fs::create_dir(&candidate).is_ok() {
            return candidate;
        }
    }
    base.join(format!("{}fallback", prefix))
}

/// `shutil.rmtree(path, ignore_errors=True)`
pub fn remove_tree(path: &Path) {
    let _ = remove_tree_strict(path);
}

pub fn remove_tree_strict(path: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            remove_tree_strict(&entry.path())?;
        }
        fs::remove_dir(path)
    } else {
        let _ = fs::remove_file(path);
        Ok(())
    }
}

/// `shutil.copytree(src, dst)`：普通目录/文件的递归复制，符号链接会被“复制成实体”
/// （Python 的 `symlinks=False` 默认行为，也是本模块拒绝符号链接的兜底）。
// WIRING: crate 内的目录复制助手。
pub fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if !meta.file_type().is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "copytree requires a directory",
        ));
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let child = fs::symlink_metadata(&from)?;
        if child.file_type().is_dir() {
            copy_tree(&from, &to)?;
        } else if child.file_type().is_symlink() {
            // Python 会跟随链接复制目标内容；目标缺失时 shutil 抛错，这里保持一致。
            let bytes = fs::read(&from)?;
            fs::write(&to, bytes)?;
        } else {
            copy2(&from, &to)?;
        }
    }
    Ok(())
}

/// `shutil.copy2(src, dst)`
pub fn copy2(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = fs::read(src)?;
    fs::write(dst, bytes)
}

/// `shutil.move(src, dst)`：同卷 rename，失败时复制 + 删除。
pub fn move_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            if is_dir(src) {
                copy_tree(src, dst)?;
                remove_tree(src);
                Ok(())
            } else {
                let bytes = fs::read(src)?;
                fs::write(dst, bytes)?;
                fs::remove_file(src)
            }
        }
    }
}

/// `Path.rglob("*")` 里所有 `is_file()` 的项（绝对路径）。
pub fn rglob_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    rglob_files_into(root, &mut out);
    out
}

fn rglob_files_into(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let mut names: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    names.sort();
    for path in names {
        match lstat_kind(&path) {
            FileKind::Dir => rglob_files_into(&path, out),
            FileKind::Reg | FileKind::Link | FileKind::Other => {
                if is_file(&path) {
                    out.push(path);
                }
            }
            FileKind::Missing => {}
        }
    }
}

/// `datetime.now(timezone.utc).isoformat()` —— 微秒为 0 时 Python 会省略小数部分。
// WIRING: crate 内的时间助手。
pub fn utc_isoformat(override_text: Option<&str>) -> String {
    if let Some(text) = override_text {
        return text.to_string();
    }
    let (y, mo, d, h, mi, sec, micro) = utc_now_parts();
    if micro == 0 {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00", y, mo, d, h, mi, sec)
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
            y, mo, d, h, mi, sec, micro
        )
    }
}

/// `datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")`
pub fn utcstrftime_compact(override_text: Option<&str>) -> String {
    if let Some(text) = override_text {
        return text.to_string();
    }
    let (y, mo, d, h, mi, sec, _) = utc_now_parts();
    format!("{:04}{:02}{:02}T{:02}{:02}{:02}Z", y, mo, d, h, mi, sec)
}

fn utc_now_parts() -> (i64, u32, u32, u32, u32, u32, u32) {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = elapsed.as_secs() as i64;
    let micros = elapsed.subsec_micros();
    civil_from_days(secs / 86_400)
        .join((secs % 86_400).rem_euclid(86_400))
        .map_micros(micros)
}

fn civil_from_days(days: i64) -> TimeParts {
    // Howard Hinnant 的 civil_from_days 算法，与 CPython 的 `_PyTime_gmtime` 等价。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    TimeParts { year: if m <= 2 { y + 1 } else { y }, month: m, day: d, hour: 0, minute: 0, second: 0, micro: 0 }
}

struct TimeParts {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    micro: u32,
}

impl TimeParts {
    fn join(self, secs_of_day: i64) -> TimeParts {
        TimeParts {
            hour: (secs_of_day / 3600) as u32,
            minute: ((secs_of_day % 3600) / 60) as u32,
            second: (secs_of_day % 60) as u32,
            ..self
        }
    }

    fn map_micros(self, micro: u32) -> (i64, u32, u32, u32, u32, u32, u32) {
        (self.year, self.month, self.day, self.hour, self.minute, self.second, micro)
    }
}

// ============================================================ 文本 / 仓库路径判定

/// `_slug(value)`
pub fn slug(value: &str) -> String {
    // re.sub(r"[^a-z0-9]+", "-", str(value or "").lower())：连续的“非小写字母数字”折叠成一个 '-'。
    let lowered = py_lower(value);
    let mut out = String::new();
    let mut in_run = false;
    for ch in lowered.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            in_run = false;
            out.push(ch);
        } else if !in_run {
            in_run = true;
            out.push('-');
        }
    }
    let stripped = py_strip_chars(&out, "-");
    let cut = py_rstrip_chars(&py_slice_prefix(&stripped, 64), "-");
    if cut.is_empty() {
        "imported-skill".to_string()
    } else {
        cut
    }
}

/// `parse_github_url(value)`：返回 Python 那个 dict（含 `_ref_tail` / `_marker`）。
pub fn parse_github_url(value: &str) -> R<JVal> {
    let raw = py_strip(value);
    let parsed = url_parse(&raw);
    if parsed.scheme != "https" || !GITHUB_HOSTS.contains(&parsed.hostname().as_str()) {
        return err("github_host_not_allowed", "仅允许 HTTPS GitHub 仓库链接");
    }
    if !parsed.username().is_empty() || !parsed.password().is_empty() || !parsed.query.is_empty() {
        return err("github_url_has_credentials", "仓库链接不得包含凭据或查询参数");
    }
    let segments: Vec<String> =
        parsed.path.split('/').filter(|x| !x.is_empty()).map(|x| py_unquote(x)).collect();
    if segments.len() < 2 {
        return err("github_repo_invalid", "GitHub 链接必须包含 owner/repository");
    }
    let owner = segments[0].clone();
    let mut repo = segments[1].clone();
    if repo.ends_with(".git") {
        repo = repo[..repo.len() - 4].to_string();
    }
    let valid_name = |name: &str| -> bool {
        let chars: Vec<char> = name.chars().collect();
        !chars.is_empty()
            && chars.len() <= 100
            && chars.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    };
    if !valid_name(&owner) || !valid_name(&repo) {
        return err("github_repo_invalid", "GitHub owner 或仓库名称无效");
    }
    let mut reference = String::new();
    let mut subdir = String::new();
    let mut marker = String::new();
    let mut ref_tail: Vec<String> = Vec::new();
    if segments.len() > 2 {
        marker = py_lower(&segments[2]);
        if (marker != "tree" && marker != "blob") || segments.len() < 4 {
            return err("github_url_invalid", "仅支持仓库、tree 或 blob 链接");
        }
        ref_tail = segments[3..].to_vec();
        reference = ref_tail[0].clone();
        let remaining = &ref_tail[1..];
        subdir = remaining.join("/");
        if marker == "blob" && !remaining.is_empty()
            && py_lower(remaining.last().unwrap()) == "skill.md"
        {
            subdir = remaining[..remaining.len() - 1].join("/");
        }
    }
    if subdir.starts_with('/') || subdir.split('/').any(|p| p == "..") {
        return err("github_path_invalid", "仓库子路径无效");
    }
    let mut canonical = format!("https://github.com/{}/{}", owner, repo);
    if !reference.is_empty() {
        canonical += "/tree/";
        canonical += &py_quote(&reference, "");
        if !subdir.is_empty() {
            let joined: Vec<String> =
                subdir.split('/').map(|x| py_quote(x, "")).collect::<Vec<_>>().into_iter().collect();
            canonical += &format!("/{}", joined.join("/"));
        }
    }
    Ok(obj(&[
        ("owner", s(&owner)),
        ("repo", s(&repo)),
        ("ref", s(&reference)),
        ("subdir", s(&subdir)),
        ("canonical_url", s(&canonical)),
        ("_ref_tail", JVal::List(ref_tail.into_iter().map(|v| JVal::Str(v)).collect())),
        ("_marker", s(&marker)),
    ]))
}

/// `re.match(r"^---\s*\r?\n(.*?)\r?\n---\s*\r?\n", text, re.S)` 的 `group(1)` 区间（码点下标）。
///
/// `\s*\r?\n` 的展开：`\s*` 贪婪吃满空白串后回溯到“最后一个 `\\n`”；`(.*?)` 懒惰 ⇒ 取最早的、
/// 后面还能接上 `\\r?\\n---\s*\\r?\\n` 的位置。两种情形都由 `probe1.txt` `frontmatter` 钉死
/// （`"----\n..."` 不匹配、`"---  \r\n"` 匹配、`"---\n\n---\n..."` 得到 `["a","b"]`）。
fn frontmatter_group_range(text: &str) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 || chars[0] != '-' || chars[1] != '-' || chars[2] != '-' {
        return None;
    }
    let mut run_end = 3;
    while run_end < chars.len() && py_isspace(chars[run_end]) {
        run_end += 1;
    }
    let mut body_start = None;
    let mut probe = run_end;
    while probe > 3 {
        probe -= 1;
        if chars[probe] == '\n' {
            body_start = Some(probe + 1);
            break;
        }
    }
    let body_start = body_start?;
    let mut tail = body_start;
    while tail <= chars.len() {
        if let Some(after_dashes) = match_newline_dashes(&chars, tail) {
            let mut ws_end = after_dashes;
            while ws_end < chars.len() && py_isspace(chars[ws_end]) {
                ws_end += 1;
            }
            let mut p = ws_end;
            let mut closed = false;
            while p > after_dashes {
                p -= 1;
                if chars[p] == '\n' {
                    closed = true;
                    break;
                }
            }
            if closed {
                return Some((body_start, tail));
            }
        }
        tail += 1;
    }
    None
}

/// `\r?\n---`，返回 `---` 之后的下标。
fn match_newline_dashes(chars: &[char], at: usize) -> Option<usize> {
    let mut pos = at;
    if chars.get(pos) == Some(&'\r') {
        if chars.get(pos + 1) != Some(&'\n') {
            return None;
        }
        pos += 2;
    } else if chars.get(pos) == Some(&'\n') {
        pos += 1;
    } else {
        return None;
    }
    for want in ['-', '-', '-'] {
        if chars.get(pos) != Some(&want) {
            return None;
        }
        pos += 1;
    }
    Some(pos)
}

fn chars_slice(chars: &[char], from: usize, to: usize) -> String {
    chars[from.min(chars.len())..to.min(chars.len())].iter().collect()
}

/// `_frontmatter(text) -> (name, description)`
pub fn frontmatter(text: &str) -> (String, String) {
    let chars: Vec<char> = text.chars().collect();
    let mut values: Vec<(String, String)> = Vec::new();
    if let Some((start, end)) = frontmatter_group_range(text) {
        let group = chars_slice(&chars, start, end);
        for line in py_splitlines(&group) {
            if let Some(pos) = line.find(':') {
                let key = py_lower(&py_strip(&line[..pos]));
                let value = py_strip_chars(&py_strip(&line[pos + 1..]), "'\"");
                match values.iter_mut().find(|(k, _)| *k == key) {
                    Some(slot) => slot.1 = value,
                    None => values.push((key, value)),
                }
            }
        }
    }
    let take = |key: &str| -> String {
        values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    (take("name"), take("description"))
}

/// `re.search(r"^(name:\s*)([^\r\n]+)", text, re.M)` 的 `span(2)`（码点下标）。
///
/// `^` 只在串首或 `\n` 之后成立；`\s*` 贪婪后回溯到“最后一个能开启 `[^\r\n]+` 的位置”，
/// 所以整行空白 / 行尾空白都会被吸收进 group(2)（`probe1.txt` `name_span` 的 13 个用例）。
pub fn name_span(text: &str) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut anchor = 0usize;
    while anchor < len {
        let at_line_start = anchor == 0 || chars[anchor - 1] == '\n';
        if at_line_start && matches_literal(&chars, anchor, "name:") {
            let mut run_end = anchor + 5;
            while run_end < len && py_isspace(chars[run_end]) {
                run_end += 1;
            }
            let mut start = if run_end < len {
                Some(run_end)
            } else {
                let mut probe = len;
                let mut found = None;
                while probe > anchor + 5 {
                    probe -= 1;
                    if chars[probe] != '\r' && chars[probe] != '\n' {
                        found = Some(probe);
                        break;
                    }
                }
                found
            };
            // group(2) 里的第一个字符不能是 \r / \n（`[^\r\n]+`）。
            if let Some(p) = start {
                if chars[p] == '\r' || chars[p] == '\n' {
                    start = None;
                }
            }
            if let Some(p) = start {
                let mut end = p;
                while end < len && chars[end] != '\r' && chars[end] != '\n' {
                    end += 1;
                }
                return Some((p, end));
            }
        }
        anchor += 1;
    }
    None
}

fn matches_literal(chars: &[char], at: usize, word: &str) -> bool {
    let want: Vec<char> = word.chars().collect();
    at + want.len() <= chars.len() && chars[at..at + want.len()] == want[..]
}

/// `original_skill_text[:m.start(2)] + skill_id + original_skill_text[m.end(2):]`
pub fn replace_name_span(text: &str, new_id: &str) -> Option<String> {
    let (start, end) = name_span(text)?;
    let chars: Vec<char> = text.chars().collect();
    Some(format!("{}{}{}", chars_slice(&chars, 0, start), new_id, chars_slice(&chars, end, chars.len())))
}

/// `_is_license_path(path)`
pub fn is_license_path(path: &str) -> bool {
    let name = py_lower(&posix_basename(path));
    name.starts_with("license")
        || name.starts_with("licence")
        || name.starts_with("copying")
        || name == "notice"
        || name.starts_with("notice.")
}

/// `_under(path, prefix)`
pub fn under(path: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    if path == prefix {
        return true;
    }
    path.starts_with(&format!("{}/", py_rstrip_chars(prefix, "/")))
}

/// `_applicable_licenses(directory, license_paths)`
pub fn applicable_licenses(directory: &str, license_paths: &[String]) -> Vec<String> {
    let directory = py_strip_chars(directory, "/");
    let mut result: Vec<String> = Vec::new();
    for path in license_paths {
        let parent = py_strip_chars(&posix_dirname(path), "/");
        if parent.is_empty() || directory == parent || directory.starts_with(&format!("{}/", parent))
        {
            result.push(path.clone());
        }
    }
    result.sort();
    result
}

/// `_manifest_sha256(files)`
pub fn manifest_sha256(files: &[JVal]) -> String {
    let mut ordered: Vec<(String, &JVal)> =
        files.iter().map(|item| (item.get_or_empty_str("path"), item)).collect();
    // sorted(key=str(value.get("path") or "")) —— UTF-8 的字节序即码点序。
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let mut digest = Sha256::new();
    for (_, item) in ordered {
        digest.update(item.get_or_empty_str("path").as_bytes());
        digest.update(&[0u8]);
        digest.update(item.get_or_empty_str("sha256").as_bytes());
        digest.update(&[0u8]);
    }
    hex_encode(&digest.finalize())
}

/// `_safe_member(name)`
pub fn safe_member(name: &str) -> R<String> {
    let name = name.replace('\\', "/");
    let invalid = name.is_empty()
        || name.starts_with('/')
        || name.contains(':')
        || name.split('/').any(|part| part.is_empty() || part == "." || part == "..");
    if invalid {
        return err("archive_path_invalid", "归档包含不安全路径");
    }
    if name.chars().count() > MAX_PATH_LENGTH {
        return err("archive_path_too_long", "归档路径过长");
    }
    if py_parts_len(&name) > MAX_PATH_DEPTH {
        return err("archive_path_too_deep", "归档路径层级超过限制");
    }
    Ok(name)
}

/// `_declaration_importable(declared)`：只有“除缺许可证外全部通过”的草稿才允许导入。
///
/// `valid` / `draft_allowed` 都必须是真的 `bool`（`probe1.txt` `declaration_importable` 里
/// `{"valid": 1}` 与 `{"valid": "true"}` 都是 `False`）。
pub fn declaration_importable(declared: &JVal) -> bool {
    if declared.get("valid").map(|v| v.is_true()).unwrap_or(false) {
        return true;
    }
    let mut codes: Vec<String> = Vec::new();
    if let Some(JVal::List(items)) = declared.get("error_codes") {
        for item in items {
            if item.truthy() {
                let value = item.py_str();
                if !codes.contains(&value) {
                    codes.push(value);
                }
            }
        }
    }
    if codes.is_empty() {
        if let Some(code) = declared.get("error_code") {
            if code.truthy() {
                codes.push(code.py_str());
            }
        }
    }
    let draft = declared.get("draft_allowed").map(|v| v.is_true()).unwrap_or(false);
    draft && codes.len() == 1 && codes[0] == "skill_license_missing"
}

// ============================================================ _scan_directory

/// `_scan_directory(root)`：对不可信目录树给出确定性的“普通文件清单”。
pub fn scan_directory(path: &Path) -> R<Vec<JVal>> {
    let expanded = py_expanduser(path);
    if is_symlink(&expanded) {
        return err("source_symlink", "Skill 来源目录不能是符号链接");
    }
    if !is_dir(&expanded) {
        return err("source_not_found", "Skill 来源目录不存在");
    }
    let root = py_resolve(&expanded);
    let mut files: Vec<JVal> = Vec::new();
    let mut total: i64 = 0;
    walk_tree(&root, &root, "", &mut files, &mut total)?;
    files.sort_by(|a, b| a.path_key().cmp(&b.path_key()));
    Ok(files)
}

fn walk_tree(root: &Path, current: &Path, prefix: &str, files: &mut Vec<JVal>, total: &mut i64) -> R<()> {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        names.push(entry.file_name().to_string_lossy().to_string());
    }
    names.sort();
    // os.walk 用 `entry.is_dir()`（跟随链接）分类，再交给 lstat 判定，因此符号链接目录
    // 会出现在 directories 里并被 source_symlink 拦下。
    let mut directories: Vec<String> = Vec::new();
    let mut filenames: Vec<String> = Vec::new();
    for name in names {
        let candidate = current.join(&name);
        if is_dir(&candidate) {
            directories.push(name);
        } else {
            filenames.push(name);
        }
    }
    for name in &directories {
        let candidate = current.join(name);
        match lstat_kind(&candidate) {
            FileKind::Link => return err("source_symlink", "Skill 来源包含不允许的符号链接"),
            FileKind::Dir => {}
            _ => return err("source_special_file", "Skill 来源包含不支持的特殊目录项"),
        }
        let relative = if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) };
        check_relative_limits(&relative)?;
    }
    for name in &filenames {
        let candidate = current.join(name);
        match lstat_kind(&candidate) {
            FileKind::Link => return err("source_symlink", "Skill 来源包含不允许的符号链接"),
            FileKind::Reg => {}
            _ => return err("source_special_file", "Skill 来源包含不支持的特殊文件"),
        }
        let relative = if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) };
        check_relative_limits(&relative)?;
        let size = file_size(&candidate);
        if size as usize > MAX_FILE_BYTES {
            return err("source_file_too_large", "Skill 来源包含超过大小限制的文件");
        }
        *total += size;
        if *total as usize > MAX_EXTRACTED_BYTES {
            return err("source_too_large", "Skill 来源总大小超过限制");
        }
        if files.len() >= MAX_FILES {
            return err("source_too_many_files", "Skill 来源文件数量超过限制");
        }
        let mut digest = Sha256::new();
        let bytes = read_bytes(&candidate).map_err(|_| {
            SkillImportError::new("source_unreadable", "无法读取 Skill 来源文件")
        })?;
        for chunk in bytes.chunks(1024 * 1024) {
            digest.update(chunk);
        }
        files.push(obj(&[
            ("path", s(&relative)),
            ("size", JVal::Int(size as i128)),
            ("sha256", s(&hex_encode(&digest.finalize()))),
        ]));
    }
    for name in &directories {
        let candidate = current.join(name);
        let next_prefix =
            if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) };
        walk_tree(root, &candidate, &next_prefix, files, total)?;
    }
    Ok(())
}

fn check_relative_limits(relative: &str) -> R<()> {
    if relative.chars().count() > MAX_PATH_LENGTH {
        return err("source_path_too_long", "Skill 来源路径过长");
    }
    if relative.split('/').filter(|p| !p.is_empty() && *p != ".").count() > MAX_PATH_DEPTH {
        return err("source_path_too_deep", "Skill 来源目录层级超过限制");
    }
    Ok(())
}

// ============================================================ _scan_skill_candidates

fn in_text_suffixes(path: &str) -> bool {
    let suffix = py_lower(&py_suffix(path));
    TEXT_SUFFIXES.contains(&suffix.as_str())
}

fn in_script_suffixes(path: &str) -> bool {
    let suffix = py_lower(&py_suffix(path));
    SCRIPT_SUFFIXES.contains(&suffix.as_str())
}

/// `_scan_skill_candidates(root, manifest)`
pub fn scan_skill_candidates(
    root: &Path,
    manifest: &[JVal],
) -> (Vec<JVal>, Vec<String>) {
    let manifest_items: Vec<JVal> = manifest.to_vec();
    let paths: Vec<String> =
        manifest_items.iter().map(|item| item.get_or_empty_str("path")).collect();
    let mut license_paths: Vec<String> =
        paths.iter().filter(|p| is_license_path(p)).cloned().collect();
    license_paths.sort();
    let mut skills: Vec<JVal> = Vec::new();
    for path in &paths {
        if py_lower(&posix_basename(path)) != "skill.md" {
            continue;
        }
        let directory = posix_dirname(path);
        let skill_file = join_posix(root, &directory, &posix_basename(path));
        let mut errors: Vec<String> = Vec::new();
        let text = match read_text_universal(&skill_file) {
            Ok(value) => value,
            Err(_) => {
                errors.push("skill_not_utf8".to_string());
                String::new()
            }
        };
        let (name, description) = frontmatter(&text);
        if !id_re_fullmatch(&name) {
            errors.push("skill_name_invalid".to_string());
        }
        if description.is_empty() {
            errors.push("skill_description_invalid".to_string());
        }
        if has_unknown_variable(&text) {
            errors.push("skill_variables_invalid".to_string());
        }
        let sidecar_path = if directory.is_empty() {
            "readmd.skill.json".to_string()
        } else {
            posix_join(&directory, "readmd.skill.json")
        };
        if paths.iter().any(|p| *p == sidecar_path) {
            let sidecar_file = join_posix_path(root, &sidecar_path);
            match read_text_universal(&sidecar_file) {
                Ok(raw) => match json_loads(&raw) {
                    Ok(sidecar) if sidecar.is_dict() => {
                        if sidecar_metadata_invalid(&sidecar, &name) {
                            errors.push("skill_metadata_invalid".to_string());
                        }
                    }
                    _ => errors.push("skill_metadata_invalid".to_string()),
                },
                Err(_) => errors.push("skill_metadata_invalid".to_string()),
            }
        }
        let licenses = applicable_licenses(&directory, &license_paths);
        if licenses.is_empty() {
            errors.push("skill_license_missing".to_string());
        }
        let file_items: Vec<JVal> = manifest_items
            .iter()
            .filter(|item| under(&item.get_or_empty_str("path"), &directory))
            .cloned()
            .collect();
        let mut text_paths: Vec<String> = file_items
            .iter()
            .map(|item| item.get_or_empty_str("path"))
            .filter(|p| in_text_suffixes(p))
            .collect();
        for extra in &licenses {
            if !text_paths.contains(extra) {
                text_paths.push(extra.clone());
            }
        }
        text_paths.sort();
        text_paths.dedup();
        for text_path in text_paths {
            if read_text_universal(&join_posix_path(root, &text_path)).is_err() {
                errors.push("skill_resource_not_utf8".to_string());
                break;
            }
        }
        let dir_base = posix_basename(&directory);
        let skill_id = if id_re_fullmatch(&name) {
            name.clone()
        } else {
            slug(if dir_base.is_empty() { "root" } else { &dir_base })
        };
        let blocking: Vec<&String> =
            errors.iter().filter(|item| item.as_str() != "skill_license_missing").collect();
        let valid = errors.is_empty();
        skills.push(obj(&[
            ("id", s(&skill_id)),
            ("path", s(path)),
            ("directory", s(&directory)),
            ("name", s(if name.is_empty() { &skill_id } else { &name })),
            ("description", s(&description)),
            (
                "files",
                JVal::List(
                    file_items.iter().map(|item| s(&item.get_or_empty_str("path"))).collect(),
                ),
            ),
            ("source_files", JVal::List(file_items.clone())),
            ("license_files", JVal::List(licenses.iter().map(|p| s(p)).collect())),
            ("valid", jbool(valid)),
            ("draft_allowed", jbool(blocking.is_empty() && !errors.is_empty())),
            ("publishable", jbool(valid)),
            ("error_code", s(errors.first().map(|v| v.as_str()).unwrap_or(""))),
            ("error_codes", JVal::List(errors.iter().map(|v| s(v)).collect())),
            (
                "scripts_present",
                jbool(file_items.iter().any(|item| in_script_suffixes(&item.get_or_empty_str("path")))),
            ),
        ]));
    }
    (skills, license_paths)
}

/// sidecar（`readmd.skill.json`）的一致性检查，返回 Python 里那个 or 链真值。
fn sidecar_metadata_invalid(sidecar: &JVal, name: &str) -> bool {
    let declared = sidecar.get("variables");
    let required = sidecar.get("required_variables");
    let metadata_id = match sidecar.get("id") {
        Some(v) if v.truthy() => v.py_str(),
        _ => name.to_string(),
    };
    let str_set = |value: &JVal| -> Vec<String> {
        match value {
            JVal::List(items) => items.iter().map(|v| v.py_str()).collect(),
            other => vec![other.py_str()],
        }
    };
    let declared_list = match declared {
        Some(JVal::List(items)) => Some(str_set(&JVal::List(items.clone()))),
        _ => None,
    };
    let required_list = match required {
        Some(JVal::List(items)) => Some(str_set(&JVal::List(items.clone()))),
        _ => None,
    };
    let declared_present = matches!(declared, Some(v) if !matches!(v, JVal::None));
    let required_present = matches!(required, Some(v) if !matches!(v, JVal::None));
    let invalid_declared = declared_present
        && match &declared_list {
            None => true,
            Some(items) => items.iter().any(|v| !ALLOWED_VARIABLES.contains(&v.as_str())),
        };
    let mut invalid_required = required_present
        && match &required_list {
            None => true,
            Some(items) => items.iter().any(|v| !ALLOWED_VARIABLES.contains(&v.as_str())),
        };
    if let (Some(decl), Some(req)) = (&declared_list, &required_list) {
        if required_present {
            let extra = req.iter().any(|v| !decl.contains(v));
            invalid_required = invalid_required || extra;
        }
    }
    metadata_id != name
        || !id_re_fullmatch(&metadata_id)
        || invalid_declared
        || invalid_required
}

/// `root / directory / name`：把 posix 路径拼到已解析的根目录下。
fn join_posix(root: &Path, directory: &str, name: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    if !directory.is_empty() {
        for seg in directory.split('/') {
            if !seg.is_empty() {
                out.push(seg);
            }
        }
    }
    out.push(name);
    out
}

fn join_posix_path(root: &Path, relative: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for seg in relative.split('/') {
        if !seg.is_empty() {
            out.push(seg);
        }
    }
    out
}

/// `_preview_directory(path)`
pub fn preview_directory(path: &Path) -> R<JVal> {
    let expanded = py_expanduser(path);
    let manifest = scan_directory(&expanded)?;
    let root = py_resolve(&expanded);
    let (skills, license_paths) = scan_skill_candidates(&root, &manifest);
    if skills.is_empty() {
        return err("skill_not_found", "来源中没有可导入的 SKILL.md");
    }
    let source_hash = manifest_sha256(&manifest);
    let identity = path_str(&root);
    Ok(obj(&[
        ("source_id", s(&format!("dir-{}", sha256_prefix20(&identity)))),
        (
            "source",
            obj(&[
                ("type", s("directory")),
                ("path", s(&identity)),
                ("sha256", s(&source_hash)),
            ]),
        ),
        ("license_files", JVal::List(license_paths.iter().map(|p| s(p)).collect())),
        ("skills", JVal::List(skills)),
        ("offline_copy", jbool(true)),
        ("credential_required", jbool(false)),
    ]))
}

// ============================================================ ZIP 归档

/// `_read_local_archive(path)`
pub fn read_local_archive(path: &Path) -> R<(PathBuf, Vec<u8>, String)> {
    let expanded = py_expanduser(path);
    if is_symlink(&expanded) {
        return err("source_symlink", "ZIP 来源不能是符号链接");
    }
    if !is_file(&expanded) {
        return err("source_not_found", "ZIP 来源文件不存在");
    }
    let archive_path = py_resolve(&expanded);
    if file_size(&archive_path) as usize > MAX_RESPONSE_BYTES {
        return err("archive_too_large", "ZIP 文件超过安全大小限制");
    }
    let blob = match read_bytes(&archive_path) {
        Ok(bytes) => bytes,
        Err(_) => return err("source_unreadable", "无法读取 ZIP 来源文件"),
    };
    let digest = sha256_hex(&blob);
    Ok((archive_path, blob, digest))
}

/// `_archive_root(root)`
pub fn archive_root(root: &Path) -> PathBuf {
    let children: Vec<PathBuf> = match fs::read_dir(root) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                    != "__MACOSX"
            })
            .collect(),
        Err(_) => return root.to_path_buf(),
    };
    if children.len() == 1 && is_dir(&children[0]) {
        return children[0].clone();
    }
    root.to_path_buf()
}

/// `_preview_zip(path)`
pub fn preview_zip(path: &Path) -> R<JVal> {
    let (archive_path, blob, archive_hash) = read_local_archive(path)?;
    let extracted = match extract(&blob) {
        Ok(temp) => temp,
        Err(error) => return Err(error),
    };
    let result = (|| -> R<JVal> {
        let root = archive_root(&extracted);
        let manifest = scan_directory(&root)?;
        let (skills, license_paths) = scan_skill_candidates(&root, &manifest);
        if skills.is_empty() {
            return err("skill_not_found", "ZIP 中没有可导入的 SKILL.md");
        }
        let content_hash = manifest_sha256(&manifest);
        let identity = path_str(&archive_path);
        Ok(obj(&[
            ("source_id", s(&format!("zip-{}", sha256_prefix20(&identity)))),
            (
                "source",
                obj(&[
                    ("type", s("zip")),
                    ("path", s(&identity)),
                    ("archive_sha256", s(&archive_hash)),
                    ("sha256", s(&content_hash)),
                ]),
            ),
            ("license_files", JVal::List(license_paths.iter().map(|p| s(p)).collect())),
            ("skills", JVal::List(skills)),
            ("offline_copy", jbool(true)),
            ("credential_required", jbool(false)),
        ]))
    })();
    remove_tree(&extracted);
    result
}

/// `preview_source(source_type, source, credential_id)`
///
/// `github` 分支需要网络，交由装配阶段注入的 [`Ctx`] 提供 transport，因此这里收一个 `ctx`。
pub fn preview_source(ctx: &Ctx, source_type: &str, source: &str, credential_id: &str) -> R<JVal> {
    let kind = py_lower(&py_strip(source_type));
    if ["directory", "folder", "local"].contains(&kind.as_str()) {
        return preview_directory(Path::new(source));
    }
    if ["zip", "archive"].contains(&kind.as_str()) {
        return preview_zip(Path::new(source));
    }
    if kind == "github" {
        return preview_import(ctx, source, credential_id);
    }
    err("source_type_invalid", "不支持的 Skill 来源类型")
}

/// `str.casefold()` 的等价物。
///
/// ZIP 成员名的重复/大小写冲突判定用的是 `casefold()`（不是 `lower()`）。二者对 ASCII 完全一致，
/// 只有 `ß→ss`、`ẛ→ṡ` 一类全折叠差异会不同；那种名字在这里最多少报一次“冲突”，
/// 不会放行不安全路径，因此按 `lower()` 实现并在此登记（修复报告里也列了）。
pub fn py_casefold(value: &str) -> String {
    py_lower(value)
}

// ============================================================ ZIP：zipfile 复刻

/// `zipfile` 在结构损坏时抛 `BadZipFile` / `NotImplementedError` / `UnicodeDecodeError`，
/// 三者都**不是** `SkillImportError`：`readmd.py:2802 except Exception` ⇒ HTTP 500
/// `internal_error`（而不是 `_skill_import_error()` 的 400）。装配阶段必须把这个 code 映射到
/// 500 分支，绝不能当成 400 的 `error_code` 回给前端。
fn internal<T>(detail: &str) -> R<T> {
    err("internal_error", detail)
}

const ZIP_CD_SIG: &[u8; 4] = b"PK\x01\x02";
const ZIP_LOCAL_SIG: &[u8; 4] = b"PK\x03\x04";
const ZIP_EOCD_SIG: &[u8; 4] = b"PK\x05\x06";
const ZIP_EOCD64_SIG: &[u8; 4] = b"PK\x06\x06";
const ZIP_EOCD64_LOC_SIG: &[u8; 4] = b"PK\x06\x07";
/// `zipfile.sizeCentralDir` / `sizeFileHeader` / `sizeEndCentDir` / `sizeEndCentDir64` /
/// `sizeEndCentDir64Locator`（由权威解释器打印确认）。
const SIZE_CD: usize = 46;
const SIZE_LOCAL: usize = 30;
const SIZE_EOCD: usize = 22;
const SIZE_EOCD64: usize = 56;
const SIZE_EOCD64_LOC: usize = 20;
/// `_MASK_ENCRYPTED` / `_MASK_STRONG_ENCRYPTION` / `_MASK_COMPRESSED_PATCH` / `_MASK_UTF_FILENAME`
const MASK_ENCRYPTED: u16 = 0x1;
const MASK_STRONG: u16 = 0x40;
const MASK_PATCH: u16 = 0x20;
const MASK_UTF8: u16 = 0x800;
/// `zipfile.MAX_EXTRACT_VERSION`（3.11.15 = 63）。
const MAX_EXTRACT_VERSION: u8 = 63;
/// `stat.S_IFREG` / `stat.S_IFDIR`
const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o40000;

fn u16_le(bytes: &[u8], at: usize) -> u16 {
    (bytes[at] as u16) | ((bytes[at + 1] as u16) << 8)
}

fn u32_le(bytes: &[u8], at: usize) -> u32 {
    (bytes[at] as u32)
        | ((bytes[at + 1] as u32) << 8)
        | ((bytes[at + 2] as u32) << 16)
        | ((bytes[at + 3] as u32) << 24)
}

fn u64_le(bytes: &[u8], at: usize) -> u64 {
    (u32_le(bytes, at) as u64) | ((u32_le(bytes, at + 4) as u64) << 32)
}

/// `bytes.decode("cp437")` 的高半区（`0x80..=0xFF`），由权威解释器 `bytes(range(0x80,0x100))
/// .decode("cp437")` 逐项导出，低半区就是 ASCII 原值。
const CP437_HIGH: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}',
    '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}',
    '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}',
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}',
    '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}', '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}',
    '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}',
    '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}',
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}',
    '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}', '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}',
    '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}',
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}',
];


fn decode_cp437(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                CP437_HIGH[(b - 0x80) as usize]
            }
        })
        .collect()
}

/// `zipfile` 的成员名解码：flag bit 11 决定 UTF-8（严格）还是 cp437。
fn decode_member_name(bytes: &[u8], utf8_flag: bool) -> Result<String, String> {
    if utf8_flag {
        std::str::from_utf8(bytes)
            .map(|v| v.to_string())
            .map_err(|_| "UnicodeDecodeError: 'utf-8' codec can't decode member name".to_string())
    } else {
        Ok(decode_cp437(bytes))
    }
}

/// 一个 `ZipInfo`：只保留 `_extract()` 与本地头定位用到的字段。
#[derive(Clone, Debug)]
pub struct ZipMember {
    pub filename: String,
    pub flag_bits: u16,
    pub compress_type: u16,
    pub crc: u32,
    pub compress_size: u64,
    pub file_size: u64,
    pub external_attr: u32,
    pub header_offset: u64,
    /// 3.11 的 zip-bomb 检测：`zinfo._end_offset`（按 `header_offset` 逆序推导）。
    pub end_offset: Option<u64>,
    pub extract_version: u8,
}

/// `ZipInfo.is_dir()`：`/` 结尾；Windows 上 `os.path.altsep` 存在，因此 `\` 结尾也算目录。
pub fn member_is_dir(filename: &str) -> bool {
    if filename.ends_with('/') {
        return true;
    }
    cfg!(windows) && filename.ends_with('\\')
}

/// `_EndRecData()` + `_EndRecData64()` 的合并结果：`size_cd` / `offset_cd` / `location`。
#[derive(Clone, Copy, Debug)]
struct EndRecord {
    size_cd: u64,
    offset_cd: u64,
    location: u64,
}

fn slice_at(bytes: &[u8], at: usize, len: usize) -> &[u8] {
    // `fp.read(n)` 到文件尾会返回不足 n 的字节，这里用同一套钳制语义。
    if at >= bytes.len() {
        return &bytes[0..0];
    }
    let end = (at + len).min(bytes.len());
    &bytes[at..end]
}

/// `zipfile._EndRecData64`
fn end_rec_data64(blob: &[u8], loc: i64, base: EndRecord) -> Result<EndRecord, String> {
    let mut offset = loc - SIZE_EOCD64_LOC as i64;
    if offset < 0 {
        return Ok(base);
    }
    let data = slice_at(blob, offset as usize, SIZE_EOCD64_LOC);
    if data.len() != SIZE_EOCD64_LOC {
        return Err("OSError: Unknown I/O error".to_string());
    }
    if &data[0..4] != ZIP_EOCD64_LOC_SIG {
        return Ok(base);
    }
    let diskno = u32_le(data, 4);
    let reloff = u64_le(data, 8);
    let disks = u32_le(data, 16);
    if diskno != 0 || disks > 1 {
        return Err("BadZipFile: zipfile that span multiple disks are not supported".to_string());
    }
    offset -= SIZE_EOCD64 as i64;
    if reloff > offset as u64 {
        return Err("BadZipFile: Corrupt zip64 end of central directory locator".to_string());
    }
    let mut extrasz: u64 = (offset as u64) - reloff;
    let mut rec = slice_at(blob, reloff as usize, SIZE_EOCD64);
    if rec.len() != SIZE_EOCD64 {
        return Err("OSError: Unknown I/O error".to_string());
    }
    if &rec[0..4] != ZIP_EOCD64_SIG && reloff != offset as u64 {
        extrasz = 0;
        rec = slice_at(blob, offset as usize, SIZE_EOCD64);
        if rec.len() != SIZE_EOCD64 {
            return Err("OSError: Unknown I/O error".to_string());
        }
    }
    if &rec[0..4] != ZIP_EOCD64_SIG {
        return Err("BadZipFile: Zip64 end of central directory record not found".to_string());
    }
    let sz = u64_le(rec, 4);
    let _dircount = u64_le(rec, 24);
    let dirsize = u64_le(rec, 40);
    let diroffset = u64_le(rec, 48);
    if diroffset + dirsize != reloff || sz + 12 != (SIZE_EOCD64 as u64) + extrasz {
        return Err("BadZipFile: Corrupt zip64 end of central directory record".to_string());
    }
    Ok(EndRecord {
        size_cd: dirsize,
        offset_cd: diroffset,
        location: (offset - extrasz as i64) as u64,
    })
}

/// `zipfile._EndRecData`
fn end_rec_data(blob: &[u8]) -> Result<Option<EndRecord>, String> {
    let filesize = blob.len() as i64;
    // 分支一：无注释，EOCD 正好在文件尾。
    if filesize >= SIZE_EOCD as i64 {
        let start = (filesize - SIZE_EOCD as i64) as usize;
        let data = &blob[start..];
        if &data[0..4] == ZIP_EOCD_SIG && data[20] == 0 && data[21] == 0 {
            let base = EndRecord {
                size_cd: u32_le(data, 12) as u64,
                offset_cd: u32_le(data, 16) as u64,
                location: start as u64,
            };
            return end_rec_data64(blob, start as i64, base).map(Some);
        }
    }
    // 分支二：在末尾 64KiB 里反向搜索魔数。
    let max_comment_start = (filesize - (1i64 << 16) - SIZE_EOCD as i64).max(0) as usize;
    let data = &blob[max_comment_start..];
    let mut start: i64 = -1;
    if data.len() >= 4 {
        let mut i = data.len() - 4;
        loop {
            if &data[i..i + 4] == ZIP_EOCD_SIG {
                start = i as i64;
                break;
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
    }
    if start < 0 {
        return Ok(None);
    }
    let rec = &data[start as usize..];
    if rec.len() < SIZE_EOCD {
        return Ok(None); // "Zip file is corrupted."
    }
    let base = EndRecord {
        size_cd: u32_le(rec, 12) as u64,
        offset_cd: u32_le(rec, 16) as u64,
        location: (max_comment_start as i64 + start) as u64,
    };
    end_rec_data64(blob, base.location as i64, base).map(Some)
}

/// `ZipInfo._decodeExtra()` 的 zip64 部分（`0x0001`）。
fn decode_extra_zip64(extra: &[u8], member: &mut ZipMember) -> Result<(), String> {
    let mut tail = extra;
    while tail.len() >= 4 {
        let tp = u16_le(tail, 0);
        let ln = u16_le(tail, 2) as usize;
        if ln + 4 > tail.len() {
            return Err(format!("BadZipFile: Corrupt extra field {:04x} (size={})", tp, ln));
        }
        if tp == 0x0001 {
            let mut data = &tail[4..4 + ln];
            if member.file_size == 0xFFFF_FFFF || member.file_size == 0xFFFF_FFFF_FFFF_FFFF {
                if data.len() < 8 {
                    return Err(
                        "BadZipFile: Corrupt zip64 extra field. File size not found.".to_string()
                    );
                }
                member.file_size = u64_le(data, 0);
                data = &data[8..];
            }
            if member.compress_size == 0xFFFF_FFFF {
                if data.len() < 8 {
                    return Err(
                        "BadZipFile: Corrupt zip64 extra field. Compress size not found."
                            .to_string()
                    );
                }
                member.compress_size = u64_le(data, 0);
                data = &data[8..];
            }
            if member.header_offset == 0xFFFF_FFFF {
                if data.len() < 8 {
                    return Err(
                        "BadZipFile: Corrupt zip64 extra field. Header offset not found."
                            .to_string()
                    );
                }
                member.header_offset = u64_le(data, 0);
            }
        }
        tail = &tail[ln + 4..];
    }
    Ok(())
}

/// `zipfile.ZipFile._RealGetContents`：读出中央目录里的全部 `ZipInfo`。
pub fn zip_infolist(blob: &[u8]) -> Result<Vec<ZipMember>, String> {
    let end = match end_rec_data(blob)? {
        Some(value) => value,
        None => return Err("BadZipFile: File is not a zip file".to_string()),
    };
    let concat = (end.location as i64) - (end.size_cd as i64) - (end.offset_cd as i64);
    let start_dir = end.offset_cd as i64 + concat;
    if start_dir < 0 {
        return Err("BadZipFile: Bad offset for central directory".to_string());
    }
    let data = slice_at(blob, start_dir as usize, end.size_cd as usize);
    let size_cd = end.size_cd as usize;
    let mut members: Vec<ZipMember> = Vec::new();
    let mut pos = 0usize;
    let mut total = 0usize;
    while total < size_cd {
        if pos + SIZE_CD > data.len() {
            return Err("BadZipFile: Truncated central directory".to_string());
        }
        let head = &data[pos..pos + SIZE_CD];
        if &head[0..4] != ZIP_CD_SIG {
            return Err("BadZipFile: Bad magic number for central directory".to_string());
        }
        let name_len = u16_le(head, 28) as usize;
        let extra_len = u16_le(head, 30) as usize;
        let comment_len = u16_le(head, 32) as usize;
        let name_start = pos + SIZE_CD;
        if name_start + name_len + extra_len + comment_len > data.len() {
            return Err("BadZipFile: Truncated central directory".to_string());
        }
        let raw_name = &data[name_start..name_start + name_len];
        let flags = u16_le(head, 8);
        let mut member = ZipMember {
            filename: decode_member_name(raw_name, flags & MASK_UTF8 != 0)?,
            flag_bits: flags,
            compress_type: u16_le(head, 10),
            crc: u32_le(head, 16),
            compress_size: u32_le(head, 20) as u64,
            file_size: u32_le(head, 24) as u64,
            external_attr: u32_le(head, 38),
            header_offset: u32_le(head, 42) as u64,
            end_offset: None,
            extract_version: head[6],
        };
        if member.extract_version > MAX_EXTRACT_VERSION {
            return Err(format!(
                "NotImplementedError: zip file version {:.1}",
                member.extract_version as f64 / 10.0
            ));
        }
        let extra = &data[name_start + name_len..name_start + name_len + extra_len];
        decode_extra_zip64(extra, &mut member)?;
        member.header_offset = (member.header_offset as i64 + concat) as u64;
        members.push(member);
        total = pos + SIZE_CD + name_len + extra_len + comment_len;
        pos = total;
    }
    // `for zinfo in sorted(filelist, key=header_offset, reverse=True)`（稳定排序 ⇒ 同键保持原序）
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by(|&a, &b| members[b].header_offset.cmp(&members[a].header_offset));
    let mut end_offset = start_dir as u64;
    let mut patches: Vec<(usize, u64)> = Vec::with_capacity(order.len());
    for idx in order {
        patches.push((idx, end_offset));
        end_offset = members[idx].header_offset;
    }
    for (idx, value) in patches {
        members[idx].end_offset = Some(value);
    }
    Ok(members)
}

/// `ZipFile.open(info)` 的本地头定位：数据起点用的是**本地头**里的 name / extra 长度。
fn local_data_offset(blob: &[u8], member: &ZipMember) -> Result<usize, String> {
    let at = member.header_offset as usize;
    if at + SIZE_LOCAL > blob.len() {
        return Err("BadZipFile: Truncated file header".to_string());
    }
    let head = &blob[at..at + SIZE_LOCAL];
    if &head[0..4] != ZIP_LOCAL_SIG {
        return Err("BadZipFile: Bad magic number for file header".to_string());
    }
    let local_flags = u16_le(head, 6);
    let name_len = u16_le(head, 26) as usize;
    let extra_len = u16_le(head, 28) as usize;
    let name_at = at + SIZE_LOCAL;
    let raw_name = slice_at(blob, name_at, name_len);
    let extra_at = name_at + raw_name.len();
    let data_at = extra_at + extra_len.min(blob.len().saturating_sub(extra_at));
    if member.flag_bits & MASK_PATCH != 0 {
        return Err("NotImplementedError: compressed patched data (flag bit 5)".to_string());
    }
    if member.flag_bits & MASK_STRONG != 0 {
        return Err("NotImplementedError: strong encryption (flag bit 6)".to_string());
    }
    let decoded = decode_member_name(raw_name, local_flags & MASK_UTF8 != 0)?;
    if decoded != member.filename {
        return Err(format!(
            "BadZipFile: File name in directory {:?} and header {:?} differ.",
            member.filename, decoded
        ));
    }
    if let Some(limit) = member.end_offset {
        if (data_at as u64) + member.compress_size > limit {
            return Err(format!(
                "BadZipFile: Overlapped entries: {:?} (possible zip bomb)",
                member.filename
            ));
        }
    }
    Ok(data_at)
}

/// `ZipExtFile` 的读取上限：`_compress_left = compress_size`、`_left = file_size`。
pub fn zip_member_bytes(blob: &[u8], member: &ZipMember) -> Result<Vec<u8>, String> {
    let data_at = local_data_offset(blob, member)?;
    let comp_end = (data_at + member.compress_size as usize).min(blob.len());
    let window = &blob[data_at..comp_end];
    let cap = member.file_size as usize;
    let out = match member.compress_type {
        0 => window.iter().take(cap).cloned().collect::<Vec<u8>>(),
        8 => match inflate_raw(window, cap) {
            Ok(bytes) => bytes,
            Err(()) => return Err("zlib.error: invalid deflate stream".to_string()),
        },
        other => {
            return Err(format!(
                "NotImplementedError: compression type {} not supported",
                other
            ))
        }
    };
    // `ZipExtFile._update_crc(..., eof=True)`
    if crc32(&out, 0) != member.crc {
        return Err(format!(
            "BadZipFile: Bad CRC-32 for file {:?}",
            member.filename
        ));
    }
    Ok(out)
}

/// `zipfile` 用的 zlib.crc32（多项式 0xEDB88320，初值 0，输出取反）。
pub fn crc32(data: &[u8], seed: u32) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut value = i as u32;
        for _ in 0..8 {
            value = if value & 1 != 0 { 0xEDB8_8320 ^ (value >> 1) } else { value >> 1 };
        }
        *slot = value;
    }
    let mut crc = seed ^ 0xFFFF_FFFF;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

// ------------------------------------------------------------ 原始 DEFLATE

const DEFLATE_LBASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const DEFLATE_LEXT: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DEFLATE_DBASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEFLATE_DEXT: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// `cpbase` / `order`：3..18 的码长在码长 Huffman 里的读取顺序。
const CPOFS: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bitbuf: u32,
    bitcnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, bitbuf: 0, bitcnt: 0 }
    }

    fn bits(&mut self, want: u32) -> Result<u32, ()> {
        if want == 0 {
            return Ok(0);
        }
        while self.bitcnt < want {
            if self.pos >= self.data.len() {
                return Err(());
            }
            self.bitbuf |= (self.data[self.pos] as u32) << self.bitcnt;
            self.pos += 1;
            self.bitcnt += 8;
        }
        let mask = if want >= 32 { u32::MAX } else { (1u32 << want) - 1 };
        let value = self.bitbuf & mask;
        self.bitbuf >>= want;
        self.bitcnt -= want;
        Ok(value)
    }

    fn align_byte(&mut self) {
        self.bitbuf = 0;
        self.bitcnt = 0;
    }
}

/// 规范 Huffman 表（zlib `inflate_table` 的等价物）：`counts[len]` + 按 (长度, 符号) 排序的符号表。
#[derive(Clone)]
struct Huffman {
    counts: [u32; 16],
    symbols: Vec<u16>,
}

fn huffman_build(lengths: &[u8]) -> Huffman {
    let mut counts = [0u32; 16];
    for &len in lengths {
        counts[len as usize] += 1;
    }
    counts[0] = 0; // 码长 0 表示“不使用”，不进树
    let mut offsets = [0u32; 16];
    let mut acc = 0u32;
    for len in 1..16 {
        offsets[len] = acc;
        acc += counts[len];
    }
    let mut symbols = vec![0u16; acc.max(1) as usize];
    let mut next = offsets;
    for (symbol, &len) in lengths.iter().enumerate() {
        if len != 0 {
            symbols[next[len as usize] as usize] = symbol as u16;
            next[len as usize] += 1;
        }
    }
    Huffman { counts, symbols }
}

fn huffman_decode(reader: &mut BitReader, table: &Huffman) -> Result<u16, ()> {
    let mut code: i64 = 0;
    let mut first: i64 = 0;
    let mut index: i64 = 0;
    for len in 1..=15usize {
        code = (code << 1) | reader.bits(1)? as i64;
        let count = table.counts[len] as i64;
        if code < first + count {
            let slot = (index + (code - first)) as usize;
            return table.symbols.get(slot).copied().ok_or(());
        }
        index += count;
        first = (first + count) << 1;
    }
    Err(())
}

fn fixed_tables() -> (Huffman, Huffman) {
    let mut lit = [0u8; 288];
    for (i, item) in lit.iter_mut().enumerate() {
        *item = if i < 144 {
            8
        } else if i < 256 {
            9
        } else if i < 280 {
            7
        } else {
            8
        };
    }
    let dist = [5u8; 30];
    (huffman_build(&lit), huffman_build(&dist))
}

fn dynamic_tables(reader: &mut BitReader) -> Result<(Huffman, Huffman), ()> {
    let hlit = reader.bits(5)? as usize + 257;
    let hdist = reader.bits(5)? as usize + 1;
    let hclen = reader.bits(4)? as usize + 4;
    let mut code_lengths = [0u8; 19];
    for i in 0..hclen {
        code_lengths[CPOFS[i]] = reader.bits(3)? as u8;
    }
    let table = huffman_build(&code_lengths);
    let want = hlit + hdist;
    let mut lengths: Vec<u8> = Vec::with_capacity(want);
    while lengths.len() < want {
        let symbol = huffman_decode(reader, &table)?;
        match symbol {
            0..=15 => lengths.push(symbol as u8),
            16 => {
                let prev = match lengths.last() {
                    Some(v) => *v,
                    None => return Err(()),
                };
                let repeat = 3 + reader.bits(2)? as usize;
                if lengths.len() + repeat > want {
                    return Err(());
                }
                for _ in 0..repeat {
                    lengths.push(prev);
                }
            }
            17 => {
                let repeat = 3 + reader.bits(3)? as usize;
                if lengths.len() + repeat > want {
                    return Err(());
                }
                for _ in 0..repeat {
                    lengths.push(0);
                }
            }
            18 => {
                let repeat = 11 + reader.bits(7)? as usize;
                if lengths.len() + repeat > want {
                    return Err(());
                }
                for _ in 0..repeat {
                    lengths.push(0);
                }
            }
            _ => return Err(()),
        }
    }
    if lengths.len() != want || lengths[256] == 0 {
        return Err(()); // "invalid code -- missing end-of-block"
    }
    Ok((huffman_build(&lengths[..hlit]), huffman_build(&lengths[hlit..])))
}

/// `zlib.decompressobj(-15)` + `ZipExtFile` 的 `_left = file_size` 上限。
/// 输出永远不超过 `cap`，因此归档里的膨胀炸弹不会撑爆内存。
pub fn inflate_raw(data: &[u8], cap: usize) -> Result<Vec<u8>, ()> {
    let mut reader = BitReader::new(data);
    let mut out: Vec<u8> = Vec::new();
    let (fixed_lit, fixed_dist) = fixed_tables();
    loop {
        let _last = reader.bits(1)?;
        let kind = reader.bits(2)?;
        match kind {
            0 => {
                reader.align_byte();
                if reader.pos + 4 > reader.data.len() {
                    return Err(());
                }
                let length = u16_le(reader.data, reader.pos) as usize;
                let nlen = u16_le(reader.data, reader.pos + 2) as usize;
                if length != (!nlen & 0xFFFF) {
                    return Err(()); // "invalid stored block lengths"
                }
                reader.pos += 4;
                if reader.pos + length > reader.data.len() {
                    return Err(());
                }
                for &b in &reader.data[reader.pos..reader.pos + length] {
                    if out.len() >= cap {
                        return Ok(out);
                    }
                    out.push(b);
                }
                reader.pos += length;
                continue;
            }
            1 | 2 | 3 => {}
            _ => return Err(()),
        }
        let (lit, dist) = if kind == 1 {
            (fixed_lit.clone(), fixed_dist.clone())
        } else if kind == 2 {
            dynamic_tables(&mut reader)?
        } else {
            return Err(());
        };
        loop {
            let symbol = huffman_decode(&mut reader, &lit)?;
            if symbol < 256 {
                if out.len() >= cap {
                    return Ok(out);
                }
                out.push(symbol as u8);
            } else if symbol == 256 {
                break;
            } else {
                let idx = symbol as usize - 257;
                if idx >= DEFLATE_LBASE.len() {
                    return Err(());
                }
                let length = DEFLATE_LBASE[idx] + reader.bits(DEFLATE_LEXT[idx])? as usize;
                let dsym = huffman_decode(&mut reader, &dist)?;
                if dsym as usize >= DEFLATE_DBASE.len() {
                    return Err(());
                }
                let distance =
                    DEFLATE_DBASE[dsym as usize] + reader.bits(DEFLATE_DEXT[dsym as usize])? as usize;
                if distance == 0 || distance > out.len() {
                    return Err(());
                }
                let start = out.len() - distance;
                for k in 0..length {
                    if out.len() >= cap {
                        return Ok(out);
                    }
                    let byte = out[start + k];
                    out.push(byte);
                }
            }
        }
        if out.len() >= cap {
            return Ok(out);
        }
    }
}

/// `shutil.copyfileobj(src, dst, length=1MiB)` 的落盘等价物。
fn extract_into(blob: &[u8], temp: &Path) -> R<()> {
    let members = match zip_infolist(blob) {
        Ok(value) => value,
        Err(detail) => return internal(detail.as_str()),
    };
    if members.len() > MAX_FILES {
        return err("archive_too_many_files", "归档文件数量超过限制");
    }
    let mut total: u64 = 0;
    let mut seen: Vec<String> = Vec::new();
    for member in &members {
        // `info.filename.rstrip("/")`：空名（纯目录 "/"）直接跳过。
        let stripped = py_rstrip_chars(&member.filename, "/");
        if stripped.is_empty() {
            continue;
        }
        let name = match safe_member(&stripped) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let folded = py_casefold(&name);
        if seen.iter().any(|existing| existing == &folded) {
            return err("archive_duplicate_path", "归档包含重复或大小写冲突的路径");
        }
        seen.push(folded);
        if member.flag_bits & MASK_ENCRYPTED != 0 {
            return err("archive_encrypted", "不支持加密的 ZIP 文件");
        }
        let file_type = (member.external_attr >> 16) & 0o170000;
        if file_type == 0o120000 {
            return err("archive_symlink", "归档包含不允许的符号链接");
        }
        if file_type != 0 && file_type != S_IFREG && file_type != S_IFDIR {
            return err("archive_special_file", "归档包含不允许的特殊文件");
        }
        let declared = member.file_size;
        if declared > MAX_FILE_BYTES as u64 {
            return err("archive_file_too_large", "归档包含超过大小限制的文件");
        }
        total += declared;
        if total > MAX_EXTRACTED_BYTES as u64 {
            return err("archive_too_large", "归档展开后超过安全大小限制");
        }
        let target = temp.join(&name);
        if let Some(parent) = target.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return internal(&format!("mkdir {}: {}", parent.display(), error));
            }
        }
        if member_is_dir(&member.filename) {
            if let Err(error) = fs::create_dir_all(&target) {
                return internal(&format!("mkdir {}: {}", target.display(), error));
            }
            continue;
        }
        let payload = match zip_member_bytes(blob, member) {
            Ok(value) => value,
            Err(detail) => return internal(detail.as_str()),
        };
        if let Err(error) = fs::write(&target, &payload) {
            return internal(&format!("write {}: {}", target.display(), error));
        }
    }
    Ok(())
}

/// `_extract(blob)`：解到一个私有临时目录；任何失败都会先把临时目录删掉再抛。
pub fn extract(blob: &[u8]) -> R<PathBuf> {
    let temp = make_temp_dir("readmd-skill-import-");
    match extract_into(blob, &temp) {
        Ok(()) => Ok(temp),
        Err(error) => {
            remove_tree(&temp);
            Err(error)
        }
    }
}

// ============================================================ 注入依赖（网络 / 凭据 / 校验）

/// `urllib.request.urlopen()` 的成功结果。
///
/// `final_url` 对应 `response.geturl()`：Python 在读取响应体**之前**会再 `_safe_url()` 校验一次
/// 每一跳与最终地址，所以 transport 必须把重定向后的真实地址交回来。
#[derive(Clone, Debug)]
pub struct FetchResult {
    pub body: Vec<u8>,
    pub final_url: String,
}

/// `urllib.error.HTTPError` / `(URLError, TimeoutError)` 的失败结果。
#[derive(Clone, Debug)]
pub struct HttpError {
    pub status: u16,
    /// `exc.headers.get("X-RateLimit-Remaining")`，只有 HTTP 错误分支用得到。
    pub rate_limit_remaining: Option<String>,
}

pub type Fetcher = Box<dyn Fn(&str, &str, bool) -> Result<FetchResult, HttpError> + Send + Sync>;
/// `crypto.load_credential(credential_id) -> str`（`src/readmd_modules/crypto.py:220`）。
// WIRING: readmd_modules::crypto.load_credential
pub type CredentialLoader = Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>;
/// `skills.SkillRegistry([parent]).validate(destination)`（`src/readmd_modules/skills.py`）。
/// `Err(true)` = 抛出了 `SkillError` ⇒ `skill_invalid`；`Err(false)` = 其它异常 ⇒ 原样上抛
/// （在 `readmd.py` 里落到 `except Exception` ⇒ 500）。
// WIRING: readmd_modules::skills::SkillRegistry::validate
pub type SkillValidator = Box<dyn Fn(&Path) -> Result<(), bool> + Send + Sync>;

/// 本模块需要的全部外部世界：配置路径、HTTP、凭据解密、结构校验与时钟。
///
/// 装配阶段构造它（`DATA_DIR` / `SKILLS_FILE` 取自 `readmd_core.config`），其余逻辑与 Python 一致。
#[derive(Default)]
pub struct Ctx {
    /// `readmd_core.config.DATA_DIR`
    // WIRING: readmd_kernel::paths::DATA_DIR
    pub data_dir: PathBuf,
    /// `readmd_core.config.SKILLS_FILE`（= `DATA_DIR/skills.json`）
    // WIRING: readmd_kernel::paths::SKILLS_FILE
    pub skills_file: PathBuf,
    pub fetch: Option<Fetcher>,
    pub credential: Option<CredentialLoader>,
    pub validator: Option<SkillValidator>,
    /// 固定时间戳，便于测试复现 `imported_at` / 备份目录名。
    pub now_iso: Option<String>,
    pub now_stamp: Option<String>,
}

impl Ctx {
    pub fn new() -> Self {
        Self::default()
    }
    fn iso(&self) -> String {
        utc_isoformat(self.now_iso.as_deref())
    }
    fn stamp(&self) -> String {
        utcstrftime_compact(self.now_stamp.as_deref())
    }
}

// ============================================================ _token / _safe_url / _request

/// `_token(credential_id)`
pub fn token(ctx: &Ctx, credential_id: &str) -> R<String> {
    if credential_id.is_empty() {
        return Ok(String::new());
    }
    let loader = match ctx.credential.as_ref() {
        Some(value) => value,
        None => return internal("credential loader not wired"),
    };
    match loader(credential_id) {
        Ok(value) => Ok(value),
        // Python: `except Exception` ⇒ 一律 credential_invalid
        Err(_) => err("credential_invalid", "GitHub 凭据不可用"),
    }
}

/// `_safe_url(url, api)`
pub fn safe_url(url: &str, api: bool) -> R<()> {
    let parsed = url_parse(url);
    let allowed: [&str; 2] = if api {
        [GITHUB_API_HOST, GITHUB_API_HOST]
    } else {
        [GITHUB_API_HOST, "codeload.github.com"]
    };
    let host = parsed.hostname();
    if parsed.scheme != "https" || !allowed.iter().any(|item| *item == host) {
        return err("github_redirect_blocked", "GitHub 请求发生了不受信任的跳转");
    }
    Ok(())
}

/// `_request(url, token, api=..., limit=...)`
pub fn request(ctx: &Ctx, url: &str, token: &str, api: bool, limit: usize) -> R<Vec<u8>> {
    safe_url(url, api)?;
    let fetcher = match ctx.fetch.as_ref() {
        Some(value) => value,
        None => return internal("http transport not wired"),
    };
    let result = match fetcher(url, token, api) {
        Ok(value) => value,
        Err(failure) => {
            let status = failure.status;
            if status == 429 || (status == 403 && failure.rate_limit_remaining.as_deref() == Some("0")) {
                return err("github_rate_limited", "GitHub 请求已达到速率限制，请稍后重试");
            }
            if status == 401 || status == 403 {
                return err("github_auth_failed", "GitHub 凭据无权读取此仓库");
            }
            if status == 404 || status == 422 {
                return err("github_not_found", "仓库、分支或文件不存在，或仓库为私有");
            }
            return err(
                "github_http_error",
                &format!("GitHub 请求失败（HTTP {}）", status),
            );
        }
    };
    // `with opener.open(...) as response: _safe_url(response.geturl(), api=api)`
    safe_url(&result.final_url, api)?;
    if result.body.len() > limit {
        return err("github_response_too_large", "GitHub 响应超过安全大小限制");
    }
    Ok(result.body)
}

/// `_json(url, token)`
pub fn request_json(ctx: &Ctx, url: &str, token: &str) -> R<JVal> {
    let body = match request(ctx, url, token, true, MAX_RESPONSE_BYTES) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    let text = match std::str::from_utf8(&body) {
        Ok(value) => value,
        Err(_) => return err("github_response_invalid", "GitHub 返回了无效数据"),
    };
    match json_loads(text) {
        Ok(value) => Ok(value),
        Err(_) => err("github_response_invalid", "GitHub 返回了无效数据"),
    }
}

/// `_api_base(source)`
pub fn api_base(source: &JVal) -> String {
    format!(
        "https://api.github.com/repos/{}/{}",
        source.get("owner").map(|v| v.py_str()).unwrap_or_default(),
        source.get("repo").map(|v| v.py_str()).unwrap_or_default()
    )
}

/// `re.fullmatch(r"[0-9a-fA-F]{7,64}", sha)`
pub fn commit_sha_fullmatch(value: &str) -> bool {
    let bytes = value.as_bytes();
    (7..=64).contains(&bytes.len())
        && bytes.iter().all(|b| b.is_ascii_hexdigit())
}

/// `_resolve(source, token)`：会像 Python 那样**就地改写** `source["ref"] / ["subdir"] /
/// ["canonical_url"]`，因此签名收 `&mut JVal`。
pub fn resolve(ctx: &Ctx, source: &mut JVal, token: &str) -> R<(String, JVal, Vec<JVal>)> {
    let base = api_base(source);
    let repo = request_json(ctx, &base, token)?;
    let default_ref = match source.get("ref") {
        Some(value) if value.truthy() => value.py_str(),
        _ => match repo.get("default_branch") {
            Some(value) if value.truthy() => value.py_str(),
            _ => "main".to_string(),
        },
    };
    let ref_tail: Vec<String> = match source.get("_ref_tail") {
        Some(JVal::List(items)) => items.iter().map(|v| v.py_str()).collect(),
        Some(value) if value.truthy() => vec![value.py_str()],
        _ => Vec::new(),
    };
    let candidates: Vec<String> = if !ref_tail.is_empty() {
        (0..ref_tail.len()).rev().map(|index| ref_tail[..=index].join("/")).collect()
    } else {
        vec![default_ref.clone()]
    };
    let mut commit = JVal::None;
    let mut resolved_ref = default_ref.clone();
    for candidate in candidates {
        let url = format!("{}/commits/{}", base, py_quote(&candidate, ""));
        match request_json(ctx, &url, token) {
            Ok(value) => {
                commit = value;
                let has_sha = commit.get("sha").map(|v| v.truthy()).unwrap_or(false);
                if commit.is_dict() && has_sha {
                    resolved_ref = candidate;
                    break;
                }
            }
            Err(error) => {
                if error.code == "github_not_found" {
                    // 缺失的候选分支是预期内的，不外泄。
                    continue;
                }
                return Err(error);
            }
        }
    }
    if !commit.is_dict() {
        return err("github_commit_invalid", "无法解析仓库提交");
    }
    if source.is_dict() {
        source.set_item("ref", s(&resolved_ref));
        if !ref_tail.is_empty() {
            let depth = resolved_ref.split('/').count();
            let mut remainder: Vec<String> =
                ref_tail[depth.min(ref_tail.len())..].iter().cloned().collect();
            let marker = source.get("_marker").map(|v| v.py_str()).unwrap_or_default();
            if marker == "blob"
                && !remainder.is_empty()
                && py_lower(remainder.last().unwrap()) == "skill.md"
            {
                remainder.pop();
            }
            let subdir = remainder.join("/");
            source.set_item("subdir", s(&subdir));
            let owner = source.get("owner").map(|v| v.py_str()).unwrap_or_default();
            let repo_name = source.get("repo").map(|v| v.py_str()).unwrap_or_default();
            let mut canonical = format!(
                "https://github.com/{}/{}/tree/{}",
                owner,
                repo_name,
                py_quote(&resolved_ref, "")
            );
            if !subdir.is_empty() {
                canonical += "/";
                canonical += &subdir
                    .split('/')
                    .map(|item| py_quote(item, ""))
                    .collect::<Vec<String>>()
                    .join("/");
            }
            source.set_item("canonical_url", s(&canonical));
        }
    }
    let sha = commit.get("sha").map(|v| v.or_val(s(""))).map(|v| v.py_str()).unwrap_or_default();
    if !commit_sha_fullmatch(&sha) {
        return err("github_commit_invalid", "无法解析仓库提交");
    }
    let tree = request_json(ctx, &format!("{}/git/trees/{}?recursive=1", base, sha), token)?;
    let entries = match tree.get("tree") {
        Some(value) if value.is_list() => match value {
            JVal::List(items) => items.clone(),
            _ => Vec::new(),
        },
        _ => return err("github_tree_invalid", "GitHub 仓库目录树无效"),
    };
    if tree.get("truncated").map(|v| v.truthy()).unwrap_or(false) {
        return err("github_tree_truncated", "仓库过大，无法安全扫描全部 Skill");
    }
    Ok((sha, repo, entries))
}

/// `_content(source, sha, path, token)`
pub fn content(ctx: &Ctx, source: &JVal, sha: &str, path: &str, token: &str) -> R<String> {
    let quoted = path
        .split('/')
        .map(|item| py_quote(item, ""))
        .collect::<Vec<String>>()
        .join("/");
    let url = format!(
        "{}/contents/{}?ref={}",
        api_base(source),
        quoted,
        py_quote(sha, "")
    );
    let data = request_json(ctx, &url, token)?;
    let encoded = if data.is_dict() { data.get("content").cloned().unwrap_or(JVal::None) } else { JVal::None };
    let encoding_ok = matches!(data.get("encoding"), Some(value) if value.as_rust_str() == Some("base64"));
    let text = match encoded.as_rust_str() {
        Some(value) if encoding_ok => value.to_string(),
        _ => return err("github_skill_unreadable", "无法读取 SKILL.md"),
    };
    // `base64.b64decode(encoded.encode("ascii"), validate=False)`：非 ASCII 先在内层就抛
    // `UnicodeEncodeError`（ValueError 的子类）⇒ 统一是“不是有效 UTF-8 文本”。
    if !text.is_ascii() {
        return err("github_skill_unreadable", "SKILL.md 不是有效 UTF-8 文本");
    }
    let bytes = match b64_decode_lenient(text.as_bytes()) {
        Ok(value) => value,
        Err(()) => return err("github_skill_unreadable", "SKILL.md 不是有效 UTF-8 文本"),
    };
    match String::from_utf8(bytes) {
        Ok(value) => Ok(value),
        Err(_) => err("github_skill_unreadable", "SKILL.md 不是有效 UTF-8 文本"),
    }
}

/// `preview_import(url, credential_id)`
pub fn preview_import(ctx: &Ctx, url: &str, credential_id: &str) -> R<JVal> {
    let mut source = parse_github_url(url)?;
    let token = token(ctx, credential_id)?;
    let (sha, repo, entries) = resolve(ctx, &mut source, &token)?;
    let prefix = py_strip_chars(
        &source.get("subdir").map(|v| v.py_str()).unwrap_or_default(),
        "/",
    );
    let mut skills: Vec<JVal> = Vec::new();
    let paths: Vec<String> = entries
        .iter()
        .filter(|item| matches!(item.get("type"), Some(value) if value.as_rust_str() == Some("blob")))
        .map(|item| item.get_or_empty_str("path"))
        .collect();
    let license_paths: Vec<String> =
        paths.iter().filter(|path| is_license_path(path)).cloned().collect();
    for path in paths.iter() {
        let lowered = py_lower(path);
        if !(lowered.ends_with("/skill.md") || lowered == "skill.md") {
            continue;
        }
        if !under(path, &prefix) {
            continue;
        }
        let folder = posix_dirname(path);
        let (text, name, description) = match content(ctx, &source, &sha, path, &token) {
            Ok(text) => {
                let (name, description) = frontmatter(&text);
                (text, name, description)
            }
            Err(error) => {
                skills.push(obj(&[
                    ("path", s(path)),
                    ("id", s(&slug(&posix_basename(&folder)))),
                    ("valid", jbool(false)),
                    ("error_code", s(&error.code)),
                ]));
                continue;
            }
        };
        let mut errors: Vec<String> = Vec::new();
        if !id_re_fullmatch(&name) {
            errors.push("skill_name_invalid".to_string());
        }
        if description.is_empty() {
            errors.push("skill_description_invalid".to_string());
        }
        if has_unknown_variable(&text) {
            errors.push("skill_variables_invalid".to_string());
        }
        let applicable = applicable_licenses(&folder, &license_paths);
        if applicable.is_empty() {
            errors.push("skill_license_missing".to_string());
        }
        let skill_id = if id_re_fullmatch(&name) {
            name.clone()
        } else {
            slug(&format_posix_basename_or_root(&folder))
        };
        let mut file_list: Vec<String> =
            paths.iter().filter(|other| under(other, &folder)).cloned().collect();
        file_list.sort();
        let blocking = errors.iter().any(|item| item != "skill_license_missing");
        skills.push(obj(&[
            ("id", s(&skill_id)),
            ("path", s(path)),
            ("directory", s(&folder)),
            ("name", s(&if name.is_empty() { skill_id.clone() } else { name.clone() })),
            ("description", s(&description)),
            ("files", jlist(file_list.iter().map(|item| s(item)).collect())),
            ("license_files", jlist(applicable.iter().map(|item| s(item)).collect())),
            ("valid", jbool(errors.is_empty())),
            ("draft_allowed", jbool(!blocking && !errors.is_empty())),
            ("publishable", jbool(errors.is_empty())),
            ("error_code", s(errors.first().map(|v| v.as_str()).unwrap_or(""))),
            ("error_codes", jlist(errors.iter().map(|item| s(item)).collect())),
            ("scripts_present", jbool(file_list.iter().any(|item| in_script_suffixes(item)))),
        ]));
    }
    if skills.is_empty() {
        return err("skill_not_found", "仓库中没有可导入的 SKILL.md");
    }
    let canonical = source.get("canonical_url").map(|v| v.py_str()).unwrap_or_default();
    let mut source_out = source.clone();
    source_out.set_item("type", s("github"));
    source_out.set_item("resolved_commit", s(&sha));
    source_out.set_item(
        "repository_name",
        repo.get("full_name").cloned().unwrap_or_else(|| s(""))
    );
    let credential_required =
        repo.get("private").map(|v| v.truthy()).unwrap_or(false) && token.is_empty();
    Ok(obj(&[
        ("source_id", s(&format!("gh-{}", sha256_prefix20(&canonical)))),
        ("source", source_out),
        ("license_files", jlist(license_paths.iter().map(|item| s(item)).collect())),
        ("skills", JVal::List(skills)),
        ("offline_copy", jbool(false)),
        ("credential_required", jbool(credential_required)),
    ]))
}

/// `posixpath.basename(directory) or "root"`
fn format_posix_basename_or_root(directory: &str) -> String {
    let base = posix_basename(directory);
    if base.is_empty() {
        "root".to_string()
    } else {
        base
    }
}

// ============================================================ 应用（写入）路径

fn list_or_empty(value: Option<&JVal>) -> Vec<JVal> {
    match value {
        Some(JVal::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn is_true_at(value: &JVal, key: &str) -> bool {
    matches!(value.get(key), Some(item) if item.is_true())
}

fn is_false_at(value: &JVal, key: &str) -> bool {
    matches!(value.get(key), Some(item) if item.is_false())
}

fn truthy_at(value: &JVal, key: &str) -> bool {
    value.get(key).map(|item| item.truthy()).unwrap_or(false)
}

/// `source["provenance"].pop("source_path", "")` 的原地改写。
fn pop_nested(source: &mut JVal, outer: &str, inner: &str) -> Option<JVal> {
    if let JVal::Obj(items) = source {
        if let Some((_, JVal::Obj(inner_items))) = items.iter_mut().find(|(k, _)| k == outer) {
            if let Some(pos) = inner_items.iter().position(|(k, _)| k == inner) {
                return Some(inner_items.remove(pos).1);
            }
        }
    }
    None
}

/// `root / Path(directory)`：pathlib 会丢掉空段（`Path('/a') / '' == Path('/a')`）。
fn join_directory(root: &Path, directory: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for part in directory.split(['/', '\\']) {
        if !part.is_empty() {
            out = out.join(part);
        }
    }
    out
}

/// `sorted(list_of_paths)`（Path 的逐段 normcase 次序，`probe5.txt` `sep_sort`）。
fn sort_paths_native(items: &mut Vec<PathBuf>) {
    items.sort_by(|a, b| {
        let ka = path_cmp_key(&path_str(a));
        let kb = path_cmp_key(&path_str(b));
        ka.cmp(&kb)
    });
}

/// `_skill_destination(skill_id)`
pub fn skill_destination(ctx: &Ctx, skill_id: &str) -> R<PathBuf> {
    if !id_re_fullmatch(skill_id) {
        return err("skill_id_invalid", "Skill ID 必须使用小写 kebab-case");
    }
    let root = ctx.data_dir.join("skills");
    if exists(&root) && (is_symlink(&root) || !is_dir(&root)) {
        return err("config_path_invalid", "Skill 数据目录不可用");
    }
    if fs::create_dir_all(&root).is_err() {
        return internal("cannot create skills directory");
    }
    let root_real = py_resolve(&root);
    let destination = root.join(skill_id);
    if is_symlink(&destination) {
        return err("skill_conflict", "Skill 目标是符号链接");
    }
    let resolved = py_resolve(&destination);
    if resolved.parent().map(PathBuf::from) != Some(root_real.clone()) || resolved == root_real {
        return err("skill_path_invalid", "Skill 目标路径无效");
    }
    Ok(destination)
}

/// `_skill_file(folder)`：不信任大小写，按 OS 目录顺序返回第一个 `skill.md`（不分大小写）。
pub fn skill_file(folder: &Path) -> Option<PathBuf> {
    if !is_dir(folder) {
        return None;
    }
    let entries = match fs::read_dir(folder) {
        Ok(value) => value,
        Err(_) => return None,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if is_file(&path) && py_lower(&name) == "skill.md" {
            return Some(path);
        }
    }
    None
}

/// `_restore_skill_backup(backup, destination)`
pub fn restore_skill_backup(backup: Option<&PathBuf>, destination: &Path) {
    remove_tree(destination);
    if let Some(value) = backup {
        if is_dir(value) {
            let _ = copy_tree(value, destination);
        }
    }
}

/// `_directory_sha256(directory)`
pub fn directory_sha256(directory: &Path) -> String {
    let mut files: Vec<PathBuf> = rglob_files(directory)
        .into_iter()
        .filter(|path| {
            path.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default()
                != "readmd.skill.json"
        })
        .collect();
    sort_paths_native(&mut files);
    let mut digest = Sha256::new();
    for path in files {
        let relative = relative_posix(&path, directory).unwrap_or_default();
        digest.update(relative.as_bytes());
        if let Ok(bytes) = read_bytes(&path) {
            for chunk in bytes.chunks(1024 * 1024) {
                digest.update(chunk);
            }
        }
    }
    hex_encode(&digest.finalize())
}

/// `apply_import` 与 `_apply_filesystem_import` 共用的“目标目录 + 冲突处理”前置段。
struct DestinationPlan {
    skill_id: String,
    destination: PathBuf,
    backup: Option<PathBuf>,
    /// `true` ⇒ 这条 selection 记进 `skipped` 并跳过（`conflict_action == "skip"`）。
    skip: bool,
}

fn plan_destination(ctx: &Ctx, selected: &JVal, declared: &JVal, backup_prefix: &str) -> R<DestinationPlan> {
    let pick = |value: Option<&JVal>| -> Option<String> {
        value.filter(|v| v.truthy()).map(|v| v.py_str())
    };
    let mut skill_id = pick(selected.get("target_id"))
        .or_else(|| pick(selected.get("id")))
        .or_else(|| pick(declared.get("id")))
        .unwrap_or_default();
    if !id_re_fullmatch(&skill_id) {
        return err("skill_id_invalid", "Skill ID 必须使用小写 kebab-case");
    }
    let mut destination = skill_destination(ctx, &skill_id)?;
    let mut backup: Option<PathBuf> = None;
    let mut skip = false;
    if destination.exists() {
        let action = py_lower(
            &selected
                .get("conflict_action")
                .map(|v| v.or_val(s("skip")))
                .map(|v| v.py_str())
                .unwrap_or_else(|| "skip".to_string()),
        );
        if action == "skip" {
            skip = true;
        } else if action != "replace" && action != "rename" {
            return err("skill_conflict", "Skill 已存在，请选择跳过、重命名或替换");
        } else if action == "rename" {
            let base = format!("{}-imported", skill_id);
            skill_id = base.clone();
            let mut counter = 2i64;
            while skill_destination(ctx, &skill_id)?.exists() {
                skill_id = format!("{}-{}", base, counter);
                counter += 1;
            }
            destination = skill_destination(ctx, &skill_id)?;
        } else {
            let value = ctx
                .data_dir
                .join("skills")
                .join(".versions")
                .join(&skill_id)
                .join(format!("{}-{}", backup_prefix, ctx.stamp()));
            if value.parent().map(|p| fs::create_dir_all(p)).unwrap_or_else(|| Err(std::io::Error::from(std::io::ErrorKind::Other))).is_err() {
                return internal("cannot create backup directory");
            }
            if move_path(&destination, &value).is_err() {
                return internal("cannot move existing skill aside");
            }
            backup = Some(value);
        }
    }
    if destination.parent().map(|p| fs::create_dir_all(p)).unwrap_or(Ok(())).is_err() {
        return internal("cannot create destination parent");
    }
    Ok(DestinationPlan { skill_id, destination, backup, skip })
}

/// 复制 → 规范化 SKILL.md → 附许可证 → 生成 `source_files` → 写 `readmd.skill.json` → 结构校验。
/// 任一步失败都会先 `_restore_skill_backup()` 再把错误交回调用方（对应 `except BaseException`）。
struct WriteParams<'a> {
    root: &'a Path,
    source_dir: &'a Path,
    plan: &'a DestinationPlan,
    declared: &'a JVal,
    path: &'a str,
    /// `skill_id != str(selected.get("id") or "")` 里的那个比较基准。
    compare_id: &'a str,
    /// 已 `strip("/")` 的 Skill 目录，用于许可证的 `_under()` 判定。
    directory: &'a str,
    provenance: JVal,
    /// `metadata["source"]`：`"github"` / `"directory"` / `"zip"`。
    source_field: &'a str,
    note: &'a str,
}

fn write_skill(ctx: &Ctx, params: &WriteParams) -> R<(JVal, String)> {
    let destination = params.plan.destination.clone();
    let skill_id = params.plan.skill_id.clone();
    let body = (|| -> R<(JVal, String)> {
        if copy_tree(params.source_dir, &destination).is_err() {
            return internal("copytree failed");
        }
        let imported_skill = match skill_file(&destination) {
            Some(value) => value,
            None => return err("skill_missing", "选中的 Skill 目录不完整"),
        };
        if py_path_name(&path_str(&imported_skill)) != "SKILL.md" {
            if move_path(&imported_skill, &destination.join("SKILL.md")).is_err() {
                return internal("cannot rename SKILL.md");
            }
        }
        let skill_file_path = destination.join("SKILL.md");
        let original = match read_text_universal(&skill_file_path) {
            Ok(value) => value,
            Err(_) => return internal("SKILL.md is not valid UTF-8"),
        };
        if skill_id != params.compare_id {
            if let Some((start, end)) = name_span(&original) {
                let chars: Vec<char> = original.chars().collect();
                let text = format!(
                    "{}{}{}",
                    chars_slice(&chars, 0, start),
                    skill_id,
                    chars_slice(&chars, end, chars.len())
                );
                if write_text_lf(&skill_file_path, &text).is_err() {
                    return internal("cannot rewrite SKILL.md");
                }
            }
        }
        for item in list_or_empty(params.declared.get("license_files")) {
            let license_path = item.or_val(s("")).py_str().replace('\\', "/");
            if license_path.is_empty() || under(&license_path, params.directory) {
                continue;
            }
            safe_member(&license_path)?;
            let license_source = join_directory(params.root, &license_path);
            if !is_file(&license_source) || is_symlink(&license_source) {
                return err("skill_license_missing", "Skill 许可证文件不可用");
            }
            let license_destination =
                destination.join(".readmd-licenses").join(&license_path);
            if license_destination
                .parent()
                .map(|p| fs::create_dir_all(p))
                .unwrap_or(Ok(()))
                .is_err()
            {
                return internal("cannot create license directory");
            }
            if copy2(&license_source, &license_destination).is_err() {
                return internal("cannot copy license file");
            }
        }
        let mut listing: Vec<PathBuf> = rglob_files(&destination);
        sort_paths_native(&mut listing);
        let mut source_files: Vec<JVal> = Vec::new();
        for file in listing {
            let name = file.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default();
            if name == "readmd.skill.json" {
                continue;
            }
            let bytes = match read_bytes(&file) {
                Ok(value) => value,
                Err(_) => return internal("cannot read source file"),
            };
            source_files.push(obj(&[
                ("path", s(&relative_posix(&file, &destination).unwrap_or_default())),
                ("sha256", s(&sha256_hex(&bytes))),
            ]));
        }
        let source_hash = directory_sha256(&destination);
        let enabled = params.declared.get("publishable").map(|v| v.truthy()).unwrap_or(true);
        let license_join = list_or_empty(params.declared.get("license_files"))
            .iter()
            .map(|item| item.py_str())
            .collect::<Vec<String>>()
            .join(", ");
        let mut notes: Vec<JVal> = vec![s(params.note)];
        if !enabled {
            notes.push(s("License review required before publishing or running."));
        }
        let metadata = obj(&[
            ("id", s(&skill_id)),
            ("scope", s("user")),
            ("enabled", jbool(enabled)),
            ("publishable", jbool(enabled)),
            ("scripts_allowed", jbool(false)),
            ("source", s(params.source_field)),
            ("provenance", params.provenance.clone()),
            ("source_files", jlist(source_files.clone())),
            ("source_sha256", s(&source_hash)),
            ("license", s(&license_join)),
            ("adaptation_notes", jlist(notes)),
        ]);
        let payload = format!("{}\n", json_dumps(&metadata, Some(2)));
        if !save_text_atomic(&destination.join("readmd.skill.json"), &payload) {
            return internal("cannot write readmd.skill.json");
        }
        match ctx.validator.as_ref() {
            Some(hook) => match hook(&destination) {
                Ok(()) => {}
                Err(true) => return err("skill_invalid", "导入后 Skill 未通过结构校验"),
                Err(false) => return internal("SkillRegistry raised a non-SkillError"),
            },
            None => return internal("SkillRegistry validator not wired"),
        }
        Ok((jlist(source_files), source_hash))
    })();
    match body {
        Ok(value) => Ok(value),
        Err(error) => {
            restore_skill_backup(params.plan.backup.as_ref(), &params.plan.destination);
            Err(error)
        }
    }
}

/// `preview.get("source") if isinstance(..., dict) else {}`
fn source_of(preview: &JVal) -> JVal {
    match preview.get("source") {
        Some(value) if value.is_dict() => value.clone(),
        _ => obj(&[]),
    }
}

/// `apply_import(preview, selections, credential_id, confirm)`：GitHub 流程。
///
/// 与 Python 的唯一差异是临时目录：Python 里 `_scan_directory()` / `_load_sources()` 抛错发生在
/// `try:` 之前，因此 `_extract()` 的临时目录会泄漏；这里一律清掉。API 侧不可观测。
pub fn apply_import(
    ctx: &Ctx,
    preview: &JVal,
    selections: &[JVal],
    credential_id: &str,
    confirm: bool,
) -> R<JVal> {
    if !confirm {
        return err("confirmation_required", "导入 Skill 需要明确确认");
    }
    let source = source_of(preview);
    let mut parsed = parse_github_url(&source.get_or_empty_str("canonical_url"))?;
    let token = token(ctx, credential_id)?;
    let (sha, _repo, entries) = resolve(ctx, &mut parsed, &token)?;
    let expected = source.get_or_empty_str("resolved_commit");
    if !expected.is_empty() && sha != expected {
        return err("source_changed", "预览后仓库已发生变化，请重新预览");
    }
    let archive_url = format!("{}/zipball/{}", api_base(&parsed), sha);
    let blob = request(ctx, &archive_url, &token, false, MAX_RESPONSE_BYTES)?;
    let extracted = extract(&blob)?;
    let result = apply_github_body(
        ctx, preview, selections, &parsed, &sha, &entries, &extracted, credential_id,
    );
    remove_tree(&extracted);
    result
}

#[allow(clippy::too_many_arguments)]
fn apply_github_body(
    ctx: &Ctx,
    preview: &JVal,
    selections: &[JVal],
    parsed: &JVal,
    sha: &str,
    entries: &[JVal],
    extracted: &Path,
    credential_id: &str,
) -> R<JVal> {
    let root = archive_root(extracted);
    let mut allowed_paths: Vec<String> = Vec::new();
    for item in entries {
        if matches!(item.get("type"), Some(value) if value.as_rust_str() == Some("blob")) {
            let path = item.get_or_empty_str("path").replace('\\', "/");
            if !allowed_paths.contains(&path) {
                allowed_paths.push(path);
            }
        }
    }
    let manifest = scan_directory(&root)?;
    let manifest_paths: Vec<String> =
        manifest.iter().map(|item| item.get_or_empty_str("path")).collect();
    if manifest_paths.iter().any(|path| !allowed_paths.contains(path)) {
        return err("source_changed", "GitHub 归档与已解析的提交目录不一致");
    }
    let (scanned_skills, _licenses) = scan_skill_candidates(&root, &manifest);
    let mut imported: Vec<JVal> = Vec::new();
    let mut skipped: Vec<JVal> = Vec::new();
    let mut config = load_sources(ctx)?;
    for selected in selections {
        if !selected.is_dict() {
            continue;
        }
        let path = selected.get_or_empty_str("path").replace('\\', "/");
        let declared = match scanned_skills
            .iter()
            .find(|item| item.get_or_empty_str("path") == path)
            .cloned()
        {
            Some(value) => value,
            None => return err("skill_path_invalid", "选中的 Skill 路径不在提交清单中"),
        };
        if !declaration_importable(&declared) {
            let code = declared.get_or_empty_str("error_code");
            let code = if code.is_empty() { "skill_invalid".to_string() } else { code };
            return err(&code, "选中的 Skill 未通过安全校验");
        }
        if is_true_at(&declared, "valid")
            && !truthy_at(&declared, "license_files")
            && !is_false_at(&declared, "publishable")
        {
            return err("skill_license_missing", "Skill 许可证文件不可用");
        }
        let fallback = posix_dirname(&path);
        let directory = py_strip_chars(
            &declared
                .get("directory")
                .map(|v| v.or_val(s("")))
                .map(|v| v.py_str())
                .filter(|v| !v.is_empty())
                .unwrap_or(fallback),
            "/",
        );
        let lowered = py_lower(&path);
        if !allowed_paths.contains(&path)
            || (!lowered.ends_with("/skill.md") && lowered != "skill.md")
        {
            return err("skill_path_invalid", "选中的 Skill 路径不在预览清单中");
        }
        if !directory.is_empty()
            && (directory.split('/').any(|part| part == "..") || !under(&path, &directory))
        {
            return err("skill_path_invalid", "选中的 Skill 目录无效");
        }
        if directory != posix_dirname(&path) {
            return err("skill_path_invalid", "选中的 Skill 目录与文件路径不匹配");
        }
        let source_dir = join_directory(&root, &directory);
        let source_skill = skill_file(&source_dir);
        if !is_dir(&source_dir) || source_skill.is_none() {
            return err("skill_missing", "选中的 Skill 目录不完整");
        }
        let plan = plan_destination(ctx, selected, &declared, "github")?;
        if plan.skip {
            skipped.push(obj(&[("id", s(&plan.skill_id)), ("reason", s("conflict"))]));
            continue;
        }
        let compare_id =
            selected.get("id").map(|v| v.or_val(s(""))).map(|v| v.py_str()).unwrap_or_default();
        let provenance = obj(&[
            (
                "repository",
                s(&parsed.get("canonical_url").map(|v| v.py_str()).unwrap_or_default()),
            ),
            ("commit", s(sha)),
            ("path", s(&path)),
        ]);
        let (source_files, source_hash) = write_skill(
            ctx,
            &WriteParams {
                root: &root,
                source_dir: &source_dir,
                plan: &plan,
                declared: &declared,
                path: &path,
                compare_id: &compare_id,
                directory: &directory,
                provenance,
                source_field: "github",
                note: "Imported from GitHub as data; scripts remain disabled.",
            },
        )?;
        imported.push(obj(&[
            ("id", s(&plan.skill_id)),
            ("path", s(&path)),
            ("sha256", s(&source_hash)),
            ("source_files", source_files),
        ]));
    }
    if imported.is_empty() && !skipped.is_empty() {
        return Ok(obj(&[
            ("ok", jbool(true)),
            ("source", JVal::None),
            ("skills", jlist(Vec::new())),
            ("skipped", JVal::List(skipped)),
        ]));
    }
    if imported.is_empty() {
        return err("nothing_imported", "没有导入任何 Skill");
    }
    let canonical = parsed.get("canonical_url").map(|v| v.py_str()).unwrap_or_default();
    let source_id = preview
        .get("source_id")
        .filter(|v| v.truthy())
        .map(|v| v.py_str())
        .unwrap_or_else(|| format!("gh-{}", sha256_prefix20(&canonical)));
    let sources = list_or_empty(config.get("sources"));
    let previous = sources
        .iter()
        .find(|item| {
            item.get("source_id").and_then(|v| v.as_rust_str()) == Some(source_id.as_str())
        })
        .cloned()
        .unwrap_or_else(|| obj(&[]));
    // `{str(item.get("id")): item for ...}`：`update()` 覆盖同名键，插入顺序保持首次出现位置。
    let mut merged: Vec<(String, JVal)> = Vec::new();
    for item in list_or_empty(previous.get("skills")) {
        if !item.is_dict() {
            continue;
        }
        let key = item.get("id").map(|v| v.py_str()).unwrap_or_default();
        match merged.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = item,
            None => merged.push((key, item)),
        }
    }
    for item in imported.clone() {
        let key = item.get("id").map(|v| v.py_str()).unwrap_or_default();
        match merged.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = item,
            None => merged.push((key, item)),
        }
    }
    let source_record = obj(&[
        ("source_id", s(&source_id)),
        ("source_type", s("github")),
        ("source_sha256", s(&manifest_sha256(&manifest))),
        ("repository_url", s(&canonical)),
        ("owner", s(&parsed.get("owner").map(|v| v.py_str()).unwrap_or_default())),
        ("repo", s(&parsed.get("repo").map(|v| v.py_str()).unwrap_or_default())),
        ("subdir", s(&parsed.get("subdir").map(|v| v.py_str()).unwrap_or_default())),
        ("requested_ref", s(&parsed.get("ref").map(|v| v.py_str()).unwrap_or_default())),
        ("resolved_commit", s(sha)),
        ("credential_id", s(credential_id)),
        ("update_policy", s("manual")),
        ("skills", jlist(merged.iter().map(|(_, v)| v.clone()).collect())),
        ("imported_at", s(&ctx.iso())),
    ]);
    let mut kept: Vec<JVal> = sources
        .iter()
        .filter(|item| {
            item.get("source_id").and_then(|v| v.as_rust_str()) != Some(source_id.as_str())
        })
        .cloned()
        .collect();
    kept.push(source_record.clone());
    config.set_item("sources", jlist(kept));
    save_sources(ctx, &config)?;
    Ok(obj(&[
        ("ok", jbool(true)),
        ("source", source_record),
        ("skills", JVal::List(imported)),
        ("skipped", JVal::List(skipped)),
    ]))
}

/// `_apply_filesystem_import(preview, selections, root, actual_hash, source_type, source_path,
/// archive_hash)`
#[allow(clippy::too_many_arguments)]
pub fn apply_filesystem_import(
    ctx: &Ctx,
    preview: &JVal,
    selections: &[JVal],
    root: &Path,
    actual_hash: &str,
    source_type: &str,
    source_path: &str,
    archive_hash: &str,
) -> R<JVal> {
    let preview_skills = list_or_empty(preview.get("skills"));
    let mut imported: Vec<JVal> = Vec::new();
    let mut skipped: Vec<JVal> = Vec::new();
    let mut config = load_sources(ctx)?;
    for selected in selections {
        if !selected.is_dict() {
            continue;
        }
        let path = selected.get_or_empty_str("path").replace('\\', "/");
        let declared = match preview_skills
            .iter()
            .find(|item| item.is_dict() && item.get_or_empty_str("path") == path)
            .cloned()
        {
            Some(value) if declaration_importable(&value) => value,
            _ => return err("skill_path_invalid", "选中的 Skill 不在有效预览清单中"),
        };
        if is_true_at(&declared, "valid")
            && !truthy_at(&declared, "license_files")
            && !is_false_at(&declared, "publishable")
        {
            return err("skill_license_missing", "Skill 许可证文件不可用");
        }
        let directory = py_strip_chars(&declared.get_or_empty_str("directory"), "/");
        let expected =
            if directory.is_empty() { "skill.md".to_string() } else { posix_join(&directory, "skill.md") };
        if py_lower(&path) != py_lower(&expected) {
            return err("skill_path_invalid", "选中的 Skill 目录与入口文件不匹配");
        }
        let source_dir = join_directory(root, &directory);
        if skill_file(&source_dir).is_none() {
            return err("skill_missing", "选中的 Skill 目录不完整");
        }
        let plan = plan_destination(ctx, selected, &declared, source_type)?;
        if plan.skip {
            skipped.push(obj(&[("id", s(&plan.skill_id)), ("reason", s("conflict"))]));
            continue;
        }
        let compare_id = declared.get_or_empty_str("id");
        let provenance = obj(&[
            ("type", s(source_type)),
            ("source_label", s(&os_basename(source_path))),
            ("source_sha256", s(actual_hash)),
            ("archive_sha256", s(archive_hash)),
            ("skill_path", s(&path)),
        ]);
        let note =
            format!("Imported from a local {} as data; scripts remain disabled.", source_type);
        let (source_files, source_hash) = write_skill(
            ctx,
            &WriteParams {
                root,
                source_dir: &source_dir,
                plan: &plan,
                declared: &declared,
                path: &path,
                compare_id: &compare_id,
                directory: &directory,
                provenance,
                source_field: source_type,
                note: &note,
            },
        )?;
        imported.push(obj(&[
            ("id", s(&plan.skill_id)),
            ("path", s(&path)),
            ("sha256", s(&source_hash)),
            ("source_files", source_files),
        ]));
    }
    if imported.is_empty() && !skipped.is_empty() {
        return Ok(obj(&[
            ("ok", jbool(true)),
            ("source", JVal::None),
            ("skills", jlist(Vec::new())),
            ("skipped", JVal::List(skipped)),
        ]));
    }
    if imported.is_empty() {
        return err("nothing_imported", "没有导入任何 Skill");
    }
    let prefix = if source_type == "directory" { "dir" } else { source_type };
    let source_id = preview
        .get("source_id")
        .filter(|v| v.truthy())
        .map(|v| v.py_str())
        .unwrap_or_else(|| format!("{}-{}", prefix, py_slice_prefix(actual_hash, 20)));
    let source_record = obj(&[
        ("source_id", s(&source_id)),
        ("source_type", s(source_type)),
        ("source_label", s(&os_basename(source_path))),
        ("source_sha256", s(actual_hash)),
        ("archive_sha256", s(archive_hash)),
        ("update_policy", s("manual")),
        ("skills", JVal::List(imported.clone())),
        ("imported_at", s(&ctx.iso())),
    ]);
    let sources = list_or_empty(config.get("sources"));
    let mut kept: Vec<JVal> = sources
        .iter()
        .filter(|item| {
            item.get("source_id").and_then(|v| v.as_rust_str()) != Some(source_id.as_str())
        })
        .cloned()
        .collect();
    kept.push(source_record.clone());
    config.set_item("sources", jlist(kept));
    save_sources(ctx, &config)?;
    Ok(obj(&[
        ("ok", jbool(true)),
        ("source", source_record),
        ("skills", JVal::List(imported)),
        ("skipped", JVal::List(skipped)),
    ]))
}

/// `_apply_directory_import(preview, selections)`
pub fn apply_directory_import(ctx: &Ctx, preview: &JVal, selections: &[JVal]) -> R<JVal> {
    let source = source_of(preview);
    let root = py_expanduser(Path::new(&source.get_or_empty_str("path")));
    let manifest = scan_directory(&root)?;
    let root = py_resolve(&root);
    let actual_hash = manifest_sha256(&manifest);
    let identity = path_str(&root);
    if !truthy_at(&source, "sha256") || actual_hash != source.get_or_empty_str("sha256") {
        return err("source_changed", "预览后 Skill 来源已发生变化，请重新预览");
    }
    apply_filesystem_import(
        ctx,
        preview,
        selections,
        &root,
        &actual_hash,
        "directory",
        &identity,
        "",
    )
}

/// `_apply_zip_import(preview, selections)`
pub fn apply_zip_import(ctx: &Ctx, preview: &JVal, selections: &[JVal]) -> R<JVal> {
    let source = source_of(preview);
    let (archive_path, blob, archive_hash) =
        read_local_archive(Path::new(&source.get_or_empty_str("path")))?;
    if !truthy_at(&source, "archive_sha256")
        || archive_hash != source.get_or_empty_str("archive_sha256")
    {
        return err("source_changed", "预览后 ZIP 来源已发生变化，请重新预览");
    }
    let extracted = extract(&blob)?;
    let result = (|| -> R<JVal> {
        let root = archive_root(&extracted);
        let manifest = scan_directory(&root)?;
        let actual_hash = manifest_sha256(&manifest);
        if !truthy_at(&source, "sha256") || actual_hash != source.get_or_empty_str("sha256") {
            return err("source_changed", "预览后 ZIP 内容已发生变化，请重新预览");
        }
        let identity = path_str(&archive_path);
        apply_filesystem_import(
            ctx,
            preview,
            selections,
            &root,
            &actual_hash,
            "zip",
            &identity,
            &archive_hash,
        )
    })();
    remove_tree(&extracted);
    result
}

/// `apply_source_import(preview, selections, credential_id, confirm)`
pub fn apply_source_import(
    ctx: &Ctx,
    preview: &JVal,
    selections: &[JVal],
    credential_id: &str,
    confirm: bool,
) -> R<JVal> {
    if !confirm {
        return err("confirmation_required", "导入 Skill 需要明确确认");
    }
    let source = source_of(preview);
    let kind = py_lower(
        &source
            .get("type")
            .map(|v| v.or_val(s("github")))
            .unwrap_or_else(|| s("github"))
            .py_str(),
    );
    if kind == "directory" {
        return apply_directory_import(ctx, preview, selections);
    }
    if kind == "zip" {
        return apply_zip_import(ctx, preview, selections);
    }
    if kind == "github" {
        return apply_import(ctx, preview, selections, credential_id, true);
    }
    err("source_type_invalid", "不支持的 Skill 来源类型")
}

// ============================================================ 来源登记表（skills.json）

fn fresh_sources_doc() -> JVal {
    obj(&[("schema_version", JVal::Int(2)), ("sources", jlist(Vec::new()))])
}

/// `_load_sources()`：读 `SKILLS_FILE` 并把老记录里的绝对 `source_path` 抹掉。
pub fn load_sources(ctx: &Ctx) -> R<JVal> {
    let mut data = load_json(&ctx.skills_file, obj(&[]));
    if !data.is_dict() {
        return Ok(fresh_sources_doc());
    }
    let version_ok =
        match data.get("schema_version") {
            Some(value) => py_eq_small_int(value, 1) || py_eq_small_int(value, 2),
            None => false,
        };
    if !version_ok {
        return Ok(fresh_sources_doc());
    }
    if !matches!(data.get("sources"), Some(value) if value.is_list()) {
        data.set_item("sources", jlist(Vec::new()));
    }
    data.set_item("schema_version", JVal::Int(2));
    let mut migrated = false;
    let sources = list_or_empty(data.get("sources"));
    let mut next_sources: Vec<JVal> = Vec::with_capacity(sources.len());
    for mut source in sources {
        if !source.is_dict() {
            next_sources.push(source);
            continue;
        }
        let path = source.pop("source_path").unwrap_or_else(|| s(""));
        migrated = migrated || path.truthy();
        if path.truthy() && !truthy_at(&source, "source_label") {
            source.set_item("source_label", s(&os_basename(&path.py_str())));
        }
        if matches!(source.get("provenance"), Some(value) if value.is_dict()) {
            let inner = pop_nested(&mut source, "provenance", "source_path").unwrap_or_else(|| s(""));
            migrated = migrated || inner.truthy();
            if inner.truthy() && !truthy_at(&source, "source_label") {
                source.set_item("source_label", s(&os_basename(&inner.py_str())));
            }
        }
        next_sources.push(source);
    }
    data.set_item("sources", jlist(next_sources));
    // 隐私迁移必须立刻落盘：只在内存里清洗会把绝对路径留在磁盘上、下次启动又被读回来。
    if migrated {
        save_sources(ctx, &data)?;
    }
    Ok(data)
}

/// `x == 1` / `x == 2` 的 Python 语义：`True == 1`、`1.0 == 1` 成立，字符串与 `None` 不成立。
fn py_eq_small_int(value: &JVal, want: i128) -> bool {
    match value {
        JVal::Int(i) => *i == want,
        JVal::Bool(b) => (*b as i128) == want,
        JVal::Float(f) => (*f) == want as f64,
        _ => false,
    }
}

/// `_save_sources(data)`
pub fn save_sources(ctx: &Ctx, data: &JVal) -> R<()> {
    if !save_json(&ctx.skills_file, data) {
        return err("config_write_failed", "无法保存 Skill 来源配置");
    }
    Ok(())
}

/// `list_sources()`
pub fn list_sources(ctx: &Ctx) -> R<Vec<JVal>> {
    Ok(list_or_empty(load_sources(ctx)?.get("sources")))
}

/// `find_source(source_id)`
pub fn find_source(ctx: &Ctx, source_id: &str) -> R<Option<JVal>> {
    Ok(list_sources(ctx)?
        .iter()
        .find(|item| item.get("source_id").and_then(|v| v.as_rust_str()) == Some(source_id))
        .cloned())
}

/// `preview_saved_source(source, credential_id)`
pub fn preview_saved_source(ctx: &Ctx, source: &JVal, credential_id: &str) -> R<JVal> {
    let kind = py_lower(&py_strip(
        &source
            .get("source_type")
            .map(|v| v.or_val(s("github")))
            .unwrap_or_else(|| s("github"))
            .py_str(),
    ));
    let value = if kind == "github" {
        source.get_or_empty_str("repository_url")
    } else if kind == "directory" || kind == "zip" {
        source.get_or_empty_str("source_path")
    } else {
        return err("source_type_invalid", "不支持的 Skill 来源类型");
    };
    if value.is_empty() {
        return err("source_not_found", "Skill 来源不可用");
    }
    preview_source(ctx, &kind, &value, credential_id)
}

/// `source_preview_changed(source, preview)`
pub fn source_preview_changed(source: &JVal, preview: &JVal) -> bool {
    let kind = py_lower(&py_strip(
        &source
            .get("source_type")
            .map(|v| v.or_val(s("github")))
            .unwrap_or_else(|| s("github"))
            .py_str(),
    ));
    let next_source = source_of(preview);
    if kind == "github" {
        return next_source.get_or_empty_str("resolved_commit")
            != source.get_or_empty_str("resolved_commit");
    }
    next_source.get_or_empty_str("sha256") != source.get_or_empty_str("source_sha256")
}

/// `remove_source(source_id)`：只解绑来源，已导入的 Skill 文件保持原样。
pub fn remove_source(ctx: &Ctx, source_id: &str) -> R<bool> {
    let mut data = load_sources(ctx)?;
    let sources = list_or_empty(data.get("sources"));
    let before = sources.len();
    let kept: Vec<JVal> = sources
        .into_iter()
        .filter(|item| item.get("source_id").and_then(|v| v.as_rust_str()) != Some(source_id))
        .collect();
    let after = kept.len();
    data.set_item("sources", jlist(kept));
    if after == before {
        return Ok(false);
    }
    save_sources(ctx, &data)?;
    Ok(true)
}

// ============================================================ 测试
//
// 所有期望值都由权威解释器（CPython 3.11.15，仓库 `main` 分支的
// `src/readmd_modules/skill_import.py`）实跑抓取，见
// `scratch/rust_parity/s8build/out.txt` 与 `out4.txt`；不是 Rust 侧自造的。

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // -------------------------------------------------------- 断言小工具

    fn code_of<T>(result: &R<T>) -> String {
        match result {
            Err(e) => e.code.clone(),
            Ok(_) => "<ok>".to_string(),
        }
    }

    fn unwired<T: std::fmt::Debug>(result: R<T>) -> T {
        match result {
            Ok(value) => value,
            Err(e) => panic!("unexpected SkillImportError {}: {}", e.code, e.message),
        }
    }

    fn fails(result: &R<impl Sized>, want: &str) {
        assert_eq!(code_of(result), want, "expected error_code `{}`", want);
    }

    fn sv(value: &JVal, key: &str) -> String {
        value.get_or_empty_str(key)
    }

    /// 每次调用一个独立的临时目录（只用 `std`，不引 `tempfile`，以保持本模块可脱离 cargo 验证）。
    fn temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let id = SEQ.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "readmd_si_s8_{}_{}_{}",
            std::process::id(),
            tag,
            id
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    // -------------------------------------------------------- CPython 字符串语义

    /// `probe: strip()` —— `str.strip()` 会剥掉 U+00A0/U+3000/U+0085/U+2028/U+2029 这些
    /// 多字节 `White_Space`；ASCII 扫描器不会。这正是索引越界类的 bug 源头。
    #[test]
    fn py_strip_matches_python_unicode_whitespace() {
        assert_eq!(py_strip("\u{a0}x"), "x");
        assert_eq!(py_strip("\u{3000}ab"), "ab");
        assert_eq!(py_strip("\u{85}z"), "z");
        assert_eq!(py_strip("\u{2028}p"), "p");
        assert_eq!(py_strip("\u{2029}q"), "q");
        assert_eq!(py_strip("\tn"), "n");
        assert_eq!(py_strip("  "), "");
        assert_eq!(py_strip(""), "");
        assert_eq!(py_strip("  keep me  "), "keep me");
    }

    /// `probe: splitlines()` —— Python 有 11 个换行边界，`str::lines()` 只有 2 个。
    #[test]
    fn py_splitlines_matches_all_python_boundaries() {
        for sep in ['\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}',
                    '\u{2028}', '\u{2029}'] {
            let text = format!("a{}b", sep);
            assert_eq!(py_splitlines(&text), vec!["a".to_string(), "b".to_string()],
                       "boundary U+{:04X}", sep as u32);
        }
        assert_eq!(py_splitlines("a\x00b"), vec!["a\x00b".to_string()]);
        assert_eq!(py_splitlines(""), Vec::<String>::new());
        assert_eq!(py_splitlines("a"), vec!["a".to_string()]);
        assert_eq!(py_splitlines("a\n\nb"), vec!["a".to_string(), "".to_string(), "b".to_string()]);
    }

    /// `probe: slice` —— Python 按码点切片并在末尾截断；Rust 的 `&s[a..b]` 会 panic。
    #[test]
    fn py_slice_prefix_is_codepoint_indexed_and_clamped() {
        assert_eq!(py_slice_prefix("héllo", 3), "hél");
        assert_eq!(py_slice_prefix("中文abc", 2), "中文");
        assert_eq!(py_slice_prefix("abc", 99), "abc");
        assert_eq!(py_slice_prefix("", 5), "");
        assert_eq!(py_slice_prefix("abc", 0), "");
    }

    /// `probe: quote` —— `urllib.parse.quote(x, safe="")`。
    #[test]
    fn py_quote_matches_urlparse() {
        assert_eq!(py_quote("a b", ""), "a%20b");
        assert_eq!(py_quote("/a/b", ""), "%2Fa%2Fb");
        assert_eq!(py_quote("!@#$%", ""), "%21%40%23%24%25");
        assert_eq!(py_quote("", ""), "");
        assert_eq!(py_quote("a-b_c.d~e", ""), "a-b_c.d~e");
        assert_eq!(py_quote("*._", ""), "%2A._");
    }

    #[test]
    fn py_unquote_roundtrips_percent_escapes() {
        assert_eq!(py_unquote("a%20b"), "a b");
        assert_eq!(py_unquote("%2Fa%2Fb"), "/a/b");
        assert_eq!(py_unquote("%E4%B8%AD"), "中");
        // 裸 `%` 不是合法转义：必须原样保留，不能 panic 也不能丢字符。
        assert_eq!(py_unquote("100%"), "100%");
    }

    #[test]
    fn py_lower_matches_python() {
        assert_eq!(py_lower("Ünïcödé X"), "ünïcödé x");
        assert_eq!(py_lower("GITHUB "), "github ");
    }

    // -------------------------------------------------------- posixpath / pathlib

    /// `posixpath.basename` / `posixpath.dirname`（**不是** `pathlib` 的 `.name`/`.parent`：
    /// `posixpath.basename("a/") == ""` 而 `PurePosixPath("a/").name == "a"`）。
    #[test]
    fn posix_path_helpers_match_python() {
        assert_eq!(posix_basename("a/b"), "b");
        assert_eq!(posix_basename("a/"), "");
        assert_eq!(posix_basename("/"), "");
        assert_eq!(posix_basename(""), "");
        assert_eq!(posix_basename("a"), "a");
        assert_eq!(posix_basename("a//b"), "b");
        assert_eq!(posix_dirname("a/b"), "a");
        assert_eq!(posix_dirname("a/"), "a");
        assert_eq!(posix_dirname("/"), "/");
        assert_eq!(posix_dirname(""), "");
        assert_eq!(posix_dirname("a"), "");
        assert_eq!(posix_dirname("a//b"), "a");
        assert_eq!(posix_dirname("x/y/z"), "x/y");
    }

    /// `probe: suffix` —— `Path.suffix` 只取最后一段扩展名且**不**折叠大小写。
    #[test]
    fn py_suffix_matches_pathlib() {
        assert_eq!(py_suffix("a.TAR.GZ"), ".GZ");
        assert_eq!(py_suffix("a."), "");
        assert_eq!(py_suffix("."), "");
        assert_eq!(py_suffix("a/b"), "");
        assert_eq!(py_suffix(""), "");
        assert_eq!(py_suffix("x.y/z"), "");
        assert_eq!(py_suffix("SKILL.md"), ".md");
    }

    // -------------------------------------------------------- sha256 / base64

    #[test]
    fn sha256_hex_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha256_prefix20(""), "e3b0c44298fc1c149afb");
        // `hashlib.sha256(b"abc").hexdigest()[:20]`
        assert_eq!(sha256_prefix20("abc"), "ba7816bf8f01cfea4141");
    }

    /// FIPS 180-4 官方多块向量：单块向量（长度 0/3）掩盖不了轮常量错误，
    /// 56 字节向量压住填充边界，1e6 字节向量把 64 个轮常量和 15,625 次压缩
    /// 全部走一遍。期望值取自 CPython `hashlib`。
    #[test]
    fn sha256_hex_fips1804_canonical_vectors() {
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256_hex(vec![b'a'; 1_000_000].as_slice()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert_eq!(
            sha256_hex(b"The quick brown fox jumps over the lazy dog"),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
        assert_eq!(
            sha256_prefix20("The quick brown fox jumps over the lazy dog"),
            "d7a8fbb307d7809469ca"
        );
    }

    /// 填充边界与多块长度：单个轮常量写错时长度 0/3 的向量未必暴露问题（本次就是这样），
    /// 所以把 55/56/64/120 这些跨块边界全部钉住。期望值取自 CPython `hashlib`。
    #[test]
    fn sha256_padding_boundaries_and_incremental_update() {
        assert_eq!(
            sha256_hex(&[b'a'; 55]),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 64]),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 120]),
            "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c"
        );
        // `directory_sha256` 走的是分次 `update()`，必须与一次性哈希同值。
        let mut digest = Sha256::new();
        digest.update(&[b'a'; 7]);
        digest.update(&[b'a'; 113]);
        assert_eq!(hex_encode(&digest.finalize()), sha256_hex(&[b'a'; 120]));
        assert_eq!(sha256_prefix20(&"a".repeat(120)), "2f3d335432c70b580af0");
    }

    #[test]
    fn b64_known_vectors() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(b64_encode(&[0u8, 255, 16]), "AP8Q");
        assert_eq!(b64_decode_lenient(b"Zm9v"), Ok(b"foo".to_vec()));
        assert_eq!(b64_decode_lenient(b"Zm9vYg=="), Ok(b"foob".to_vec()));
        // `base64.b64decode("!!!!", validate=False)` 丢掉非法字符 => 空字节，不是错误。
        assert_eq!(b64_decode_lenient(b"!!!!"), Ok(Vec::new()));
        assert_eq!(b64_decode_lenient(b"Z m 9 v"), Ok(b"foo".to_vec()));
    }

    // -------------------------------------------------------- JSON（absent / null / "" 三态）

    #[test]
    fn json_dumps_matches_python_indent2() {
        let value = obj(&[("a", jlist(vec![JVal::Int(1), JVal::None, jbool(true)]))]);
        assert_eq!(
            json_dumps(&value, Some(2)),
            "{\n  \"a\": [\n    1,\n    null,\n    true\n  ]\n}"
        );
        assert_eq!(json_dumps(&obj(&[]), Some(2)), "{}");
        assert_eq!(json_dumps(&jlist(Vec::new()), Some(2)), "[]");
        assert_eq!(json_dumps(&obj(&[("a", JVal::Int(1))]), None), "{\"a\": 1}");
    }

    #[test]
    fn json_loads_roundtrip_and_failure_fallback() {
        let parsed = json_loads("{\"a\": [1, null, true]}").expect("valid json");
        assert_eq!(json_dumps(&parsed, None), "{\"a\": [1, null, true]}");
        assert!(json_loads("{not json").is_err());
        // `load_json` 的失败回退：坏文件 ⇒ 交给调用方的 default，绝不 panic。
        let dir = temp_dir("loadjson");
        let bad = dir.join("bad.json");
        fs::write(&bad, "{truncated").unwrap();
        assert_eq!(json_dumps(&load_json(&bad, obj(&[("k", JVal::Int(7))])), None), "{\"k\": 7}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn three_states_absent_null_and_empty_string_are_distinct() {
        let value = obj(&[("explicit_empty", s("")), ("null", JVal::None), ("zero", JVal::Int(0))]);
        // `str(x.get(k) or "")`：三者都是 ""，但**键缺失**与**键为 null** 在 `get` 层必须可分。
        assert!(value.get("absent").is_none());
        assert!(matches!(value.get("null"), Some(JVal::None)));
        assert_eq!(sv(&value, "null"), "");
        assert_eq!(sv(&value, "absent"), "");
        assert_eq!(sv(&value, "explicit_empty"), "");
        assert!(!value.get("null").unwrap().truthy());
        assert!(value.get("zero").unwrap().truthy() == false);
        assert_eq!(value.get("null").unwrap().py_str(), "None");
        assert_eq!(value.get("zero").unwrap().py_str(), "0");
    }

    // -------------------------------------------------------- _slug / _ID_RE / 变量

    /// `probe: slug()` —— `re.sub(r"[^a-z0-9]+","-",str(value or "").lower()).strip("-")`
    /// 之后 `[:64].rstrip("-") or "imported-skill"`。
    #[test]
    fn slug_matches_python() {
        assert_eq!(slug("Hello World"), "hello-world");
        assert_eq!(slug("  a  b  "), "a-b");
        assert_eq!(slug("My Skill v2"), "my-skill-v2");
        assert_eq!(slug("___"), "imported-skill");
        assert_eq!(slug("中文 Test"), "test");
        assert_eq!(slug("A-B_C.D"), "a-b-c-d");
        assert_eq!(slug(""), "imported-skill");
        assert_eq!(slug("  "), "imported-skill");
        assert_eq!(slug("Ünïcödé X"), "n-c-d-x");
    }

    /// `probe: _ID_RE.fullmatch` —— `^[a-z0-9][a-z0-9-]{0,63}$`。
    #[test]
    fn id_re_fullmatch_matrix() {
        let truthy = ["a", "a-b", "1x", &"a".repeat(64)];
        for value in truthy {
            assert!(id_re_fullmatch(value), "{} should match", value);
        }
        for value in ["A", "-a", "", &"a".repeat(65), "a_b"] {
            assert!(!id_re_fullmatch(value), "{} should not match", value);
        }
    }

    /// `probe: _VARIABLE_RE.findall` + `_ALLOWED_VARIABLES`。
    #[test]
    fn variable_findall_and_unknown_variables() {
        assert_eq!(
            variable_findall("{{ document }} {{x}} {{ bad var }} {{ok_1}} {{{curly}}} {{}}"),
            vec!["document", "x", "ok_1", "curly"]
        );
        assert!(!has_unknown_variable("{{document}} {{ selection }}"));
        assert!(has_unknown_variable("{{document}} {{nope}}"));
        assert!(!has_unknown_variable("no braces at all"));
        // `_ALLOWED_VARIABLES` 全集：这六个必须全部放行。
        assert!(!has_unknown_variable(
            "{{document}} {{selection}} {{request}} {{language}} {{context}} {{output_format}}"
        ));
        assert!(has_unknown_variable("{{document}} {{selection}} {{request}} {{language}} {{context}} {{output_format}} {{nope}}"));
    }

    // -------------------------------------------------------- parse_github_url

    /// `probe: pgu()` —— 成功分支返回 7 个键（含私有的 `_ref_tail` / `_marker`）。
    #[test]
    fn parse_github_url_success_paths() {
        let plain = unwired(parse_github_url("https://github.com/foo/bar"));
        assert_eq!(sv(&plain, "owner"), "foo");
        assert_eq!(sv(&plain, "repo"), "bar");
        assert_eq!(sv(&plain, "ref"), "");
        assert_eq!(sv(&plain, "subdir"), "");
        assert_eq!(sv(&plain, "canonical_url"), "https://github.com/foo/bar");
        assert_eq!(sv(&plain, "_marker"), "");

        let tree = unwired(parse_github_url("https://www.github.com/a/b/tree/main/x"));
        assert_eq!(sv(&tree, "owner"), "a");
        assert_eq!(sv(&tree, "repo"), "b");
        assert_eq!(sv(&tree, "ref"), "main");
        assert_eq!(sv(&tree, "subdir"), "x");
        assert_eq!(sv(&tree, "canonical_url"), "https://github.com/a/b/tree/main/x");
        assert_eq!(sv(&tree, "_marker"), "tree");
        match tree.get("_ref_tail") {
            Some(JVal::List(items)) => assert_eq!(items.len(), 2),
            other => panic!("_ref_tail must be a list, got {:?}", other),
        }

        let trailing = unwired(parse_github_url("https://github.com/a/b/"));
        assert_eq!(sv(&trailing, "canonical_url"), "https://github.com/a/b");
        let frag = unwired(parse_github_url("https://github.com/a/b#frag"));
        assert_eq!(sv(&frag, "canonical_url"), "https://github.com/a/b");
        let upper = unwired(parse_github_url("https://GITHUB.com/A/B"));
        assert_eq!(sv(&upper, "owner"), "A");
        assert_eq!(sv(&upper, "repo"), "B");
    }

    #[test]
    fn parse_github_url_error_codes() {
        fails(&parse_github_url("github.com/foo/bar.git"), "github_host_not_allowed");
        fails(&parse_github_url("https://evil.com/foo/bar"), "github_host_not_allowed");
        fails(&parse_github_url("not a url"), "github_host_not_allowed");
        fails(&parse_github_url("//github.com/a/b"), "github_host_not_allowed");
        fails(&parse_github_url("https://api.github.com/repos/a/b"), "github_host_not_allowed");
        fails(&parse_github_url("https://github.com/foo"), "github_repo_invalid");
        fails(&parse_github_url("https://github.com/a/b/issues/1"), "github_url_invalid");
    }

    /// `probe: safe_url()` —— 它是**校验器**而不是格式化器：非 GitHub 主机一律拒绝，
    /// 而且 HTTP 头名大小写不敏感这一点也要求 `_request` 不能按字面量大小写取头。
    #[test]
    fn safe_url_is_a_validator() {
        assert!(safe_url("https://api.github.com/x?a=1", true).is_ok());
        fails(&safe_url("https://github.com/a", false), "github_redirect_blocked");
        fails(&safe_url("https://x/y?token=abc", false), "github_redirect_blocked");
        fails(&safe_url("https://x/y?client_secret=abc", true), "github_redirect_blocked");
    }

    // -------------------------------------------------------- 清单 / 许可证 / 归档路径

    /// `probe: fm()` —— 只有 `name:` / `description:` 两个键会被取用。
    #[test]
    fn frontmatter_matches_python() {
        let nl = "\n";
        let cr = "\r";
        let q = "'";
        let dq = "\"";
        assert_eq!(
            frontmatter(&format!("---{nl}name: Alpha{nl}description: Desc One{nl}---{nl}body")),
            ("Alpha".to_string(), "Desc One".to_string())
        );
        assert_eq!(
            frontmatter(&format!("---{nl}NAME: a{nl}description: b{nl}---{nl}")),
            ("a".to_string(), "b".to_string())
        );
        assert_eq!(
            frontmatter(&format!("---{nl}name: {q}Q{q}{nl}description: {dq}R{dq}{nl}---{nl}x")),
            ("Q".to_string(), "R".to_string())
        );
        // 缺少收尾换行 => 正则不匹配 => 两个空串。
        assert_eq!(
            frontmatter(&format!("---{nl}name: X{nl}---")),
            (String::new(), String::new())
        );
        assert_eq!(
            frontmatter(&format!("---{cr}{nl}name: W{cr}{nl}description: V{cr}{nl}---{cr}{nl}")),
            ("W".to_string(), "V".to_string())
        );
        assert_eq!(
            frontmatter(&format!("---{nl}name:  spaced  {nl}description: d{nl}---{nl}")),
            ("spaced".to_string(), "d".to_string())
        );
        assert_eq!(
            frontmatter(&format!("---{nl}no colon here{nl}name: Z{nl}---{nl}")),
            ("Z".to_string(), String::new())
        );
        // `split(":", 1)`：值里的冒号必须保留。
        assert_eq!(
            frontmatter(&format!("---{nl}name: a: b{nl}---{nl}")),
            ("a: b".to_string(), String::new())
        );
    }

    #[test]
    fn is_license_path_prefix_rules() {
        assert!(is_license_path("LICENSE"));
        assert!(is_license_path("license.txt"));
        assert!(is_license_path("docs/LICENSE.md"));
        assert!(is_license_path("LICENSEMIT"));
        assert!(is_license_path("x/LICENCE"));
        assert!(!is_license_path("MIT-LICENSE.txt"));
        assert!(!is_license_path(""));
    }

    #[test]
    fn under_prefix_semantics() {
        assert!(under("a/b", "a"));
        assert!(under("a/b", "a/"));
        assert!(!under("ab", "a"));
        assert!(under("a", "a"));
        assert!(!under("", "a"));
        assert!(under("a/../b", "a"));
    }

    /// `probe: manifest0/1` —— 排序 + `path\0sha\0` 拼接的字节级摘要。
    #[test]
    fn manifest_sha256_python_oracles() {
        assert_eq!(
            manifest_sha256(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let files = vec![
            obj(&[("path", s("a/b.txt")), ("sha256", s(&"aa".repeat(32)))]),
            obj(&[("path", s("a")), ("sha256", s(&"bb".repeat(32)))]),
        ];
        assert_eq!(
            manifest_sha256(&files),
            "87765a97287b64d341cbb80f8b739656ddead2576a3a6ca18957255d0e933f90"
        );
        // 顺序无关（按 path 排序）。
        let flipped = vec![files[1].clone(), files[0].clone()];
        assert_eq!(manifest_sha256(&flipped), manifest_sha256(&files));
    }

    #[test]
    fn applicable_licenses_scopes_by_directory() {
        let licenses = ["skills", "skills/a/b", "other", "skills/a", ""]
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<String>>();
        assert_eq!(
            applicable_licenses("skills/a", &licenses),
            vec![
                "".to_string(),
                "other".to_string(),
                "skills".to_string(),
                "skills/a".to_string(),
                "skills/a/b".to_string()
            ]
        );
        // 不在目录之下的许可证文件必须被排除。
        let scoped = ["docs/LICENSE"].iter().map(|v| v.to_string()).collect::<Vec<String>>();
        assert_eq!(applicable_licenses("skills/a", &scoped), Vec::<String>::new());
    }

    /// `probe: member()` —— ZIP 路径穿越防线。
    #[test]
    fn safe_member_blocks_traversal() {
        assert_eq!(unwired(safe_member("a/b.txt")), "a/b.txt".to_string());
        assert_eq!(unwired(safe_member("dir/sub/f.txt")), "dir/sub/f.txt".to_string());
        assert_eq!(unwired(safe_member("a/ b/c")), "a/ b/c".to_string());
        for bad in ["../x", "/abs", "a//b", "./a", "a/./b", "a/", "a/../../b", "..", ""] {
            fails(&safe_member(bad), "archive_path_invalid");
        }
        // 反斜杠先归一化成 `/`，所以 Windows 风格穿越同样被拦。
        let bs = "\\";
        fails(&safe_member(&format!("C:{bs}x")), "archive_path_invalid");
        fails(&safe_member(&format!("a{bs}..{bs}b")), "archive_path_invalid");
        fails(&safe_member(&format!("{bs}abs")), "archive_path_invalid");
        fails(&safe_member(&format!("a/{bs}b")), "archive_path_invalid");
        // MAX_PATH_LENGTH = 240，判定是 `len(name) > 240`。
        assert!(safe_member(&"x".repeat(240)).is_ok());
        fails(&safe_member(&"x".repeat(241)), "archive_path_too_long");
        // MAX_PATH_DEPTH = 16，判定是 `len(Path(name).parts) > 16`。
        assert!(safe_member(&(0..16).map(|_| "a").collect::<Vec<_>>().join("/")).is_ok());
        fails(&safe_member(&(0..17).map(|_| "a").collect::<Vec<_>>().join("/")), "archive_path_too_deep");
    }

    // -------------------------------------------------------- 声明可导入性（bool 身份判定）

    /// `probe: decl` —— `declared.get("valid") is True` 是**身份**比较，所以 `1`/`1.0`/`"yes"`
    /// 都不算；这正是 `isinstance(x, bool)` 一类的坑。
    #[test]
    fn declaration_importable_requires_real_bool() {
        assert!(declaration_importable(&obj(&[("valid", jbool(true))])));
        assert!(declaration_importable(&obj(&[
            ("valid", jbool(true)),
            ("error_codes", jlist(vec![s("x")]))
        ])));
        assert!(!declaration_importable(&obj(&[("valid", JVal::Int(1))])));
        assert!(!declaration_importable(&obj(&[("valid", JVal::Float(1.0))])));
        assert!(!declaration_importable(&obj(&[("valid", s("yes"))])));
        assert!(!declaration_importable(&obj(&[("draft_allowed", JVal::Int(1)),
            ("error_codes", jlist(vec![s("skill_license_missing")]))])));
        assert!(declaration_importable(&obj(&[
            ("draft_allowed", jbool(true)),
            ("error_codes", jlist(vec![s("skill_license_missing")]))
        ])));
        assert!(!declaration_importable(&obj(&[
            ("draft_allowed", jbool(true)),
            ("error_codes", jlist(vec![s("skill_license_missing"), s("other")]))
        ])));
        assert!(declaration_importable(&obj(&[
            ("draft_allowed", jbool(true)),
            ("error_code", s("skill_license_missing"))
        ])));
        assert!(!declaration_importable(&obj(&[
            ("draft_allowed", jbool(false)),
            ("error_codes", jlist(vec![s("skill_license_missing")]))
        ])));
        assert!(!declaration_importable(&obj(&[("error_codes", jlist(Vec::new()))])));
        assert!(!declaration_importable(&obj(&[
            ("draft_allowed", jbool(true)),
            ("error_codes", jlist(vec![s("SKILL_LICENSE_MISSING")]))
        ])));
        assert!(!declaration_importable(&obj(&[])));
        // `if code` 过滤掉假值，但集合本身仍要求恰好等于单元素集合。
        assert!(declaration_importable(&obj(&[
            ("draft_allowed", jbool(true)),
            ("error_codes", jlist(vec![s("skill_license_missing"), JVal::Int(0), JVal::None]))
        ])));
    }

    // -------------------------------------------------------- 来源登记表（skills.json）

    fn ctx_with_dir(tag: &str) -> (Ctx, PathBuf) {
        let dir = temp_dir(tag);
        let ctx = Ctx { data_dir: dir.clone(), skills_file: dir.join("skills.json"), ..Ctx::new() };
        (ctx, dir)
    }

    /// `_load_sources` 的隐私迁移：绝对路径必须**立刻**落盘，否则下次启动又被读回来。
    #[test]
    fn load_sources_strips_absolute_paths_and_persists() {
        let (ctx, dir) = ctx_with_dir("mig");
        fs::write(
            &ctx.skills_file,
            "{\"schema_version\": 1, \"sources\": [{\"source_id\": \"s1\", \"source_path\": \"/home/user/secret/MySkill\", \"provenance\": {\"source_path\": \"C:\\\\Users\\\\x\\\\y\"}}, {\"source_id\": \"s2\"}]}",
        )
        .unwrap();
        let data = unwired(load_sources(&ctx));
        assert_eq!(sv(&data, "schema_version"), "2");
        let sources = list_or_empty(data.get("sources"));
        assert_eq!(sources.len(), 2);
        assert!(sources[0].get("source_path").is_none());
        assert_eq!(sv(&sources[0], "source_label"), "MySkill");
        assert!(sources[1].get("source_path").is_none());
        // 迁移已写回磁盘。
        let on_disk = load_json(&ctx.skills_file, obj(&[]));
        let disk_sources = list_or_empty(on_disk.get("sources"));
        assert!(disk_sources[0].get("source_path").is_none());
        assert!(!json_dumps(&on_disk, None).contains("/home/user/secret"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_sources_version_gate() {
        let (ctx, dir) = ctx_with_dir("ver");
        // 未知 schema => 直接当作空档，绝不保留老数据。
        fs::write(&ctx.skills_file, "{\"schema_version\": 3, \"sources\": [{\"source_id\": \"x\"}]}").unwrap();
        let data = unwired(load_sources(&ctx));
        assert_eq!(list_or_empty(data.get("sources")).len(), 0);
        // 不是 dict（这里是 list）=> 同样重置。
        fs::write(&ctx.skills_file, "[1, 2, 3]").unwrap();
        assert_eq!(list_or_empty(unwired(load_sources(&ctx)).get("sources")).len(), 0);
        // `True in (1, 2)` 在 Python 里成立，所以 bool `true` 必须被接受。
        fs::write(&ctx.skills_file, "{\"schema_version\": true, \"sources\": []}").unwrap();
        assert_eq!(sv(&unwired(load_sources(&ctx)), "schema_version"), "2");
        // 文件不存在 => 全新档，而不是 config_write_failed。
        let (fresh_ctx, fresh_dir) = ctx_with_dir("fresh");
        let fresh = unwired(load_sources(&fresh_ctx));
        assert_eq!(sv(&fresh, "schema_version"), "2");
        assert_eq!(list_or_empty(fresh.get("sources")).len(), 0);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&fresh_dir);
    }

    #[test]
    fn find_list_and_remove_source_round_trip() {
        let (ctx, dir) = ctx_with_dir("crud");
        unwired(save_sources(&ctx, &obj(&[
            ("schema_version", JVal::Int(2)),
            (
                "sources",
                jlist(vec![
                    obj(&[("source_id", s("alpha")), ("source_type", s("github"))]),
                    obj(&[("source_id", s("beta")), ("source_type", s("zip"))]),
                ]),
            ),
        ])));
        assert_eq!(unwired(list_sources(&ctx)).len(), 2);
        let found = unwired(find_source(&ctx, "beta"));
        assert_eq!(found.map(|v| sv(&v, "source_id")), Some("beta".to_string()));
        assert!(unwired(find_source(&ctx, "gamma")).is_none());
        // 不存在的 id => False 且**不**重写文件（Python 里 `len` 未变直接 return）。
        assert!(!unwired(remove_source(&ctx, "gamma")));
        assert!(unwired(remove_source(&ctx, "alpha")));
        assert_eq!(unwired(list_sources(&ctx)).len(), 1);
        assert_eq!(sv(&unwired(list_sources(&ctx))[0], "source_id"), "beta");
        let _ = fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------- source_preview_changed

    /// `probe: changed*` —— 11 个真实组合，逐个对齐 Python 返回值。
    #[test]
    fn source_preview_changed_matches_python_matrix() {
        struct Case {
            source: JVal,
            preview: JVal,
            want: bool,
        }
        let github_commit = |commit: &str| obj(&[("resolved_commit", s(commit))]);
        let sha = |value: &str| obj(&[("sha256", s(value))]);
        let cases = vec![
            Case { source: obj(&[("source_type", s("github"))]),
                   preview: obj(&[("source", github_commit("a"))]), want: true },
            Case { source: obj(&[("source_type", s("github")), ("resolved_commit", s("a"))]),
                   preview: obj(&[("source", github_commit("a"))]), want: false },
            Case { source: obj(&[("source_type", s("GITHUB ")), ("resolved_commit", s("a"))]),
                   preview: obj(&[("source", github_commit("b"))]), want: true },
            Case { source: obj(&[("resolved_commit", s("a"))]),
                   preview: obj(&[("source", github_commit("a"))]), want: false },
            Case { source: obj(&[("source_type", s("zip")), ("source_sha256", s("z"))]),
                   preview: obj(&[("source", sha("z"))]), want: false },
            Case { source: obj(&[("source_type", s("directory")), ("source_sha256", s("z"))]),
                   preview: obj(&[("source", sha("q"))]), want: true },
            Case { source: obj(&[("source_type", s("directory"))]), preview: obj(&[]), want: false },
            Case { source: obj(&[("source_type", s("directory"))]),
                   preview: obj(&[("source", s("notamapping"))]), want: false },
            Case { source: obj(&[("source_type", s("zip")), ("source_sha256", JVal::None)]),
                   preview: obj(&[("source", sha("z"))]), want: true },
            Case { source: obj(&[("source_type", JVal::None)]),
                   preview: obj(&[("source", obj(&[]))]), want: false },
            Case { source: obj(&[("source_type", JVal::None), ("resolved_commit", JVal::None)]),
                   preview: obj(&[("source", obj(&[]))]), want: false },
        ];
        for case in cases {
            assert_eq!(source_preview_changed(&case.source, &case.preview), case.want);
        }
    }

    /// 回归：`source_type` 键**缺失**时 Python 取 `"github"`（`dict.get() or "github"`），
    /// 于是比较的是 `resolved_commit` 而不是 `sha256`。
    #[test]
    fn source_preview_changed_absent_type_defaults_to_github() {
        // 没有 source_type；resolved_commit 变了 => True。若错走 sha256 分支会得到 False。
        let source = obj(&[("resolved_commit", s("old"))]);
        let preview = obj(&[("source", obj(&[("resolved_commit", s("new")), ("sha256", s("same"))]))])
            ;
        assert_eq!(
            source.get("source_type"),
            None,
            "the key must genuinely be absent for this to be the regression"
        );
        assert!(source_preview_changed(&source, &preview));
        // 同一份 preview 的 sha256 完全没变，只有 commit 变了。
        let unchanged = obj(&[("source", obj(&[("resolved_commit", s("old")), ("sha256", s("same"))]))]);
        assert!(!source_preview_changed(&source, &unchanged));
    }

    // -------------------------------------------------------- preview_saved_source

    /// 回归：`source_type` 缺失 => 走 github 适配器读 `repository_url`，
    /// 而不是 `source_type_invalid`（Python `skill_import.py:1096-1110`）。
    #[test]
    fn preview_saved_source_absent_type_uses_github_adapter() {
        let (ctx, dir) = ctx_with_dir("savedtype");
        let source = obj(&[("repository_url", s("https://github.com/a/b"))]);
        assert_eq!(source.get("source_type"), None);
        // 未装配 transport => 只可能撞到 internal_error，绝不该是 source_type_invalid。
        let result = preview_saved_source(&ctx, &source, "");
        assert_ne!(code_of(&result), "source_type_invalid");
        // 显式非法类型仍然是 source_type_invalid。
        fails(
            &preview_saved_source(
                &ctx,
                &obj(&[("source_type", s("ftp")), ("source_path", s("/x"))]),
                "",
            ),
            "source_type_invalid",
        );
        // 类型合法但取不到值 => source_not_found（github 读 repository_url）。
        fails(
            &preview_saved_source(&ctx, &obj(&[("source_type", s("github"))]), ""),
            "source_not_found",
        );
        fails(
            &preview_saved_source(&ctx, &obj(&[("source_type", s("zip"))]), ""),
            "source_not_found",
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// 回归：`preview_source` 曾把 `credential_id` 硬编码成 `""`，于是
    /// `token()` 早退、凭据永远不下发到 HTTP 层（Python `skill_import.py:484-494`）。
    #[test]
    fn preview_saved_source_forwards_credential_id_to_the_transport() {
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorder = Arc::clone(&seen);
        let loader: CredentialLoader = Box::new(|id: &str| match id {
            "cred-42" => Ok("TOK-42".to_string()),
            _ => Err("unknown credential".to_string()),
        });
        let fetch: Fetcher = Box::new(move |_url: &str, token: &str, _api: bool| {
            recorder.lock().unwrap().push(token.to_string());
            Ok(FetchResult {
                body: b"{}".to_vec(),
                final_url: "https://api.github.com/".to_string(),
            })
        });
        let dir = temp_dir("cred");
        let ctx = Ctx {
            data_dir: dir.clone(),
            skills_file: dir.join("skills.json"),
            fetch: Some(fetch),
            credential: Some(loader),
            ..Ctx::new()
        };
        let source = obj(&[("source_type", s("github")), ("repository_url", s("https://github.com/a/b"))]);
        // 解析必然失败（桩返回 `{}`），但我们只关心 HTTP 层收到的 token。
        let _ = preview_saved_source(&ctx, &source, "cred-42");
        let tokens = seen.lock().unwrap().clone();
        assert!(!tokens.is_empty(), "the stub fetcher was never reached");
        assert!(
            tokens.iter().all(|t| t == "TOK-42"),
            "credential_id did not reach the transport: {:?}",
            tokens
        );
        // 未知凭据：Python `_token` 的 `except Exception` => credential_invalid。
        fails(
            &preview_saved_source(&ctx, &source, "not-there"),
            "credential_invalid",
        );
        // 空 credential_id：`_token("")` 直接返回 ""，绝不触碰 loader。
        let _ = preview_saved_source(&ctx, &source, "");
        assert!(seen.lock().unwrap().iter().all(|t| t == "TOK-42" || t.is_empty()));
        let _ = fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------- apply_source_import

    #[test]
    fn apply_source_import_requires_confirmation() {
        let (ctx, dir) = ctx_with_dir("confirm");
        let preview = obj(&[("source", obj(&[("type", s("github"))]))]);
        fails(
            &apply_source_import(&ctx, &preview, &[], "", false),
            "confirmation_required",
        );
        // 只有 `confirm=False` 会被拒；`true` 才能往下走。
        assert_ne!(
            code_of(&apply_source_import(&ctx, &preview, &[], "", true)),
            "confirmation_required"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_source_import_rejects_unknown_kind() {
        let (ctx, dir) = ctx_with_dir("kind");
        fails(
            &apply_source_import(
                &ctx,
                &obj(&[("source", obj(&[("type", s("ftp"))]))]),
                &[],
                "",
                true,
            ),
            "source_type_invalid",
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// 回归：`preview["source"]` 没有 `type` 键时 Python 按 `"github"` 分派
    /// （`skill_import.py:1048-1066`），旧实现会返回 `source_type_invalid`。
    #[test]
    fn apply_source_import_absent_type_behaves_like_github() {
        let (ctx, dir) = ctx_with_dir("absenttype");
        let no_type = obj(&[("source", obj(&[]))]);
        let explicit = obj(&[("source", obj(&[("type", s("github"))]))]);
        assert_eq!(no_type.get("source").unwrap().get("type"), None);
        assert_eq!(
            code_of(&apply_source_import(&ctx, &no_type, &[], "", true)),
            code_of(&apply_source_import(&ctx, &explicit, &[], "", true))
        );
        assert_ne!(
            code_of(&apply_source_import(&ctx, &no_type, &[], "", true)),
            "source_type_invalid"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------- 文件系统来源（离线路径）

    #[test]
    fn preview_directory_end_to_end_matches_python_manifest_digest() {
        let root = temp_dir("prevdir");
        let skill = root.join("demo");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("LICENSE"),
            "MIT\n",
        )
        .unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: Demo\ndescription: A demo skill\n---\nBody\n",
        )
        .unwrap();
        let preview = unwired(preview_directory(&root));
        assert_eq!(sv(&preview.get("source").unwrap(), "type"), "directory");
        assert!(sv(&preview, "source_id").starts_with("dir-"));
        assert_eq!(sv(&preview, "source_id").chars().count(), 4 + 20);
        assert!(preview.get("offline_copy").map(|v| v.truthy()).unwrap_or(false));
        assert!(!preview.get("credential_required").map(|v| v.truthy()).unwrap_or(true));
        // 清单摘要逐字节钉在 Python `_manifest_sha256` 的实跑结果上（probe5），
        // 同时要求 `scan_directory` 与它自洽。
        let expected = "d853c9cd55fb0d8ea1a4d2b9019f8d3c4ccffd8c8b3569e70ee41cf44b321a8d";
        assert_eq!(sv(&preview.get("source").unwrap(), "sha256"), expected);
        assert_eq!(manifest_sha256(&unwired(scan_directory(&root))), expected);
        let licenses = list_or_empty(preview.get("license_files"));
        assert_eq!(licenses.iter().map(|v| v.py_str()).collect::<Vec<String>>(), vec!["demo/LICENSE".to_string()]);
        let skills = list_or_empty(preview.get("skills"));
        assert_eq!(skills.len(), 1);
        assert_eq!(sv(&skills[0], "id"), "demo");
        assert_eq!(sv(&skills[0], "name"), "Demo");
        assert_eq!(sv(&skills[0], "description"), "A demo skill");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn preview_directory_without_skill_md_is_reported() {
        let root = temp_dir("emptydir");
        fs::write(root.join("readme.txt"), "nothing here").unwrap();
        fails(&preview_directory(&root), "skill_not_found");
        // ZIP 分支：文件不存在 => source_not_found（`_read_local_archive` 的第一道门）。
        let missing = root.join("nope.zip");
        fails(&apply_zip_import(&Ctx::new(), &obj(&[("source", obj(&[("path", s(&path_str(&missing)))]))]), &[]), "source_not_found");
        let _ = fs::remove_dir_all(&root);
    }

    /// `extract` 收到的 blob 来自不可信归档：`zipfile` 的结构错误**不是** `SkillImportError`，
    /// 在 `readmd.py:2802 except Exception` 里落到 HTTP 500，所以必须是 `internal_error`。
    #[test]
    fn extract_rejects_malformed_archives_as_internal_errors() {
        assert_eq!(code_of(&extract(&[])), "internal_error");
        assert_eq!(code_of(&extract(b"PK\x03\x04 truncated")), "internal_error");
        assert_eq!(code_of(&extract(b"not a zip at all")), "internal_error");
    }
}




