//! Stable machine-readable codes carried next to the legacy Chinese
//! `error` / `note` / `warns` text, so the frontend can localise every
//! failure, notice and export warning (`error.<code>`, `note.<code>`,
//! `warn.<code>` in `assets/i18n/*.json`).
//!
//! Only additive: existing keys stay byte-identical, the codes ride along as
//! `error_code`, `note_code` and `warn_items`.

use serde_json::{json, Value};

// ---- error_code ------------------------------------------------------------
pub const FILE_NOT_FOUND: &str = "file_not_found";
pub const CONVERSION_FAILED: &str = "conversion_failed";
pub const UNSUPPORTED_FORMAT: &str = "unsupported_format";
pub const LEGACY_OFFICE_PARSE_FAILED: &str = "legacy_office_parse_failed";
pub const OCR_FAILED: &str = "ocr_failed";
pub const OCR_NO_ENGINE: &str = "ocr_no_engine";
pub const ENCODING_UNREPRESENTABLE: &str = "encoding_unrepresentable";
pub const ENCODING_UNKNOWN: &str = "encoding_unknown";
pub const DIALOG_UNAVAILABLE: &str = "dialog_unavailable";
pub const PATH_NOT_FOUND: &str = "path_not_found";
pub const OPEN_FAILED: &str = "open_failed";
pub const PRESET_NAME_CONFLICT: &str = "preset_name_conflict";
pub const INTERNAL_ERROR: &str = "internal_error";
pub const ZIP_CORRUPT: &str = "zip_corrupt";
pub const ZIP_TOO_LARGE: &str = "zip_too_large";
pub const ZIP_UNSUPPORTED: &str = "zip_unsupported";
pub const CANCELLED: &str = "cancelled";

// ---- note_code -------------------------------------------------------------
pub const CONVERT_NO_TEXT: &str = "convert_no_text";
pub const OCR_NO_TEXT: &str = "ocr_no_text";
pub const TRANSCRIBE_UNAVAILABLE: &str = crate::transcribe::TRANSCRIBE_UNAVAILABLE;

// ---- warn_items[].code -----------------------------------------------------
pub const WARN_IMAGE_MISSING: &str = "image_missing";
pub const WARN_IMAGE_REMOTE: &str = "image_remote";
pub const WARN_IMAGE_UNSUPPORTED: &str = "image_unsupported";
pub const WARN_FONT_FALLBACK: &str = "font_fallback";
pub const WARN_GLYPH_MISSING: &str = "glyph_missing";
pub const WARN_FORMULA_FALLBACK: &str = "formula_fallback";
pub const WARN_ASSET_COPY_FAILED: &str = "asset_copy_failed";
pub const WARN_OTHER: &str = "other";

/// Every code the frontend must be able to localise (checked by
/// `tools/check-i18n.mjs`).
pub const ERROR_CODES: &[&str] = &[
    FILE_NOT_FOUND, CONVERSION_FAILED, UNSUPPORTED_FORMAT, LEGACY_OFFICE_PARSE_FAILED, OCR_FAILED,
    OCR_NO_ENGINE, ENCODING_UNREPRESENTABLE, ENCODING_UNKNOWN, DIALOG_UNAVAILABLE, PATH_NOT_FOUND,
    OPEN_FAILED, PRESET_NAME_CONFLICT, INTERNAL_ERROR, ZIP_CORRUPT, ZIP_TOO_LARGE, ZIP_UNSUPPORTED,
    CANCELLED,
];
pub const NOTE_CODES: &[&str] = &[CONVERT_NO_TEXT, OCR_NO_TEXT, TRANSCRIBE_UNAVAILABLE];
pub const WARN_CODES: &[&str] = &[
    WARN_IMAGE_MISSING, WARN_IMAGE_REMOTE, WARN_IMAGE_UNSUPPORTED, WARN_FONT_FALLBACK,
    WARN_GLYPH_MISSING, WARN_FORMULA_FALLBACK, WARN_ASSET_COPY_FAILED,
];

/// Prefix → (code, name of the single `{param}`).  The warning producers keep
/// their historical Chinese text; this table is the one place that knows
/// their shapes.  Longer prefixes first.
const WARN_PREFIXES: &[(&str, &str, &str)] = &[
    ("远程/内联图片不支持嵌入，已跳过：", WARN_IMAGE_REMOTE, "src"),
    ("图片不存在，已跳过：", WARN_IMAGE_MISSING, "src"),
    ("图片无法嵌入：", WARN_IMAGE_UNSUPPORTED, "src"),
    ("EPUB 不支持该图片格式，已跳过：", WARN_IMAGE_UNSUPPORTED, "src"),
    ("图片读取失败，已跳过：", WARN_IMAGE_MISSING, "src"),
    ("DOCX 不支持该图片格式", WARN_IMAGE_UNSUPPORTED, "src"),
    ("所选字体不可用，已使用中文后备字体：", WARN_FONT_FALLBACK, "font"),
    ("未找到可嵌入的 CJK 字体", WARN_FONT_FALLBACK, ""),
    ("嵌入字体缺少以下字符，已使用阅读器内置字体：", WARN_GLYPH_MISSING, "chars"),
    ("公式无法渲染，已按文本保留：", WARN_FORMULA_FALLBACK, "latex"),
    ("资源复制失败：", WARN_ASSET_COPY_FAILED, "name"),
];

/// Classify one legacy warning string.
pub fn warn_item(text: &str) -> Value {
    for (prefix, code, param) in WARN_PREFIXES {
        if let Some(rest) = text.strip_prefix(prefix) {
            let mut params = serde_json::Map::new();
            if !param.is_empty() {
                // `DOCX 不支持该图片格式（png），已跳过：src` keeps the tail after the last `：`.
                let value = if *code == WARN_IMAGE_UNSUPPORTED && prefix.starts_with("DOCX") {
                    rest.rsplit_once('：').map(|(_, s)| s).unwrap_or(rest)
                } else {
                    rest
                };
                params.insert((*param).to_string(), json!(value));
            }
            return json!({ "code": code, "params": params, "text": text });
        }
    }
    json!({ "code": WARN_OTHER, "params": {}, "text": text })
}

/// `warns: [String]` → `warn_items: [{code, params, text}]`.
pub fn warn_items<S: AsRef<str>>(warns: &[S]) -> Value {
    Value::Array(warns.iter().map(|w| warn_item(w.as_ref())).collect())
}

/// Add `warn_items` next to an existing `warns` string array (no-op otherwise).
pub fn attach_warn_items(body: &mut Value) {
    let Some(map) = body.as_object_mut() else { return };
    let items = match map.get("warns") {
        Some(Value::Array(list)) => {
            let texts: Vec<&str> = list.iter().filter_map(|v| v.as_str()).collect();
            warn_items(&texts)
        }
        _ => return,
    };
    map.insert("warn_items".to_string(), items);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_producer_prefix_maps_to_a_declared_code() {
        for (_, code, _) in WARN_PREFIXES {
            assert!(WARN_CODES.contains(code), "{code}");
        }
        let cases = [
            ("图片不存在，已跳过：a.png", WARN_IMAGE_MISSING, Some(("src", "a.png"))),
            ("远程/内联图片不支持嵌入，已跳过：https://x/y.png", WARN_IMAGE_REMOTE, Some(("src", "https://x/y.png"))),
            ("DOCX 不支持该图片格式（svg），已跳过：a.svg", WARN_IMAGE_UNSUPPORTED, Some(("src", "a.svg"))),
            ("公式无法渲染，已按文本保留：a+b", WARN_FORMULA_FALLBACK, Some(("latex", "a+b"))),
            ("嵌入字体缺少以下字符，已使用阅读器内置字体：𠀀", WARN_GLYPH_MISSING, Some(("chars", "𠀀"))),
            ("未找到可嵌入的 CJK 字体，非拉丁文字…", WARN_FONT_FALLBACK, None),
            ("something new", WARN_OTHER, None),
        ];
        for (text, code, param) in cases {
            let v = warn_item(text);
            assert_eq!(v["code"], code, "{text}");
            assert_eq!(v["text"], text);
            if let Some((k, want)) = param {
                assert_eq!(v["params"][k], want, "{text}");
            }
        }
    }

    #[test]
    fn attach_keeps_legacy_warns_and_adds_items() {
        let mut body = json!({ "ok": true, "warns": ["图片不存在，已跳过：x.png"] });
        attach_warn_items(&mut body);
        assert_eq!(body["warns"][0], "图片不存在，已跳过：x.png");
        assert_eq!(body["warn_items"][0]["code"], WARN_IMAGE_MISSING);
        let mut none = json!({ "ok": false });
        attach_warn_items(&mut none);
        assert!(none.get("warn_items").is_none());
    }
}
