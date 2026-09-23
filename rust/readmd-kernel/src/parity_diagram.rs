//! P6 —— `/api/diagram/render` 与 `/api/diagram/capabilities` 的 parity handler。
//!
//! 权威参考：`readmd.py:1879`（`_api_diagram_render`）与 `readmd.py:1942`
//! （`_api_diagram_capabilities`），底层能力全部来自 [`crate::diagrams`]。
//!
//! 复刻要点（与 Python 逐分支对齐，见 `scratch/rust_parity/contract.json`）：
//! * render **不校验方法**（Python 同一 dispatcher 处理 GET/POST），参数只从 JSON body 读
//!   `engine` / `code` / `allow_remote`；body 缺失即 `engine = "mermaid"`。
//! * 任何 JSON 解析失败 / body 不是对象 / `Content-Length` 非法 → Python 落到裸 `except` →
//!   **500 `diagram_render_failed`**（不是 400）。
//! * `engine` 规范化 = `str(body.get('engine','mermaid') or 'mermaid').strip().lower()`。
//! * 分支：puml|plantuml → 本地 java / `allow_remote is True` 出网 / 422 依赖缺失；
//!   tikz → 200 html（**没有** `engine` 键）；vega|vega-lite → 200 svg；
//!   wsd|d2|ditaa → 422 `diagram_engine_unavailable`；其余（mermaid/wavedrom/viz/chart/未知）
//!   → 422 `diagram_client_renderer_required`。绝不把源码当渲染结果回显成 200。
//! * capabilities **只接受 GET**（否则 405 `method_not_allowed`），且只探测随包文件与
//!   可选本地进程，绝不出网。

use crate::diagrams::{self, DiagramError};
use crate::error::ApiResult;
use crate::server::{Request, Response};
use crate::App;
use serde_json::{json, Value};
use std::sync::Arc;

/// Python `_api_diagram_render` 里 422 依赖缺失分支的固定文案（逐字符一致）。
const PLANTUML_REMOTE_REASON: &str =
    "PlantUML 本地环境未就绪。默认禁止静默联网上传图表源码。";

// ------------------------------------------------------------- http handlers

/// `GET /api/diagram/capabilities`（非 GET → 405）。
pub fn h_diagram_capabilities(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(diagram_capabilities_response(req))
}

/// `POST /api/diagram/render`（Python 不校验方法，GET 也走同一分支）。
pub fn h_diagram_render(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(diagram_render_response(req))
}

// ------------------------------------------------------------- capabilities

fn diagram_capabilities_response(req: &Request) -> Response {
    if req.method != "GET" {
        return Response::json_status(405, &json!({ "ok": false, "error_code": "method_not_allowed" }));
    }
    // Python: self._send_json(200, {'ok': True, **diagrams.get_diagram_capabilities()})
    Response::json(&diagrams::capabilities_payload())
}

// -------------------------------------------------------------------- render

fn diagram_render_response(req: &Request) -> Response {
    let body = match parse_json_body(req) {
        Ok(body) => body,
        // Python 的裸 `except Exception` 覆盖 json.loads / int(Content-Length) / body.get。
        Err(_) => return Response::json_status(500, &json!({ "ok": false, "error_code": "diagram_render_failed" })),
    };
    let engine = normalized_engine(&body);
    match engine.as_str() {
        "puml" | "plantuml" => render_plantuml(&body, &engine),
        "tikz" => {
            let html = diagrams::format_tikz_html(&code_or_empty(&body));
            Response::json(&json!({ "ok": true, "type": "html", "html": html }))
        }
        "vega" | "vega-lite" => match diagrams::render_vega_svg(
            &code_or_empty(&body),
            &engine,
            diagrams::VEGA_TIMEOUT,
        ) {
            Ok(svg) => Response::json(&json!({
                "ok": true,
                "type": "svg",
                "svg": svg,
                "engine": engine
            })),
            Err(err) => diagram_error_response(err),
        },
        "wsd" | "d2" | "ditaa" => Response::json_status(
            422,
            &json!({ "ok": false, "error_code": "diagram_engine_unavailable", "engine": engine }),
        ),
        _ => Response::json_status(
            422,
            &json!({
                "ok": false,
                "error_code": "diagram_client_renderer_required",
                "engine": engine,
            }),
        ),
    }
}

/// PlantUML 双通道：本地运行时优先，联网只允许显式 `allow_remote: true`。
fn render_plantuml(body: &Value, engine: &str) -> Response {
    if diagrams::has_local_plantuml() {
        // Python 的 render_plantuml_svg 自己做 `str(code or '')` 与 @startuml 包裹。
        let rendered = diagrams::render_plantuml_svg(&code_or_empty(body), diagrams::PLANTUML_TIMEOUT);
        return match rendered {
            Ok(svg) => Response::json(&json!({
                "ok": true,
                "type": "svg",
                "svg": svg,
                "engine": engine,
                "requires_network": false
            })),
            Err(err) => diagram_error_response(err),
        };
    }
    if !remote_allowed(body) {
        return Response::json_status(
            422,
            &json!({
                "ok": false,
                "error_code": "diagram_dependency_missing",
                "remote_available": true,
                "requires_confirmation": true,
                "reason": PLANTUML_REMOTE_REASON
            }),
        );
    }
    // 只有字面 true 才会出网；到这里才允许触碰网络栈。
    let code = match body.get("code") {
        None => String::new(),
        Some(Value::String(text)) => text.clone(),
        // Python: get_plantuml_svg_url 对非字符串调 .strip() -> AttributeError -> 500。
        Some(_) => return diagram_error_response(DiagramError::Fatal),
    };
    match diagrams::fetch_plantuml_svg(&code, diagrams::PLANTUML_TIMEOUT) {
        Ok(svg) => Response::json(&json!({
            "ok": true,
            "type": "svg",
            "svg": svg,
            "engine": engine,
            "requires_network": true
        })),
        Err(err) => diagram_error_response(err),
    }
}

fn diagram_error_response(err: DiagramError) -> Response {
    match err {
        DiagramError::Expected(code) => Response::json_status(
            422,
            &json!({ "ok": false, "error_code": if code.is_empty() { "diagram_render_failed".to_string() } else { code } }),
        ),
        DiagramError::Fatal => {
            Response::json_status(500, &json!({ "ok": false, "error_code": "diagram_render_failed" }))
        }
    }
}

// ------------------------------------------------------------ body 解析辅助

/// Python `n = int(headers.get('Content-Length', 0) or 0)` + `json.loads(read(n).decode('utf-8'))`。
///
/// 任何一步失败都返回 `Err(())`，由调用方映射成 500 `diagram_render_failed`。
fn parse_json_body(req: &Request) -> Result<Value, ()> {
    let declared = match req.header("content-length") {
        Some(value) => value,
        None => return Ok(json!({})),
    };
    let length = if declared.is_empty() {
        0i64
    } else {
        python_int(declared).ok_or(())?
    };
    if length == 0 {
        return Ok(json!({}));
    }
    // Python 用 `rfile.read(n)`；内核已按同一 Content-Length 读完 body，
    // 声明长度超过实到长度时 Python 会一直阻塞（Rust 改为按实到字节解析）。
    let take = if length < 0 {
        req.body.len()
    } else {
        (length as usize).min(req.body.len())
    };
    // Python `bytes.decode('utf-8')` 严格解码，坏字节 -> UnicodeDecodeError -> 500。
    let text = std::str::from_utf8(&req.body[..take]).map_err(|_| ())?;
    let value: Value = serde_json::from_str(text).map_err(|_| ())?;
    if !value.is_object() {
        // Python 对 list/str/int body 调 `.get` -> AttributeError -> 500。
        return Err(());
    }
    Ok(value)
}

/// `int(str)`：允许前后空白、正负号与数字间下划线，其它一律失败。
fn python_int(text: &str) -> Option<i64> {
    let trimmed = diagrams::py_strip(text);
    let (sign, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    let mut previous_digit = false;
    for ch in digits.chars() {
        if ch == '_' {
            if !previous_digit {
                return None;
            }
            previous_digit = false;
            continue;
        }
        let digit = ch.to_digit(10)?;
        value = value.checked_mul(10)?.checked_add(digit as i64)?;
        previous_digit = true;
    }
    if !previous_digit {
        return None;
    }
    Some(sign * value)
}

/// `str(body.get('engine', 'mermaid') or 'mermaid').strip().lower()`。
fn normalized_engine(body: &Value) -> String {
    let default = json!("mermaid");
    let raw = match body.get("engine") {
        Some(value) if python_truthy(value) => value,
        _ => &default,
    };
    diagrams::py_strip(&python_str(raw)).to_lowercase()
}

/// `body.get('code', '')` 之后所有消费点都写作 `str(code or '')`。
fn code_or_empty(body: &Value) -> String {
    match body.get("code") {
        Some(value) if python_truthy(value) => python_str(value),
        _ => String::new(),
    }
}

/// `body.get('allow_remote') is True`：数字 1、字符串 "true" 都不算授权。
fn remote_allowed(body: &Value) -> bool {
    matches!(body.get("allow_remote"), Some(Value::Bool(true)))
}

fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => match number.as_f64() {
            Some(double) => double != 0.0,
            None => true,
        },
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Python `str(json_value)`：容器走 `repr()`，布尔走 `True/False`。
fn python_str(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => if *flag { "True" } else { "False" }.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        other => python_repr(other),
    }
}

fn python_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => if *flag { "True" } else { "False" }.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => python_repr_str(text),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(python_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Object(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("{}: {}", python_repr_str(key), python_repr(item)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

fn python_repr_str(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn request(method: &str, body: &str) -> Request {
        let mut headers = HashMap::new();
        if !body.is_empty() {
            headers.insert("content-length".to_string(), body.len().to_string());
        }
        Request {
            method: method.to_string(),
            path: "/api/diagram/render".to_string(),
            query: HashMap::new(),
            headers,
            body: body.as_bytes().to_vec(),
        }
    }

    fn render(body: &str) -> Response {
        diagram_render_response(&request("POST", body))
    }

    fn payload(res: &Response) -> Value {
        serde_json::from_slice(&res.body).expect("json response")
    }

    fn keys(res: &Response) -> Vec<String> {
        let mut names: Vec<String> = payload(res)
            .as_object()
            .expect("object body")
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    #[test]
    fn unknown_engine_is_422_client_renderer_required() {
        // 这些引擎由浏览器的离线 dispatcher 渲染，HTTP 端点绝不能回 200 假绿。
        for (engine, echoed) in [
            ("mermaid", "mermaid"),
            ("wavedrom", "wavedrom"),
            ("bitfield", "bitfield"),
            ("viz", "viz"),
            ("dot", "dot"),
            ("chart", "chart"),
            ("totally-unknown", "totally-unknown"),
        ] {
            let body = format!(r#"{{"engine":"{engine}","code":"graph TD;A-->B;"}}"#);
            let res = render(&body);
            assert_eq!(res.status, 422, "engine {engine}");
            assert_eq!(
                payload(&res),
                json!({ "ok": false, "error_code": "diagram_client_renderer_required", "engine": echoed })
            );
            assert_eq!(keys(&res), vec!["engine", "error_code", "ok"]);
        }
    }

    #[test]
    fn engine_defaults_and_normalization_match_python() {
        // 缺 engine / null / false / "" / 0 / [] 全部落到 "mermaid"。
        for body in [
            r#"{"code":"x"}"#,
            r#"{"engine":null}"#,
            r#"{"engine":false}"#,
            r#"{"engine":""}"#,
            r#"{"engine":0}"#,
            r#"{"engine":[]}"#,
            r#"{"engine":{}}"#,
        ] {
            let res = render(body);
            assert_eq!(res.status, 422, "{body}");
            assert_eq!(payload(&res)["engine"], json!("mermaid"), "{body}");
            assert_eq!(payload(&res)["error_code"], json!("diagram_client_renderer_required"));
        }
        // 大小写与空白规范化（与本机是否有 PlantUML 无关的引擎）。
        let wsd = render(r#"{"engine":"  WSD ","code":"x"}"#);
        assert_eq!(payload(&wsd)["engine"], json!("wsd"));
        assert_eq!(payload(&wsd)["error_code"], json!("diagram_engine_unavailable"));
        let mixed = render(r#"{"engine":"  MeRmaId "}"#);
        assert_eq!(payload(&mixed)["engine"], json!("mermaid"));
        assert_eq!(
            payload(&mixed)["error_code"],
            json!("diagram_client_renderer_required")
        );
        assert_eq!(payload(&render(r#"{"engine":5}"#))["engine"], json!("5"));
        assert_eq!(payload(&render(r#"{"engine":true}"#))["engine"], json!("true"));
        assert_eq!(payload(&render(r#"{"engine":[]}"#))["engine"], json!("mermaid"));
    }

    #[test]
    fn unavailable_engines_report_engine_unavailable() {
        for engine in ["wsd", "d2", "ditaa"] {
            let res = render(&format!(r#"{{"engine":"{engine}","code":"x"}}"#));
            assert_eq!(res.status, 422, "{engine}");
            assert_eq!(
                payload(&res),
                json!({ "ok": false, "error_code": "diagram_engine_unavailable", "engine": engine })
            );
        }
    }

    #[test]
    fn plantuml_without_local_runtime_never_leaves_the_machine() {
        // 本机没有本地 PlantUML 时，未授权出网必须是 422 + 五个键的确认门。
        if diagrams::has_local_plantuml() {
            return;
        }
        for body in [
            r#"{"engine":"plantuml","code":"@startuml\nA->B\n@enduml"}"#,
            r#"{"engine":"puml","code":"A->B","allow_remote":false}"#,
            r#"{"engine":"puml","code":"A->B","allow_remote":1}"#,
            r#"{"engine":"puml","code":"A->B","allow_remote":"true"}"#,
            r#"{"engine":"puml","code":"A->B","allow_remote":{"a":1}}"#,
        ] {
            let res = render(body);
            assert_eq!(res.status, 422, "{body}");
            assert_eq!(
                payload(&res),
                json!({
                    "ok": false,
                    "error_code": "diagram_dependency_missing",
                    "remote_available": true,
                    "requires_confirmation": true,
                    "reason": PLANTUML_REMOTE_REASON
                })
            );
            assert_eq!(keys(&res), vec!["error_code", "ok", "reason", "remote_available", "requires_confirmation"]);
        }
    }

    #[test]
    fn plantuml_non_string_code_is_fatal_not_network() {
        // allow_remote: true 但 code 非字符串：Python 在 URL 构造处 AttributeError -> 500。
        if diagrams::has_local_plantuml() {
            return;
        }
        for body in [
            r#"{"engine":"puml","code":5,"allow_remote":true}"#,
            r#"{"engine":"puml","code":null,"allow_remote":true}"#,
            r#"{"engine":"puml","code":[1],"allow_remote":true}"#,
        ] {
            let res = render(body);
            assert_eq!(res.status, 500, "{body}");
            assert_eq!(
                payload(&res),
                json!({ "ok": false, "error_code": "diagram_render_failed" })
            );
        }
    }

    #[test]
    fn tikz_returns_html_without_engine_key() {
        let res = render(r#"{"engine":"tikz","code":"a</script>b"}"#);
        assert_eq!(res.status, 200);
        assert_eq!(keys(&res), vec!["html", "ok", "type"]);
        assert_eq!(payload(&res)["type"], json!("html"));
        assert_eq!(
            payload(&res)["html"],
            json!("<script type=\"text/tikz\">\n\\begin{tikzpicture}\na<\\/script>b\n\\end{tikzpicture}\n</script>")
        );
        // code 缺失与 falsy code 都按 `str(code or '')` 处理。
        assert_eq!(
            payload(&render(r#"{"engine":"tikz"}"#))["html"],
            json!("<script type=\"text/tikz\">\n\\begin{tikzpicture}\n\n\\end{tikzpicture}\n</script>")
        );
        assert_eq!(
            payload(&render(r#"{"engine":"tikz","code":0}"#))["html"],
            json!("<script type=\"text/tikz\">\n\\begin{tikzpicture}\n\n\\end{tikzpicture}\n</script>")
        );
    }

    #[test]
    fn malformed_bodies_are_500_not_400() {
        for body in [
            "{",
            "[]",
            "\"text\"",
            "3",
            "null",
            "   ",
            r#"{"engine":"mermaid","code":""}"#, // 合法：不是这里断言的 500
        ] {
            let res = render(body);
            if body.contains("mermaid") {
                assert_eq!(res.status, 422);
                continue;
            }
            assert_eq!(res.status, 500, "body {body:?}");
            assert_eq!(
                payload(&res),
                json!({ "ok": false, "error_code": "diagram_render_failed" }),
                "body {body:?}"
            );
            assert_eq!(keys(&res), vec!["error_code", "ok"]);
        }
    }

    #[test]
    fn empty_body_defaults_to_mermaid() {
        // 无 Content-Length -> Python body = {} -> mermaid 客户端渲染门。
        let mut req = request("GET", "");
        req.path = "/api/diagram/render".to_string();
        let res = diagram_render_response(&req);
        assert_eq!(res.status, 422);
        assert_eq!(
            payload(&res),
            json!({ "ok": false, "error_code": "diagram_client_renderer_required", "engine": "mermaid" })
        );
    }

    #[test]
    fn bad_content_length_is_500() {
        let mut req = request("POST", "{\"engine\":\"tikz\"}");
        req.headers
            .insert("content-length".to_string(), "abc".to_string());
        let res = diagram_render_response(&req);
        assert_eq!(res.status, 500);
        assert_eq!(payload(&res)["error_code"], json!("diagram_render_failed"));

        let mut req = request("POST", "{\"engine\":\"tikz\"}");
        req.headers.insert("content-length".to_string(), " ".to_string());
        assert_eq!(diagram_render_response(&req).status, 500);

        let mut req = request("POST", "{\"engine\":\"tikz\"}");
        req.headers.insert("content-length".to_string(), "".to_string());
        assert_eq!(diagram_render_response(&req).status, 422); // -> 0 -> {} -> mermaid
    }

    #[test]
    fn capabilities_method_and_shape() {
        let posted = diagram_capabilities_response(&request("POST", ""));
        assert_eq!(posted.status, 405);
        assert_eq!(payload(&posted), json!({ "ok": false, "error_code": "method_not_allowed" }));

        let got = diagram_capabilities_response(&request("GET", ""));
        assert_eq!(got.status, 200);
        let body = payload(&got);
        assert_eq!(keys(&got), vec!["engines", "offline", "ok", "schema_version"]);
        assert_eq!(body["ok"], json!(true));
        assert_eq!(body["schema_version"], json!(1));
        assert!(body["offline"].is_boolean());

        let engines = body["engines"].as_object().expect("engines");
        // 随包浏览器资产：路径存在 => available && offline，且 requires_network 恒为 false。
        let root = diagrams::repo_root();
        let vendor = root.join("assets").join("vendor").join("diagrams");
        let browser: [(&str, &str); 6] = [
            ("mermaid", "mermaid/mermaid.min.js"),
            ("wavedrom", "wavedrom/wavedrom.min.js"),
            ("bitfield", "bitfield/bitfield.min.js"),
            ("viz", "viz/viz-standalone.js"),
            ("tikz", "tikzjax/tikzjax.js"),
            ("chart", "chart/chart.umd.js"),
        ];
        for (engine, relative) in browser {
            let expected_path = relative.split('/').fold(vendor.clone(), |acc, part| acc.join(part));
            let exists = expected_path.is_file();
            assert_eq!(engines[engine]["available"], json!(exists), "engine {engine}");
            assert_eq!(engines[engine]["offline"], json!(exists), "engine {engine}");
            assert_eq!(engines[engine]["renderer"], json!("browser"), "engine {engine}");
            assert_eq!(engines[engine]["requires_network"], json!(false), "engine {engine}");
            assert!(
                engines[engine].get("reason").is_none(),
                "browser engines carry no reason key: {engine}"
            );
            // 本仓库随包 assets 必须存在，否则 mermaid 等会被误报成可用。
            assert!(exists, "vendored asset missing: {expected_path:?}");
        }
        for alias in ["chartjs", "chart.js"] {
            assert_eq!(engines[alias], engines["chart"], "alias {alias}");
        }
        for engine in ["vega", "vega-lite"] {
            assert_eq!(engines[engine]["renderer"], json!("node"), "engine {engine}");
            assert_eq!(engines[engine]["offline"], engines[engine]["available"]);
            assert_eq!(engines[engine]["requires_network"], json!(false));
            let ready = engines[engine]["available"].as_bool().expect("bool");
            assert_eq!(
                engines[engine]["reason"],
                json!(if ready { "" } else { "diagram_dependency_missing" })
            );
        }
        for engine in ["plantuml", "puml"] {
            assert_eq!(engines[engine]["remote_available"], json!(true), "engine {engine}");
            assert_eq!(engines[engine]["available"], json!(diagrams::has_local_plantuml()));
            assert_eq!(engines[engine]["requires_network"], json!(!diagrams::has_local_plantuml()));
            assert_eq!(
                engines[engine]["renderer"],
                json!(if diagrams::has_local_plantuml() { "java" } else { "remote" })
            );
        }
        for engine in ["wsd", "d2"] {
            assert_eq!(
                engines[engine],
                json!({
                    "available": false,
                    "offline": false,
                    "renderer": "none",
                    "requires_network": false,
                    "reason": "diagram_engine_unavailable"
                }),
                "engine {engine}"
            );
        }
        // ditaa 只在 render 分支里，capabilities 里没有它（与 Python 一致）。
        assert!(engines.get("ditaa").is_none());
        let mut names: Vec<&str> = engines.keys().map(|s| s.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "bitfield", "chart", "chart.js", "chartjs", "d2", "mermaid", "plantuml", "puml",
                "tikz", "vega", "vega-lite", "viz", "wavedrom", "wsd"
            ]
        );
    }

    #[test]
    fn python_int_matches_int_str_semantics() {
        for (text, want) in [
            ("0", Some(0)),
            ("  12 ", Some(12)),
            ("+7", Some(7)),
            ("-3", Some(-3)),
            ("1_0", Some(10)),
            ("", None),
            ("  ", None),
            ("abc", None),
            ("1.5", None),
            ("0x10", None),
            ("_1", None),
            ("1_", None),
        ] {
            assert_eq!(python_int(text), want, "int({text:?})");
        }
    }

    /// Python `str()` 对标量字符串**原样返回**（实测 `str("it's") == "it's"`），
    /// 只有容器才递归走 `repr()`（实测 `str(["it's", 'a"b']) == '["it\'s", \'a"b\']'`）。
    /// `repr()` 的引号选择：含 `'` 且不含 `"` 时用双引号（实测 `repr("it's") == '"it\'s"'`）。
    #[test]
    fn python_str_and_truthiness_semantics() {
        assert!(!python_truthy(&json!(null)));
        assert!(!python_truthy(&json!(false)));
        assert!(!python_truthy(&json!(0)));
        assert!(!python_truthy(&json!("")));
        assert!(!python_truthy(&json!([])));
        assert!(!python_truthy(&json!({})));
        assert!(python_truthy(&json!("0")));
        assert!(python_truthy(&json!([0])));
        assert!(python_truthy(&json!({"a": 1})));
        assert!(python_truthy(&json!(-1)));
        assert_eq!(python_str(&json!("a")), "a");
        assert_eq!(python_str(&json!(5)), "5");
        assert_eq!(python_str(&json!(true)), "True");
        assert_eq!(python_str(&json!(false)), "False");
        assert_eq!(python_str(&json!(null)), "None");
        // 实测 str(5.0) == '5.0'：Python 的 float repr 保留小数点。
        assert_eq!(python_str(&json!(5.0)), "5.0");
        assert_eq!(python_str(&json!({"a": 1})), "{'a': 1}");
        assert_eq!(python_str(&json!(["a", 1, null])), "['a', 1, None]");
        assert_eq!(python_str(&json!("it's")), "it's");
        // 容器里的字符串元素走 repr：单引号串被双引号包住。
        assert_eq!(python_str(&json!(["it's", "a\"b"])), "[\"it's\", 'a\"b']");
        assert_eq!(python_str(&json!({"a": "it's"})), "{'a': \"it's\"}");
        // repr 本身（Python 实测逐条对齐）。
        assert_eq!(python_repr(&json!("it's")), "\"it's\"");
        assert_eq!(python_repr(&json!("a\"b'c")), "'a\"b\\'c'");
        assert_eq!(python_repr(&json!("")), "''");
        assert_eq!(python_repr(&json!("a\nb\tc\r\\d")), "'a\\nb\\tc\\r\\\\d'");
    }

    /// PlantUML 6-bit 字母表反解 —— 等价于 plantuml.com 解码 URL 的第一步。
    /// 尾部不足 8 bit 的是编码时补的 0 padding，服务端同样丢弃。
    fn plantuml_url_decode_base64(encoded: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut acc: u32 = 0;
        let mut held = 0u32;
        for ch in encoded.chars() {
            let index = diagrams::PLANTUML_CHARS
                .iter()
                .position(|slot| *slot as char == ch)
                .unwrap_or_else(|| panic!("non PlantUML alphabet char {ch:?} in {encoded:?}"));
            acc = (acc << 6) | index as u32;
            held += 6;
            if held >= 8 {
                held -= 8;
                bytes.push(((acc >> held) & 0xFF) as u8);
            }
        }
        bytes
    }

    /// 裸 deflate 解压 —— 等价于 plantuml.com 解码 URL 的第二步
    /// （Python 侧 `zlib.compress(...)[2:-4]` 取出的正是无头无 adler32 的裸流）。
    fn deflate_inflate(payload: &[u8]) -> Vec<u8> {
        use std::io::Read;
        let mut out = Vec::new();
        let mut decoder = flate2::bufread::DeflateDecoder::new(payload);
        decoder
            .read_to_end(&mut out)
            .unwrap_or_else(|err| panic!("raw deflate payload must inflate: {err}"));
        out
    }

    /// `src/readmd_modules/diagrams.py:42` 的 `plantuml_encode`：
    /// 裸 deflate（`zlib.compress(text.encode('utf-8'))[2:-4]`）+ PlantUML 6-bit 字母表
    /// （`PLANTUML_CHARS`，`diagrams.py:26`）+ 残缺组截尾（`diagrams.py:59-62`）。
    ///
    /// Python 承诺的**可观察契约**（`diagrams.py:68-77` 与 `fetch_plantuml_svg` 的
    /// `'generated a bad URL' in svg` 判据，`diagrams.py:105`）只有两条：载荷只用该字母表、
    /// 且服务端「6-bit 反解 + 裸 inflate」后必须还原同一份源码。deflate 的**比特流**属于
    /// 压缩器实现细节：CPython 用自带的 zlib，Rust 走 flate2 的 rust_backend(miniz_oxide)
    /// （见 `rust/Cargo.lock`：flate2 1.1.0 -> miniz_oxide；`libz-sys` 只被 curl-sys 使用），
    /// 两者对个别输入会给出不同但同样合法的流 —— 已实测该流被 plantuml.com 正常渲染成
    /// 200 + `<svg`（`scratch/rust_parity/_pd_net_out.txt`），Python 自己的用例也只锁 URL
    /// 结构（`tests/test_diagrams_expanded.py:53-59`）。
    ///
    /// 因此这里：两实现逐字节一致的 4 条样本继续**钉死字节**（守住字母表 / `[2:-4]` 剥框 /
    /// 残缺组截尾不回退）；仅比特流不同的 2 条样本**钉死解码等价**，并顺带用 Rust 反解器
    /// 复核 Python golden 本身没有过期（golden 值实测记录：`scratch/rust_parity/_pd_pu_out.txt`）。
    #[test]
    fn plantuml_url_encoding_matches_python_goldens() {
        let alice = "@startuml\nAlice -> Bob\n@enduml";
        let bob = "@startuml\nBob -> Alice : hello\n@enduml";
        let repeated = "a".repeat(200);
        // (源码, Python golden, 是否逐字节锁定)
        let cases: [(&str, &str, bool); 6] = [
            ("", "0m0", true),
            ("hello", "oqZDoSa700", true),
            (alice, "SoWkIImgAStDuNBCoKnELT2rKt3AJ-9oICrB0Ge200", true),
            ("digraph {a->b}", "IybCBqeio52eJjIrIwe500", true),
            (bob, "SoWkIImgAStDuNBAJrBGjLDmpCbCJbMmKiX8pSd9vt98pKi1IW80", false),
            (repeated.as_str(), "IqmS7W00", false),
        ];
        for (source, golden, pin_bytes) in cases {
            let encoded = diagrams::plantuml_encode(source);
            if pin_bytes {
                assert_eq!(encoded, golden, "source {source:?}");
            }
            // 契约 1：只出现 PlantUML 字母表字符（服务端据此才能解出偏移）。
            assert!(
                encoded.is_ascii()
                    && encoded
                        .bytes()
                        .all(|byte| diagrams::PLANTUML_CHARS.contains(&byte)),
                "non alphabet char in {encoded:?}"
            );
            // 契约 2：服务端解码步骤后回到同一份 UTF-8 源码。
            let rust_decoded = deflate_inflate(&plantuml_url_decode_base64(&encoded));
            let python_decoded = deflate_inflate(&plantuml_url_decode_base64(golden));
            assert_eq!(python_decoded, source.as_bytes(), "python golden stale: {golden}");
            assert_eq!(rust_decoded, python_decoded, "source {source:?}: rust={encoded}");
            assert_eq!(rust_decoded, source.as_bytes(), "source {source:?}");
        }
    }

    #[test]
    fn tikz_wrapper_matches_python_goldens() {
        assert_eq!(
            diagrams::format_tikz_html("\\draw (0,0) -- (1,1);"),
            "<script type=\"text/tikz\">\n\\begin{tikzpicture}\n\\draw (0,0) -- (1,1);\n\\end{tikzpicture}\n</script>"
        );
        assert_eq!(
            diagrams::format_tikz_html("\\begin{tikzpicture}\\draw (0,0);\\end{tikzpicture}"),
            "<script type=\"text/tikz\">\n\\begin{tikzpicture}\\draw (0,0);\\end{tikzpicture}\n</script>"
        );
        assert_eq!(
            diagrams::format_tikz_html("   \n \\foo  "),
            "<script type=\"text/tikz\">\n\\begin{tikzpicture}\n\\foo\n\\end{tikzpicture}\n</script>"
        );
        assert_eq!(
            diagrams::format_tikz_html("\\end{tikzpicture}"),
            "<script type=\"text/tikz\">\n\\begin{tikzpicture}\n\\end{tikzpicture}\n\\end{tikzpicture}\n</script>"
        );
    }

    #[test]
    fn vega_render_is_offline_or_honest_failure() {
        let root = diagrams::repo_root();
        let vendor = root.join("assets").join("vendor").join("diagrams");
        let ready = diagrams::node_runtime(&root).is_some()
            && vendor.join("vega").join("vega.min.js").is_file()
            && vendor.join("vega-lite").join("vega-lite.min.js").is_file();
        let spec = r#"{"$schema":"https://vega.github.io/schema/vega-lite/v5.json","data":{"values":[{"a":1,"b":2}],"name":"d"},"mark":"bar","encoding":{"x":{"field":"a","type":"quantitative"},"y":{"field":"b","type":"quantitative"}}}"#;
        let res = render(&format!(r#"{{"engine":"vega-lite","code":{}}}"#, json!(spec)));
        if !ready {
            // 没有 Node/随包 bundle：必须是依赖缺失的稳定码，绝不能生成假图。
            assert_eq!(res.status, 422);
            assert_eq!(payload(&res)["error_code"], json!("diagram_dependency_missing"));
            assert!(payload(&res).get("svg").is_none());
            return;
        }
        assert_eq!(res.status, 200, "{:?}", payload(&res));
        let body = payload(&res);
        assert_eq!(keys(&res), vec!["engine", "ok", "svg", "type"]);
        assert_eq!(body["type"], json!("svg"));
        assert_eq!(body["engine"], json!("vega-lite"));
        assert!(body["svg"].as_str().unwrap_or("").starts_with("<svg"));
        let svg = body["svg"].as_str().unwrap_or("");
        assert!(
            svg.len() >= 1000 && svg.contains("</svg>"),
            "vega-lite 必须真出图（Python 同 spec 为 5160 字节），实际 {} 字节",
            svg.len()
        );

        // 语法正确但语义无效的 spec：稳定码 + 永不带 svg。
        // （Python 实测：`{"a": 1}` + vega 其实能出图，见 scratch/p6_vega_check.txt，
        //   所以这里用 Python 真正拒绝的 `{"marks":[{"type":"nope-nope"}]}`。）
        let bad = render(r#"{"engine":"vega","code":"{\"marks\":[{\"type\":\"nope-nope\"}]}"}"#);
        assert_eq!(bad.status, 422, "{:?}", payload(&bad));
        assert_eq!(keys(&bad), vec!["error_code", "ok"]);
        assert_eq!(payload(&bad)["ok"], json!(false));
        assert_eq!(payload(&bad)["error_code"], json!("diagram_render_failed"));
    }
}
