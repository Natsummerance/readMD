// -*- coding: utf-8 -*-
//! `src/readmd_core/window_state.py` —— `WindowStateManager` 的逐行复刻。
//!
//! 权威源：
//! * `src/readmd_core/window_state.py:16-81`（四个常量 + 五个方法）
//! * `src/readmd_core/utils.py:43-92`（`load_json` / `save_json` 的失败回退与原子替换）
//! * `os.path` 的 `ntpath` / `posixpath` 语义（`normpath` / `abspath` / `dirname` / `basename`）
//!
//! 本文件目前只依赖 `std`，因此可以脱离 cargo 单独验证：
//! `rustc --edition 2021 --test -A warnings src/window_state.rs`。
//! 表驱动的 Unicode 常量由 `scratch/rust_parity/ws_uniprobe.py` 从 CPython 直接生成。
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================ CPython 异常模型

/// Python 侧真正会冒出 `load_geometry` / `save_geometry` 的四种异常。
/// 权威源码没有任何 `try/except`，所以这些错误必须原样传播给调用方。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyErr {
    pub kind: &'static str,
    pub message: String,
}

impl PyErr {
    fn new(kind: &'static str, message: String) -> Self {
        PyErr { kind, message }
    }
    fn attribute(message: String) -> Self {
        PyErr::new("AttributeError", message)
    }
    fn type_error(message: String) -> Self {
        PyErr::new("TypeError", message)
    }
    fn value_error(message: String) -> Self {
        PyErr::new("ValueError", message)
    }
    fn overflow(message: String) -> Self {
        PyErr::new("OverflowError", message)
    }
    /// `str(exc)` 前带上类型名，便于与 `*_golden.txt` 里的期望串直接比对。
    pub fn raised(&self) -> String {
        format!("{}: {}", self.kind, self.message)
    }
}

pub type R<T> = Result<T, PyErr>;

impl std::fmt::Display for PyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raised())
    }
}

impl std::error::Error for PyErr {}

// ============================================================ JSON 值（保序）

/// Python 的 `dict` 记住插入顺序，且对已存在的键重新赋值时保持原位
/// （`ws_golden.txt:122 reorder` 已实测），所以这里用 `Vec` 而不是 map。
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

/// 已知偏差：CPython 的 `int` 位数不限，这里压到 `i128`（±1.7e38）。
/// 与 `main.rs:1543` 的既有约定一致——超大字面量饱和，真实设置文件不会触达。
fn intsatur(v: i128) -> i128 {
    v
}

impl JVal {
    fn none() -> JVal {
        JVal::None
    }
    fn obj() -> JVal {
        JVal::Obj(Vec::new())
    }
    fn list() -> JVal {
        JVal::List(Vec::new())
    }
    fn s(v: &str) -> JVal {
        JVal::Str(v.to_string())
    }
    fn i(v: i128) -> JVal {
        JVal::Int(intsatur(v))
    }
    fn f(v: f64) -> JVal {
        JVal::Float(v)
    }

    /// `type(x).__name__`，只覆盖 JSON 能表示的类型。
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

    /// `x.get(k[, d])`。对非 mapping 调用会抛 `AttributeError`
    /// （`float_golden.txt:40-43` 实测），因此返回 `R`。
    pub fn get(&self, key: &str) -> R<Option<&JVal>> {
        match self {
            JVal::Obj(map) => Ok(map.iter().find(|(k, _)| k == key).map(|(_, v)| v)),
            other => Err(PyErr::attribute(format!(
                "'{}' object has no attribute 'get'",
                other.type_name()
            ))),
        }
    }

    /// `x.get(k, default)`——注意 JSON 的 `null` 是"键存在"，会返回 `None` 值本身。
    pub fn get_or<'a>(&'a self, key: &str, default: &'a JVal) -> R<&'a JVal> {
        Ok(self.get(key)?.unwrap_or(default))
    }

    /// `d[k] = v`：新键追加，旧键原地替换。非 mapping 抛 `TypeError`。
    pub fn set_item(&mut self, key: &str, value: JVal) -> R<()> {
        match self {
            JVal::Obj(map) => {
                match map.iter_mut().find(|(k, _)| k == key) {
                    Some(slot) => slot.1 = value,
                    None => map.push((key.to_string(), value)),
                }
                Ok(())
            }
            JVal::List(_) => Err(PyErr::type_error(
                "list indices must be integers or slices, not str".to_string(),
            )),
            other => Err(PyErr::type_error(format!(
                "'{}' object does not support item assignment",
                other.type_name()
            ))),
        }
    }

    /// `bool(x)` 真值判断（`ws_golden.txt:95-108`）：`'0'` 与 `'false'` 都是 True。
    pub fn truthy(&self) -> bool {
        match self {
            JVal::None => false,
            JVal::Bool(b) => *b,
            JVal::Int(i) => *i != 0,
            JVal::Float(x) => *x != 0.0, // NaN != 0.0 → True，与 CPython 一致
            JVal::Str(s) => !s.is_empty(),
            JVal::List(v) => !v.is_empty(),
            JVal::Obj(m) => !m.is_empty(),
        }
    }

    /// 供调用方取用的字符串视图（`isinstance(p, str)` 的快捷写法）。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JVal::Str(s) => Some(s),
            _ => None,
        }
    }

    /// 递归转成 `serde_json` 兼容值留给接线点使用；此处只暴露文本序列化。
    pub fn dumps(&self) -> String {
        json_dumps(self)
    }
}

// ============================================================ int() 强制转换

// CPython `int(str)` 允许的空白字符分两条路径：
//   * 纯 ASCII 串走 C locale：\t \n \v \f \r 空格（0x09-0x0D、0x20）
//   * 含非 ASCII 时走 Py_UNICODE_ISSPACE（下表）
// 实测见 float_golden.txt:46-61——'\x1c12\x1c' 与 '\x0012\x00' 会抛 ValueError，
// 而 '\x85' '\xa0' U+2002 U+3000 能被剥掉，U+200B 仍然报错。
pub static DECIMAL_RUNS: [(u32, u32); 66] = [
    (0x0030, 10),
    (0x0660, 10),
    (0x06F0, 10),
    (0x07C0, 10),
    (0x0966, 10),
    (0x09E6, 10),
    (0x0A66, 10),
    (0x0AE6, 10),
    (0x0B66, 10),
    (0x0BE6, 10),
    (0x0C66, 10),
    (0x0CE6, 10),
    (0x0D66, 10),
    (0x0DE6, 10),
    (0x0E50, 10),
    (0x0ED0, 10),
    (0x0F20, 10),
    (0x1040, 10),
    (0x1090, 10),
    (0x17E0, 10),
    (0x1810, 10),
    (0x1946, 10),
    (0x19D0, 10),
    (0x1A80, 10),
    (0x1A90, 10),
    (0x1B50, 10),
    (0x1BB0, 10),
    (0x1C40, 10),
    (0x1C50, 10),
    (0xA620, 10),
    (0xA8D0, 10),
    (0xA900, 10),
    (0xA9D0, 10),
    (0xA9F0, 10),
    (0xAA50, 10),
    (0xABF0, 10),
    (0xFF10, 10),
    (0x104A0, 10),
    (0x10D30, 10),
    (0x11066, 10),
    (0x110F0, 10),
    (0x11136, 10),
    (0x111D0, 10),
    (0x112F0, 10),
    (0x11450, 10),
    (0x114D0, 10),
    (0x11650, 10),
    (0x116C0, 10),
    (0x11730, 10),
    (0x118E0, 10),
    (0x11950, 10),
    (0x11C50, 10),
    (0x11D50, 10),
    (0x11DA0, 10),
    (0x16A60, 10),
    (0x16AC0, 10),
    (0x16B50, 10),
    (0x1D7CE, 10),
    (0x1D7D8, 10),
    (0x1D7E2, 10),
    (0x1D7EC, 10),
    (0x1D7F6, 10),
    (0x1E140, 10),
    (0x1E2F0, 10),
    (0x1E950, 10),
    (0x1FBF0, 10),
];
pub static SPACE_RANGES: [(u32, u32); 10] = [
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

fn is_c_locale_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r' | ' ')
}

fn is_unicode_space(c: char) -> bool {
    let cp = c as u32;
    SPACE_RANGES.iter().any(|(a, b)| cp >= *a && cp <= *b)
}

fn py_decimal_digit(c: char) -> Option<u8> {
    let cp = c as u32;
    for (start, len) in DECIMAL_RUNS.iter() {
        if cp >= *start && cp < start + *len {
            return Some((cp - start) as u8);
        }
    }
    None
}

/// Python 的 `repr(str)`。错误消息专用，因此打印性判定按 CPython 的
/// `Py_UNICODE_ISPRINTABLE` 近似（Cc/Cf/Zl/Zp/Zs/私有区一律转义）。
fn py_repr_str(s: &str) -> String {
    let has_single = s.contains('\'');
    let has_double = s.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() || is_nonprintable(c) => {
                let cp = c as u32;
                if cp <= 0xFF {
                    out.push_str(&format!("\\x{cp:02x}"));
                } else if cp <= 0xFFFF {
                    out.push_str(&format!("\\u{cp:04x}"));
                } else {
                    out.push_str(&format!("\\U{cp:08x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn is_nonprintable(c: char) -> bool {
    let cp = c as u32;
    cp == 0x00A0
        || (0x2000..=0x200F).contains(&cp)
        || cp == 0x2028
        || cp == 0x2029
        || cp == 0x205F
        || cp == 0x3000
        || (0xD800..=0xF8FF).contains(&cp)
        || (0xFFF9..=0xFFFB).contains(&cp)
}

fn int_strip_ends(s: &str) -> &str {
    // CPython 只在整串都是 ASCII 时用 C locale 集合。
    let space: fn(char) -> bool = if s.is_ascii() {
        is_c_locale_space
    } else {
        is_unicode_space
    };
    let trimmed_start = s.trim_start_matches(space);
    trimmed_start.trim_end_matches(space)
}

/// `int(text)` —— 只支持 base 10，下划线必须夹在数字之间。
fn py_int_str(text: &str) -> R<i128> {
    let invalid = || {
        PyErr::value_error(format!(
            "invalid literal for int() with base 10: {}",
            py_repr_str(text)
        ))
    };
    let body = int_strip_ends(text);
    let chars: Vec<char> = body.chars().collect();
    if chars.is_empty() {
        return Err(invalid());
    }
    let mut idx = 0usize;
    let mut negative = false;
    match chars[0] {
        '+' => idx = 1,
        '-' => {
            negative = true;
            idx = 1;
        }
        _ => {}
    }
    let mut digits: Vec<u8> = Vec::new();
    let mut prev_was_digit = false;
    while idx < chars.len() {
        let c = chars[idx];
        if c == '_' {
            // 下划线只能出现在两个数字之间。
            if !prev_was_digit || idx + 1 >= chars.len() {
                return Err(invalid());
            }
            match py_decimal_digit(chars[idx + 1]) {
                Some(_) => {}
                None => return Err(invalid()),
            }
            prev_was_digit = false;
            idx += 1;
            continue;
        }
        match py_decimal_digit(c) {
            Some(d) => {
                digits.push(d);
                prev_was_digit = true;
            }
            None => return Err(invalid()),
        }
        idx += 1;
    }
    if digits.is_empty() {
        return Err(invalid());
    }
    let mut acc: i128 = 0;
    for d in digits {
        acc = acc.saturating_mul(10).saturating_add(i128::from(d));
    }
    Ok(if negative {
        acc.saturating_neg()
    } else {
        acc
    })
}

/// `int(obj)` 的完整分派（`ws_golden.txt:83-94`、`float_golden.txt:32-39`）。
pub fn py_int(value: &JVal) -> R<i128> {
    match value {
        JVal::Bool(b) => Ok(if *b { 1 } else { 0 }),
        JVal::Int(i) => Ok(*i),
        JVal::Float(x) => {
            if x.is_nan() {
                return Err(PyErr::value_error(
                    "cannot convert float NaN to integer".to_string(),
                ));
            }
            if x.is_infinite() {
                return Err(PyErr::overflow(
                    "cannot convert float infinity to integer".to_string(),
                ));
            }
            // 向零截断；1e21 这类大值在 i128 内保持精确。
            let t = x.trunc();
            if t >= i128::MIN as f64 && t <= i128::MAX as f64 {
                Ok(t as i128)
            } else if t > 0.0 {
                Ok(i128::MAX)
            } else {
                Ok(i128::MIN)
            }
        }
        JVal::Str(s) => py_int_str(s),
        other => Err(PyErr::type_error(format!(
            "int() argument must be a string, a bytes-like object or a real number, not '{}'",
            other.type_name()
        ))),
    }
}

/// `max(a, b)`。
fn py_max(a: i128, b: i128) -> i128 {
    if a > b {
        a
    } else {
        b
    }
}

// ============================================================ 浮点文本

/// `repr(float)` —— 最短往返数字，指数区间 (-4, 16] 之外才用科学计数法。
/// 实测表：float_golden.txt:1-30。
pub fn py_repr_f64(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    let plain = format!("{:?}", v); // Rust 与 CPython 都取最短往返
    let (sign, digits_all, exp10) = scan_f64_digits(&plain);
    let mantissa = {
        let mut s = String::new();
        s.push_str(&digits_all[0..1]);
        if digits_all.len() > 1 {
            s.push('.');
            s.push_str(&digits_all[1..]);
        }
        s
    };
    let decpt = exp10 + 1; // digits_all[0] 位于 10^(decpt-1)
    let mut out = String::new();
    out.push_str(sign);
    if decpt <= -4 || decpt > 16 {
        out.push_str(&mantissa);
        out.push('e');
        let e = decpt - 1;
        if e < 0 {
            out.push('-');
            out.push_str(&format!("{:02}", -e));
        } else {
            out.push('+');
            out.push_str(&format!("{:02}", e));
        }
    } else if decpt <= 0 {
        out.push('0');
        out.push('.');
        for _ in 0..-decpt {
            out.push('0');
        }
        out.push_str(&digits_all);
    } else if decpt >= digits_all.len() as i32 {
        out.push_str(&digits_all);
        for _ in 0..(decpt - digits_all.len() as i32) {
            out.push('0');
        }
        out.push_str(".0");
    } else {
        let d = decpt as usize;
        out.push_str(&digits_all[..d]);
        out.push('.');
        out.push_str(&digits_all[d..]);
    }
    out
}

/// 从 Rust 的最短十进制表示里取出 `(符号, 数字串, 10 的指数)`。
fn scan_f64_digits(s: &str) -> (&'static str, String, i32) {
    let mut body = s;
    let mut sign = "";
    if let Some(rest) = body.strip_prefix('-') {
        sign = "-";
        body = rest;
    }
    let (mant, exp) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], body[i + 1..].parse::<i32>().unwrap_or(0)),
        None => (body, 0),
    };
    let mut digits = String::new();
    let mut point: Option<i32> = None;
    let mut frac_len: i32 = 0;
    for c in mant.chars() {
        if c == '.' {
            point = Some(digits.len() as i32);
            continue;
        }
        digits.push(c);
        if point.is_some() {
            frac_len += 1;
        }
    }
    while digits.len() > 1 && digits.starts_with('0') {
        digits.remove(0);
        if let Some(p) = point.as_mut() {
            *p -= 1;
        }
    }
    let int_digits = point.unwrap_or(digits.len() as i32);
    let exp10 = int_digits - 1 + exp;
    let _ = frac_len;
    // 去掉尾随的无效零不影响最短性，但 Rust 已经给出最短形式。
    (sign, digits, exp10)
}

// ============================================================ json.dumps

/// `json.dumps(x, ensure_ascii=False, indent=2)`（`utils.py:67`）。
pub fn json_dumps(value: &JVal) -> String {
    let mut out = String::new();
    write_json(&mut out, value, 0);
    out
}

fn json_escape_nonascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_float(v: f64) -> String {
    // json.dumps 的 allow_nan 分支（ws_golden.txt:28-30）。
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    py_repr_f64(v)
}

fn write_json(out: &mut String, value: &JVal, level: usize) {
    match value {
        JVal::None => out.push_str("null"),
        JVal::Bool(true) => out.push_str("true"),
        JVal::Bool(false) => out.push_str("false"),
        JVal::Int(i) => out.push_str(&i.to_string()),
        JVal::Float(f) => out.push_str(&json_float(*f)),
        JVal::Str(s) => out.push_str(&json_escape_nonascii(s)),
        JVal::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(out, level + 1);
                write_json(out, item, level + 1);
            }
            out.push('\n');
            push_indent(out, level);
            out.push(']');
        }
        JVal::Obj(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (n, (k, v)) in map.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(out, level + 1);
                out.push_str(&json_escape_nonascii(k));
                out.push_str(": ");
                write_json(out, v, level + 1);
            }
            out.push('\n');
            push_indent(out, level);
            out.push('}');
        }
    }
}

fn push_indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

// ============================================================ json.loads

/// `json.loads(text)`（`utils.py:49`）。Python 默认允许 `Infinity` / `-Infinity` /
/// `NaN`，拒绝注释、尾逗号与前导零（`ws_golden.txt:124-135`）。
pub fn json_loads(text: &str) -> R<JVal> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = JsonParser { c: chars, i: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i != p.c.len() {
        return Err(PyErr::value_error("Extra data".to_string()));
    }
    Ok(v)
}

struct JsonParser {
    c: Vec<char>,
    i: usize,
}

impl JsonParser {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        // Python 的 json 只剥 ' \t\n\r'。
        while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n') | Some('\r')) {
            self.i += 1;
        }
    }
    fn fail(&self, msg: &str) -> PyErr {
        PyErr::value_error(format!("Expecting {msg}") )
    }
    fn expect(&mut self, ch: char, msg: &str) -> R<()> {
        if self.peek() == Some(ch) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.fail(msg))
        }
    }
    fn literal(&mut self, word: &str) -> bool {
        let want = word.chars().collect::<Vec<char>>();
        if self.c.len() >= self.i + want.len()
            && self.c[self.i..self.i + want.len()] == want[..]
        {
            // `self.c` 是 char 数组，前进量必须是码点数；用 word.len()（UTF-8 字节数）
            // 只在对 ASCII 关键字时恰好相等，非 ASCII 字面量会把游标推到错误位置。
            self.i += want.len();
            return true;
        }
        false
    }
    fn value(&mut self) -> R<JVal> {
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(JVal::Str(self.string()?)),
            Some('t') => {
                if self.literal("true") {
                    Ok(JVal::Bool(true))
                } else {
                    Err(self.fail("value"))
                }
            }
            Some('f') => {
                if self.literal("false") {
                    Ok(JVal::Bool(false))
                } else {
                    Err(self.fail("value"))
                }
            }
            Some('n') => {
                if self.literal("null") {
                    Ok(JVal::None)
                } else {
                    Err(self.fail("value"))
                }
            }
            Some('I') => {
                if self.literal("Infinity") {
                    Ok(JVal::Float(f64::INFINITY))
                } else {
                    Err(self.fail("value"))
                }
            }
            Some('N') => {
                if self.literal("NaN") {
                    Ok(JVal::Float(f64::NAN))
                } else {
                    Err(self.fail("value"))
                }
            }
            Some('-') => {
                if self.c.get(self.i + 1) == Some(&'I') {
                    self.i += 1;
                    if self.literal("Infinity") {
                        return Ok(JVal::Float(f64::NEG_INFINITY));
                    }
                    return Err(self.fail("value"));
                }
                self.number()
            }
            Some(c) if c.is_ascii_digit() => self.number(),
            _ => Err(self.fail("value")),
        }
    }
    fn string(&mut self) -> R<String> {
        self.expect('"', "string")?;
        let mut out = String::new();
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err(PyErr::value_error(
                    "Unterminated string starting at".to_string(),
                )),
            };
            self.i += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.peek().ok_or_else(|| {
                        PyErr::value_error("Invalid \\escape".to_string())
                    })?;
                    self.i += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{08}'),
                        'f' => out.push('\u{0c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let cp = self.hex4()?;
                            // 代理对：与 CPython y_scanner 一致，只有后随合法低代理时才消费
                            // 第二个 \uXXXX；否则回退指针，让孤立代理各自独立解码。
                            if (0xD800..0xDC00).contains(&cp) {
                                let saved = self.i;
                                if self.peek() == Some('\\')
                                    && self.c.get(self.i + 1) == Some(&'u')
                                {
                                    self.i += 2;
                                    match self.hex4() {
                                        Ok(lo) if (0xDC00..0xE000).contains(&lo) => {
                                            let combined =
                                                0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                            out.push(
                                                char::from_u32(combined).unwrap_or('\u{fffd}'),
                                            );
                                            continue;
                                        }
                                        Ok(_) | Err(_) => self.i = saved,
                                    }
                                }
                                out.push('\u{fffd}');
                            } else {
                                out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                            }
                        }
                        _ => {
                            return Err(PyErr::value_error(format!(
                                "Invalid \\escape: '\\{e}'"
                            )))
                        }
                    }
                }
                c if (c as u32) < 0x20 => {
                    return Err(PyErr::value_error(
                        "Invalid control character".to_string(),
                    ))
                }
                c => out.push(c),
            }
        }
    }
    fn hex4(&mut self) -> R<u32> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let c = self.peek().ok_or_else(|| {
                PyErr::value_error("Invalid \\uXXXX escape".to_string())
            })?;
            self.i += 1;
            let d = c
                .to_digit(16)
                .ok_or_else(|| PyErr::value_error("Invalid \\uXXXX escape".to_string()))?;
            v = v * 16 + d;
        }
        Ok(v)
    }
    fn object(&mut self) -> R<JVal> {
        self.expect('{', "property name enclosed in double quotes")?;
        let mut map: Vec<(String, JVal)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(JVal::Obj(map));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err(self.fail("property name enclosed in double quotes"));
            }
            let key = self.string()?;
            self.skip_ws();
            self.expect(':', "':'")?;
            self.skip_ws();
            let val = self.value()?;
            // 重复键：后值覆盖前值，位置保持第一次出现（ws_golden.txt:135）。
            match map.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = val,
                None => map.push((key, val)),
            }
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                    self.skip_ws();
                    if self.peek() == Some('}') {
                        return Err(self.fail("property name enclosed in double quotes"));
                    }
                }
                Some('}') => {
                    self.i += 1;
                    return Ok(JVal::Obj(map));
                }
                _ => return Err(self.fail("',' delimiter or '}'")),
            }
        }
    }
    fn array(&mut self) -> R<JVal> {
        self.expect('[', "']'")?;
        let mut items: Vec<JVal> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.i += 1;
            return Ok(JVal::List(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                    self.skip_ws();
                    if self.peek() == Some(']') {
                        return Err(self.fail("a value"));
                    }
                }
                Some(']') => {
                    self.i += 1;
                    return Ok(JVal::List(items));
                }
                _ => return Err(self.fail("',' delimiter or ']'")),
            }
        }
    }
    fn number(&mut self) -> R<JVal> {
        let start = self.i;
        let mut is_float = false;
        if self.peek() == Some('-') {
            self.i += 1;
        }
        // 整数部分：0 或 [1-9]\d*
        match self.peek() {
            Some('0') => {
                self.i += 1;
                if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    return Err(PyErr::value_error(
                        "Expecting ',' delimiter".to_string(),
                    ));
                }
            }
            Some(c) if c.is_ascii_digit() => {
                while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            _ => return Err(self.fail("value")),
        }
        if self.peek() == Some('.') {
            is_float = true;
            self.i += 1;
            if !matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                return Err(self.fail("value"));
            }
            while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            self.i += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                return Err(self.fail("value"));
            }
            while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text: String = self.c[start..self.i].iter().collect();
        if is_float {
            Ok(JVal::Float(text.parse::<f64>().unwrap_or(0.0)))
        } else {
            Ok(JVal::Int(parse_big_int(&text)))
        }
    }
}

/// 十进制整数字面量 → `i128`（越界饱和，见 `intsatur` 的说明）。
fn parse_big_int(text: &str) -> i128 {
    let (neg, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let mut acc: i128 = 0;
    for d in digits.bytes() {
        acc = acc.saturating_mul(10).saturating_add(i128::from(d - b'0'));
    }
    if neg {
        acc.saturating_neg()
    } else {
        acc
    }
}

// ============================================================ os.path

/// `os.path.normpath`（当前平台）。
pub fn py_normpath(p: &str) -> String {
    if cfg!(windows) {
        nt_normpath(p)
    } else {
        posix_normpath(p)
    }
}

/// `os.path.abspath`（当前平台）。
pub fn py_abspath(p: &str) -> String {
    if cfg!(windows) {
        nt_abspath(p)
    } else {
        posix_abspath(p)
    }
}

/// `os.path.dirname`。
pub fn py_dirname(p: &str) -> String {
    if cfg!(windows) {
        nt_dirname(p)
    } else {
        posix_dirname(p)
    }
}

/// `os.path.basename`。
pub fn py_basename(p: &str) -> String {
    if cfg!(windows) {
        nt_basename(p)
    } else {
        posix_basename(p)
    }
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// `ntpath.normpath`：分隔符统一、去掉 `.` 与重复分隔符、折叠 `..`。
///
/// 本机 CPython 3.11.15 的 `ntpath.normpath` 是 C 实现（`os.path_normpath`），
/// 没有可读的 Python 源码，因此保真度不靠回忆，而靠 327 行逐条实测的 golden 表
/// （见 `test_normpath_windows_table`；生成器 `ws_table_gen.py`，
/// 复核器 `ws_probe8.py`，327 行对 live CPython 0 mismatch）。
pub fn nt_normpath(p: &str) -> String {
    let s = p.replace('/', "\\");
    let (prefix, body) = nt_split_prefix(&s);
    let anchored = prefix.ends_with('\\');
    if !prefix.is_empty() && !anchored {
        // 驱动器相对前缀（"C:"）或以整体形式充当前缀的 UNC / 设备命名空间。
        if body.is_empty() {
            return prefix;
        }
        let joined = nt_fold(body.split('\\'), true).join("\\");
        return if joined.is_empty() { prefix } else { format!("{prefix}{joined}") };
    }
    let mut parts = nt_fold(body.split('\\'), false);
    if anchored {
        // 根之上不允许再向上：前导 ".." 全部丢弃。
        while parts.first().map(|x| *x == "..").unwrap_or(false) {
            parts.remove(0);
        }
    }
    let joined = parts.join("\\");
    if prefix.is_empty() {
        return if joined.is_empty() { ".".to_string() } else { joined };
    }
    format!("{prefix}{joined}")
}

/// `..` 折叠。`keep_leading_dot` 复现驱动器相对前缀下 `.` 被当普通组件保留
/// （`C:.\a` → `C:.\a`），而无前缀时 `.` 会被丢掉（`.\a` → `a`）。
fn nt_fold<'a>(comps: impl Iterator<Item = &'a str>, keep_leading_dot: bool) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = Vec::new();
    let mut first = true;
    for c in comps {
        if c.is_empty() {
            continue;
        }
        if c == "." {
            if keep_leading_dot && first {
                out.push(c);
            }
            first = false;
            continue;
        }
        if c == ".." {
            if out.last().map(|x| *x != "..").unwrap_or(false) {
                out.pop();
            } else {
                out.push(c);
            }
            first = false;
            continue;
        }
        out.push(c);
        first = false;
    }
    out
}

fn nt_split_prefix(s: &str) -> (String, &str) {
    if s.starts_with("\\\\") {
        // UNC 或 \\?\ / \\.\ 命名空间：整体保留前两段（或设备前缀）。
        let after_server = &s[2..];
        if let Some(i) = after_server.find('\\') {
            let server = &after_server[..i];
            let after_share = &after_server[i + 1..];
            if server == "?" || server == "." {
                // \\?\C:\... 与 \\.\COM1：\\?\ 之后再接一段作为前缀。
                if let Some(j) = after_share.find('\\') {
                    let cut = 2 + i + 1 + j + 1;
                    return (s[..cut].to_string(), &s[cut..]);
                }
                return (s.to_string(), "");
            }
            if after_share.is_empty() {
                // \\server\share —— 保留单个尾随反斜杠的形态由 normpath 决定
                return (format!("\\\\{server}\\{after_share}"), "");
            }
            if let Some(j) = after_share.find('\\') {
                let cut = 2 + i + 1 + j + 1;
                return (s[..cut].to_string(), &s[cut..]);
            }
            return (s.to_string(), "");
        }
        return (s.to_string(), "");
    }
    if s.starts_with('\\') {
        return ("\\".to_string(), &s[1..]);
    }
    let b = s.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        if b.len() >= 3 && b[2] == b'\\' {
            return ((&s[..3]).to_string(), &s[3..]);
        }
        return ((&s[..2]).to_string(), &s[2..]);
    }
    (String::new(), s)
}

/// `ntpath.abspath` ≈ Win32 `GetFullPathName`。
/// 关键点：不带驱动器号但带根的串挂到当前盘的 cwd 上（`/tmp` → `T:\tmp`），
/// 驱动器相关的串相对该盘的 cwd 解析（`C:..\a` → `C:\a`）。
pub fn nt_abspath(p: &str) -> String {
    let cwd_str = cwd().to_string_lossy().into_owned();
    nt_abspath_in(p, &cwd_str)
}

fn nt_abspath_in(p: &str, cwd_str: &str) -> String {
    let s = p.replace('/', "\\");
    let cwd_prefix_drive = drive_of(cwd_str);
    if s.starts_with("\\\\") {
        // UNC / 命名空间：只规范化，不挂 cwd。
        return nt_normpath(&s);
    }
    let (drive, rest) = match s.get(0..2) {
        Some(d) if d.ends_with(':') && d.as_bytes()[0].is_ascii_alphabetic() => {
            (d.to_string(), &s[2..])
        }
        _ => (String::new(), s.as_str()),
    };
    if drive.is_empty() {
        if s.starts_with('\\') {
            return nt_normpath(&format!("{cwd_prefix_drive}{s}"));
        }
        return nt_normpath(&join_nt(cwd_str, &s));
    }
    if rest.starts_with('\\') {
        return nt_normpath(&s);
    }
    // 驱动器相关：该盘的 cwd（本机只能可靠知道当前盘，其他盘退化为盘根）。
    let base = if drive.eq_ignore_ascii_case(&cwd_prefix_drive) {
        cwd_str.to_string()
    } else {
        format!("{drive}\\")
    };
    nt_normpath(&join_nt(&base, rest))
}

fn drive_of(path: &str) -> String {
    let b = path.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        path[..2].to_ascii_uppercase()
    } else {
        String::new()
    }
}

fn join_nt(base: &str, rest: &str) -> String {
    if rest.is_empty() {
        return base.to_string();
    }
    if base.ends_with('\\') {
        format!("{base}{rest}")
    } else {
        format!("{base}\\{rest}")
    }
}

fn nt_dirname(p: &str) -> String {
    let (prefix, body) = nt_split_prefix(p);
    if prefix.is_empty() {
        let cut = match p.rfind('\\') {
            Some(i) => i,
            None => return String::new(),
        };
        // 只有分隔符时结果是该分隔符本身
        if cut == 0 {
            return "\\".to_string();
        }
        return trim_trailing_seps(&p[..cut]);
    }
    let tail = body.rfind('\\').map(|i| &body[..i]).unwrap_or("");
    let head = &p[..p.len() - body.len()];
    trim_trailing_seps(&format!("{head}{tail}"))
}

fn nt_basename(p: &str) -> String {
    match p.rfind(['\\', '/']) {
        Some(i) => p[i + 1..].to_string(),
        None => p.to_string(),
    }
}

fn trim_trailing_seps(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut end = bytes.len();
    // 保留根形态（"\"、"C:\"、"\\srv\share\"）
    while end > 0 && bytes[end - 1] == b'\\' && !is_nt_root(&s[..end]) {
        end -= 1;
    }
    s[..end].to_string()
}

fn is_nt_root(s: &str) -> bool {
    s == "\\" || (s.len() == 3 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'\\') || {
        // \\server\share\
        let b = s.as_bytes();
        b.len() >= 8
            && b[0] == b'\\'
            && b[1] == b'\\'
            && b[b.len() - 1] == b'\\'
            && s[2..b.len() - 1].matches('\\').count() == 1
    }
}

/// `posixpath.normpath`（`ws_golden.txt:34-45`）：正好两个前导斜杠会被保留。
pub fn posix_normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".to_string();
    }
    let absolute = p.starts_with('/');
    let slash = if p.starts_with("//") && !p.starts_with("///") {
        "//"
    } else if absolute {
        "/"
    } else {
        ""
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            // 前导/连续的 ".." 保留为组件，其余折叠掉一个普通组件。
            let append_it = (!absolute && parts.is_empty())
                || parts.last().map(|x| *x == "..").unwrap_or(false);
            if append_it {
                parts.push("..");
            } else if !parts.is_empty() {
                parts.pop();
            }
            continue;
        }
        parts.push(seg);
    }
    let joined = parts.join("/");
    if !absolute {
        return if joined.is_empty() { ".".to_string() } else { joined };
    }
    format!("{slash}{joined}")
}

pub fn posix_abspath(p: &str) -> String {
    let cwd_str = cwd().to_string_lossy().into_owned();
    let joined = if p.is_empty() {
        cwd_str
    } else if p.starts_with('/') {
        p.to_string()
    } else {
        let base = cwd_str.trim_end_matches('/');
        if base.is_empty() {
            format!("/{p}")
        } else {
            format!("{base}/{p}")
        }
    };
    posix_normpath(&joined)
}

fn posix_dirname(p: &str) -> String {
    match p.rfind('/') {
        Some(i) if i == 0 => "/".to_string(),
        Some(i) => p[..i].to_string(),
        None => String::new(),
    }
}

fn posix_basename(p: &str) -> String {
    match p.rfind('/') {
        Some(i) => p[i + 1..].to_string(),
        None => p.to_string(),
    }
}

// ============================================================ utils.load/save_json

/// `utils.load_json(path, default)`：`os.path.isfile` 在 `try` 之前，所以**缺失文件
/// （以及目录路径）静默返回 default，一行日志都不打**；只有真的读到/解析失败才 warn。
pub fn load_json(path: &str, default: JVal) -> JVal {
    load_json_logged(path, default, &|line| log_warning(line))
}

fn load_json_logged(path: &str, default: JVal, warn: &dyn Fn(&str)) -> JVal {
    // utils.py:45-46 `if not os.path.isfile(path): return default` —— 在 try 之外，
    // 所以文件缺失 / 路径是目录时 Python 一行日志都不打。
    if !py_isfile(path) {
        return default;
    }
    match load_json_checked(path) {
        Ok(v) => v,
        Err(exc) => {
            // utils.py:51 logging.warning("读取 JSON 失败 %s: %s", path, e)
            warn(&format!("读取 JSON 失败 {path}: {}", exc.raised()));
            default
        }
    }
}

/// `os.path.isfile`：跟随符号链接（用 `fs::metadata` 而非 `symlink_metadata`），
/// 任何 stat 失败都算“不是文件”。
fn py_isfile(path: &str) -> bool {
    matches!(fs::metadata(path), Ok(m) if m.is_file())
}

fn load_json_checked(path: &str) -> R<JVal> {
    // 只有在 `isfile` 与读之间存在竞态时才会走到这两个分支；那时 CPython 的
    // `open()` 同样抛异常并被 `except Exception` 捕获 + warn，所以 warn 是对的。
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Err(PyErr::value_error("file not found".to_string())),
    };
    if !meta.is_file() {
        return Err(PyErr::value_error("not a file".to_string()));
    }
    let bytes = fs::read(path).map_err(|e| io_err(&e))?;
    let text = String::from_utf8(bytes).map_err(|_| {
        PyErr::value_error("'utf-8' codec can't decode byte".to_string())
    })?;
    json_loads(&text)
}

fn io_err(e: &std::io::Error) -> PyErr {
    PyErr::new("OSError", format!("{e}"))
}

// WIRING: 接到 crate 的统一日志实现后替换；消息文本保持与 Python 一致。
fn log_warning(line: &str) {
    eprintln!("WARNING:root:{line}");
}

// WIRING: 同上（`logging.error`）。
fn log_error(line: &str) {
    eprintln!("ERROR:root:{line}");
}

/// `utils.save_json(path, data)`：唯一临时名 + 文本模式写入 + 有限次重试替换。
/// 返回 `False` 表示 Python 侧走进了 `except Exception` 分支。
pub fn save_json(path: &str, data: &JVal) -> bool {
    match save_json_inner(path, data) {
        Ok(()) => true,
        Err(exc) => {
            // utils.py:85 logging.error("保存 JSON 失败 %s: %s", path, e)
            log_error(&format!("保存 JSON 失败 {path}: {}", exc.raised()));
            false
        }
    }
}

/// `utils.py:77 except PermissionError` 能重试的 Win32 码集合。
/// 实测（本机 CPython）：`CreateFileW(share=0)` / `msvcrt.locking` / `LockFileEx`
/// 三种独占锁下 `os.replace` 全部返回 `PermissionError winerror=5 errno=13`；
/// 32 = ERROR_SHARING_VIOLATION 即 Python 文档化的 `PermissionError [WinError 32]`。
/// 33 = ERROR_LOCK_VIOLATION 经 CPython `PC/errmap.c` 映射到 EDEADLOCK（实测
/// `errno.EDEADLOCK == 36`），PermissionError 只在 errno ∈ {EPERM=1, EACCES=13} 时产生，
/// 所以 33 冒出来的是普通 OSError —— Python 不重试。
fn replace_retryable(raw: Option<i32>) -> bool {
    matches!(raw, Some(5) | Some(32))
}

fn save_json_inner(path: &str, data: &JVal) -> R<()> {
    let target = py_abspath(path);
    let parent = py_dirname(&target);
    if !parent.is_empty() {
        fs::create_dir_all(&parent).map_err(|e| io_err(&e))?;
    }
    let base = py_basename(&target);
    let tmp_path = mkstemp(&parent, &format!("{base}."), ".tmp")?;
    let result = (|| -> R<()> {
        let text = json_dumps(data);
        write_text_file(&tmp_path, &text)?;
        let mut last_error: Option<PyErr> = None;
        for attempt in 0..6 {
            match fs::rename(&tmp_path, &target) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if !replace_retryable(e.raw_os_error()) {
                        return Err(io_err(&e));
                    }
                    last_error = Some(io_err(&e));
                    if attempt == 5 {
                        return Err(io_err(&e));
                    }
                    std::thread::sleep(Duration::from_millis(30 * (attempt as u64 + 1)));
                }
            }
        }
        Err(last_error.unwrap_or_else(|| PyErr::new("OSError", "replace failed".to_string())))
    })();
    if result.is_err() {
        // utils.py:88 finally: os.unlink(tmp_path) 忽略 OSError
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

/// `tempfile.mkstemp(prefix, suffix, dir)` —— 8 位随机名，独占创建。
fn mkstemp(dir: &str, prefix: &str, suffix: &str) -> R<String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    for _ in 0..64 {
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            ^ (std::process::id() as u64) << 32
            ^ seq;
        let name = format!("{prefix}{:08x}{suffix}", nanos & 0xFFFF_FFFF);
        let candidate = if dir.is_empty() {
            name
        } else {
            join_platform(dir, &name)
        };
        let mut opts = fs::OpenOptions::new();
        match opts.write(true).create_new(true).open(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(_) => continue,
        }
    }
    Err(PyErr::new("OSError", "mkstemp failed".to_string()))
}

fn join_platform(dir: &str, name: &str) -> String {
    if cfg!(windows) {
        if dir.ends_with('\\') {
            format!("{dir}{name}")
        } else {
            format!("{dir}\\{name}")
        }
    } else if dir.ends_with('/') || dir == "/" {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// 以文本模式（`os.fdopen(fd, 'w', encoding='utf-8')`）写入：
/// Windows 上 `'\n'` 会翻译成 `'\r\n'`，与 `store.rs::save_json` 的裸 LF 不同。
fn write_text_file(path: &str, text: &str) -> R<()> {
    let mut bytes = String::with_capacity(text.len());
    if cfg!(windows) {
        for c in text.chars() {
            if c == '\n' {
                bytes.push('\r');
            }
            bytes.push(c);
        }
    } else {
        bytes.push_str(text);
    }
    let mut f = fs::File::create(path).map_err(|e| io_err(&e))?;
    f.write_all(bytes.as_bytes()).map_err(|e| io_err(&e))?;
    f.flush().map_err(|e| io_err(&e))?;
    f.sync_all().map_err(|e| io_err(&e))?;
    Ok(())
}

// ============================================================ WindowStateManager

/// `window_state.py:16-19` —— Python 侧为无限精度 int，此处按实测取值域收窄为 i128。
pub const DEFAULT_WIDTH: i128 = 1080;
pub const DEFAULT_HEIGHT: i128 = 760;
pub const MIN_WIDTH: i128 = 640;
pub const MIN_HEIGHT: i128 = 480;


/// `window_state.py:22` 的 `WindowStateManager`。
#[derive(Clone, Debug)]
pub struct WindowStateManager {
    pub settings_file: String,
    pub recent_file: String,
}

/// `load_geometry()` 返回的那个 dict。
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub width: i128,
    pub height: i128,
    pub x: Option<i128>,
    pub y: Option<i128>,
    pub maximized: bool,
}

impl Geometry {
    /// 与 Python dict 字面量同序：width, height, x, y, maximized。
    pub fn to_json(&self) -> JVal {
        let mut map = Vec::new();
        map.push(("width".to_string(), JVal::Int(self.width)));
        map.push(("height".to_string(), JVal::Int(self.height)));
        map.push((
            "x".to_string(),
            match self.x {
                Some(v) => JVal::Int(v),
                None => JVal::None,
            },
        ));
        map.push((
            "y".to_string(),
            match self.y {
                Some(v) => JVal::Int(v),
                None => JVal::None,
            },
        ));
        map.push(("maximized".to_string(), JVal::Bool(self.maximized)));
        JVal::Obj(map)
    }
}

impl WindowStateManager {
    /// 生产默认走 `config.SETTINGS_FILE` / `config.RECENT_FILE`。
    pub fn new(settings_file: &str, recent_file: &str) -> Self {
        WindowStateManager {
            settings_file: settings_file.to_string(),
            recent_file: recent_file.to_string(),
        }
    }

    /// `window_state.py:29 load_geometry`。只夹紧下界，无上界，也不做越界回居中
    /// （文档串里的"自适应居中"在权威实现中并不存在）。
    pub fn load_geometry(&self) -> R<Geometry> {
        let data = load_json(&self.settings_file, JVal::obj());
        let geo_default = JVal::obj();
        let geo = data.get_or("geometry", &geo_default)?;
        let width_default = JVal::Int(DEFAULT_WIDTH);
        let height_default = JVal::Int(DEFAULT_HEIGHT);
        let w = py_max(
            MIN_WIDTH,
            py_int(geo.get_or("width", &width_default)?)?,
        );
        let h = py_max(
            MIN_HEIGHT,
            py_int(geo.get_or("height", &height_default)?)?,
        );
        let x = geo.get("x")?;
        let y = geo.get("y")?;
        let maximized_val = geo.get_or("maximized", &JVal::Bool(false))?;
        Ok(Geometry {
            width: w,
            height: h,
            x: match x {
                Some(v) if !matches!(v, JVal::None) => Some(py_int(v)?),
                _ => None,
            },
            y: match y {
                Some(v) if !matches!(v, JVal::None) => Some(py_int(v)?),
                _ => None,
            },
            maximized: maximized_val.truthy(),
        })
    }

    /// `window_state.py:47 save_geometry` —— 桥层直接给 JSON 值的版本。
    /// `int()` / `bool()` 的失败会照 Python 原样抛出。
    pub fn save_geometry_values(
        &self,
        width: &JVal,
        height: &JVal,
        x: &JVal,
        y: &JVal,
        maximized: &JVal,
    ) -> R<bool> {
        let mut data = load_json(&self.settings_file, JVal::obj());
        let mut geo = Vec::new();
        geo.push((
            "width".to_string(),
            JVal::Int(py_max(MIN_WIDTH, py_int(width)?)),
        ));
        geo.push((
            "height".to_string(),
            JVal::Int(py_max(MIN_HEIGHT, py_int(height)?)),
        ));
        geo.push((
            "x".to_string(),
            if x == &JVal::None {
                JVal::None
            } else {
                JVal::Int(py_int(x)?)
            },
        ));
        geo.push((
            "y".to_string(),
            if y == &JVal::None {
                JVal::None
            } else {
                JVal::Int(py_int(y)?)
            },
        ));
        geo.push(("maximized".to_string(), JVal::Bool(maximized.truthy())));
        data.set_item("geometry", JVal::Obj(geo))?;
        Ok(save_json(&self.settings_file, &data))
    }

    /// 与 Python 签名一致的便捷版本。
    pub fn save_geometry(
        &self,
        width: i128,
        height: i128,
        x: Option<i128>,
        y: Option<i128>,
        maximized: bool,
    ) -> R<bool> {
        self.save_geometry_values(
            &JVal::Int(width),
            &JVal::Int(height),
            &x.map(JVal::Int).unwrap_or(JVal::None),
            &y.map(JVal::Int).unwrap_or(JVal::None),
            &JVal::Bool(maximized),
        )
    }

    /// `window_state.py:59 load_recent_files`：非 list 一律退回空表，
    /// 只保留 `isinstance(p, str)` 的元素，再 `[:limit]`。
    pub fn load_recent_files(&self, limit: i64) -> Vec<String> {
        let data = load_json(&self.recent_file, JVal::list());
        match data {
            JVal::List(items) => {
                let kept: Vec<String> = items
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                py_slice_head(&kept, limit)
            }
            _ => Vec::new(),
        }
    }

    /// `window_state.py:66 add_recent_file`：存的是 abspath，去重比较用 normpath；
    /// 先按 `limit * 2` 过读再截断；落盘失败也照样返回新列表。
    pub fn add_recent_file(&self, file_path: &str, limit: i64) -> Vec<String> {
        if file_path.is_empty() {
            return self.load_recent_files(limit);
        }
        let norm_path = py_abspath(file_path);
        let mut recents = self.load_recent_files(limit.saturating_mul(2));
        let needle = py_normpath(&norm_path);
        recents.retain(|p| py_normpath(p) != needle);
        recents.insert(0, norm_path);
        let result = py_slice_head(&recents, limit);
        let payload = JVal::List(result.iter().map(|p| JVal::s(p)).collect());
        save_json(&self.recent_file, &payload);
        result
    }

    /// `window_state.py:78 clear_recent_files`。
    pub fn clear_recent_files(&self) -> bool {
        save_json(&self.recent_file, &JVal::list())
    }
}

/// `list[:limit]`，含 Python 的负数 stop 语义（`ws_golden.txt:142-149`）。
fn py_slice_head<T: Clone>(v: &[T], limit: i64) -> Vec<T> {
    if limit >= 0 {
        v.iter().take(limit as usize).cloned().collect()
    } else {
        let n = v.len() as i64;
        let stop = n + limit;
        if stop <= 0 {
            Vec::new()
        } else {
            v[..stop as usize].to_vec()
        }
    }
}

/// 只在接线阶段需要的键序无关比较（保留给调用方做诊断快照）。
pub(crate) fn ordered_keys(v: &JVal) -> Option<Vec<String>> {
    match v {
        JVal::Obj(map) => Some(map.iter().map(|(k, _)| k.clone()).collect()),
        _ => None,
    }
}

/// `BTreeMap` 仅供测试里做"忽略顺序"的对照；不参与运行路径。
fn _unused_btreenumap_marker() -> BTreeMap<String, JVal> {
    BTreeMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let mut p = std::env::temp_dir();
        p.push(format!("readmd-window_state-{tag}-{}-{n}", std::process::id()));
        let _ = fs::create_dir_all(&p);
        p
    }

    fn mgr(tag: &str) -> (PathBuf, WindowStateManager) {
        let dir = tmp_dir(tag);
        let m = WindowStateManager::new(
            dir.join("settings.json").to_str().unwrap(),
            dir.join("recent.json").to_str().unwrap(),
        );
        (dir, m)
    }

    // ------------------------------------------------ tests/test_readmd_core_server.py

    #[test]
    fn test_window_state_manager_geometry() {
        let (_dir, m) = mgr("geo");
        assert_eq!(m.save_geometry(1200, 800, Some(100), Some(50), false), Ok(true));
        let g = m.load_geometry().unwrap();
        assert_eq!(g.width, 1200);
        assert_eq!(g.height, 800);
        assert_eq!(g.x, Some(100));
        assert_eq!(g.y, Some(50));
        assert_eq!(g.maximized, false);
        m.save_geometry(100, 100, None, None, false).unwrap();
        let g = m.load_geometry().unwrap();
        assert_eq!((g.width, g.height), (640, 480)); // clamp_w_small / clamp_h_small
        assert_eq!(g.x, None);
        assert_eq!(g.y, None);
    }

    #[test]
    fn test_window_state_recent_files() {
        let (dir, m) = mgr("recent");
        let f1 = dir.join("a.md");
        let f2 = dir.join("b.md");
        fs::write(&f1, "a").unwrap();
        fs::write(&f2, "b").unwrap();
        let recents = m.add_recent_file(f1.to_str().unwrap(), 20);
        let recents = m.add_recent_file(f2.to_str().unwrap(), 20);
        assert_eq!(recents.len(), 2);
        assert_eq!(recents[0], py_abspath(f2.to_str().unwrap()));
        let recents = m.add_recent_file(f1.to_str().unwrap(), 20);
        assert_eq!(recents.len(), 2);
        assert_eq!(recents[0], py_abspath(f1.to_str().unwrap()));
        assert_eq!(m.clear_recent_files(), true);
        assert_eq!(m.load_recent_files(20), Vec::<String>::new());
    }

    // ------------------------------------------------ 实测表：ntpath.normpath

    #[test]
    fn test_normpath_windows_golden() {
        let rows: &[(&str, &str)] = &[
            ("", "."),
            (".", "."),
            ("..", ".."),
            ("a", "a"),
            ("a/", "a"),
            ("./a", "a"),
            ("a/./b", "a\\b"),
            ("a//b", "a\\b"),
            ("a/../b", "b"),
            ("../a", "..\\a"),
            ("/..", "\\"),
            ("\\", "\\"),
            ("..\\", ".."),
            ("C:", "C:"),
            ("C:\\", "C:\\"),
            ("C:\\..", "C:\\"),
            ("C:\\a\\..\\", "C:\\"),
            ("\\\\server\\share\\a\\..", "\\\\server\\share\\"),
            ("a\\b\\..", "a"),
            ("./", "."),
            ("a/../../b", "..\\b"),
            ("//", "\\\\"),
            ("///", "\\\\\\"),
            ("C:/a/../b", "C:\\b"),
            ("C:a", "C:a"),
            ("\\\\?\\C:\\a\\..\\b", "\\\\?\\C:\\b"),
            ("\\\\.\\COM1", "\\\\.\\COM1"),
            ("C:\\a\\..\\..\\..\\b", "C:\\b"),
            ("\\\\server\\share\\..", "\\\\server\\share\\"),
            ("a\\b\\c\\..\\..", "a"),
            ("\\\\?\\UNC\\server\\share\\a", "\\\\?\\UNC\\server\\share\\a"),
            ("C:..\\a", "C:..\\a"),
            ("/a/b/../../c", "\\c"),
        ];
        if !cfg!(windows) {
            return;
        }
        for (input, want) in rows {
            assert_eq!(&nt_normpath(input), want, "normpath({input:?})");
        }
    }

    #[test]
    fn test_normpath_posix_golden() {
        let rows: &[(&str, &str)] = &[
            ("", "."),
            (".", "."),
            ("..", ".."),
            ("a", "a"),
            ("a/", "a"),
            ("./a", "a"),
            ("a//b", "a/b"),
            ("/..", "/"),
            ("//", "//"),
            ("///", "/"),
            ("/a/b/../../c", "/c"),
            ("a/../../b", "../b"),
        ];
        for (input, want) in rows {
            assert_eq!(&posix_normpath(input), want, "posix normpath({input:?})");
        }
    }

    #[test]
    fn test_abspath_golden() {
        if !cfg!(windows) {
            return;
        }
        let cwd = cwd().to_string_lossy().into_owned();
        let drive = drive_of(&cwd);
        assert_eq!(nt_abspath_in("", &cwd), cwd);
        assert_eq!(nt_abspath_in(".", &cwd), cwd);
        assert_eq!(nt_abspath_in("a", &cwd), format!("{cwd}\\a"));
        assert_eq!(nt_abspath_in("a/", &cwd), format!("{cwd}\\a"));
        assert_eq!(nt_abspath_in("./a", &cwd), format!("{cwd}\\a"));
        assert_eq!(nt_abspath_in("/tmp", &cwd), format!("{drive}\\tmp"));
        assert_eq!(nt_abspath_in("C:\\x", &cwd), "C:\\x");
        assert_eq!(nt_abspath_in("C:..\\a", &cwd), "C:\\a");
        assert_eq!(
            nt_abspath_in("..", &cwd),
            nt_normpath(&cwd).rsplit_once('\\').map(|(h, _)| h.to_string()).unwrap_or_else(|| "\\".to_string())
        );
        let other_cwd = format!("{drive}\\proj");
        assert_eq!(nt_abspath_in("a", &other_cwd), format!("{other_cwd}\\a"));
    }


    // -------------------------------------------- 实测表：normpath 全量 654 行

    // 由 scratch/rust_parity/ws_table_gen.py 生成，ws_probe8.py 对 live CPython 逐行复核。
    // NT_TABLE：327 行，全部实测，非人工整理。
    const NT_TABLE: &[(&str, &str)] = &[
        ("a", "a"), ("a\\b", "a\\b"), (".\\a", "a"),
        ("a\\.\\b", "a\\b"), ("a\\..", "."), ("a\\..\\..", ".."),
        ("..", ".."), (".", "."), (".\\..", ".."),
        ("..\\..", "..\\.."), ("a\\b\\..\\..\\c", "c"), ("a.", "a."),
        ("..a", "..a"), ("a b ", "a b "), ("x\\..", "."),
        ("..\\x", "..\\x"), ("..\\a\\..\\b", "..\\b"), ("a\\.\\..\\b", "b"),
        ("\\", "\\"), ("..\\", ".."), ("C:", "C:"),
        ("C:a", "C:a"), ("C:a\\b", "C:a\\b"), ("C:.\\a", "C:.\\a"),
        ("C:a\\.\\b", "C:a\\b"), ("C:a\\..", "C:"), ("C:a\\..\\..", "C:.."),
        ("C:..", "C:.."), ("C:.", "C:."), ("C:.\\..", "C:"),
        ("C:..\\..", "C:..\\.."), ("C:a\\b\\..\\..\\c", "C:c"), ("C:a.", "C:a."),
        ("C:..a", "C:..a"), ("C:a b ", "C:a b "), ("C:x\\..", "C:"),
        ("C:..\\x", "C:..\\x"), ("C:..\\a\\..\\b", "C:..\\b"), ("C:a\\.\\..\\b", "C:b"),
        ("C:\\", "C:\\"), ("C:..\\", "C:.."), ("C:\\a", "C:\\a"),
        ("C:\\a\\b", "C:\\a\\b"), ("C:\\.\\a", "C:\\a"), ("C:\\a\\.\\b", "C:\\a\\b"),
        ("C:\\a\\..", "C:\\"), ("C:\\a\\..\\..", "C:\\"), ("C:\\..", "C:\\"),
        ("C:\\.", "C:\\"), ("C:\\.\\..", "C:\\"), ("C:\\..\\..", "C:\\"),
        ("C:\\a\\b\\..\\..\\c", "C:\\c"), ("C:\\a.", "C:\\a."), ("C:\\..a", "C:\\..a"),
        ("C:\\a b ", "C:\\a b "), ("C:\\x\\..", "C:\\"), ("C:\\..\\x", "C:\\x"),
        ("C:\\..\\a\\..\\b", "C:\\b"), ("C:\\a\\.\\..\\b", "C:\\b"), ("C:\\\\", "C:\\"),
        ("C:\\..\\", "C:\\"), ("\\a", "\\a"), ("\\a\\b", "\\a\\b"),
        ("\\.\\a", "\\a"), ("\\a\\.\\b", "\\a\\b"), ("\\a\\..", "\\"),
        ("\\a\\..\\..", "\\"), ("\\..", "\\"), ("\\.", "\\"),
        ("\\.\\..", "\\"), ("\\..\\..", "\\"), ("\\a\\b\\..\\..\\c", "\\c"),
        ("\\a.", "\\a."), ("\\..a", "\\..a"), ("\\a b ", "\\a b "),
        ("\\x\\..", "\\"), ("\\..\\x", "\\x"), ("\\..\\a\\..\\b", "\\b"),
        ("\\a\\.\\..\\b", "\\b"), ("\\\\", "\\\\"), ("\\..\\", "\\"),
        ("\\\\srv\\", "\\\\srv\\"), ("\\\\srv\\a", "\\\\srv\\a"), ("\\\\srv\\a\\b", "\\\\srv\\a\\b"),
        ("\\\\srv\\.\\a", "\\\\srv\\.\\a"), ("\\\\srv\\a\\.\\b", "\\\\srv\\a\\b"), ("\\\\srv\\a\\..", "\\\\srv\\a\\"),
        ("\\\\srv\\a\\..\\..", "\\\\srv\\a\\"), ("\\\\srv\\..", "\\\\srv\\.."), ("\\\\srv\\.", "\\\\srv\\."),
        ("\\\\srv\\.\\..", "\\\\srv\\.\\"), ("\\\\srv\\..\\..", "\\\\srv\\..\\"), ("\\\\srv\\a\\b\\..\\..\\c", "\\\\srv\\a\\c"),
        ("\\\\srv\\a.", "\\\\srv\\a."), ("\\\\srv\\..a", "\\\\srv\\..a"), ("\\\\srv\\a b ", "\\\\srv\\a b "),
        ("\\\\srv\\x\\..", "\\\\srv\\x\\"), ("\\\\srv\\..\\x", "\\\\srv\\..\\x"), ("\\\\srv\\..\\a\\..\\b", "\\\\srv\\..\\b"),
        ("\\\\srv\\a\\.\\..\\b", "\\\\srv\\a\\b"), ("\\\\srv\\\\", "\\\\srv\\\\"), ("\\\\srv\\..\\", "\\\\srv\\..\\"),
        ("\\\\srv\\aa", "\\\\srv\\aa"), ("\\\\srv\\aa\\b", "\\\\srv\\aa\\b"), ("\\\\srv\\a.\\a", "\\\\srv\\a.\\a"),
        ("\\\\srv\\aa\\.\\b", "\\\\srv\\aa\\b"), ("\\\\srv\\aa\\..", "\\\\srv\\aa\\"), ("\\\\srv\\aa\\..\\..", "\\\\srv\\aa\\"),
        ("\\\\srv\\a..", "\\\\srv\\a.."), ("\\\\srv\\a.\\..", "\\\\srv\\a.\\"), ("\\\\srv\\a..\\..", "\\\\srv\\a..\\"),
        ("\\\\srv\\aa\\b\\..\\..\\c", "\\\\srv\\aa\\c"), ("\\\\srv\\aa.", "\\\\srv\\aa."), ("\\\\srv\\a..a", "\\\\srv\\a..a"),
        ("\\\\srv\\aa b ", "\\\\srv\\aa b "), ("\\\\srv\\ax\\..", "\\\\srv\\ax\\"), ("\\\\srv\\a..\\x", "\\\\srv\\a..\\x"),
        ("\\\\srv\\a..\\a\\..\\b", "\\\\srv\\a..\\b"), ("\\\\srv\\aa\\.\\..\\b", "\\\\srv\\aa\\b"), ("\\\\srv\\a\\", "\\\\srv\\a\\"),
        ("\\\\srv\\a..\\", "\\\\srv\\a..\\"), ("\\\\srv\\a\\a", "\\\\srv\\a\\a"), ("\\\\srv\\a\\a\\b", "\\\\srv\\a\\a\\b"),
        ("\\\\srv\\a\\.\\a", "\\\\srv\\a\\a"), ("\\\\srv\\a\\a\\.\\b", "\\\\srv\\a\\a\\b"), ("\\\\srv\\a\\a\\..", "\\\\srv\\a\\"),
        ("\\\\srv\\a\\a\\..\\..", "\\\\srv\\a\\"), ("\\\\srv\\a\\.", "\\\\srv\\a\\"), ("\\\\srv\\a\\.\\..", "\\\\srv\\a\\"),
        ("\\\\srv\\a\\a\\b\\..\\..\\c", "\\\\srv\\a\\c"), ("\\\\srv\\a\\a.", "\\\\srv\\a\\a."), ("\\\\srv\\a\\..a", "\\\\srv\\a\\..a"),
        ("\\\\srv\\a\\a b ", "\\\\srv\\a\\a b "), ("\\\\srv\\a\\x\\..", "\\\\srv\\a\\"), ("\\\\srv\\a\\..\\x", "\\\\srv\\a\\x"),
        ("\\\\srv\\a\\..\\a\\..\\b", "\\\\srv\\a\\b"), ("\\\\srv\\a\\a\\.\\..\\b", "\\\\srv\\a\\b"), ("\\\\srv\\a\\\\", "\\\\srv\\a\\"),
        ("\\\\srv\\a\\..\\", "\\\\srv\\a\\"), ("\\\\?\\C:\\", "\\\\?\\C:\\"), ("\\\\?\\C:\\a", "\\\\?\\C:\\a"),
        ("\\\\?\\C:\\a\\b", "\\\\?\\C:\\a\\b"), ("\\\\?\\C:\\.\\a", "\\\\?\\C:\\a"), ("\\\\?\\C:\\a\\.\\b", "\\\\?\\C:\\a\\b"),
        ("\\\\?\\C:\\a\\..", "\\\\?\\C:\\"), ("\\\\?\\C:\\a\\..\\..", "\\\\?\\C:\\"), ("\\\\?\\C:\\..", "\\\\?\\C:\\"),
        ("\\\\?\\C:\\.", "\\\\?\\C:\\"), ("\\\\?\\C:\\.\\..", "\\\\?\\C:\\"), ("\\\\?\\C:\\..\\..", "\\\\?\\C:\\"),
        ("\\\\?\\C:\\a\\b\\..\\..\\c", "\\\\?\\C:\\c"), ("\\\\?\\C:\\a.", "\\\\?\\C:\\a."), ("\\\\?\\C:\\..a", "\\\\?\\C:\\..a"),
        ("\\\\?\\C:\\a b ", "\\\\?\\C:\\a b "), ("\\\\?\\C:\\x\\..", "\\\\?\\C:\\"), ("\\\\?\\C:\\..\\x", "\\\\?\\C:\\x"),
        ("\\\\?\\C:\\..\\a\\..\\b", "\\\\?\\C:\\b"), ("\\\\?\\C:\\a\\.\\..\\b", "\\\\?\\C:\\b"), ("\\\\?\\C:\\\\", "\\\\?\\C:\\"),
        ("\\\\?\\C:\\..\\", "\\\\?\\C:\\"), ("\\\\.\\COM1\\", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\a", "\\\\.\\COM1\\a"),
        ("\\\\.\\COM1\\a\\b", "\\\\.\\COM1\\a\\b"), ("\\\\.\\COM1\\.\\a", "\\\\.\\COM1\\a"), ("\\\\.\\COM1\\a\\.\\b", "\\\\.\\COM1\\a\\b"),
        ("\\\\.\\COM1\\a\\..", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\a\\..\\..", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\..", "\\\\.\\COM1\\"),
        ("\\\\.\\COM1\\.", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\.\\..", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\..\\..", "\\\\.\\COM1\\"),
        ("\\\\.\\COM1\\a\\b\\..\\..\\c", "\\\\.\\COM1\\c"), ("\\\\.\\COM1\\a.", "\\\\.\\COM1\\a."), ("\\\\.\\COM1\\..a", "\\\\.\\COM1\\..a"),
        ("\\\\.\\COM1\\a b ", "\\\\.\\COM1\\a b "), ("\\\\.\\COM1\\x\\..", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\..\\x", "\\\\.\\COM1\\x"),
        ("\\\\.\\COM1\\..\\a\\..\\b", "\\\\.\\COM1\\b"), ("\\\\.\\COM1\\a\\.\\..\\b", "\\\\.\\COM1\\b"), ("\\\\.\\COM1\\\\", "\\\\.\\COM1\\"),
        ("\\\\.\\COM1\\..\\", "\\\\.\\COM1\\"), (":", ":"), (":a", ":a"),
        (":a\\b", ":a\\b"), (":.\\a", ":.\\a"), (":a\\.\\b", ":a\\b"),
        (":a\\..", "."), (":a\\..\\..", ".."), (":..", ":.."),
        (":.", ":."), (":.\\..", "."), (":..\\..", "."),
        (":a\\b\\..\\..\\c", "c"), (":a.", ":a."), (":..a", ":..a"),
        (":a b ", ":a b "), (":x\\..", "."), (":..\\x", ":..\\x"),
        (":..\\a\\..\\b", ":..\\b"), (":a\\.\\..\\b", "b"), (":\\", ":"),
        (":..\\", ":.."), ("\\\\srv", "\\\\srv"), ("\\\\srva", "\\\\srva"),
        ("\\\\srva\\b", "\\\\srva\\b"), ("\\\\srv.\\a", "\\\\srv.\\a"), ("\\\\srva\\.\\b", "\\\\srva\\.\\b"),
        ("\\\\srva\\..", "\\\\srva\\.."), ("\\\\srva\\..\\..", "\\\\srva\\..\\"), ("\\\\srv..", "\\\\srv.."),
        ("\\\\srv.", "\\\\srv."), ("\\\\srv.\\..", "\\\\srv.\\.."), ("\\\\srv..\\..", "\\\\srv..\\.."),
        ("\\\\srva\\b\\..\\..\\c", "\\\\srva\\b\\c"), ("\\\\srva.", "\\\\srva."), ("\\\\srv..a", "\\\\srv..a"),
        ("\\\\srva b ", "\\\\srva b "), ("\\\\srvx\\..", "\\\\srvx\\.."), ("\\\\srv..\\x", "\\\\srv..\\x"),
        ("\\\\srv..\\a\\..\\b", "\\\\srv..\\a\\b"), ("\\\\srva\\.\\..\\b", "\\\\srva\\.\\b"), ("\\\\srv..\\", "\\\\srv..\\"),
        ("/", "\\"), ("/a", "\\a"), ("/a\\b", "\\a\\b"),
        ("/.\\a", "\\a"), ("/a\\.\\b", "\\a\\b"), ("/a\\..", "\\"),
        ("/a\\..\\..", "\\"), ("/..", "\\"), ("/.", "\\"),
        ("/.\\..", "\\"), ("/..\\..", "\\"), ("/a\\b\\..\\..\\c", "\\c"),
        ("/a.", "\\a."), ("/..a", "\\..a"), ("/a b ", "\\a b "),
        ("/x\\..", "\\"), ("/..\\x", "\\x"), ("/..\\a\\..\\b", "\\b"),
        ("/a\\.\\..\\b", "\\b"), ("/\\", "\\\\"), ("/..\\", "\\"),
        ("//", "\\\\"), ("//a", "\\\\a"), ("//a\\b", "\\\\a\\b"),
        ("//.\\a", "\\\\.\\a"), ("//a\\.\\b", "\\\\a\\.\\b"), ("//a\\..", "\\\\a\\.."),
        ("//a\\..\\..", "\\\\a\\..\\"), ("//..", "\\\\.."), ("//.", "\\\\."),
        ("//.\\..", "\\\\.\\.."), ("//..\\..", "\\\\..\\.."), ("//a\\b\\..\\..\\c", "\\\\a\\b\\c"),
        ("//a.", "\\\\a."), ("//..a", "\\\\..a"), ("//a b ", "\\\\a b "),
        ("//x\\..", "\\\\x\\.."), ("//..\\x", "\\\\..\\x"), ("//..\\a\\..\\b", "\\\\..\\a\\b"),
        ("//a\\.\\..\\b", "\\\\a\\.\\b"), ("//\\", "\\\\\\"), ("//..\\", "\\\\..\\"),
        ("///", "\\\\\\"), ("///a", "\\\\\\a"), ("///a\\b", "\\\\\\a\\b"),
        ("///.\\a", "\\\\\\.\\a"), ("///a\\.\\b", "\\\\\\a\\b"), ("///a\\..", "\\\\\\a\\"),
        ("///a\\..\\..", "\\\\\\a\\"), ("///..", "\\\\\\.."), ("///.", "\\\\\\."),
        ("///.\\..", "\\\\\\.\\"), ("///..\\..", "\\\\\\..\\"), ("///a\\b\\..\\..\\c", "\\\\\\a\\c"),
        ("///a.", "\\\\\\a."), ("///..a", "\\\\\\..a"), ("///a b ", "\\\\\\a b "),
        ("///x\\..", "\\\\\\x\\"), ("///..\\x", "\\\\\\..\\x"), ("///..\\a\\..\\b", "\\\\\\..\\b"),
        ("///a\\.\\..\\b", "\\\\\\a\\b"), ("///\\", "\\\\\\\\"), ("///..\\", "\\\\\\..\\"),
        ("/a/", "\\a"), ("/a/a", "\\a\\a"), ("/a/a\\b", "\\a\\a\\b"),
        ("/a/.\\a", "\\a\\a"), ("/a/a\\.\\b", "\\a\\a\\b"), ("/a/a\\..", "\\a"),
        ("/a/a\\..\\..", "\\"), ("/a/..", "\\"), ("/a/.", "\\a"),
        ("/a/.\\..", "\\"), ("/a/..\\..", "\\"), ("/a/a\\b\\..\\..\\c", "\\a\\c"),
        ("/a/a.", "\\a\\a."), ("/a/..a", "\\a\\..a"), ("/a/a b ", "\\a\\a b "),
        ("/a/x\\..", "\\a"), ("/a/..\\x", "\\x"), ("/a/..\\a\\..\\b", "\\b"),
        ("/a/a\\.\\..\\b", "\\a\\b"), ("/a/\\", "\\a"), ("/a/..\\", "\\"),
        ("//srv/", "\\\\srv\\"), ("//srv/a", "\\\\srv\\a"), ("//srv/a\\b", "\\\\srv\\a\\b"),
        ("//srv/.\\a", "\\\\srv\\.\\a"), ("//srv/a\\.\\b", "\\\\srv\\a\\b"), ("//srv/a\\..", "\\\\srv\\a\\"),
        ("//srv/a\\..\\..", "\\\\srv\\a\\"), ("//srv/..", "\\\\srv\\.."), ("//srv/.", "\\\\srv\\."),
        ("//srv/.\\..", "\\\\srv\\.\\"), ("//srv/..\\..", "\\\\srv\\..\\"), ("//srv/a\\b\\..\\..\\c", "\\\\srv\\a\\c"),
        ("//srv/a.", "\\\\srv\\a."), ("//srv/..a", "\\\\srv\\..a"), ("//srv/a b ", "\\\\srv\\a b "),
        ("//srv/x\\..", "\\\\srv\\x\\"), ("//srv/..\\x", "\\\\srv\\..\\x"), ("//srv/..\\a\\..\\b", "\\\\srv\\..\\b"),
        ("//srv/a\\.\\..\\b", "\\\\srv\\a\\b"), ("//srv/\\", "\\\\srv\\\\"), ("//srv/..\\", "\\\\srv\\..\\"),
    ];

    // 由 scratch/rust_parity/ws_table_gen.py 生成，ws_probe8.py 对 live CPython 逐行复核。
    // PS_TABLE：327 行，全部实测，非人工整理。
    const PS_TABLE: &[(&str, &str)] = &[
        ("a", "a"), ("a\\b", "a\\b"), (".\\a", ".\\a"),
        ("a\\.\\b", "a\\.\\b"), ("a\\..", "a\\.."), ("a\\..\\..", "a\\..\\.."),
        ("..", ".."), (".", "."), (".\\..", ".\\.."),
        ("..\\..", "..\\.."), ("a\\b\\..\\..\\c", "a\\b\\..\\..\\c"), ("a.", "a."),
        ("..a", "..a"), ("a b ", "a b "), ("x\\..", "x\\.."),
        ("..\\x", "..\\x"), ("..\\a\\..\\b", "..\\a\\..\\b"), ("a\\.\\..\\b", "a\\.\\..\\b"),
        ("\\", "\\"), ("..\\", "..\\"), ("C:", "C:"),
        ("C:a", "C:a"), ("C:a\\b", "C:a\\b"), ("C:.\\a", "C:.\\a"),
        ("C:a\\.\\b", "C:a\\.\\b"), ("C:a\\..", "C:a\\.."), ("C:a\\..\\..", "C:a\\..\\.."),
        ("C:..", "C:.."), ("C:.", "C:."), ("C:.\\..", "C:.\\.."),
        ("C:..\\..", "C:..\\.."), ("C:a\\b\\..\\..\\c", "C:a\\b\\..\\..\\c"), ("C:a.", "C:a."),
        ("C:..a", "C:..a"), ("C:a b ", "C:a b "), ("C:x\\..", "C:x\\.."),
        ("C:..\\x", "C:..\\x"), ("C:..\\a\\..\\b", "C:..\\a\\..\\b"), ("C:a\\.\\..\\b", "C:a\\.\\..\\b"),
        ("C:\\", "C:\\"), ("C:..\\", "C:..\\"), ("C:\\a", "C:\\a"),
        ("C:\\a\\b", "C:\\a\\b"), ("C:\\.\\a", "C:\\.\\a"), ("C:\\a\\.\\b", "C:\\a\\.\\b"),
        ("C:\\a\\..", "C:\\a\\.."), ("C:\\a\\..\\..", "C:\\a\\..\\.."), ("C:\\..", "C:\\.."),
        ("C:\\.", "C:\\."), ("C:\\.\\..", "C:\\.\\.."), ("C:\\..\\..", "C:\\..\\.."),
        ("C:\\a\\b\\..\\..\\c", "C:\\a\\b\\..\\..\\c"), ("C:\\a.", "C:\\a."), ("C:\\..a", "C:\\..a"),
        ("C:\\a b ", "C:\\a b "), ("C:\\x\\..", "C:\\x\\.."), ("C:\\..\\x", "C:\\..\\x"),
        ("C:\\..\\a\\..\\b", "C:\\..\\a\\..\\b"), ("C:\\a\\.\\..\\b", "C:\\a\\.\\..\\b"), ("C:\\\\", "C:\\\\"),
        ("C:\\..\\", "C:\\..\\"), ("\\a", "\\a"), ("\\a\\b", "\\a\\b"),
        ("\\.\\a", "\\.\\a"), ("\\a\\.\\b", "\\a\\.\\b"), ("\\a\\..", "\\a\\.."),
        ("\\a\\..\\..", "\\a\\..\\.."), ("\\..", "\\.."), ("\\.", "\\."),
        ("\\.\\..", "\\.\\.."), ("\\..\\..", "\\..\\.."), ("\\a\\b\\..\\..\\c", "\\a\\b\\..\\..\\c"),
        ("\\a.", "\\a."), ("\\..a", "\\..a"), ("\\a b ", "\\a b "),
        ("\\x\\..", "\\x\\.."), ("\\..\\x", "\\..\\x"), ("\\..\\a\\..\\b", "\\..\\a\\..\\b"),
        ("\\a\\.\\..\\b", "\\a\\.\\..\\b"), ("\\\\", "\\\\"), ("\\..\\", "\\..\\"),
        ("\\\\srv\\", "\\\\srv\\"), ("\\\\srv\\a", "\\\\srv\\a"), ("\\\\srv\\a\\b", "\\\\srv\\a\\b"),
        ("\\\\srv\\.\\a", "\\\\srv\\.\\a"), ("\\\\srv\\a\\.\\b", "\\\\srv\\a\\.\\b"), ("\\\\srv\\a\\..", "\\\\srv\\a\\.."),
        ("\\\\srv\\a\\..\\..", "\\\\srv\\a\\..\\.."), ("\\\\srv\\..", "\\\\srv\\.."), ("\\\\srv\\.", "\\\\srv\\."),
        ("\\\\srv\\.\\..", "\\\\srv\\.\\.."), ("\\\\srv\\..\\..", "\\\\srv\\..\\.."), ("\\\\srv\\a\\b\\..\\..\\c", "\\\\srv\\a\\b\\..\\..\\c"),
        ("\\\\srv\\a.", "\\\\srv\\a."), ("\\\\srv\\..a", "\\\\srv\\..a"), ("\\\\srv\\a b ", "\\\\srv\\a b "),
        ("\\\\srv\\x\\..", "\\\\srv\\x\\.."), ("\\\\srv\\..\\x", "\\\\srv\\..\\x"), ("\\\\srv\\..\\a\\..\\b", "\\\\srv\\..\\a\\..\\b"),
        ("\\\\srv\\a\\.\\..\\b", "\\\\srv\\a\\.\\..\\b"), ("\\\\srv\\\\", "\\\\srv\\\\"), ("\\\\srv\\..\\", "\\\\srv\\..\\"),
        ("\\\\srv\\aa", "\\\\srv\\aa"), ("\\\\srv\\aa\\b", "\\\\srv\\aa\\b"), ("\\\\srv\\a.\\a", "\\\\srv\\a.\\a"),
        ("\\\\srv\\aa\\.\\b", "\\\\srv\\aa\\.\\b"), ("\\\\srv\\aa\\..", "\\\\srv\\aa\\.."), ("\\\\srv\\aa\\..\\..", "\\\\srv\\aa\\..\\.."),
        ("\\\\srv\\a..", "\\\\srv\\a.."), ("\\\\srv\\a.\\..", "\\\\srv\\a.\\.."), ("\\\\srv\\a..\\..", "\\\\srv\\a..\\.."),
        ("\\\\srv\\aa\\b\\..\\..\\c", "\\\\srv\\aa\\b\\..\\..\\c"), ("\\\\srv\\aa.", "\\\\srv\\aa."), ("\\\\srv\\a..a", "\\\\srv\\a..a"),
        ("\\\\srv\\aa b ", "\\\\srv\\aa b "), ("\\\\srv\\ax\\..", "\\\\srv\\ax\\.."), ("\\\\srv\\a..\\x", "\\\\srv\\a..\\x"),
        ("\\\\srv\\a..\\a\\..\\b", "\\\\srv\\a..\\a\\..\\b"), ("\\\\srv\\aa\\.\\..\\b", "\\\\srv\\aa\\.\\..\\b"), ("\\\\srv\\a\\", "\\\\srv\\a\\"),
        ("\\\\srv\\a..\\", "\\\\srv\\a..\\"), ("\\\\srv\\a\\a", "\\\\srv\\a\\a"), ("\\\\srv\\a\\a\\b", "\\\\srv\\a\\a\\b"),
        ("\\\\srv\\a\\.\\a", "\\\\srv\\a\\.\\a"), ("\\\\srv\\a\\a\\.\\b", "\\\\srv\\a\\a\\.\\b"), ("\\\\srv\\a\\a\\..", "\\\\srv\\a\\a\\.."),
        ("\\\\srv\\a\\a\\..\\..", "\\\\srv\\a\\a\\..\\.."), ("\\\\srv\\a\\.", "\\\\srv\\a\\."), ("\\\\srv\\a\\.\\..", "\\\\srv\\a\\.\\.."),
        ("\\\\srv\\a\\a\\b\\..\\..\\c", "\\\\srv\\a\\a\\b\\..\\..\\c"), ("\\\\srv\\a\\a.", "\\\\srv\\a\\a."), ("\\\\srv\\a\\..a", "\\\\srv\\a\\..a"),
        ("\\\\srv\\a\\a b ", "\\\\srv\\a\\a b "), ("\\\\srv\\a\\x\\..", "\\\\srv\\a\\x\\.."), ("\\\\srv\\a\\..\\x", "\\\\srv\\a\\..\\x"),
        ("\\\\srv\\a\\..\\a\\..\\b", "\\\\srv\\a\\..\\a\\..\\b"), ("\\\\srv\\a\\a\\.\\..\\b", "\\\\srv\\a\\a\\.\\..\\b"), ("\\\\srv\\a\\\\", "\\\\srv\\a\\\\"),
        ("\\\\srv\\a\\..\\", "\\\\srv\\a\\..\\"), ("\\\\?\\C:\\", "\\\\?\\C:\\"), ("\\\\?\\C:\\a", "\\\\?\\C:\\a"),
        ("\\\\?\\C:\\a\\b", "\\\\?\\C:\\a\\b"), ("\\\\?\\C:\\.\\a", "\\\\?\\C:\\.\\a"), ("\\\\?\\C:\\a\\.\\b", "\\\\?\\C:\\a\\.\\b"),
        ("\\\\?\\C:\\a\\..", "\\\\?\\C:\\a\\.."), ("\\\\?\\C:\\a\\..\\..", "\\\\?\\C:\\a\\..\\.."), ("\\\\?\\C:\\..", "\\\\?\\C:\\.."),
        ("\\\\?\\C:\\.", "\\\\?\\C:\\."), ("\\\\?\\C:\\.\\..", "\\\\?\\C:\\.\\.."), ("\\\\?\\C:\\..\\..", "\\\\?\\C:\\..\\.."),
        ("\\\\?\\C:\\a\\b\\..\\..\\c", "\\\\?\\C:\\a\\b\\..\\..\\c"), ("\\\\?\\C:\\a.", "\\\\?\\C:\\a."), ("\\\\?\\C:\\..a", "\\\\?\\C:\\..a"),
        ("\\\\?\\C:\\a b ", "\\\\?\\C:\\a b "), ("\\\\?\\C:\\x\\..", "\\\\?\\C:\\x\\.."), ("\\\\?\\C:\\..\\x", "\\\\?\\C:\\..\\x"),
        ("\\\\?\\C:\\..\\a\\..\\b", "\\\\?\\C:\\..\\a\\..\\b"), ("\\\\?\\C:\\a\\.\\..\\b", "\\\\?\\C:\\a\\.\\..\\b"), ("\\\\?\\C:\\\\", "\\\\?\\C:\\\\"),
        ("\\\\?\\C:\\..\\", "\\\\?\\C:\\..\\"), ("\\\\.\\COM1\\", "\\\\.\\COM1\\"), ("\\\\.\\COM1\\a", "\\\\.\\COM1\\a"),
        ("\\\\.\\COM1\\a\\b", "\\\\.\\COM1\\a\\b"), ("\\\\.\\COM1\\.\\a", "\\\\.\\COM1\\.\\a"), ("\\\\.\\COM1\\a\\.\\b", "\\\\.\\COM1\\a\\.\\b"),
        ("\\\\.\\COM1\\a\\..", "\\\\.\\COM1\\a\\.."), ("\\\\.\\COM1\\a\\..\\..", "\\\\.\\COM1\\a\\..\\.."), ("\\\\.\\COM1\\..", "\\\\.\\COM1\\.."),
        ("\\\\.\\COM1\\.", "\\\\.\\COM1\\."), ("\\\\.\\COM1\\.\\..", "\\\\.\\COM1\\.\\.."), ("\\\\.\\COM1\\..\\..", "\\\\.\\COM1\\..\\.."),
        ("\\\\.\\COM1\\a\\b\\..\\..\\c", "\\\\.\\COM1\\a\\b\\..\\..\\c"), ("\\\\.\\COM1\\a.", "\\\\.\\COM1\\a."), ("\\\\.\\COM1\\..a", "\\\\.\\COM1\\..a"),
        ("\\\\.\\COM1\\a b ", "\\\\.\\COM1\\a b "), ("\\\\.\\COM1\\x\\..", "\\\\.\\COM1\\x\\.."), ("\\\\.\\COM1\\..\\x", "\\\\.\\COM1\\..\\x"),
        ("\\\\.\\COM1\\..\\a\\..\\b", "\\\\.\\COM1\\..\\a\\..\\b"), ("\\\\.\\COM1\\a\\.\\..\\b", "\\\\.\\COM1\\a\\.\\..\\b"), ("\\\\.\\COM1\\\\", "\\\\.\\COM1\\\\"),
        ("\\\\.\\COM1\\..\\", "\\\\.\\COM1\\..\\"), (":", ":"), (":a", ":a"),
        (":a\\b", ":a\\b"), (":.\\a", ":.\\a"), (":a\\.\\b", ":a\\.\\b"),
        (":a\\..", ":a\\.."), (":a\\..\\..", ":a\\..\\.."), (":..", ":.."),
        (":.", ":."), (":.\\..", ":.\\.."), (":..\\..", ":..\\.."),
        (":a\\b\\..\\..\\c", ":a\\b\\..\\..\\c"), (":a.", ":a."), (":..a", ":..a"),
        (":a b ", ":a b "), (":x\\..", ":x\\.."), (":..\\x", ":..\\x"),
        (":..\\a\\..\\b", ":..\\a\\..\\b"), (":a\\.\\..\\b", ":a\\.\\..\\b"), (":\\", ":\\"),
        (":..\\", ":..\\"), ("\\\\srv", "\\\\srv"), ("\\\\srva", "\\\\srva"),
        ("\\\\srva\\b", "\\\\srva\\b"), ("\\\\srv.\\a", "\\\\srv.\\a"), ("\\\\srva\\.\\b", "\\\\srva\\.\\b"),
        ("\\\\srva\\..", "\\\\srva\\.."), ("\\\\srva\\..\\..", "\\\\srva\\..\\.."), ("\\\\srv..", "\\\\srv.."),
        ("\\\\srv.", "\\\\srv."), ("\\\\srv.\\..", "\\\\srv.\\.."), ("\\\\srv..\\..", "\\\\srv..\\.."),
        ("\\\\srva\\b\\..\\..\\c", "\\\\srva\\b\\..\\..\\c"), ("\\\\srva.", "\\\\srva."), ("\\\\srv..a", "\\\\srv..a"),
        ("\\\\srva b ", "\\\\srva b "), ("\\\\srvx\\..", "\\\\srvx\\.."), ("\\\\srv..\\x", "\\\\srv..\\x"),
        ("\\\\srv..\\a\\..\\b", "\\\\srv..\\a\\..\\b"), ("\\\\srva\\.\\..\\b", "\\\\srva\\.\\..\\b"), ("\\\\srv..\\", "\\\\srv..\\"),
        ("/", "/"), ("/a", "/a"), ("/a\\b", "/a\\b"),
        ("/.\\a", "/.\\a"), ("/a\\.\\b", "/a\\.\\b"), ("/a\\..", "/a\\.."),
        ("/a\\..\\..", "/a\\..\\.."), ("/..", "/"), ("/.", "/"),
        ("/.\\..", "/.\\.."), ("/..\\..", "/..\\.."), ("/a\\b\\..\\..\\c", "/a\\b\\..\\..\\c"),
        ("/a.", "/a."), ("/..a", "/..a"), ("/a b ", "/a b "),
        ("/x\\..", "/x\\.."), ("/..\\x", "/..\\x"), ("/..\\a\\..\\b", "/..\\a\\..\\b"),
        ("/a\\.\\..\\b", "/a\\.\\..\\b"), ("/\\", "/\\"), ("/..\\", "/..\\"),
        ("//", "//"), ("//a", "//a"), ("//a\\b", "//a\\b"),
        ("//.\\a", "//.\\a"), ("//a\\.\\b", "//a\\.\\b"), ("//a\\..", "//a\\.."),
        ("//a\\..\\..", "//a\\..\\.."), ("//..", "//"), ("//.", "//"),
        ("//.\\..", "//.\\.."), ("//..\\..", "//..\\.."), ("//a\\b\\..\\..\\c", "//a\\b\\..\\..\\c"),
        ("//a.", "//a."), ("//..a", "//..a"), ("//a b ", "//a b "),
        ("//x\\..", "//x\\.."), ("//..\\x", "//..\\x"), ("//..\\a\\..\\b", "//..\\a\\..\\b"),
        ("//a\\.\\..\\b", "//a\\.\\..\\b"), ("//\\", "//\\"), ("//..\\", "//..\\"),
        ("///", "/"), ("///a", "/a"), ("///a\\b", "/a\\b"),
        ("///.\\a", "/.\\a"), ("///a\\.\\b", "/a\\.\\b"), ("///a\\..", "/a\\.."),
        ("///a\\..\\..", "/a\\..\\.."), ("///..", "/"), ("///.", "/"),
        ("///.\\..", "/.\\.."), ("///..\\..", "/..\\.."), ("///a\\b\\..\\..\\c", "/a\\b\\..\\..\\c"),
        ("///a.", "/a."), ("///..a", "/..a"), ("///a b ", "/a b "),
        ("///x\\..", "/x\\.."), ("///..\\x", "/..\\x"), ("///..\\a\\..\\b", "/..\\a\\..\\b"),
        ("///a\\.\\..\\b", "/a\\.\\..\\b"), ("///\\", "/\\"), ("///..\\", "/..\\"),
        ("/a/", "/a"), ("/a/a", "/a/a"), ("/a/a\\b", "/a/a\\b"),
        ("/a/.\\a", "/a/.\\a"), ("/a/a\\.\\b", "/a/a\\.\\b"), ("/a/a\\..", "/a/a\\.."),
        ("/a/a\\..\\..", "/a/a\\..\\.."), ("/a/..", "/"), ("/a/.", "/a"),
        ("/a/.\\..", "/a/.\\.."), ("/a/..\\..", "/a/..\\.."), ("/a/a\\b\\..\\..\\c", "/a/a\\b\\..\\..\\c"),
        ("/a/a.", "/a/a."), ("/a/..a", "/a/..a"), ("/a/a b ", "/a/a b "),
        ("/a/x\\..", "/a/x\\.."), ("/a/..\\x", "/a/..\\x"), ("/a/..\\a\\..\\b", "/a/..\\a\\..\\b"),
        ("/a/a\\.\\..\\b", "/a/a\\.\\..\\b"), ("/a/\\", "/a/\\"), ("/a/..\\", "/a/..\\"),
        ("//srv/", "//srv"), ("//srv/a", "//srv/a"), ("//srv/a\\b", "//srv/a\\b"),
        ("//srv/.\\a", "//srv/.\\a"), ("//srv/a\\.\\b", "//srv/a\\.\\b"), ("//srv/a\\..", "//srv/a\\.."),
        ("//srv/a\\..\\..", "//srv/a\\..\\.."), ("//srv/..", "//"), ("//srv/.", "//srv"),
        ("//srv/.\\..", "//srv/.\\.."), ("//srv/..\\..", "//srv/..\\.."), ("//srv/a\\b\\..\\..\\c", "//srv/a\\b\\..\\..\\c"),
        ("//srv/a.", "//srv/a."), ("//srv/..a", "//srv/..a"), ("//srv/a b ", "//srv/a b "),
        ("//srv/x\\..", "//srv/x\\.."), ("//srv/..\\x", "//srv/..\\x"), ("//srv/..\\a\\..\\b", "//srv/..\\a\\..\\b"),
        ("//srv/a\\.\\..\\b", "//srv/a\\.\\..\\b"), ("//srv/\\", "//srv/\\"), ("//srv/..\\", "//srv/..\\"),
    ];

    #[test]
    fn test_normpath_windows_table() {
        assert_eq!(NT_TABLE.len(), 327);
        for (input, want) in NT_TABLE {
            assert_eq!(&nt_normpath(input), want, "nt normpath({input:?})");
        }
    }

    #[test]
    fn test_normpath_posix_table() {
        assert_eq!(PS_TABLE.len(), 327);
        for (input, want) in PS_TABLE {
            assert_eq!(&posix_normpath(input), want, "posix normpath({input:?})");
        }
    }

    // ------------------------------------------------ 实测表：int()

    #[test]
    fn test_int_str_golden() {
        let rows: &[(&str, &str)] = &[
            ("12", "12"),
            (" 12 ", "12"),
            ("+12", "12"),
            ("-12", "-12"),
            ("1_2", "12"),
            ("0x10", "ValueError: invalid literal for int() with base 10: '0x10'"),
            ("", "ValueError: invalid literal for int() with base 10: ''"),
            (" ", "ValueError: invalid literal for int() with base 10: ' '"),
            ("1 2", "ValueError: invalid literal for int() with base 10: '1 2'"),
            ("\t12\n", "12"),
            ("١٢", "12"),
            ("１２", "12"),
            ("1__2", "ValueError: invalid literal for int() with base 10: '1__2'"),
            ("_12", "ValueError: invalid literal for int() with base 10: '_12'"),
            ("12_", "ValueError: invalid literal for int() with base 10: '12_'"),
            ("- 12", "ValueError: invalid literal for int() with base 10: '- 12'"),
            ("007", "7"),
            ("1.0", "ValueError: invalid literal for int() with base 10: '1.0'"),
            ("1e3", "ValueError: invalid literal for int() with base 10: '1e3'"),
            ("١٢٣", "123"),
            ("1_2_3", "123"),
            ("  +1_2  ", "12"),
        ];
        for (input, want) in rows {
            let got = match py_int_str(input) {
                Ok(v) => v.to_string(),
                Err(e) => e.raised(),
            };
            assert_eq!(got, *want, "int({input:?})");
        }
    }

    #[test]
    fn test_int_whitespace_matrix() {
        // float_golden.txt:46-61 —— 两条剥空白路径的分界。
        let cases: &[(&str, Option<i128>)] = &[
            ("\t", Some(12)),
            ("\n", Some(12)),
            ("\r", Some(12)),
            ("\u{0b}", Some(12)),
            ("\u{0c}", Some(12)),
            ("\u{1c}", None),
            ("\u{1d}", None),
            ("\u{1e}", None),
            ("\u{1f}", None),
            (" ", Some(12)),
            ("\u{00}", None),
            ("\u{85}", Some(12)),
            ("\u{a0}", Some(12)),
            ("\u{2002}", Some(12)),
            ("\u{3000}", Some(12)),
            ("\u{200b}", None),
        ];
        for (ws, want) in cases {
            let text = format!("{ws}12{ws}");
            let got = py_int_str(&text).ok();
            assert_eq!(&got, want, "int({text:?})");
        }
    }

    #[test]
    fn test_int_object_golden() {
        let rows: &[(JVal, &str)] = &[
            (JVal::f(12.9), "12"),
            (JVal::f(-12.9), "-12"),
            (JVal::f(0.0), "0"),
            (JVal::f(f64::INFINITY), "OverflowError: cannot convert float infinity to integer"),
            (JVal::f(f64::NEG_INFINITY), "OverflowError: cannot convert float infinity to integer"),
            (JVal::f(f64::NAN), "ValueError: cannot convert float NaN to integer"),
            (JVal::Bool(true), "1"),
            (JVal::Bool(false), "0"),
            (JVal::None, "TypeError: int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"),
            (JVal::List(vec![JVal::i(1)]), "TypeError: int() argument must be a string, a bytes-like object or a real number, not 'list'"),
            (JVal::obj(), "TypeError: int() argument must be a string, a bytes-like object or a real number, not 'dict'"),
        ];
        for (v, want) in rows {
            let got = match py_int(v) {
                Ok(x) => x.to_string(),
                Err(e) => e.raised(),
            };
            assert_eq!(got, *want, "int({v:?})");
        }
    }

    #[test]
    fn test_int_clamps() {
        // float_golden.txt:32-39
        assert_eq!(py_max(MIN_WIDTH, py_int(&JVal::f(1200.9999)).unwrap()), 1200);
        assert_eq!(py_max(MIN_HEIGHT, py_int(&JVal::f(-1200.9999)).unwrap()), 480);
        assert_eq!(py_max(MIN_HEIGHT, py_int(&JVal::f(639.9999)).unwrap()), 639);
        assert_eq!(py_int(&JVal::f(1e21)).unwrap(), 1_000_000_000_000_000_000_000);
        assert_eq!(py_max(MIN_WIDTH, py_int(&JVal::f(1e21)).unwrap()), 1_000_000_000_000_000_000_000);
        assert_eq!(py_max(MIN_WIDTH, py_int(&JVal::f(-1e21)).unwrap()), MIN_WIDTH);
        assert_eq!(py_max(MIN_WIDTH, py_int(&JVal::Bool(true)).unwrap()), MIN_WIDTH);
    }

    #[test]
    fn test_bool_truthiness_golden() {
        let rows: &[(JVal, bool)] = &[
            (JVal::Bool(false), false),
            (JVal::Bool(true), true),
            (JVal::i(0), false),
            (JVal::f(0.0), false),
            (JVal::s(""), false),
            (JVal::s("0"), true),
            (JVal::s("false"), true),
            (JVal::list(), false),
            (JVal::obj(), false),
            (JVal::None, false),
            (JVal::List(vec![JVal::i(0)]), true),
            (
                JVal::Obj(vec![("a".to_string(), JVal::i(0))]),
                true,
            ),
            (JVal::f(f64::NAN), true),
            (JVal::f(f64::INFINITY), true),
        ];
        for (v, want) in rows {
            assert_eq!(v.truthy(), *want, "bool({v:?})");
        }
    }

    // ------------------------------------------------ 实测表：float repr

    #[test]
    fn test_float_repr_golden() {
        let rows: &[(f64, &str, &str)] = &[
            (1.5, "1.5", "1.5"),
            (-1.5, "-1.5", "-1.5"),
            (2.0, "2.0", "2.0"),
            (0.0, "0.0", "0.0"),
            (-0.0, "-0.0", "-0.0"),
            (1.0, "1.0", "1.0"),
            (0.1, "0.1", "0.1"),
            (0.5, "0.5", "0.5"),
            (1e15, "1000000000000000.0", "1000000000000000.0"),
            (1e16, "1e+16", "1e+16"),
            (1e21, "1e+21", "1e+21"),
            (1e100, "1e+100", "1e+100"),
            (1e-5, "1e-05", "1e-05"),
            (0.0001, "0.0001", "0.0001"),
            (3.14159265358979, "3.14159265358979", "3.14159265358979"),
            (1200.0, "1200.0", "1200.0"),
            (f64::MAX, "1.7976931348623157e+308", "1.7976931348623157e+308"),
            (f64::MIN_POSITIVE / 2.0, "1.1125369292536007e-308", "1.1125369292536007e-308"),
            (f64::from_bits(1), "5e-324", "5e-324"),
            (f64::MIN_POSITIVE, "2.2250738585072014e-308", "2.2250738585072014e-308"),
            (123456789.123, "123456789.123", "123456789.123"),
            (1e6, "1000000.0", "1000000.0"),
            (1e7, "10000000.0", "10000000.0"),
            (6.02e23, "6.02e+23", "6.02e+23"),
            (-3.5e-9, "-3.5e-09", "-3.5e-09"),
            (9007199254740992.0, "9007199254740992.0", "9007199254740992.0"),
            (0.30000000000000004, "0.30000000000000004", "0.30000000000000004"),
            (f64::INFINITY, "inf", "Infinity"),
            (f64::NEG_INFINITY, "-inf", "-Infinity"),
            (f64::NAN, "nan", "NaN"),
        ];
        for (v, want_repr, want_dumps) in rows {
            assert_eq!(py_repr_f64(*v), *want_repr, "repr({v})");
            assert_eq!(json_float(*v), *want_dumps, "dumps({v})");
        }
    }

    #[test]
    fn test_float_roundtrip() {
        // 每条数字都必须能读回同一个 f64（最短性的定义）。
        for v in [0.1f64, 1.0 / 3.0, 1e21, 5e-324, 123456789.123, 2f64.powi(1023)] {
            let text = json_float(v);
            let back: f64 = text.replace("e+", "e").parse().unwrap();
            assert_eq!(back.to_bits(), v.to_bits(), "{text}");
        }
    }

    // ------------------------------------------------ 实测表：json.dumps 形状

    #[test]
    fn test_dumps_shapes_golden() {
        // ws_golden.txt:112-123
        let mut settings = Vec::new();
        settings.push(("theme".to_string(), JVal::s("dark")));
        let mut geo = Vec::new();
        geo.push(("width".to_string(), JVal::i(1200)));
        geo.push(("height".to_string(), JVal::i(800)));
        geo.push(("x".to_string(), JVal::i(100)));
        geo.push(("y".to_string(), JVal::i(50)));
        geo.push(("maximized".to_string(), JVal::Bool(false)));
        settings.push(("geometry".to_string(), JVal::Obj(geo)));
        settings.push(("last".to_string(), JVal::s("T:\\x\\中文.md")));
        assert_eq!(
            json_dumps(&JVal::Obj(settings)),
            "{\n  \"theme\": \"dark\",\n  \"geometry\": {\n    \"width\": 1200,\n    \"height\": 800,\n    \"x\": 100,\n    \"y\": 50,\n    \"maximized\": false\n  },\n  \"last\": \"T:\\\\x\\\\中文.md\"\n}"
        );

        let mut g2 = Vec::new();
        g2.push(("width".to_string(), JVal::i(1200)));
        g2.push(("height".to_string(), JVal::i(800)));
        g2.push(("x".to_string(), JVal::i(100)));
        g2.push(("y".to_string(), JVal::i(50)));
        g2.push(("maximized".to_string(), JVal::Bool(false)));
        assert_eq!(
            json_dumps(&JVal::Obj(g2)),
            "{\n  \"width\": 1200,\n  \"height\": 800,\n  \"x\": 100,\n  \"y\": 50,\n  \"maximized\": false\n}"
        );

        assert_eq!(json_dumps(&JVal::obj()), "{}");
        assert_eq!(json_dumps(&JVal::list()), "[]");
        assert_eq!(
            json_dumps(&JVal::List(vec![JVal::s("a"), JVal::s("b")])),
            "[\n  \"a\",\n  \"b\"\n]"
        );
        let nested = JVal::Obj(vec![(
            "a".to_string(),
            JVal::Obj(vec![
                (
                    "b".to_string(),
                    JVal::List(vec![
                        JVal::i(1),
                        JVal::Obj(vec![("c".to_string(), JVal::i(2))]),
                    ]),
                ),
                ("d".to_string(), JVal::list()),
            ]),
        )]);
        assert_eq!(
            json_dumps(&nested),
            "{\n  \"a\": {\n    \"b\": [\n      1,\n      {\n        \"c\": 2\n      }\n    ],\n    \"d\": []\n  }\n}"
        );
        assert_eq!(
            json_dumps(&JVal::Obj(vec![
                ("w".to_string(), JVal::f(1.5)),
                ("e".to_string(), JVal::f(1e21)),
                ("big".to_string(), JVal::i(2i128.pow(70))),
            ])),
            "{\n  \"w\": 1.5,\n  \"e\": 1e+21,\n  \"big\": 1180591620717411303424\n}"
        );
        assert_eq!(
            json_dumps(&JVal::Obj(vec![(
                "s".to_string(),
                JVal::s("q\"q\\q\nq\tq\u{1}q\u{2028}é中")
            )])),
            "{\n  \"s\": \"q\\\"q\\\\q\\nq\\tq\\u0001q\u{2028}é中\"\n}"
        );
    }

    #[test]
    fn test_dict_order_preserved_on_reassign() {
        // ws_golden.txt:122 reorder / 123 fresh
        let mut data = JVal::Obj(vec![
            ("geometry".to_string(), JVal::obj()),
            ("theme".to_string(), JVal::s("dark")),
        ]);
        data.set_item("geometry", JVal::Obj(vec![("width".to_string(), JVal::i(1200))]))
            .unwrap();
        assert_eq!(ordered_keys(&data).unwrap(), vec!["geometry", "theme"]);
        data.set_item("zoom", JVal::i(1)).unwrap();
        assert_eq!(ordered_keys(&data).unwrap(), vec!["geometry", "theme", "zoom"]);
    }

    // ------------------------------------------------ 实测表：json.loads

    #[test]
    fn test_loads_tolerance_golden() {
        // ws_golden.txt:124-135
        let ok: &[(&str, JVal)] = &[
            ("Infinity", JVal::f(f64::INFINITY)),
            ("-Infinity", JVal::f(f64::NEG_INFINITY)),
            (r#"{"a":"\u00e9"}"#, JVal::Obj(vec![("a".to_string(), JVal::s("é"))])),
            (r#"{"a": 1, "a": 2}"#, JVal::Obj(vec![("a".to_string(), JVal::i(2))])),
            ("  {\"a\": 1}  \n", JVal::Obj(vec![("a".to_string(), JVal::i(1))])),
            ("[1, 2.0, null, true]", JVal::List(vec![JVal::i(1), JVal::f(2.0), JVal::None, JVal::Bool(true)])),
        ];
        for (text, want) in ok {
            assert_eq!(&json_loads(text).unwrap(), want, "loads({text})");
        }
        let bad: &[&str] = &[
            "{\"a\": 1,}",
            "{\"a\": 1} extra",
            "nan",
            "{\"a\": .5}",
            "{\"a\": 01}",
            "{\"a\": 1}{",
            "  \n",
            "",
            "{\"a\" 1}",
        ];
        for text in bad {
            assert!(json_loads(text).is_err(), "loads({text:?}) should fail");
        }
    }

    #[test]
    fn test_loads_dumps_roundtrip() {
        let text = "{\n  \"geometry\": {\n    \"width\": 1200,\n    \"height\": 800.0,\n    \"x\": null,\n    \"y\": -0.0,\n    \"maximized\": false,\n    \"ratio\": 1e+21\n  }\n}";
        let v = json_loads(text).unwrap();
        assert_eq!(json_dumps(&v), text);
    }

    // ------------------------------------------------ geometry 语义

    #[test]
    fn test_geometry_defaults_and_three_state() {
        let (_dir, m) = mgr("defaults");
        fs::write(&m.settings_file, "{}").unwrap();
        let g = m.load_geometry().unwrap();
        assert_eq!((g.width, g.height, g.x, g.y, g.maximized), (1080, 760, None, None, false));
        // dump_xnull：x/y 为 JSON null 时保持 None，而不是 int(None) 报错
        fs::write(
            &m.settings_file,
            "{\"geometry\": {\"width\": 640, \"height\": 480, \"x\": null, \"y\": null, \"maximized\": true}}",
        )
        .unwrap();
        let g = m.load_geometry().unwrap();
        assert_eq!((g.x, g.y, g.maximized), (None, None, true));
        // 键存在但为 "" 与键缺失是三种不同状态里的第三种
        fs::write(&m.settings_file, "{\"geometry\": {\"maximized\": \"0\"}}").unwrap();
        let g = m.load_geometry().unwrap();
        assert_eq!((g.width, g.maximized), (1080, true));
    }

    #[test]
    fn test_geometry_error_propagates() {
        // window_state.py 里没有 try/except：坏值必须冒出来
        let (_dir, m) = mgr("errors");
        fs::write(&m.settings_file, "{\"geometry\": []}").unwrap();
        let e = m.load_geometry().unwrap_err();
        assert_eq!(e.raised(), "AttributeError: 'list' object has no attribute 'get'");
        for (bad, want) in [
            ("\"\"", "AttributeError: 'str' object has no attribute 'get'"),
            ("null", "AttributeError: 'NoneType' object has no attribute 'get'"),
            ("5", "AttributeError: 'int' object has no attribute 'get'"),
        ] {
            fs::write(&m.settings_file, format!("{{\"geometry\": {bad}}}")).unwrap();
            assert_eq!(m.load_geometry().unwrap_err().raised(), want);
        }
        fs::write(&m.settings_file, "{\"geometry\": {\"width\": \"abc\"}}").unwrap();
        assert_eq!(
            m.load_geometry().unwrap_err().raised(),
            "ValueError: invalid literal for int() with base 10: 'abc'"
        );
        fs::write(&m.settings_file, "{\"geometry\": {\"x\": 1.9}}").unwrap();
        assert_eq!(m.load_geometry().unwrap().x, Some(1));
    }

    #[test]
    fn test_geometry_empty_file_falls_back() {
        let (_dir, m) = mgr("fallback");
        assert_eq!(m.load_geometry().unwrap().width, 1080); // 文件不存在
        fs::write(&m.settings_file, "{not json").unwrap();
        assert_eq!(m.load_geometry().unwrap().width, 1080); // 坏 JSON → default
        fs::write(&m.settings_file, "").unwrap();
        assert_eq!(m.load_geometry().unwrap().width, 1080); // 空文件
    }

    #[test]
    fn test_save_geometry_keeps_other_keys() {
        let (_dir, m) = mgr("keepkeys");
        fs::write(&m.settings_file, "{\"theme\": \"dark\"}").unwrap();
        m.save_geometry(1200, 800, None, None, true).unwrap();
        let text = fs::read_to_string(&m.settings_file).unwrap();
        let v = json_loads(&text).unwrap();
        assert_eq!(ordered_keys(&v).unwrap(), vec!["theme", "geometry"]);
    }

    #[test]
    fn test_save_geometry_rejects_non_mapping_root() {
        let (_dir, m) = mgr("rootlist");
        fs::write(&m.settings_file, "[1, 2]").unwrap();
        let e = m.save_geometry(1000, 700, None, None, false).unwrap_err();
        assert_eq!(e.raised(), "TypeError: list indices must be integers or slices, not str");
    }

    // ------------------------------------------------ recent 语义

    #[test]
    fn test_recent_filter_and_slice_golden() {
        // ws_golden.txt:136-149
        // want 一律写成 JSON 数组；两侧都走同一个 dumps()，因此是逐元素精确比较。
        let cases: &[(&str, &str)] = &[
            ("notalist", "[]"),
            ("{\"a\": 1}", "[]"),
            ("5", "[]"),
            ("null", "[]"),
            ("[\"a\", 1, \"b\", null, \"\"]", "[\"a\", \"b\", \"\"]"),
            ("[]", "[]"),
        ];
        for (payload, want) in cases {
            let (dir, m) = mgr("filter");
            fs::write(dir.join("recent.json"), payload).unwrap();
            let got = m.load_recent_files(20);
            let got_d = JVal::List(got.iter().map(|s| JVal::s(s)).collect::<Vec<_>>()).dumps();
            assert_eq!(got_d, json_loads(want).unwrap().dumps(), "recent {payload}");
            let _ = fs::remove_dir_all(&dir);
        }
        let v: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        assert!(py_slice_head(&v, 0).is_empty());
        assert_eq!(py_slice_head(&v, -1), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(py_slice_head(&v, -2), vec!["a".to_string()]);
        assert_eq!(py_slice_head(&v, 20).len(), 3);
        assert_eq!(py_slice_head(&v, 1_000_000_000).len(), 3);
    }

    #[test]
    fn test_recent_dedupe_and_limit() {
        // dedupe_norm：不同分隔符写法应折叠成同一条
        let (dir, m) = mgr("dedupe");
        let target = dir.join("note.md");
        fs::write(&target, "x").unwrap();
        let raw = target.to_str().unwrap();
        let mixed = raw.replace('\\', "/");
        let first = m.add_recent_file(raw, 20);
        let second = m.add_recent_file(&mixed, 20);
        if cfg!(windows) {
            assert_eq!(second.len(), first.len());
        }
        assert_eq!(second[0], py_abspath(&mixed));
        // limit 截断，且过读窗口是 limit*2
        let many: Vec<String> = (0..30).map(|i| dir.join(format!("f{i}.md")).to_string_lossy().into_owned()).collect();
        for p in &many {
            m.add_recent_file(p, 5);
        }
        assert_eq!(m.load_recent_files(5).len(), 5);
        assert_eq!(m.load_recent_files(5)[0], py_abspath(many.last().unwrap()));
        // 空串短路：不改写文件
        let before = fs::read_to_string(dir.join("recent.json")).unwrap();
        let again = m.add_recent_file("", 5);
        assert_eq!(again.len(), 5);
        assert_eq!(fs::read_to_string(dir.join("recent.json")).unwrap(), before);
    }

    #[test]
    fn test_add_recent_empty_path_on_missing_file() {
        let (_dir, m) = mgr("emptyrecent");
        assert_eq!(m.add_recent_file("", 20), Vec::<String>::new());
    }

    // ------------------------------------------------ save_json 落盘细节

    #[test]
    fn test_save_json_creates_missing_parents() {
        let (dir, _m) = mgr("parents");
        let deep = dir.join("a").join("b").join("c.json");
        assert_eq!(save_json(deep.to_str().unwrap(), &JVal::obj()), true);
        assert!(deep.exists());
    }

    #[test]
    fn test_save_json_no_temp_left_behind() {
        let (dir, _m) = mgr("tmpclean");
        let target = dir.join("settings.json");
        save_json(target.to_str().unwrap(), &JVal::obj());
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    }

    #[test]
    fn test_save_json_crlf_on_windows() {
        // utils.py:66 用 os.fdopen(fd, 'w') —— 文本模式，Windows 下 LF 变 CRLF
        let (dir, _m) = mgr("crlf");
        let target = dir.join("settings.json");
        let data = JVal::List(vec![JVal::s("a"), JVal::s("b")]);
        save_json(target.to_str().unwrap(), &data);
        let bytes = fs::read(&target).unwrap();
        let dump = json_dumps(&data);
        assert!(dump.contains('\n'), "payload must contain LF");
        let expected = if cfg!(windows) {
            dump.replace('\n', "\r\n")
        } else {
            dump
        };
        assert_eq!(String::from_utf8(bytes).unwrap(), expected);
    }

    #[test]
    fn test_save_json_failure_returns_false() {
        // 目标是已存在的目录：os.replace 必然失败，Python 返回 False
        let (dir, _m) = mgr("fail");
        let blocker = dir.join("blocked");
        fs::create_dir_all(&blocker).unwrap();
        fs::create_dir_all(blocker.join("inner")).unwrap();
        assert_eq!(save_json(blocker.to_str().unwrap(), &JVal::obj()), false);
    }

    #[test]
    fn test_load_json_directory_is_default() {
        let (dir, _m) = mgr("isdir");
        assert_eq!(load_json(&dir.to_string_lossy(), JVal::i(7)), JVal::Int(7));
    }

    #[test]
    fn test_load_json_missing_file_logs_nothing() {
        // 实测 CPython：utils.load_json('.../nope.json', {'d':1}) -> RESULT= {'d': 1}，
        // stderr 里没有任何 WARNING:root: 行（isfile 检查在 try 之前）。
        let (dir, _m) = mgr("silent_missing");
        let missing = dir.join("never-written.json");
        let lines = std::cell::RefCell::new(Vec::new());
        let got = load_json_logged(
            missing.to_str().unwrap(),
            JVal::i(7),
            &|line: &str| lines.borrow_mut().push(line.to_string()),
        );
        assert_eq!(got, JVal::Int(7));
        assert!(lines.borrow().is_empty(), "missing file must be silent, got {:?}", *lines.borrow());
        // 首次启动的真实读取点（settings）同样必须静音。
        let (_d2, m2) = mgr("silent_first_launch");
        assert_eq!(m2.load_geometry().unwrap().width, 1080);
    }

    #[test]
    fn test_load_json_directory_logs_nothing() {
        // os.path.isfile(目录) == False -> 同样静默 default。
        let (dir, _m) = mgr("silent_dir");
        let lines = std::cell::RefCell::new(Vec::new());
        let got = load_json_logged(
            dir.to_str().unwrap(),
            JVal::i(7),
            &|line: &str| lines.borrow_mut().push(line.to_string()),
        );
        assert_eq!(got, JVal::Int(7));
        assert!(lines.borrow().is_empty(), "a directory path must be silent, got {:?}", *lines.borrow());
        assert!(!py_isfile(dir.to_str().unwrap()));
    }

    #[test]
    fn test_load_json_corrupt_file_still_warns() {
        // utils.py:51 只在真的读/解析失败时 warn，且只 warn 一次。
        let (dir, _m) = mgr("warn_corrupt");
        let bad = dir.join("bad.json");
        fs::write(&bad, "{oops").unwrap();
        let lines = std::cell::RefCell::new(Vec::new());
        let got = load_json_logged(
            bad.to_str().unwrap(),
            JVal::i(7),
            &|line: &str| lines.borrow_mut().push(line.to_string()),
        );
        assert_eq!(got, JVal::Int(7));
        let logged = lines.borrow();
        assert_eq!(logged.len(), 1, "corrupt file must warn exactly once: {logged:?}");
        assert!(
            logged[0].starts_with(&format!("读取 JSON 失败 {}: ", bad.to_string_lossy())),
            "unexpected warning: {:?}",
            logged[0]
        );
    }

    #[test]
    fn test_replace_retryable_is_permission_error_codes_only() {
        // 5 = ERROR_ACCESS_DENIED、32 = ERROR_SHARING_VIOLATION 都会以 PermissionError
        // 冒出来（本机实测 winerror=5 errno=13；32 是 Python 文档化的 WinError 32），
        // 33 = ERROR_LOCK_VIOLATION 映射到 EDEADLOCK(36) -> 普通 OSError，Python 不重试。
        assert!(replace_retryable(Some(5)));
        assert!(replace_retryable(Some(32)));
        assert!(!replace_retryable(Some(33)));
        assert!(!replace_retryable(Some(2)));
        assert!(!replace_retryable(Some(18)));
        assert!(!replace_retryable(None));
    }

    #[test]
    fn test_parser_literal_advances_by_code_points() {
        // Vec<char> 缓冲区上的游标必须以码点计，word.len() 是 UTF-8 字节数。
        let mut p = JsonParser { c: "nuléx".chars().collect(), i: 0 };
        assert!(p.literal("nulé"));
        assert_eq!(p.i, 4, "4 code points, not the 5 UTF-8 bytes of \"nulé\"");
        assert_eq!(p.peek(), Some('x'));
        let mut q = JsonParser { c: "中文tail".chars().collect(), i: 0 };
        assert!(q.literal("中文"));
        assert_eq!(q.i, 2);
        assert_eq!(q.peek(), Some('t'));
        // ASCII 关键字的前进量与修复前完全一致（word.len() == chars().count()）。
        let mut r = JsonParser { c: "true,false".chars().collect(), i: 0 };
        assert!(r.literal("true"));
        assert_eq!(r.i, 4);
        assert!(!r.literal("false"), "c[4] is ',' so the keyword does not match");
        assert_eq!(r.i, 4, "a non-matching literal must not move the cursor");
    }

    // ------------------------------------------------ Unicode 数字表

    #[test]
    fn test_unicode_decimal_digits() {
        let rows: &[(&str, i128)] = &[
            ("٠٩", 9),
            ("١٢", 12),
            ("١٢٣", 123),
            ("۵۶", 56),
            ("૭", 7),
            ("๗", 7),
            ("৭", 7),
            ("１２", 12),
        ];
        for (text, want) in rows {
            assert_eq!(py_int_str(text).unwrap(), *want, "int({text:?})");
        }
        assert_eq!(DECIMAL_RUNS.len(), 66);
        assert_eq!(SPACE_RANGES.len(), 10);
    }

    #[test]
    fn test_generated_tables_are_consistent() {
        // 每一段必须恰好覆盖 0..9，且空白表不含 ASCII 之外的 C0 控制符误判
        for (start, len) in DECIMAL_RUNS.iter() {
            assert!(*len <= 10);
        }
        assert!(is_unicode_space('\u{85}'));
        assert!(is_unicode_space('\u{a0}'));
        assert!(!is_unicode_space('\u{200b}'));
        assert!(is_unicode_space('\u{1c}')); // Py_UNICODE_ISSPACE 含 0x1C..0x1F
        assert!(is_c_locale_space('\u{0b}'));
        assert!(!is_c_locale_space('\u{85}'));
    }
}
