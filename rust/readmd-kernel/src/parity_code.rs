//! `/api/code/run` 的 HTTP 边界（P7 parity 包）。
//!
//! 权威实现：`readmd.py::Handler._api_code_run`（第 1604-1644 行）。本文件只负责
//! 请求闸门与响应信封，真正的执行交给 [`super::execute_code_chunk_dict`]。
//!
//! Python 侧逐条语义（顺序即优先级）：
//! 1. `Content-Length` 用 `int()` 解析，解析失败 → 走 except → 500 `execution_failed`；
//! 2. `n < 0 || n > 256 * 1024` → 413 `request_too_large`（Python 同时关闭连接）；
//! 3. `n == 0` → body 视为 `{}`；否则按 UTF-8 严格解码 + `json.loads`，任何异常 → 500；
//! 4. body 不是 dict → 400 `invalid_request`；
//! 5. `confirm` 不是 JSON 真正的 `true` → 400 `confirmation_required`（现状唯一 MATCH 项）；
//! 6. `lang` 缺省 `"python"`、`code` 缺省 `""`，二者非字符串 → 400 `invalid_request`；
//! 7. `len(code) > 200_000` 字符 → 413 `code_too_large`；
//! 8. `cwd = body.get('cwd') or None`（假值一律回落沙箱），`int(body.get('timeout', 10))`
//!    失败 → 500；
//! 9. `ok` 为假时补 `error_code`（已知码直传，含「超时」→ `execution_timeout`，
//!    否则 `execution_failed`），并弹出 `error` / `stderr` 两个诊断键；
//! 10. 其它一切情况 → 200。

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::server::{Request, Response};
use crate::ApiResult;

/// Python: `256 * 1024`。
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
/// Python: `len(code) > 200_000`。
pub const MAX_CODE_CHARS: usize = 200_000;

/// `_api_code_run` 里那组「已知诊断文本可直升为 error_code」的白名单。
const KNOWN_ERROR_CODES: [&str; 5] = [
    "network_not_allowed",
    "path_access_not_allowed",
    "cwd_not_found",
    "cwd_not_allowed",
    "output_truncated",
];

/// 接线后的入口（`server::ROUTES` 指向这里）。
pub fn h_code_run(_app: &Arc<crate::App>, req: &Request) -> ApiResult<Response> {
    Ok(code_run_response(req))
}

/// 独立出来便于单测：永不返回 `Err`，所有失败都用 Python 自己的信封表达。
pub fn code_run_response(req: &Request) -> Response {
    // 1) Content-Length —— `int(self.headers.get('Content-Length', 0) or 0)`
    let declared = match py_int_from_str(req.header("content-length").unwrap_or("0")) {
        Some(n) => n,
        None => return internal_failed(),
    };
    // 2) 体积闸门
    if declared < 0 || declared as usize > MAX_REQUEST_BYTES {
        return api_error(413, "request_too_large");
    }

    // 3) 解析 body
    let body: Value = if declared == 0 {
        json!({})
    } else if req.body.len() as i64 != declared {
        // `_read_request_body_limited` 的 incomplete_request
        return internal_failed();
    } else {
        match serde_json::from_slice::<Value>(&req.body) {
            Ok(value) => value,
            Err(_) => return internal_failed(),
        }
    };

    // 4) 必须是对象
    let Some(obj) = body.as_object() else {
        return api_error(400, "invalid_request");
    };

    // 5) confirm 必须是真正的 true
    if obj.get("confirm") != Some(&Value::Bool(true)) {
        return api_error(400, "confirmation_required");
    }

    // 6) lang / code 的类型闸门
    let lang: Cow<'_, str> = match obj.get("lang") {
        None => Cow::Borrowed("python"),
        Some(Value::String(s)) => Cow::Borrowed(s.as_str()),
        Some(_) => return api_error(400, "invalid_request"),
    };
    let code: Cow<'_, str> = match obj.get("code") {
        None => Cow::Borrowed(""),
        Some(Value::String(s)) => Cow::Borrowed(s.as_str()),
        Some(_) => return api_error(400, "invalid_request"),
    };

    // 7) 源码长度
    if code.chars().count() > MAX_CODE_CHARS {
        return api_error(413, "code_too_large");
    }

    // 8) cwd 与 timeout
    let cwd_text = match obj.get("cwd") {
        Some(value) if py_truthy(value) => Some(py_str(value)),
        _ => None,
    };
    let timeout_value = obj.get("timeout").cloned().unwrap_or_else(|| json!(10));
    let Some(timeout) = py_int(&timeout_value) else {
        return internal_failed();
    };
    let effective = effective_timeout(timeout);

    // 9) 执行 + 边界清洗
    let mut res = match super::execute_code_chunk_dict(
        &code,
        &lang,
        true,
        effective,
        cwd_text.as_ref().map(|p| Path::new(p.as_str())),
    ) {
        Ok(map) => map,
        Err(_) => return internal_failed(),
    };

    let ok = res.get("ok").map(py_truthy).unwrap_or(false);
    if !ok {
        let raw_error = match res.get("error") {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            Some(value) if !value.is_null() => py_str(value),
            _ => String::new(),
        };
        if !res.contains_key("error_code") {
            let code = if KNOWN_ERROR_CODES.contains(&raw_error.as_str()) {
                raw_error
            } else if raw_error.contains("超时") {
                "execution_timeout".to_string()
            } else {
                "execution_failed".to_string()
            };
            res.insert("error_code".to_string(), Value::String(code));
        }
        res.remove("error");
        res.remove("stderr");
    }

    Response::json_status(200, &Value::Object(res))
}

fn api_error(status: u16, error_code: &str) -> Response {
    Response::json_status(status, &json!({ "ok": false, "error_code": error_code }))
}

/// `except Exception: self._send_json(500, {'ok': False, 'error_code': 'execution_failed'})`
fn internal_failed() -> Response {
    api_error(500, "execution_failed")
}

/// `max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS))`。
/// 负数在 Python 里落到 1，0 会先被 `or` 换成 10。
fn effective_timeout(timeout: i64) -> u64 {
    if timeout == 0 {
        return super::EXECUTION_TIMEOUT;
    }
    if timeout < 0 {
        return 1;
    }
    timeout.min(super::MAX_TIMEOUT_SECONDS as i64) as u64
}

/// Python 真值判定。
fn py_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Python `str(value)` 的够用近似（只用于非字符串 cwd 这类必然被拒的输入）。
fn py_str(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            format!("[{}]", items.iter().map(py_str).collect::<Vec<_>>().join(", "))
        }
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("'{k}': {}", py_str(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Python `int(value)`：bool 直转、float 向零截断、字符串按十进制（允许下划线分隔）。
fn py_int(value: &Value) -> Option<i64> {
    match value {
        Value::Bool(b) => Some(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return Some(i);
            }
            let f = n.as_f64()?;
            if !f.is_finite() {
                return None; // int(inf) → OverflowError
            }
            Some(f.trunc().max(i64::MIN as f64).min(i64::MAX as f64) as i64)
        }
        Value::String(s) => py_int_from_str(s),
        _ => None,
    }
}

/// `int(" 10 ")`、`int("1_0")` 合法，`int("")`、`int("0x10")`、`int("_1")` 抛错。
fn py_int_from_str(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if digits.is_empty() {
        return None;
    }
    let mut mantissa = String::new();
    let mut after_underscore = true; // 前导下划线非法
    for ch in digits.chars() {
        if ch == '_' {
            if after_underscore {
                return None;
            }
            after_underscore = true;
            continue;
        }
        if !ch.is_ascii_digit() {
            return None;
        }
        mantissa.push(ch);
        after_underscore = false;
    }
    if after_underscore || mantissa.is_empty() {
        return None;
    }
    let trimmed_digits = mantissa.trim_start_matches('0');
    if trimmed_digits.len() > 18 {
        // 远超 i64：Python 得到一个大整数，钳位后必然落在 10 或 1。
        return Some(if negative { i64::MIN } else { i64::MAX });
    }
    let padded = if trimmed_digits.is_empty() { "0" } else { trimmed_digits };
    let magnitude: i64 = padded.parse().ok()?;
    Some(if negative { magnitude.checked_neg()? } else { magnitude })
}

/// 只为方便测试断言键集合。
#[cfg(test)]
fn keys_of(response: &Response) -> Vec<String> {
    let value: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
    let mut keys: Vec<String> = match &value {
        Value::Object(map) => map.keys().cloned().collect(),
        _ => return vec![],
    };
    keys.sort();
    keys
}

#[cfg(test)]
fn body_of(response: &Response) -> Value {
    serde_json::from_slice(&response.body).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use std::collections::HashMap;

    fn post(body: &str) -> Request {
        Request {
            method: "POST".into(),
            path: "/api/code/run".into(),
            query: HashMap::new(),
            headers: HashMap::from([("content-length".to_string(), body.len().to_string())]),
            body: body.as_bytes().to_vec(),
        }
    }

    fn json_post(payload: Value) -> Request {
        post(&payload.to_string())
    }

    fn interpreter_ready() -> bool {
        ["python", "python3", "py"]
            .iter()
            .any(|name| super::super::which(name).is_some())
    }

    fn cheap_shell() -> Option<(&'static str, &'static str)> {
        if cfg!(target_os = "windows") {
            Some(("cmd", "@echo off & echo 42"))
        } else if super::super::which("sh").is_some() {
            Some(("sh", "echo 42"))
        } else {
            None
        }
    }

    // ---------------------------------------------------------------- allow

    #[test]
    fn allowed_run_returns_pythons_seven_keys() {
        let Some((lang, code)) = cheap_shell() else { return };
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": lang, "code": code
        })));
        assert_eq!(res.status, 200, "Python 允许的运行一律 200");
        assert_eq!(
            keys_of(&res),
            vec!["exit_code", "images", "lang", "ok", "stderr", "stdout", "warning"],
            "响应键集合必须与 Python 完全一致"
        );
        let body = body_of(&res);
        assert_eq!(body["ok"], json!(true), "{body}");
        assert_eq!(body["stdout"], json!("42"), "{body}");
        assert_eq!(body["exit_code"], json!(0));
        assert_eq!(body["lang"], json!(lang));
        assert!(body["warning"].is_null(), "未截断时 warning 必须是 null 而非缺席");
        assert!(body["images"].is_array());
        // 关键：允许路径不得泄露 error / error_code。
        assert!(body.get("error").is_none() && body.get("error_code").is_none());
    }

    #[test]
    fn python_lang_defaults_when_key_absent() {
        if !interpreter_ready() {
            return;
        }
        let res = code_run_response(&json_post(json!({"confirm": true, "code": "print(6*7)"})));
        let body = body_of(&res);
        assert_eq!(body["ok"], json!(true), "{body}");
        assert_eq!(body["stdout"], json!("42"));
        assert_eq!(body["lang"], json!("python"));
    }

    #[test]
    fn frontend_language_key_is_not_a_body_contract() {
        // corpus 用的是 `language`，Python 只读 `lang`，因此它等同「缺省 python」。
        if !interpreter_ready() {
            return;
        }
        let res = code_run_response(&json_post(json!({
            "confirm": true, "language": "node", "code": "print(1)"
        })));
        assert_eq!(body_of(&res)["lang"], json!("python"), "不得私扩键名契约");
    }

    // ---------------------------------------------------------------- deny

    #[test]
    fn network_deny_keeps_403_class_semantics_and_hides_stderr() {
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "import requests"
        })));
        assert_eq!(res.status, 200, "Python 的拒绝走 200 + error_code");
        assert_eq!(
            keys_of(&res),
            vec!["error_code", "exit_code", "images", "lang", "ok", "stdout"]
        );
        let body = body_of(&res);
        assert_eq!(body["error_code"], json!("network_not_allowed"));
        assert_eq!(body["ok"], json!(false));
        assert_eq!(body["exit_code"], json!(1));
        assert_eq!(body["stdout"], json!(""));
    }

    #[test]
    fn path_escape_deny_reports_path_access_not_allowed() {
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "open('/etc/passwd').read()"
        })));
        assert_eq!(res.status, 200);
        assert_eq!(body_of(&res)["error_code"], json!("path_access_not_allowed"));
        assert_eq!(
            keys_of(&res),
            vec!["error_code", "exit_code", "images", "lang", "ok", "stdout"]
        );
    }

    #[test]
    fn unsupported_language_is_execution_failed_not_4xx() {
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "brainfuck", "code": "++"
        })));
        assert_eq!(res.status, 200);
        let body = body_of(&res);
        assert_eq!(body["error_code"], json!("execution_failed"));
        assert_eq!(body["lang"], json!("brainfuck"));
        assert_eq!(
            keys_of(&res),
            vec!["error_code", "exit_code", "images", "lang", "ok", "stdout"]
        );
    }

    // ------------------------------------------------------------- confirm

    #[test]
    fn confirm_gate_is_strict_boolean_true() {
        for payload in [
            json!({"lang": "python", "code": "print(1)"}),
            json!({"confirm": false, "lang": "python", "code": "print(1)"}),
            json!({"confirm": 1, "lang": "python", "code": "print(1)"}),
            json!({"confirm": "true", "lang": "python", "code": "print(1)"}),
            json!({"confirm": null, "lang": "python", "code": "print(1)"}),
            json!({}),
        ] {
            let res = code_run_response(&json_post(payload.clone()));
            assert_eq!(res.status, 400, "{payload} 必须被 confirm 闸门拒绝");
            assert_eq!(
                keys_of(&res),
                vec!["error_code", "ok"],
                "确认门信封只能有 ok + error_code"
            );
            assert_eq!(body_of(&res)["error_code"], json!("confirmation_required"));
            assert_eq!(body_of(&res)["ok"], json!(false));
        }
    }

    #[test]
    fn confirm_gate_runs_before_any_language_check() {
        // 未确认时即使 lang 是非法类型也先报 confirmation_required。
        let res = code_run_response(&json_post(json!({"lang": 42, "code": "print(1)"})));
        assert_eq!(res.status, 400);
        assert_eq!(body_of(&res)["error_code"], json!("confirmation_required"));
    }

    #[test]
    fn empty_body_defaults_to_no_confirm() {
        let res = code_run_response(&post(""));
        assert_eq!(res.status, 400);
        assert_eq!(body_of(&res)["error_code"], json!("confirmation_required"));
    }

    #[test]
    fn zero_content_length_ignores_the_body_like_python() {
        // Python: `json.loads(...) if n else {}` —— 声明 0 长度时根本不看 body。
        let mut req = json_post(json!({"confirm": true, "lang": "python", "code": "print(1)"}));
        req.headers.insert("content-length".into(), "0".into());
        let res = code_run_response(&req);
        assert_eq!(res.status, 400);
        assert_eq!(body_of(&res)["error_code"], json!("confirmation_required"));
    }

    // ------------------------------------------------------------- request

    #[test]
    fn non_object_body_is_invalid_request() {
        for raw in [
            "[1, 2]",
            "\"print(1)\"",
            "42",
            "null",
            "{not json",
        ] {
            let res = code_run_response(&post(raw));
            // 语法非法 → 500 execution_failed；合法但不是 dict → 400 invalid_request
            if raw == "{not json" {
                assert_eq!(res.status, 500, "{raw}");
                assert_eq!(body_of(&res)["error_code"], json!("execution_failed"));
            } else {
                assert_eq!(res.status, 400, "{raw}");
                assert_eq!(body_of(&res)["error_code"], json!("invalid_request"));
            }
        }
    }

    #[test]
    fn lang_and_code_must_be_strings() {
        for payload in [
            json!({"confirm": true, "lang": 12, "code": "print(1)"}),
            json!({"confirm": true, "lang": null, "code": "print(1)"}),
            json!({"confirm": true, "lang": "python", "code": ["print(1)"]}),
            json!({"confirm": true, "lang": "python", "code": 1}),
        ] {
            let res = code_run_response(&json_post(payload.clone()));
            assert_eq!(res.status, 400, "{payload}");
            assert_eq!(body_of(&res)["error_code"], json!("invalid_request"));
        }
    }

    #[test]
    fn oversized_body_is_request_too_large() {
        let big = "x".repeat(MAX_CODE_CHARS + 100_000);
        let req = json_post(json!({"confirm": true, "lang": "python", "code": big}));
        assert!(req.body.len() > MAX_REQUEST_BYTES);
        let res = code_run_response(&req);
        assert_eq!(res.status, 413);
        assert_eq!(keys_of(&res), vec!["error_code", "ok"]);
        assert_eq!(body_of(&res)["error_code"], json!("request_too_large"));
    }

    #[test]
    fn oversized_source_is_code_too_large() {
        let big = "x".repeat(MAX_CODE_CHARS + 1);
        let payload = json!({"confirm": true, "lang": "python", "code": big}).to_string();
        assert!(payload.len() < MAX_REQUEST_BYTES, "用例必须落在两个闸门之间");
        let res = code_run_response(&post(&payload));
        assert_eq!(res.status, 413);
        assert_eq!(body_of(&res)["error_code"], json!("code_too_large"));
    }

    #[test]
    fn request_size_gate_precedes_code_size_gate() {
        let res = code_run_response(&post(&json!({
            "confirm": true, "lang": "python", "code": "x".repeat(MAX_CODE_CHARS * 2)
        }).to_string()));
        assert_eq!(res.status, 413);
        assert_eq!(body_of(&res)["error_code"], json!("request_too_large"));
    }

    #[test]
    fn unparsable_content_length_is_internal_failure() {
        let mut req = json_post(json!({"confirm": true, "lang": "python", "code": "print(1)"}));
        req.headers.insert("content-length".into(), "0x10".into());
        let res = code_run_response(&req);
        assert_eq!(res.status, 500);
        assert_eq!(body_of(&res)["error_code"], json!("execution_failed"));
    }

    #[test]
    fn short_body_against_declared_length_is_internal_failure() {
        let mut req = json_post(json!({"confirm": true, "lang": "python", "code": "print(1)"}));
        req.body.truncate(3);
        let res = code_run_response(&req);
        assert_eq!(res.status, 500);
        assert_eq!(body_of(&res)["error_code"], json!("execution_failed"));
    }

    // ------------------------------------------------------------- timeout

    #[test]
    fn timeout_keeps_error_code_and_drops_warning_key() {
        if !interpreter_ready() {
            return;
        }
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python",
            "code": "import time\nwhile True:\n    time.sleep(0.1)\n", "timeout": 1
        })));
        assert_eq!(res.status, 200, "超时仍是 200，错误码承载语义");
        assert_eq!(
            keys_of(&res),
            vec!["error_code", "exit_code", "images", "lang", "ok", "stdout"],
            "超时字典没有 warning 键（与 Python 一致）"
        );
        let body = body_of(&res);
        assert_eq!(body["error_code"], json!("execution_timeout"));
        assert_eq!(body["exit_code"], json!(-1));
        assert_eq!(body["ok"], json!(false));
    }

    #[test]
    fn timeout_upper_bound_and_zero_fallback_match_python() {
        assert_eq!(effective_timeout(0), 10);
        assert_eq!(effective_timeout(-7), 1);
        assert_eq!(effective_timeout(1), 1);
        assert_eq!(effective_timeout(999), 10);
        assert_eq!(effective_timeout(3), 3);
    }

    #[test]
    fn timeout_value_follows_python_int() {
        assert_eq!(py_int(&json!(true)), Some(1));
        assert_eq!(py_int(&json!(2.9)), Some(2));
        assert_eq!(py_int(&json!(-2.9)), Some(-2));
        assert_eq!(py_int(&json!(" 10 ")), Some(10));
        assert_eq!(py_int(&json!("1_0")), Some(10));
        assert_eq!(py_int(&json!("0x10")), None);
        assert_eq!(py_int(&json!("")), None);
        assert_eq!(py_int(&json!("_1")), None);
        assert_eq!(py_int(&json!("1_")), None);
        assert_eq!(py_int(&json!(null)), None);
        assert_eq!(py_int(&json!([1])), None);
        assert_eq!(py_int(&json!({})), None);
        assert_eq!(py_int(&json!(1e40)), Some(i64::MAX));
    }

    #[test]
    fn unparsable_timeout_is_internal_failure() {
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "print(1)", "timeout": null
        })));
        assert_eq!(res.status, 500);
        assert_eq!(body_of(&res)["error_code"], json!("execution_failed"));
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "print(1)", "timeout": "soon"
        })));
        assert_eq!(res.status, 500);
    }

    // ---------------------------------------------------------- truncation

    #[test]
    fn oversized_output_is_truncated_and_warned() {
        if !interpreter_ready() {
            return;
        }
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "print('汉' * 250000)"
        })));
        assert_eq!(res.status, 200);
        let body = body_of(&res);
        assert_eq!(body["ok"], json!(true), "{body}");
        assert_eq!(body["warning"], json!("output_truncated"));
        let stdout = body["stdout"].as_str().unwrap_or("");
        assert_eq!(stdout.chars().count(), super::super::MAX_OUTPUT_CHARS, "按字符而非字节截断");
        assert!(stdout.chars().all(|c| c == '汉'));
        assert_eq!(
            keys_of(&res),
            vec!["exit_code", "images", "lang", "ok", "stderr", "stdout", "warning"]
        );
    }

    // ---------------------------------------------------------------- cwd

    #[test]
    fn cwd_outside_sandbox_is_cwd_not_allowed() {
        let outside = if cfg!(target_os = "windows") { "C:\\Windows" } else { "/etc" };
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "print(1)", "cwd": outside
        })));
        assert_eq!(res.status, 200);
        assert_eq!(body_of(&res)["error_code"], json!("cwd_not_allowed"));
        assert_eq!(
            keys_of(&res),
            vec!["error_code", "exit_code", "images", "lang", "ok", "stdout"]
        );
    }

    #[test]
    fn cwd_missing_directory_is_cwd_not_found() {
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "print(1)",
            "cwd": "/tmp/readmd-no-such-dir-42"
        })));
        assert_eq!(body_of(&res)["error_code"], json!("cwd_not_found"));
    }

    #[test]
    fn falsy_cwd_values_fall_back_to_the_sandbox() {
        for cwd in [json!(null), json!(false), json!(0), json!(""), json!([]), json!({})] {
            let payload = json!({"confirm": true, "lang": "python", "code": "x", "cwd": cwd});
            let res = code_run_response(&json_post(payload));
            // 沙箱可用 → 走到「不支持的语言/字符串 x」语义，而不是 cwd 拒绝。
            let code = body_of(&res)["error_code"].as_str().unwrap_or_default().to_string();
            assert!(
                code != "cwd_not_allowed" && code != "cwd_not_found",
                "假值 cwd 必须回落沙箱，实得 {code}"
            );
        }
    }

    #[test]
    fn truthy_non_string_cwd_is_rejected_like_python() {
        let res = code_run_response(&json_post(json!({
            "confirm": true, "lang": "python", "code": "print(1)", "cwd": true
        })));
        assert_eq!(body_of(&res)["error_code"], json!("cwd_not_found"));
    }

    #[test]
    fn allowed_cwd_inside_temp_is_accepted() {
        let dir = tempfile::Builder::new().prefix("readmd-code-").tempdir().unwrap();
        let res = super::super::allowed_cwd(Some(dir.path())).unwrap();
        assert!(res.is_some(), "临时目录内的 cwd 必须放行");
        assert!(super::super::allowed_cwd(None).unwrap().is_none());
    }

    // ------------------------------------------------------------ helpers

    #[test]
    fn py_str_matches_python_for_cwd_shaped_inputs() {
        assert_eq!(py_str(&json!(true)), "True");
        assert_eq!(py_str(&json!(42)), "42");
        assert_eq!(py_str(&json!([1, 2])), "[1, 2]");
        assert_eq!(py_str(&json!({"a": 1})), "{'a': 1}");
    }

    #[test]
    fn helper_map_of_keys_ignores_order() {
        let res = code_run_response(&json_post(json!({"lang": "python"})));
        assert_eq!(keys_of(&res), vec!["error_code", "ok"]);
        let mut map = Map::new();
        map.insert("b".into(), json!(1));
        assert!(map.contains_key("b"));
    }
}
