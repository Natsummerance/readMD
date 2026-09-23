//! `src/readmd_modules/mdexport/styles.py` — export style schema, presets, merge
//! and validation.
//!
//! The style dictionary is the single input every renderer reads
//! (`mdexport/__init__.py:91` builds it once with `styles.sanitize(options)` and
//! hands it to `docx_render.render` / `pdf_render.render` / `html_render`).
//! `mdexport.rs` only ever consumed the *subset* `html_render._build_css` reads
//! (`style_clamp` / `style_hex` / `style_choice` / `page_dimensions` there), so
//! the DOCX/PDF lane — which needs the whole tree, including `header`, `images`,
//! `footer`, `math`, `meta`, `cover`, `toc` and per-level `headings` — had no
//! equivalent.  This module is that missing authority port.
//!
//! Data model: the style is kept as a `serde_json::Value` tree exactly like
//! Python's `dict`, because two behaviours of `styles.py` are only observable
//! when the values stay loosely typed:
//!
//! * **int/float taint.**  `_clamp` returns `max(lo, min(hi, float(v)))`, and
//!   Python's `min`/`max` return the *argument object*, so a value pinned to a
//!   bound comes back as the `int` literal written in the source while a value
//!   inside the range comes back as a `float`.  That is why
//!   `sanitize({'page': {'width': '700'}})['page']['width'] == 600` (int) while
//!   the untouched default is `210.0` (float) — and `json.dumps` renders the
//!   difference.  Rust has to model it deliberately: `Number::from_f64` prints
//!   `210.0`, `Number::from(600i64)` prints `600`.  `clamp` therefore returns a
//!   `Value`, never an `f64`.
//! * **retained junk.**  `deep_merge` copies unknown keys straight through
//!   (`{'unknownSection': {'a': 1}}` survives), and `sanitize` only rewrites the
//!   keys it knows about.
//!
//! Fidelity notes (each pinned by the generated goldens in `tests`):
//!
//! * `str(x) or ''` truncation is by **code point** (`[:120]`), not by byte.
//! * `_hex` accepts a trailing newline: `re.match`'s `$` matches before a final
//!   `\n`, so `"#ffffff\n"` is a legal colour and reaches the renderer verbatim
//!   (same quirk `mdexport.rs::style_hex` documents).
//! * `float('nan')`/`float('inf')` parse but are rejected by `math.isfinite`, so
//!   they take the *default object* — including its int-ness.
//! * `preset_style(name)` on an unknown name is `sanitize({})`, not an error.

use serde_json::{json, Map, Value};

use crate::mdexport::{py_str, py_truthy};

/// `styles.PAGE_SIZES` (`styles.py:109`).
pub const PAGE_SIZES: &[&str] = &["A3", "A4", "A5", "B5", "Letter", "Legal", "Custom"];
/// `styles.ORIENTATIONS` (`styles.py:110`).
pub const ORIENTATIONS: &[&str] = &["portrait", "landscape"];
/// `styles.HTML_THEMES` (`styles.py:111`).
pub const HTML_THEMES: &[&str] = &["light", "dark", "sepia"];
/// `styles._FONTS` (`styles.py:112`).
pub const FONTS: &[&str] = &["MicrosoftYaHei", "SimHei", "SimSun", "KaiTi", "DengXian", "Arial"];
/// `styles._MONO` (`styles.py:113`).
pub const MONO: &[&str] = &["Consolas", "Courier New", "SimHei"];
/// `styles._ALIGNS` (`styles.py:114`).
pub const ALIGNS: &[&str] = &["left", "center", "right", "justify"];

/// `styles.DEFAULT_STYLE` (`styles.py:24-54`).  Built fresh per call, which is
/// what `copy.deepcopy(base)` gives Python — the two are only equivalent because
/// nothing here mutates the returned tree in place afterwards.
pub fn default_style() -> Value {
    json!({
        "page": {"size": "A4", "orientation": "portrait", "width": 210, "height": 297,
                 "marginTop": 20, "marginRight": 18, "marginBottom": 20, "marginLeft": 18},
        "cover": {"enabled": false, "title": "", "subtitle": "", "date": "", "align": "center"},
        "toc": {"enabled": false},
        "header": {"text": "", "align": "left"},
        "images": {"widthPct": 92, "maxHeightPct": 85},
        "typography": {"font": "MicrosoftYaHei", "size": 11, "lineHeight": 1.6,
                       "spacing": 6, "color": "#262626", "align": "left"},
        "headings": {
            "h1": {"size": 20, "color": "#1a1a1a", "bold": true, "align": "left", "before": 18, "after": 10},
            "h2": {"size": 16, "color": "#1f2937", "bold": true, "align": "left", "before": 14, "after": 8},
            "h3": {"size": 14, "color": "#2d3748", "bold": true, "align": "left", "before": 12, "after": 6},
            "h4": {"size": 12, "color": "#374151", "bold": true, "align": "left", "before": 10, "after": 6},
            "h5": {"size": 11, "color": "#4a5568", "bold": true, "align": "left", "before": 8, "after": 4},
            "h6": {"size": 10.5, "color": "#4a5568", "bold": true, "align": "left", "before": 8, "after": 4}
        },
        "table": {"headerBg": "#3b6ef5", "headerColor": "#ffffff", "headerBold": true,
                  "borderColor": "#c8cdd4", "borderWidth": 0.75, "banded": true,
                  "bandColor": "#f3f5f9", "cellSize": 10, "cellPadding": 6,
                  "align": "left", "widthPct": 100},
        "code": {"bg": "#f5f6f8", "color": "#2f3b4a", "font": "Consolas", "size": 9.5,
                 "borderColor": "#dfe3e8", "borderWidth": 0.5, "rounded": true},
        "quote": {"barColor": "#3b6ef5", "bg": "#f3f6ff", "color": "#4a5568"},
        "link": {"color": "#2b6cb0"},
        "hr": {"color": "#d8dce2"},
        "footer": {"pageNumbers": true, "text": ""},
        "math": {"dpi": 220},
        "meta": {"title": "", "author": "", "subject": ""},
        "htmlTheme": "light"
    })
}

/// One `styles.PRESETS` entry (`styles.py:57-107`) — the delta over the defaults.
pub fn preset(name: &str) -> Option<Value> {
    let value = match name {
        "minimal" => json!({
            "typography": {"size": 10.5, "lineHeight": 1.55, "color": "#333333", "align": "left"},
            "headings": {
                "h1": {"size": 18, "color": "#111111"},
                "h2": {"size": 14.5, "color": "#222222"},
                "h3": {"size": 12.5, "color": "#333333"}
            },
            "table": {"headerBg": "#eef1f5", "headerColor": "#333333", "headerBold": true,
                      "borderColor": "#ccd2da", "borderWidth": 0.5, "banded": false,
                      "cellSize": 9.5, "cellPadding": 5, "align": "left", "widthPct": 100},
            "code": {"bg": "#f7f8fa", "color": "#444444", "borderColor": "#e3e6ea", "borderWidth": 0.5},
            "quote": {"barColor": "#9aa3af", "bg": "#f6f7f9", "color": "#555555"},
            "link": {"color": "#1a73e8"},
            "hr": {"color": "#e0e3e8"}
        }),
        "classic" => json!({
            "typography": {"size": 12, "lineHeight": 1.8, "spacing": 8, "color": "#1a1a1a", "align": "left"},
            "headings": {
                "h1": {"size": 22, "color": "#000000", "align": "center"},
                "h2": {"size": 17, "color": "#111111"},
                "h3": {"size": 14.5, "color": "#222222"},
                "h4": {"size": 13, "color": "#333333"}
            },
            "table": {"headerBg": "#d9e2ec", "headerColor": "#1f2d3d", "headerBold": true,
                      "borderColor": "#8a94a6", "borderWidth": 1.0, "banded": true,
                      "bandColor": "#f2f5f8", "cellSize": 10.5, "cellPadding": 7,
                      "align": "left", "widthPct": 100},
            "code": {"bg": "#f4f4f0", "color": "#333333", "borderColor": "#c9c9c4", "borderWidth": 0.75},
            "quote": {"barColor": "#7a8699", "bg": "#f5f6f8", "color": "#3d4852"},
            "link": {"color": "#8a2be2"},
            "hr": {"color": "#b5b5ad"}
        }),
        "business" => json!({
            "typography": {"size": 11, "lineHeight": 1.65, "spacing": 6, "color": "#2c3e50", "align": "left"},
            "headings": {
                "h1": {"size": 20, "color": "#1f3864", "bold": true},
                "h2": {"size": 16, "color": "#2e5395"},
                "h3": {"size": 13.5, "color": "#3a6db5"},
                "h4": {"size": 12, "color": "#4a7fd4"}
            },
            "table": {"headerBg": "#1f3864", "headerColor": "#ffffff", "headerBold": true,
                      "borderColor": "#9fb3d1", "borderWidth": 0.75, "banded": true,
                      "bandColor": "#eef3fa", "cellSize": 10, "cellPadding": 6,
                      "align": "left", "widthPct": 100},
            "code": {"bg": "#f0f4fa", "color": "#1f3864", "borderColor": "#c3d0e4", "borderWidth": 0.5},
            "quote": {"barColor": "#1f3864", "bg": "#eef3fa", "color": "#34507c"},
            "link": {"color": "#1f3864"},
            "hr": {"color": "#b9c6da"}
        }),
        _ => return None,
    };
    Some(value)
}

/// Every key of `styles.PRESETS` (`styles.py:57`), i.e. the names `preset_style`
/// resolves.  Insertion order is preserved because `sanitize` iterates
/// `DEFAULT_STYLE.items()` — not the presets — so only membership matters here.
pub fn preset_names() -> Vec<&'static str> {
    vec!["minimal", "classic", "business"]
}

/// `styles.deep_merge(base, over)` (`styles.py:117-129`).  `None` (`null`)
/// overrides are skipped, dicts merge recursively, anything else replaces.
pub fn deep_merge(base: &Value, over: &Value) -> Value {
    let mut out = base.clone();
    let Some(map) = out.as_object_mut() else {
        return out;
    };
    let Some(over) = over.as_object() else {
        return out; // `if not isinstance(over, dict): return out`
    };
    // Collect first: the recursion reads `out` while writing it.
    let updates: Vec<(String, Value)> = over
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            let merged = match (map.get(k), v) {
                (Some(Value::Object(_)), Value::Object(_)) => deep_merge(&map[k], v),
                _ => v.clone(),
            };
            (k.clone(), merged)
        })
        .collect();
    for (k, v) in updates {
        map.insert(k, v);
    }
    out
}

/// `float(v)` for a JSON value (`_clamp`'s `try` block, `styles.py:133-136`):
/// numbers pass through, `True`/`False` are `1.0`/`0.0` (`bool` is an `int`
/// subclass), strings go through CPython's float grammar, everything else
/// raises.  `None` therefore means "the `except` branch fired".
///
/// `py_float_str` is a **local copy** of the (private) `mdexport.rs:663`
/// scanner: `float()` trims exactly Rust's `char::is_whitespace`, allows `_`
/// only between digits, and permits a single leading sign.  Duplication is
/// deliberate — `mdexport.rs` is another lane's file.
fn py_float_of(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n.as_f64(),
        Value::String(s) => py_float_str(s),
        _ => None,
    }
}

fn py_float_str(text: &str) -> Option<f64> {
    let body = text.trim_matches(|c: char| c.is_whitespace());
    let chars: Vec<char> = body.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == '_' {
            let ok = i > 0
                && i + 1 < chars.len()
                && chars[i - 1].is_ascii_digit()
                && chars[i + 1].is_ascii_digit();
            if !ok {
                return None;
            }
        }
    }
    let cleaned: String = chars.iter().filter(|c| **c != '_').collect();
    if cleaned.is_empty() {
        return None;
    }
    let (num, exp) = match cleaned.split_once(['e', 'E']) {
        Some((n, e)) => (n, Some(e.trim_start_matches(['+', '-']))),
        None => (cleaned.as_str(), None),
    };
    let mantissa = num.strip_prefix(['+', '-']).unwrap_or(num);
    let digits_ok = mantissa
        .chars()
        .filter(|c| *c != '.')
        .all(|c| c.is_ascii_digit())
        && exp.map_or(true, |e| !e.is_empty() && e.chars().all(|c| c.is_ascii_digit()));
    if !digits_ok {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// `styles._clamp(v, lo, hi, default)` (`styles.py:132-139`).
///
/// `max(lo, min(hi, f))` returns one of its **argument objects**, so the result
/// keeps the numeric type of whichever bound (or the fresh `float`) won:
/// `min(hi, f)` yields `hi` unless `f < hi`, and `max(lo, x)` yields `lo` unless
/// `x > lo`.  Ties therefore collapse to the *literal*: `_clamp(8, 8, 20, 11)`
/// is the int `8`, while `_clamp(8.5, 8, 20, 11)` is the float `8.5`.
pub fn clamp(value: Option<&Value>, lo: &Value, hi: &Value, default: &Value) -> Value {
    let parsed = value.and_then(py_float_of).filter(|f| f.is_finite());
    let Some(f) = parsed else {
        return default.clone();
    };
    let hi_f = hi.as_f64().unwrap_or(0.0);
    let lo_f = lo.as_f64().unwrap_or(0.0);
    let inner = if f < hi_f { Value::from(f) } else { hi.clone() };
    if inner.as_f64().unwrap_or(0.0) > lo_f {
        inner
    } else {
        lo.clone()
    }
}

/// `styles.re_match_hex` (`styles.py:254-256`): `re.match(r'^#[0-9a-fA-F]{6}$', v)`.
/// `re.match`'s `$` also matches immediately before one trailing newline.
pub fn re_match_hex(v: &str) -> bool {
    let body = v.strip_suffix('\n').unwrap_or(v);
    let Some(digits) = body.strip_prefix('#') else {
        return false;
    };
    digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `styles._hex(v, default)` (`styles.py:248-251`): keep the string verbatim
/// when it is a `#rrggbb`, else the default.  Case is *not* normalised here —
/// `docx_render._hex_val` upper-cases later.
pub fn hex(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(s)) if re_match_hex(s) => s.clone(),
        _ => default.to_string(),
    }
}

/// `styles._font(v)` (`styles.py:259-260`).
pub fn font(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) if FONTS.contains(&s.as_str()) => s.clone(),
        _ => "MicrosoftYaHei".to_string(),
    }
}

fn choose(value: Option<&Value>, allowed: &[&str], fallback: &str) -> String {
    match value {
        Some(Value::String(s)) if allowed.contains(&s.as_str()) => s.clone(),
        _ => fallback.to_string(),
    }
}

/// `x if x in ALLOWED else fallback` for a value that may be any JSON type.
fn py_in_str(value: &Value, allowed: &[&str]) -> bool {
    matches!(value, Value::String(s) if allowed.contains(&s.as_str()))
}

/// Mutable access to a section that `sanitize` has already forced into a dict.
fn section<'a>(style: &'a mut Value, key: &str) -> &'a mut Map<String, Value> {
    style
        .get_mut(key)
        .and_then(|v| v.as_object_mut())
        .expect("sanitize normalises every section to a dict")
}

/// `styles.sanitize(options)` (`styles.py:142-232`).
pub fn sanitize(options: &Value) -> Value {
    // `deep_merge(DEFAULT_STYLE, options or {})`
    let over = if py_truthy(options) { options.clone() } else { json!({}) };
    let mut s = deep_merge(&default_style(), &over);

    // `for key, default in DEFAULT_STYLE.items(): if isinstance(default, dict)
    //  and not isinstance(s.get(key), dict): s[key] = deepcopy(default)`
    {
        let defaults = default_style();
        let dmap = defaults.as_object().unwrap();
        for (key, dflt) in dmap.iter() {
            let needs_reset = dflt.is_object() && !s.get(key).is_some_and(Value::is_object);
            if needs_reset {
                if let Some(map) = s.as_object_mut() {
                    map.insert(key.clone(), dflt.clone());
                }
            }
        }
    }

    // --- page ---
    let default_page = default_style();
    let page = default_page.get("page").and_then(|v| v.as_object()).unwrap().clone();
    {
        let p = section(&mut s, "page");
        // `styles.py:149-150` passes the *literals* 210/297 as `_clamp`'s
        // default: an unparsable or non-finite user value can never be echoed,
        // and the fallback keeps the `int`-ness of the source literal.
        let w = clamp(p.get("width"), &json!(80), &json!(600), &json!(210));
        p.insert("width".into(), w);
        let h = clamp(p.get("height"), &json!(80), &json!(600), &json!(297));
        p.insert("height".into(), h);
        let cur = p.get("size").cloned().unwrap_or(Value::Null);
        if !py_in_str(&cur, PAGE_SIZES) {
            p.insert("size".into(), page["size"].clone());
        }
        let cur = p.get("orientation").cloned().unwrap_or(Value::Null);
        if !py_in_str(&cur, ORIENTATIONS) {
            p.insert("orientation".into(), json!("portrait"));
        }
        for key in ["marginTop", "marginRight", "marginBottom", "marginLeft"] {
            let default = page[key].clone();
            let v = p.get(key).cloned();
            let c = clamp(v.as_ref(), &json!(0), &json!(60), &default);
            p.insert(key.to_string(), c);
        }
    }

    // --- typography ---
    {
        let t = section(&mut s, "typography");
        let size = clamp(t.get("size"), &json!(8), &json!(20), &json!(11));
        t.insert("size".into(), size);
        let lh = clamp(t.get("lineHeight"), &json!(1.0), &json!(2.5), &json!(1.6));
        t.insert("lineHeight".into(), lh);
        let sp = clamp(t.get("spacing"), &json!(0), &json!(30), &json!(6));
        t.insert("spacing".into(), sp);
        let align = t.get("align").cloned().unwrap_or(Value::Null);
        let keep = py_in_str(&align, ALIGNS);
        t.insert(
            "align".into(),
            if keep { align } else { json!("left") },
        );
        let color = hex(t.get("color"), "#262626");
        t.insert("color".into(), json!(color));
        let f = font(t.get("font"));
        t.insert("font".into(), json!(f));
        let ind = clamp(t.get("firstLineIndent"), &json!(0), &json!(30), &json!(0));
        t.insert("firstLineIndent".into(), ind);
    }

    // --- header / images ---
    {
        let text = py_str_or_empty(section(&mut s, "header").get("text"));
        section(&mut s, "header").insert("text".into(), json!(head_chars(&text, 120)));
        let cur = section(&mut s, "header").get("align").cloned().unwrap_or(Value::Null);
        let keep = py_in_str(&cur, ALIGNS);
        let value = if keep { cur } else { json!("left") };
        section(&mut s, "header").insert("align".into(), value);
    }
    {
        let pct = clamp(section(&mut s, "images").get("widthPct"), &json!(10), &json!(100), &json!(92));
        section(&mut s, "images").insert("widthPct".into(), pct);
        let pct = clamp(section(&mut s, "images").get("maxHeightPct"), &json!(10), &json!(100), &json!(85));
        section(&mut s, "images").insert("maxHeightPct".into(), pct);
    }

    // --- headings h1..h6 ---
    let default_headings = default_style();
    let dh = default_headings
        .get("headings")
        .and_then(|v| v.as_object())
        .unwrap()
        .clone();
    for i in 1..=6usize {
        let name = format!("h{}", i);
        let fallback = dh[&name].clone();
        {
            let hs = section(&mut s, "headings");
            if !matches!(hs.get(&name), Some(Value::Object(_))) {
                hs.insert(name.clone(), fallback.clone());
            }
        }
        let h = section(&mut s, "headings")
            .get_mut(&name)
            .and_then(|v| v.as_object_mut())
            .expect("heading section");
        let pbb = py_truthy(h.get("pageBreakBefore").unwrap_or(&Value::Bool(false)));
        h.insert("pageBreakBefore".into(), json!(pbb));
        let fb_size = fallback.get("size").cloned().unwrap_or(json!(11));
        let size = clamp(h.get("size"), &json!(8), &json!(40), &fb_size);
        h.insert("size".into(), size);
        let bold = py_truthy(h.get("bold").unwrap_or(&Value::Bool(true)));
        h.insert("bold".into(), json!(bold));
        let align = h.get("align").cloned().unwrap_or(Value::Null);
        let keep = py_in_str(&align, ALIGNS);
        h.insert("align".into(), if keep { align } else { json!("left") });
        let color = hex(h.get("color"), "#1a1a1a");
        h.insert("color".into(), json!(color));
        let before = clamp(h.get("before"), &json!(0), &json!(60), &json!(10));
        h.insert("before".into(), before);
        let after = clamp(h.get("after"), &json!(0), &json!(40), &json!(6));
        h.insert("after".into(), after);
    }

    // --- table ---
    {
        let tb = section(&mut s, "table");
        let header_bg = hex(tb.get("headerBg"), "#3b6ef5");
        tb.insert("headerBg".into(), json!(header_bg));
        let header_color = hex(tb.get("headerColor"), "#ffffff");
        tb.insert("headerColor".into(), json!(header_color));
        let bold = py_truthy(tb.get("headerBold").unwrap_or(&Value::Bool(true)));
        tb.insert("headerBold".into(), json!(bold));
        let border = hex(tb.get("borderColor"), "#c8cdd4");
        tb.insert("borderColor".into(), json!(border));
        let bw = clamp(tb.get("borderWidth"), &json!(0), &json!(3), &json!(0.75));
        tb.insert("borderWidth".into(), bw);
        let banded = py_truthy(tb.get("banded").unwrap_or(&Value::Bool(true)));
        tb.insert("banded".into(), json!(banded));
        let band = hex(tb.get("bandColor"), "#f3f5f9");
        tb.insert("bandColor".into(), json!(band));
        let cs = clamp(tb.get("cellSize"), &json!(7), &json!(16), &json!(10));
        tb.insert("cellSize".into(), cs);
        let cp = clamp(tb.get("cellPadding"), &json!(0), &json!(20), &json!(6));
        tb.insert("cellPadding".into(), cp);
        let align = tb.get("align").cloned().unwrap_or(Value::Null);
        let keep = py_in_str(&align, ALIGNS);
        tb.insert("align".into(), if keep { align } else { json!("left") });
        let wp = clamp(tb.get("widthPct"), &json!(50), &json!(100), &json!(100));
        tb.insert("widthPct".into(), wp);
    }

    // --- code ---
    {
        let c = section(&mut s, "code");
        let bg = hex(c.get("bg"), "#f5f6f8");
        c.insert("bg".into(), json!(bg));
        let color = hex(c.get("color"), "#2f3b4a");
        c.insert("color".into(), json!(color));
        let mono = match c.get("font") {
            Some(Value::String(s)) if MONO.contains(&s.as_str()) => s.clone(),
            _ => "Consolas".to_string(),
        };
        c.insert("font".into(), json!(mono));
        let size = clamp(c.get("size"), &json!(6), &json!(16), &json!(9.5));
        c.insert("size".into(), size);
        let border = hex(c.get("borderColor"), "#dfe3e8");
        c.insert("borderColor".into(), json!(border));
        let bw = clamp(c.get("borderWidth"), &json!(0), &json!(3), &json!(0.5));
        c.insert("borderWidth".into(), bw);
        let rounded = py_truthy(c.get("rounded").unwrap_or(&Value::Bool(true)));
        c.insert("rounded".into(), json!(rounded));
    }

    // --- quote / link / hr ---
    {
        let q = section(&mut s, "quote");
        let bar = hex(q.get("barColor"), "#3b6ef5");
        q.insert("barColor".into(), json!(bar));
        let bg = hex(q.get("bg"), "#f3f6ff");
        q.insert("bg".into(), json!(bg));
        let color = hex(q.get("color"), "#4a5568");
        q.insert("color".into(), json!(color));
    }
    {
        let color = hex(section(&mut s, "link").get("color"), "#2b6cb0");
        section(&mut s, "link").insert("color".into(), json!(color));
    }
    {
        let color = hex(section(&mut s, "hr").get("color"), "#d8dce2");
        section(&mut s, "hr").insert("color".into(), json!(color));
    }

    // --- footer / math / htmlTheme ---
    {
        let f = section(&mut s, "footer");
        let on = py_truthy(f.get("pageNumbers").unwrap_or(&Value::Bool(true)));
        f.insert("pageNumbers".into(), json!(on));
        let text = py_str_or_empty(f.get("text"));
        f.insert("text".into(), json!(head_chars(&text, 80)));
    }
    {
        let dpi = clamp(section(&mut s, "math").get("dpi"), &json!(100), &json!(500), &json!(220));
        // `int(...)` — Python's int() truncates toward zero.
        let truncated = dpi.as_f64().unwrap_or(220.0).trunc();
        section(&mut s, "math").insert("dpi".into(), json!(truncated as i64));
    }
    {
        let cur = s.get("htmlTheme").cloned().unwrap_or(Value::Null);
        let keep = py_in_str(&cur, HTML_THEMES);
        if let Some(map) = s.as_object_mut() {
            map.insert(
                "htmlTheme".into(),
                if keep { cur } else { json!("light") },
            );
        }
    }

    // --- meta / cover / toc ---
    {
        let meta = section(&mut s, "meta");
        for key in ["title", "author", "subject"] {
            let text = py_str_or_empty(meta.get(key));
            meta.insert(key.to_string(), json!(head_chars(&text, 120)));
        }
    }
    {
        let cover = section(&mut s, "cover");
        let enabled = py_truthy(cover.get("enabled").unwrap_or(&Value::Bool(false)));
        cover.insert("enabled".into(), json!(enabled));
        for key in ["title", "subtitle", "date"] {
            let text = py_str_or_empty(cover.get(key));
            cover.insert(key.to_string(), json!(head_chars(&text, 120)));
        }
        let align = cover.get("align").cloned().unwrap_or(Value::Null);
        let keep = py_in_str(&align, &["left", "center", "right"]);
        cover.insert("align".into(), if keep { align } else { json!("center") });
    }
    {
        let toc = section(&mut s, "toc");
        let enabled = py_truthy(toc.get("enabled").unwrap_or(&Value::Bool(false)));
        toc.insert("enabled".into(), json!(enabled));
    }

    // `styles.py:227`: `page_dimensions` is read *after* every clamp above (so a
    // clamped `Custom` width changes the usable height) and before the margin
    // rewrite that follows it.
    let (width, height) = page_dimensions(&s);
    let pairs = [
        ("marginLeft".to_string(), "marginRight".to_string(), width),
        ("marginTop".to_string(), "marginBottom".to_string(), height),
    ];
    for (a, b, limit) in pairs {
        let (pa, pb) = {
            let p = section(&mut s, "page");
            (
                p.get(&a).and_then(|v| v.as_f64()).unwrap_or(0.0),
                p.get(&b).and_then(|v| v.as_f64()).unwrap_or(0.0),
            )
        };
        let total = pa + pb;
        if total > limit - 30.0 {
            // `p[a], p[b] = p[a]*(limit-30)/total, p[b]*(limit-30)/total` — both
            // products use the pre-assignment values, and true division makes
            // both results floats.
            let scaled = |v: f64| v * (limit - 30.0) / total;
            let (na, nb) = (scaled(pa), scaled(pb));
            let p = section(&mut s, "page");
            p.insert(a, json!(na));
            p.insert(b, json!(nb));
        }
    }
    s
}

fn py_str_or_empty(value: Option<&Value>) -> String {
    match value {
        Some(v) if py_truthy(v) => py_str(v),
        _ => String::new(),
    }
}

/// `s[:n]` on a Python `str` — the first `n` **code points**.
fn head_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `styles.page_dimensions(style)` (`styles.py:235-239`): the oriented page box
/// in millimetres.  Returns floats; Python keeps the `int`-ness of the
/// `sizes` table, which is invisible to every consumer (`Mm()`, reportlab).
pub fn page_dimensions(style: &Value) -> (f64, f64) {
    let page = style.get("page").and_then(|v| v.as_object());
    let get_str = |key: &str| -> String {
        page.and_then(|p| p.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let size = get_str("size");
    let orientation = get_str("orientation");
    let (w, h) = if size == "Custom" {
        (
            page.and_then(|p| p.get("width")).and_then(|v| v.as_f64()).unwrap_or(0.0),
            page.and_then(|p| p.get("height")).and_then(|v| v.as_f64()).unwrap_or(0.0),
        )
    } else {
        match size.as_str() {
            "A3" => (297.0, 420.0),
            "A5" => (148.0, 210.0),
            "B5" => (176.0, 250.0),
            "Letter" => (215.9, 279.4),
            "Legal" => (215.9, 355.6),
            _ => (210.0, 297.0),
        }
    };
    if orientation == "landscape" {
        (w.max(h), w.min(h))
    } else {
        (w, h)
    }
}

/// `styles.sanitize` for a *raw* `options` payload where the caller only has an
/// absent value (`mdexport.export` passes `_styles.sanitize(options)` and
/// `options` may be `None`).
pub fn sanitize_options(options: Option<&Value>) -> Value {
    sanitize(options.unwrap_or(&Value::Null))
}

/// `styles.preset_style(name)` (`styles.py:242-245`).
pub fn preset_style(name: &str) -> Value {
    sanitize(&preset(name).unwrap_or_else(|| json!({})))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(case, kind, argument, want sanitized, want page_dimensions)` generated by
    /// `scratch/docx_render_s1_style_golden_splice.py` from
    /// `scratch/docx_render_s1_style_golden.json` — every `want` is observed
    /// CPython output of `styles.sanitize` / `styles.preset_style`.
    const GOLDEN: &[(&str, &str, &str, &str, &str)] = &[
        // Auto-generated by scratch/docx_render_s1_style_golden_splice.py from
        // scratch/docx_render_s1_style_golden.json: every `want` is observed CPython
        // output of mdexport/styles.py sanitize()/preset_style()/page_dimensions().
        // Do not edit by hand.
        ("null_options", "sanitize",
            "null",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("empty", "sanitize",
            "{}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("junk_sections", "sanitize",
            "{\"code\":true,\"cover\":0,\"footer\":{},\"header\":9,\"headings\":[],\"hr\":[],\"htmlTheme\":\"neon\",\"images\":10,\"link\":\"x\",\"math\":\"3\",\"meta\":7,\"page\":5,\"quote\":1.5,\"table\":null,\"toc\":8,\"typography\":\"nope\"}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("clamp_and_types", "sanitize",
            "{\"math\":{\"dpi\":900},\"page\":{\"height\":-5,\"marginBottom\":\"abc\",\"marginLeft\":true,\"marginRight\":null,\"marginTop\":\"12\",\"orientation\":\"up\",\"size\":\"A9\",\"width\":\"700\"},\"typography\":{\"align\":\"middle\",\"color\":\"#12345\",\"firstLineIndent\":99,\"font\":\"Comic Sans\",\"lineHeight\":9,\"size\":3,\"spacing\":-4}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":500},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":80,\"marginBottom\":20,\"marginLeft\":1.0,\"marginRight\":18.0,\"marginTop\":12.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":600},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":30,\"font\":\"MicrosoftYaHei\",\"lineHeight\":2.5,\"size\":8,\"spacing\":0}}",
            "[210,297]"),
        ("hex_nan_inf", "sanitize",
            "{\"code\":{\"font\":\"SimHei\",\"rounded\":null,\"size\":\"12\"},\"table\":{\"banded\":0,\"borderWidth\":\"1e3\",\"cellPadding\":\"2.5\",\"headerBg\":\"#fff\",\"widthPct\":12},\"typography\":{\"color\":\"  #0F0F0F  \",\"lineHeight\":\"inf\",\"size\":\"nan\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"SimHei\",\"rounded\":true,\"size\":12.0},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":false,\"borderColor\":\"#c8cdd4\",\"borderWidth\":3,\"cellPadding\":2.5,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":50},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11,\"spacing\":6.0}}",
            "[210,297]"),
        ("headings_and_strings", "sanitize",
            "{\"cover\":{\"align\":\"right\",\"enabled\":1,\"title\":3.5},\"footer\":{\"pageNumbers\":0,\"text\":\"y\"},\"header\":{\"align\":\"justify\",\"text\":\"x\"},\"headings\":{\"h1\":{\"after\":900,\"align\":\"right\",\"before\":-3,\"bold\":0,\"color\":\"#000000\",\"pageBreakBefore\":true,\"size\":99},\"h2\":\"junk\",\"h4\":{},\"h6\":{\"size\":\"9.5\"}},\"htmlTheme\":\"dark\",\"meta\":{\"author\":null,\"subject\":[1],\"title\":12},\"toc\":{\"enabled\":1}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"right\",\"date\":\"\",\"enabled\":true,\"subtitle\":\"\",\"title\":\"3.5\"},\"footer\":{\"pageNumbers\":false,\"text\":\"y\"},\"header\":{\"align\":\"justify\",\"text\":\"x\"},\"headings\":{\"h1\":{\"after\":40,\"align\":\"right\",\"before\":0,\"bold\":false,\"color\":\"#000000\",\"pageBreakBefore\":true,\"size\":40},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":9.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"dark\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"[1]\",\"title\":\"12\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":true},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("page_sizes", "sanitize",
            "{\"page\":{\"orientation\":\"landscape\",\"size\":\"Letter\"},\"table\":{\"align\":\"center\",\"headerBold\":false}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"landscape\",\"size\":\"Letter\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"center\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":false,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[279.4,215.9]"),
        ("custom_page", "sanitize",
            "{\"page\":{\"height\":400,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":\"91.4\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":400.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":91.4},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[91.4,400.0]"),
        ("margin_keep", "sanitize",
            "{\"page\":{\"marginBottom\":58,\"marginLeft\":59,\"marginRight\":60,\"marginTop\":55,\"orientation\":\"landscape\",\"size\":\"A5\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":58.0,\"marginLeft\":59.0,\"marginRight\":60,\"marginTop\":55.0,\"orientation\":\"landscape\",\"size\":\"A5\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,148]"),
        ("margin_rescale", "sanitize",
            "{\"page\":{\"height\":80,\"marginBottom\":60,\"marginLeft\":55,\"marginRight\":58,\"marginTop\":60,\"size\":\"Custom\",\"width\":80}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":80,\"marginBottom\":25.0,\"marginLeft\":24.336283185840706,\"marginRight\":25.663716814159294,\"marginTop\":25.0,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":80},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[80,80]"),
        // Hand-spliced from live CPython (scratch/rust_parity/export_styles_fix_s12) to
        // exercise the _clamp width/height literals 210/297 -- see REPORT.md.
        ("custom_page_invalid_width", "sanitize",
            "{\"page\":{\"size\":\"Custom\",\"width\":\"abc\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":210},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297.0]"),
        ("custom_page_invalid_both", "sanitize",
            "{\"page\":{\"height\":\"1e400\",\"size\":\"Custom\",\"width\":\"nan\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":210},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("custom_page_missing_dimensions", "sanitize",
            "{\"page\":{\"size\":\"Custom\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210.0,297.0]"),
        ("custom_page_invalid_width_margins_overflow", "sanitize",
            "{\"page\":{\"height\":\"zz\",\"marginBottom\":60,\"marginLeft\":60,\"marginRight\":60,\"marginTop\":60,\"size\":\"Custom\",\"width\":\"abc\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297,\"marginBottom\":60,\"marginLeft\":60,\"marginRight\":60,\"marginTop\":60,\"orientation\":\"portrait\",\"size\":\"Custom\",\"width\":210},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("extra_keys_and_nested", "sanitize",
            "{\"headings\":{\"h3\":{\"extra\":[1,{\"z\":2}],\"size\":13}},\"typography\":{\"size\":14,\"unknownKey\":2},\"unknownSection\":{\"a\":1}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"extra\":[1,{\"z\":2}],\"pageBreakBefore\":false,\"size\":13.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":14.0,\"spacing\":6.0,\"unknownKey\":2},\"unknownSection\":{\"a\":1}}",
            "[210,297]"),
        ("unicode_and_truncate", "sanitize",
            "{\"cover\":{\"title\":\"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\"},\"footer\":{\"text\":\"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\"},\"header\":{\"text\":\"\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\"},\"htmlTheme\":\"sepia\",\"meta\":{\"title\":\"tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt\"}}",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\"},\"footer\":{\"pageNumbers\":true,\"text\":\"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\"},\"header\":{\"align\":\"left\",\"text\":\"\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\\u6c49\\u5b57a\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"sepia\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("preset_minimal", "preset",
            "\"minimal\"",
            "{\"code\":{\"bg\":\"#f7f8fa\",\"borderColor\":\"#e3e6ea\",\"borderWidth\":0.5,\"color\":\"#444444\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#111111\",\"pageBreakBefore\":false,\"size\":18.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#222222\",\"pageBreakBefore\":false,\"size\":14.5},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#333333\",\"pageBreakBefore\":false,\"size\":12.5},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#e0e3e8\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#1a73e8\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#9aa3af\",\"bg\":\"#f6f7f9\",\"color\":\"#555555\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":false,\"borderColor\":\"#ccd2da\",\"borderWidth\":0.5,\"cellPadding\":5.0,\"cellSize\":9.5,\"headerBg\":\"#eef1f5\",\"headerBold\":true,\"headerColor\":\"#333333\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#333333\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.55,\"size\":10.5,\"spacing\":6.0}}",
            "[210,297]"),
        ("preset_classic", "preset",
            "\"classic\"",
            "{\"code\":{\"bg\":\"#f4f4f0\",\"borderColor\":\"#c9c9c4\",\"borderWidth\":0.75,\"color\":\"#333333\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"center\",\"before\":18.0,\"bold\":true,\"color\":\"#000000\",\"pageBreakBefore\":false,\"size\":22.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#111111\",\"pageBreakBefore\":false,\"size\":17.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#222222\",\"pageBreakBefore\":false,\"size\":14.5},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#333333\",\"pageBreakBefore\":false,\"size\":13.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#b5b5ad\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#8a2be2\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#7a8699\",\"bg\":\"#f5f6f8\",\"color\":\"#3d4852\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f2f5f8\",\"banded\":true,\"borderColor\":\"#8a94a6\",\"borderWidth\":1.0,\"cellPadding\":7.0,\"cellSize\":10.5,\"headerBg\":\"#d9e2ec\",\"headerBold\":true,\"headerColor\":\"#1f2d3d\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#1a1a1a\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.8,\"size\":12.0,\"spacing\":8.0}}",
            "[210,297]"),
        ("preset_business", "preset",
            "\"business\"",
            "{\"code\":{\"bg\":\"#f0f4fa\",\"borderColor\":\"#c3d0e4\",\"borderWidth\":0.5,\"color\":\"#1f3864\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1f3864\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#2e5395\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#3a6db5\",\"pageBreakBefore\":false,\"size\":13.5},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#4a7fd4\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#b9c6da\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#1f3864\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#1f3864\",\"bg\":\"#eef3fa\",\"color\":\"#34507c\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#eef3fa\",\"banded\":true,\"borderColor\":\"#9fb3d1\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#1f3864\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#2c3e50\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.65,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
        ("preset_unknown", "preset",
            "\"nonsense\"",
            "{\"code\":{\"bg\":\"#f5f6f8\",\"borderColor\":\"#dfe3e8\",\"borderWidth\":0.5,\"color\":\"#2f3b4a\",\"font\":\"Consolas\",\"rounded\":true,\"size\":9.5},\"cover\":{\"align\":\"center\",\"date\":\"\",\"enabled\":false,\"subtitle\":\"\",\"title\":\"\"},\"footer\":{\"pageNumbers\":true,\"text\":\"\"},\"header\":{\"align\":\"left\",\"text\":\"\"},\"headings\":{\"h1\":{\"after\":10.0,\"align\":\"left\",\"before\":18.0,\"bold\":true,\"color\":\"#1a1a1a\",\"pageBreakBefore\":false,\"size\":20.0},\"h2\":{\"after\":8.0,\"align\":\"left\",\"before\":14.0,\"bold\":true,\"color\":\"#1f2937\",\"pageBreakBefore\":false,\"size\":16.0},\"h3\":{\"after\":6.0,\"align\":\"left\",\"before\":12.0,\"bold\":true,\"color\":\"#2d3748\",\"pageBreakBefore\":false,\"size\":14.0},\"h4\":{\"after\":6.0,\"align\":\"left\",\"before\":10.0,\"bold\":true,\"color\":\"#374151\",\"pageBreakBefore\":false,\"size\":12.0},\"h5\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":11.0},\"h6\":{\"after\":4.0,\"align\":\"left\",\"before\":8.0,\"bold\":true,\"color\":\"#4a5568\",\"pageBreakBefore\":false,\"size\":10.5}},\"hr\":{\"color\":\"#d8dce2\"},\"htmlTheme\":\"light\",\"images\":{\"maxHeightPct\":85.0,\"widthPct\":92.0},\"link\":{\"color\":\"#2b6cb0\"},\"math\":{\"dpi\":220},\"meta\":{\"author\":\"\",\"subject\":\"\",\"title\":\"\"},\"page\":{\"height\":297.0,\"marginBottom\":20.0,\"marginLeft\":18.0,\"marginRight\":18.0,\"marginTop\":20.0,\"orientation\":\"portrait\",\"size\":\"A4\",\"width\":210.0},\"quote\":{\"barColor\":\"#3b6ef5\",\"bg\":\"#f3f6ff\",\"color\":\"#4a5568\"},\"table\":{\"align\":\"left\",\"bandColor\":\"#f3f5f9\",\"banded\":true,\"borderColor\":\"#c8cdd4\",\"borderWidth\":0.75,\"cellPadding\":6.0,\"cellSize\":10.0,\"headerBg\":\"#3b6ef5\",\"headerBold\":true,\"headerColor\":\"#ffffff\",\"widthPct\":100},\"toc\":{\"enabled\":false},\"typography\":{\"align\":\"left\",\"color\":\"#262626\",\"firstLineIndent\":0,\"font\":\"MicrosoftYaHei\",\"lineHeight\":1.6,\"size\":11.0,\"spacing\":6.0}}",
            "[210,297]"),
    ];

    #[test]
    fn sanitize_matches_python_golden_table() {
        let mut bad = Vec::new();
        for (name, kind, arg, want, want_dims) in GOLDEN {
            let options: Value = serde_json::from_str(arg).unwrap();
            let got = if *kind == "preset" {
                preset_style(options.as_str().unwrap_or(""))
            } else {
                sanitize(&options)
            };
            let want_value: Value = serde_json::from_str(want).unwrap();
            if !golden_eq(&got, &want_value) {
                bad.push(format!(
                    "{}:\n  got ={}\n  want={}",
                    name,
                    compact(&got),
                    compact(&want_value)
                ));
                continue;
            }
            let dims: Vec<f64> = serde_json::from_str::<Vec<f64>>(want_dims).unwrap();
            let (w, h) = page_dimensions(&got);
            if [w, h] != dims[..] {
                bad.push(format!("{}: dims got {:?} want {:?}", name, [w, h], dims));
            }
        }
        assert!(!GOLDEN.is_empty(), "golden table was not generated");
        assert!(bad.is_empty(), "sanitize diverges:\n{}", bad.join("\n"));
    }

    /// serde_json renders objects in sorted-key order (BTreeMap default, the
    /// `preserve_order` feature is not enabled), which is exactly what
    /// `json.dumps(sort_keys=True)` produced, so the two are comparable as
    /// parsed values *and* as text.
    fn compact(v: &Value) -> String {
        serde_json::to_string(v).unwrap()
    }

    /// Compare a computed tree with a golden literal.
    ///
    /// serde_json's *default* float parser (the `float_roundtrip` feature is
    /// off) is only good to about 1 ULP: `from_str("24.336283185840706")`
    /// yields `0x40385616a7a56169` (`24.336283185840703`) whereas CPython's and
    /// Rust's computed value is `0x40385616a7a5616a`.  The golden table carries
    /// CPython's digits, so the computed tree is re-serialised through that same
    /// parser first and both sides then carry the identical rounding error.
    /// Everything that is not a JSON float keeps being compared bit-strictly:
    /// the int/float taint of `_clamp`, the key sets, the strings and the order.
    fn golden_eq(got: &Value, want: &Value) -> bool {
        match serde_json::from_str::<Value>(&compact(got)) {
            Ok(canonical) => canonical == *want,
            // Only reachable for a value serde_json cannot re-read; fall back to
            // the strict comparison so the failure still surfaces.
            Err(_) => got == want,
        }
    }

    /// `golden_eq` must not become a blanket tolerance: a float-variant number
    /// still has to differ from the same numeric value stored as an integer.
    #[test]
    fn golden_eq_keeps_the_int_float_taint_strict() {
        assert!(golden_eq(&json!(600.0), &serde_json::from_str("600.0").unwrap()));
        assert!(!golden_eq(&json!(600.0), &serde_json::from_str("600").unwrap()));
        assert!(!golden_eq(&json!(600), &serde_json::from_str("600.0").unwrap()));
        assert!(!golden_eq(&json!(600.0), &serde_json::from_str("601.0").unwrap()));
        // the 1-ULP case that motivated the canonicalisation
        assert!(golden_eq(
            &json!(55.0 * 50.0 / 113.0),
            &serde_json::from_str("24.336283185840706").unwrap()
        ));
    }

    /// The golden rows are compared as parsed `Value`s, so they only prove
    /// `_clamp`'s int/float taint while the harness itself distinguishes `600`
    /// from `600.0`.  serde_json keeps `Number::from(i64)` and
    /// `Number::from(f64)` in different internal variants; if that ever changes,
    /// this test fires before the golden table can silently stop
    /// discriminating.
    #[test]
    fn golden_harness_distinguishes_integers_from_floats() {
        assert_ne!(json!(600), json!(600.0));
        assert_ne!(json!(11), json!(11.0));
        assert_ne!(json!(0), json!(0.0));
        assert_eq!(compact(&json!(600)), "600");
        assert_eq!(compact(&json!(210.0)), "210.0");
    }

    #[test]
    fn clamp_returns_the_winning_argument_object() {
        // int bounds pin to ints, in-range values become floats.
        assert_eq!(clamp(Some(&json!("700")), &json!(80), &json!(600), &json!(210)), json!(600));
        assert_eq!(clamp(Some(&json!(-5)), &json!(80), &json!(600), &json!(210)), json!(80));
        assert_eq!(
            clamp(Some(&json!(210)), &json!(80), &json!(600), &json!(210)),
            json!(210.0)
        );
        // a tie at a bound yields the literal, not a float
        assert_eq!(clamp(Some(&json!(8)), &json!(8), &json!(20), &json!(11)), json!(8));
        assert_eq!(clamp(Some(&json!(20)), &json!(8), &json!(20), &json!(11)), json!(20));
        assert_eq!(clamp(Some(&json!(2.5)), &json!(1.0), &json!(2.5), &json!(1.6)), json!(2.5));
        assert_eq!(clamp(Some(&json!("abc")), &json!(0), &json!(3), &json!(0.75)), json!(0.75));
        assert_eq!(clamp(Some(&json!(null)), &json!(0), &json!(3), &json!(2)), json!(2));
    }

    #[test]
    fn clamp_rejects_every_non_finite_float_grammar() {
        for text in ["nan", "NaN", "-inf", "Infinity", "inf"] {
            assert_eq!(
                clamp(Some(&json!(text)), &json!(8), &json!(20), &json!(11)),
                json!(11),
                "float({:?}) must fall back to the default",
                text
            );
        }
    }

    #[test]
    fn clamp_float_grammar_matches_cpython() {
        assert_eq!(clamp(Some(&json!(" 12 ")), &json!(0), &json!(60), &json!(20)), json!(12.0));
        assert_eq!(clamp(Some(&json!("1_2")), &json!(0), &json!(60), &json!(20)), json!(12.0));
        assert_eq!(clamp(Some(&json!("_12")), &json!(0), &json!(60), &json!(20)), json!(20));
        assert_eq!(clamp(Some(&json!("+1e1")), &json!(0), &json!(60), &json!(20)), json!(10.0));
        assert_eq!(clamp(Some(&json!(true)), &json!(0), &json!(60), &json!(20)), json!(1.0));
        assert_eq!(clamp(Some(&json!(false)), &json!(1), &json!(60), &json!(20)), json!(1));
        assert_eq!(clamp(Some(&json!([])), &json!(0), &json!(60), &json!(20)), json!(20));
        assert_eq!(clamp(Some(&json!({"a": 1})), &json!(0), &json!(60), &json!(20)), json!(20));
    }

    #[test]
    fn deep_merge_skips_nulls_and_recurses() {
        let base = json!({"a": {"b": 1, "c": 2}, "d": 3});
        assert_eq!(
            deep_merge(&base, &json!({"a": {"b": null, "e": 9}, "f": [1]})),
            json!({"a": {"b": 1, "c": 2, "e": 9}, "d": 3, "f": [1]})
        );
        // non-dict `over` returns a copy of `base` untouched
        assert_eq!(deep_merge(&base, &json!("nope")), base);
        assert_eq!(deep_merge(&base, &json!(null)), base);
        // a dict replacing a scalar, and a scalar replacing a dict
        assert_eq!(deep_merge(&json!({"a": 1}), &json!({"a": {"b": 2}})), json!({"a": {"b": 2}}));
        assert_eq!(deep_merge(&json!({"a": {"b": 2}}), &json!({"a": 1})), json!({"a": 1}));
    }

    #[test]
    fn sanitize_never_aliases_the_defaults() {
        let mut first = sanitize(&json!({}));
        first["typography"]["size"] = json!(999);
        first["headings"]["h1"]["color"] = json!("#000000");
        first["unknown"] = json!(1);
        let second = sanitize(&json!({}));
        assert_eq!(second["typography"]["size"], json!(11.0));
        assert_eq!(second["headings"]["h1"]["color"], json!("#1a1a1a"));
        assert_eq!(second.get("unknown"), None);
    }

    #[test]
    fn sanitize_replaces_non_dict_sections_with_the_schema() {
        let junk = json!({"page": 5, "typography": "nope", "headings": [], "cover": 0,
                          "table": null, "code": true, "quote": 1.5, "htmlTheme": "neon"});
        let s = sanitize(&junk);
        assert_eq!(compact(&s), compact(&sanitize(&json!({}))));
    }

    #[test]
    fn sanitize_keeps_unknown_keys() {
        let s = sanitize(&json!({"unknownSection": {"a": 1},
                                 "typography": {"unknownKey": 2, "size": 14}}));
        assert_eq!(s["unknownSection"], json!({"a": 1}));
        assert_eq!(s["typography"]["unknownKey"], json!(2));
        assert_eq!(s["typography"]["size"], json!(14.0));
    }

    #[test]
    fn hex_colors_keep_their_case_and_their_trailing_newline() {
        assert_eq!(hex(Some(&json!("#0F0F0F")), "#262626"), "#0F0F0F");
        assert_eq!(hex(Some(&json!("#ffffff\n")), "#262626"), "#ffffff\n");
        for bad in ["#fff", "#gggggg", "ffffff", "", " #ffffff", "#ffffff\n\n", "#fffffff"] {
            assert_eq!(hex(Some(&json!(bad)), "#262626"), "#262626", "{:?}", bad);
        }
        assert!(!re_match_hex("#12345"));
        assert!(re_match_hex("#1a2B3c"));
    }

    #[test]
    fn font_and_mono_whitelists() {
        assert_eq!(font(Some(&json!("SimHei"))), "SimHei");
        assert_eq!(font(Some(&json!("Microsoft YaHei"))), "MicrosoftYaHei");
        assert_eq!(font(Some(&json!(null))), "MicrosoftYaHei");
        assert_eq!(
            choose(Some(&json!("Consolas")), MONO, "Consolas"),
            "Consolas"
        );
        assert_eq!(choose(Some(&json!("Courier New")), MONO, "Consolas"), "Courier New");
        assert_eq!(choose(Some(&json!("monospace")), MONO, "Consolas"), "Consolas");
        assert_eq!(choose(Some(&json!(5)), MONO, "Consolas"), "Consolas");
    }

    #[test]
    fn string_fields_truncate_by_code_point() {
        let s = sanitize(&json!({"header": {"text": "汉字a".repeat(45)},
                                 "footer": {"text": "f".repeat(140)},
                                 "meta": {"title": "t".repeat(140)},
                                 "cover": {"title": "c".repeat(140)}}));
        assert_eq!(s["header"]["text"].as_str().unwrap().chars().count(), 120);
        assert_eq!(s["header"]["text"].as_str().unwrap(), "汉字a".repeat(40));
        assert_eq!(s["footer"]["text"].as_str().unwrap().chars().count(), 80);
        assert_eq!(s["meta"]["title"].as_str().unwrap().chars().count(), 120);
        assert_eq!(s["cover"]["title"].as_str().unwrap(), "c".repeat(120));
    }

    #[test]
    fn string_fields_use_python_str_not_display() {
        let s = sanitize(&json!({"cover": {"title": 3.5, "enabled": 1},
                                 "meta": {"title": 12, "subject": [1]},
                                 "footer": {"text": "y", "pageNumbers": 0},
                                 "toc": {"enabled": 1}}));
        assert_eq!(s["cover"]["title"], json!("3.5"));
        assert_eq!(s["cover"]["enabled"], json!(true));
        assert_eq!(s["meta"]["title"], json!("12"));
        assert_eq!(s["meta"]["subject"], json!("[1]"));
        assert_eq!(s["footer"]["text"], json!("y"));
        assert_eq!(s["footer"]["pageNumbers"], json!(false));
        assert_eq!(s["toc"]["enabled"], json!(true));
    }

    #[test]
    fn heading_rows_get_their_own_defaults() {
        let s = sanitize(&json!({"headings": {"h2": "junk", "h4": {}, "h6": {"size": "9.5"},
                                              "h1": {"size": 99, "bold": 0, "before": -3,
                                                     "after": 900, "pageBreakBefore": true}}}));
        assert_eq!(s["headings"]["h1"]["size"], json!(40));
        assert_eq!(s["headings"]["h1"]["bold"], json!(false));
        assert_eq!(s["headings"]["h1"]["before"], json!(0));
        assert_eq!(s["headings"]["h1"]["after"], json!(40));
        assert_eq!(s["headings"]["h1"]["pageBreakBefore"], json!(true));
        assert_eq!(s["headings"]["h2"]["size"], json!(16.0));
        assert_eq!(s["headings"]["h2"]["pageBreakBefore"], json!(false));
        assert_eq!(s["headings"]["h4"]["size"], json!(12.0));
        assert_eq!(s["headings"]["h6"]["size"], json!(9.5));
        // the h6 *default* is the float 10.5, so an unparsable size keeps it
        let s = sanitize(&json!({"headings": {"h6": {"size": "x"}}}));
        assert_eq!(s["headings"]["h6"]["size"], json!(10.5));
    }

    #[test]
    fn page_dimensions_follow_size_and_orientation() {
        let dims = |size: &str, orientation: &str, w: Value, h: Value| {
            page_dimensions(&json!({"page": {"size": size, "orientation": orientation,
                                             "width": w, "height": h}}))
        };
        assert_eq!(dims("A3", "portrait", json!(210), json!(297)), (297.0, 420.0));
        assert_eq!(dims("A4", "landscape", json!(210), json!(297)), (297.0, 210.0));
        assert_eq!(dims("A5", "landscape", json!(210), json!(297)), (210.0, 148.0));
        assert_eq!(dims("B5", "portrait", json!(210), json!(297)), (176.0, 250.0));
        assert_eq!(dims("Letter", "portrait", json!(210), json!(297)), (215.9, 279.4));
        assert_eq!(dims("Legal", "portrait", json!(210), json!(297)), (215.9, 355.6));
        assert_eq!(dims("Custom", "portrait", json!(91.4), json!(400)), (91.4, 400.0));
        assert_eq!(dims("Nonsense", "portrait", json!(1), json!(2)), (210.0, 297.0));
    }

    #[test]
    fn margins_are_rescaled_proportionally_only_when_they_overflow() {
        let keep = sanitize(&json!({"page": {"size": "A5", "orientation": "landscape",
                                             "marginTop": 55, "marginBottom": 58,
                                             "marginLeft": 59, "marginRight": 60}}));
        assert_eq!(keep["page"]["marginLeft"], json!(59.0));
        assert_eq!(keep["page"]["marginRight"], json!(60));
        assert_eq!(keep["page"]["marginTop"], json!(55.0));

        let scaled = sanitize(&json!({"page": {"size": "Custom", "width": 80, "height": 80,
                                               "marginTop": 60, "marginBottom": 60,
                                               "marginLeft": 55, "marginRight": 58}}));
        assert_eq!(scaled["page"]["marginLeft"], json!(55.0 * 50.0 / 113.0));
        assert_eq!(scaled["page"]["marginRight"], json!(58.0 * 50.0 / 113.0));
        assert_eq!(scaled["page"]["marginTop"], json!(25.0));
        assert_eq!(scaled["page"]["marginBottom"], json!(25.0));
    }

    /// `styles.py:132-139` returns its **fourth argument** when `float()` fails
    /// or the value is non-finite, so an invalid `Custom` width can only ever be
    /// the literal `210` — the user's own value is unreachable.  Echoing it let
    /// `page_dimensions` see `0.0`, which drove the rescale at
    /// `styles.py:228-231` to `margin * (0 - 30) / total` = **negative** margins.
    #[test]
    fn clamp_fallback_is_the_schema_literal_not_the_user_value() {
        let s = sanitize(&json!({"page": {"size": "Custom", "width": "abc"}}));
        assert_eq!(s["page"]["width"], json!(210));
        assert_eq!(s["page"]["height"], json!(297.0));
        assert_eq!(page_dimensions(&s), (210.0, 297.0));
        assert_eq!(s["page"]["marginLeft"], json!(18.0));
        assert_eq!(s["page"]["marginBottom"], json!(20.0));

        // `"nan"` and `"1e400"` parse but are non-finite, so *both* defaults are
        // the int literals 210/297 — unlike a value merged in from DEFAULT_STYLE.
        let s = sanitize(&json!({"page": {"size": "Custom", "width": "nan", "height": "1e400"}}));
        assert_eq!(s["page"]["width"], json!(210));
        assert_eq!(s["page"]["height"], json!(297));

        // 60 + 60 cannot overflow a 210mm page, so no rescale happens at all.
        let s = sanitize(&json!({"page": {"size": "Custom", "width": "abc", "height": "zz",
                                         "marginTop": 60, "marginBottom": 60,
                                         "marginLeft": 60, "marginRight": 60}}));
        for key in ["marginLeft", "marginRight", "marginTop", "marginBottom"] {
            assert_eq!(s["page"][key], json!(60), "{}", key);
            assert!(s["page"][key].as_f64().unwrap() >= 0.0, "{}", key);
        }

        // keys only defaulted by `deep_merge` are in range, so they come back as
        // floats rather than as `_clamp`'s default object.
        let s = sanitize(&json!({"page": {"size": "Custom"}}));
        assert_eq!(s["page"]["width"], json!(210.0));
        assert_eq!(s["page"]["height"], json!(297.0));
    }

    #[test]
    fn dpi_is_truncated_to_an_integer() {
        assert_eq!(sanitize(&json!({"math": {"dpi": 220.7}}))["math"]["dpi"], json!(220));
        assert_eq!(sanitize(&json!({"math": {"dpi": "300.9"}}))["math"]["dpi"], json!(300));
        assert_eq!(sanitize(&json!({"math": {"dpi": 900}}))["math"]["dpi"], json!(500));
        assert_eq!(sanitize(&json!({"math": {"dpi": "x"}}))["math"]["dpi"], json!(220));
    }

    #[test]
    fn preset_style_matches_the_authority() {
        for name in preset_names() {
            let s = preset_style(name);
            // A preset is a *delta*: keys it does not mention keep their default.
            assert_eq!(s["page"]["size"], json!("A4"));
            assert_eq!(s["typography"]["font"], json!("MicrosoftYaHei"));
        }
        assert_eq!(compact(&preset_style("nonsense")), compact(&sanitize(&json!({}))));
        assert_eq!(preset_style("minimal")["typography"]["lineHeight"], json!(1.55));
        assert_eq!(preset_style("classic")["table"]["cellPadding"], json!(7.0));
        assert_eq!(preset_style("business")["link"]["color"], json!("#1f3864"));
        assert_eq!(preset_style("minimal")["headings"]["h4"]["size"], json!(12.0));
    }

    #[test]
    fn presets_cover_every_key_shape_the_renderers_read() {
        // A missing section here would silently change every DOCX export.
        for name in preset_names() {
            let delta = preset(name).unwrap();
            for key in delta.as_object().unwrap().keys() {
                assert!(
                    default_style().get(key).is_some() || key == &"unknown",
                    "preset {} touches unknown section {}",
                    name,
                    key
                );
            }
        }
        assert_eq!(preset("Minimal"), None);
    }

    #[test]
    fn sanitize_accepts_options_of_every_json_type() {
        let want = compact(&sanitize(&json!({})));
        for value in [json!(null), json!(5), json!("abc"), json!([]), json!(0), json!("")] {
            assert_eq!(compact(&sanitize(&value)), want, "{:?}", value);
        }
        assert_eq!(compact(&sanitize_options(None)), want);
        assert_eq!(compact(&sanitize_options(Some(&json!({"toc": {"enabled": true}})))),
                   compact(&sanitize(&json!({"toc": {"enabled": true}}))));
    }
}
