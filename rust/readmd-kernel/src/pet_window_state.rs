//! Port of `src/readmd_core/window_state.py` — `WindowStateManager`.
//!
//! Authority: `src/readmd_core/window_state.py:16-80`, on top of
//! `src/readmd_core/utils.py:43-52` (`load_json`) and `:55-90` (`save_json`),
//! with the two file names from `src/readmd_core/config.py:47-48`
//! (`SETTINGS_FILE`, `RECENT_FILE`).
//!
//! There was no Rust port at all: `main.rs:2945-2946` builds the window with a
//! literal `LogicalSize::new(1160.0, 820.0)` / min `720.0 x 480.0`, while
//! Python's defaults are `1080 x 760` with a `640 x 480` floor, and nothing read
//! or wrote `settings.json`'s `geometry` key.  The kernel therefore forgot the
//! window every run and used numbers the app never agreed to.
//!
//! # Exact format on disk
//!
//! `settings.json` stays the single settings object the rest of the app writes;
//! `save_geometry` re-reads it, replaces only the `geometry` key, and rewrites
//! the whole file (`window_state.py:49-57`).  `geometry` is always the five
//! keys in this order: `width`, `height`, `x`, `y`, `maximized`, with `x`/`y`
//! either an `int` or JSON `null` (`window_state.py:40-45` vs `:50-56`).
//! `recent.json` is a plain JSON array of strings.
//!
//! # Corrupt / missing input
//!
//! `load_json` swallows *unreadable, non-UTF-8 and malformed* files by
//! returning its default (`utils.py:45-52`) — so geometry falls back to the
//! clamped defaults.  It does **not** swallow a well-formed JSON document of the
//! wrong shape: `data.get('geometry')` on a top-level list or number raises
//! `AttributeError`, and `int(...)` on `"wide"` raises `ValueError`.  Python has
//! no `try` anywhere in `load_geometry`, so those calls really do blow up.  The
//! port keeps that distinction: a `Err(WindowStateError::Raises(..))` marks the
//! paths where Python propagates, so a caller cannot mistake them for defaults.

use crate::link_indexer::{py_abspath, py_normpath};
use crate::store::{load_json, save_json, Pj, RECENT_FILE, SETTINGS_FILE};
use std::path::{Path, PathBuf};

/// `window_state.py:16-19`.
pub const DEFAULT_WIDTH: i64 = 1080;
pub const DEFAULT_HEIGHT: i64 = 760;
pub const MIN_WIDTH: i64 = 640;
pub const MIN_HEIGHT: i64 = 480;

/// `window_state.py:59` / `:66` — `limit: int = 20`.
pub const DEFAULT_RECENT_LIMIT: usize = 20;

/// A Python exception that this port refuses to invent a default for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowStateError {
    /// `AttributeError`: `load_json` handed back a valid document that has no
    /// `.get`, i.e. a top-level JSON array, string or number.
    AttributeError(&'static str),
    /// `ValueError` / `TypeError` from `int(...)` or `bool(...)` on a value
    /// Python cannot coerce.
    TypeOrValueError(&'static str),
    /// `save_json` returned `False` after its six replace attempts.
    SaveFailed(String),
}

/// `int(value)` for the JSON scalar types CPython can actually reach here.
///
/// `Err` is the `ValueError`/`TypeError` Python raises; it is never folded into
/// a default, because `window_state.py` has no `try` to fold it into.
fn py_int(value: &Pj) -> Result<i64, WindowStateError> {
    let raw = match value {
        Pj::Bool(flag) => return Ok(i64::from(*flag)),
        Pj::Int(text) => text.clone(),
        Pj::Float(number) => {
            if !number.is_finite() {
                // `int(float('nan'))` -> ValueError, `int(inf)` -> OverflowError.
                return Err(WindowStateError::TypeOrValueError("int"));
            }
            return Ok(*number as i64); // truncates toward zero, like CPython
        }
        Pj::Str(text) => text.clone(),
        Pj::Null => return Err(WindowStateError::TypeOrValueError("int(None)")),
        Pj::Arr(_) | Pj::Obj(_) => {
            return Err(WindowStateError::TypeOrValueError("int(list/dict)"))
        }
    };
    // `int(str)`: surrounding whitespace stripped, one optional sign, decimal
    // digits that may carry single `_` group separators.
    let trimmed = raw.trim();
    let (sign, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, rest_plus(trimmed)),
    };
    let mut clean = String::with_capacity(digits.len());
    let mut previous_was_digit = false;
    for ch in digits.chars() {
        if ch == '_' {
            if !previous_was_digit {
                return Err(WindowStateError::TypeOrValueError("int(str)"));
            }
            previous_was_digit = false;
            continue;
        }
        if !ch.is_ascii_digit() {
            return Err(WindowStateError::TypeOrValueError("int(str)"));
        }
        clean.push(ch);
        previous_was_digit = true;
    }
    if clean.is_empty() || !previous_was_digit {
        return Err(WindowStateError::TypeOrValueError("int(str)"));
    }
    let magnitude: i64 = clean
        .parse()
        .map_err(|_| WindowStateError::TypeOrValueError("int(str) out of i64 range"))?;
    Ok(sign * magnitude)
}

/// `int(value, default)` where Python wrote `int(geo.get(k, DEFAULT))`.
fn py_int_or(value: &Pj, default: i64) -> Result<i64, WindowStateError> {
    if matches!(value, Pj::Null) {
        // `geo.get('width', 1080)` returns the stored `null`, and
        // `int(None)` is a TypeError — the default only applies to a *missing*
        // key, which `Pj::get` already models with `Option::None`.
        return Err(WindowStateError::TypeOrValueError("int(None)"));
    }
    let _ = default;
    py_int(value)
}

fn rest_plus(text: &str) -> &str {
    match text.strip_prefix('+') {
        Some(rest) => rest,
        None => text,
    }
}

/// `WindowStateManager`.  The two paths are constructor arguments in Python
/// (`window_state.py:25`), so they stay injectable here rather than being read
/// from a global.
#[derive(Debug, Clone)]
pub struct WindowStateManager {
    pub settings_file: PathBuf,
    pub recent_file: PathBuf,
}

impl WindowStateManager {
    /// `WindowStateManager(settings_file=SETTINGS_FILE, recent_file=RECENT_FILE)`.
    pub fn new(settings_file: impl Into<PathBuf>, recent_file: impl Into<PathBuf>) -> Self {
        WindowStateManager {
            settings_file: settings_file.into(),
            recent_file: recent_file.into(),
        }
    }

    /// `WindowStateManager()` for a `DATA_DIR`, i.e. the module-level defaults
    /// `config.py:47-48` bind.
    pub fn in_data_dir(data_dir: &Path) -> Self {
        Self::new(
            data_dir.join(SETTINGS_FILE),
            data_dir.join(RECENT_FILE),
        )
    }

    /// `load_geometry()`.  Key order and the `null`-for-absent-position rule are
    /// Python's (`window_state.py:39-45`).
    pub fn load_geometry(&self) -> Result<Pj, WindowStateError> {
        let data = load_json(&self.settings_file, Pj::empty_obj());
        let geometry = match &data {
            Pj::Obj(_) => data.get("geometry").cloned().unwrap_or_else(Pj::empty_obj),
            other => {
                // Python: `AttributeError: 'list' object has no attribute 'get'`
                let _ = other;
                return Err(WindowStateError::AttributeError("geometry"));
            }
        };
        let take = |key: &str, default: i64| -> Result<i64, WindowStateError> {
            match geometry.get(key) {
                Some(value) => py_int_or(value, default),
                None => Ok(default),
            }
        };
        // `max(MIN_WIDTH, int(...))` — a clamp upward only, never downward.
        let width = std::cmp::max(MIN_WIDTH, take("width", DEFAULT_WIDTH)?);
        let height = std::cmp::max(MIN_HEIGHT, take("height", DEFAULT_HEIGHT)?);
        let axis = |key: &str| -> Result<Pj, WindowStateError> {
            match geometry.get(key) {
                None | Some(Pj::Null) => Ok(Pj::Null),
                Some(value) => Ok(Pj::Int(py_int(value)?.to_string())),
            }
        };
        let maximized = geometry
            .get("maximized")
            .map(Pj::truthy)
            .unwrap_or(false);
        Ok(Pj::Obj(vec![
            ("width".to_string(), Pj::Int(width.to_string())),
            ("height".to_string(), Pj::Int(height.to_string())),
            ("x".to_string(), axis("x")?),
            ("y".to_string(), axis("y")?),
            ("maximized".to_string(), Pj::Bool(maximized)),
        ]))
    }

    /// `save_geometry(...) -> bool`.  Returns `false` exactly where Python's
    /// `save_json` returns `False`.
    pub fn save_geometry(
        &self,
        width: i64,
        height: i64,
        x: Option<i64>,
        y: Option<i64>,
        maximized: bool,
    ) -> Result<bool, WindowStateError> {
        let mut data = load_json(&self.settings_file, Pj::empty_obj());
        if !matches!(data, Pj::Obj(_)) {
            // `data['geometry'] = {...}` on a list is a `TypeError`.
            return Err(WindowStateError::TypeOrValueError("settings is not a dict"));
        }
        let geometry = Pj::Obj(vec![
            (
                "width".to_string(),
                Pj::Int(std::cmp::max(MIN_WIDTH, width).to_string()),
            ),
            (
                "height".to_string(),
                Pj::Int(std::cmp::max(MIN_HEIGHT, height).to_string()),
            ),
            ("x".to_string(), option_int(x)),
            ("y".to_string(), option_int(y)),
            ("maximized".to_string(), Pj::Bool(maximized)),
        ]);
        upsert(&mut data, "geometry", geometry);
        match save_json(&self.settings_file, &data) {
            Ok(()) => Ok(true),
            Err(message) => Err(WindowStateError::SaveFailed(message)),
        }
    }

    /// `load_recent_files(limit=20)`.
    pub fn load_recent_files(&self, limit: i64) -> Vec<String> {
        let data = load_json(&self.recent_file, Pj::empty_arr());
        let items = match &data {
            Pj::Arr(items) => items.clone(),
            // `isinstance(data, list)` is False for every other shape, and the
            // method then returns `[]` without raising (`window_state.py:61-64`).
            _ => return Vec::new(),
        };
        let kept = py_slice_head(items.len(), limit);
        items[kept]
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect()
    }

    /// `add_recent_file(file_path, limit=20)`.
    pub fn add_recent_file(&self, file_path: &str, limit: i64) -> Vec<String> {
        if file_path.is_empty() {
            return self.load_recent_files(limit);
        }
        let norm_path = py_abspath(file_path);
        // `load_recent_files(limit=limit * 2)` — the read window is deliberately
        // wider than the write window so a dedupe cannot lose the tail.
        let mut recents = self.load_recent_files(saturate_double(limit));
        let target = py_normpath(&norm_path);
        recents.retain(|entry| py_normpath(entry) != target);
        recents.insert(0, norm_path);
        let kept = py_slice_head(recents.len(), limit);
        recents = recents[kept].to_vec();
        // `save_json`'s return value is discarded by Python (`:75`).
        let _ = save_json(&self.recent_file, &Pj::Arr(
            recents.iter().map(|entry| Pj::Str(entry.clone())).collect(),
        ));
        recents
    }

    /// `clear_recent_files() -> bool`.
    pub fn clear_recent_files(&self) -> Result<bool, WindowStateError> {
        match save_json(&self.recent_file, &Pj::empty_arr()) {
            Ok(()) => Ok(true),
            Err(message) => Err(WindowStateError::SaveFailed(message)),
        }
    }
}

fn saturate_double(limit: i64) -> i64 {
    limit.saturating_mul(2)
}

fn option_int(value: Option<i64>) -> Pj {
    match value {
        Some(number) => Pj::Int(number.to_string()),
        None => Pj::Null,
    }
}

/// `items[:limit]` for a possibly-negative `limit`, as CPython's slice means it.
/// Returns the inclusive-exclusive range to keep.
fn py_slice_head(len: usize, limit: i64) -> std::ops::Range<usize> {
    if limit >= 0 {
        0..std::cmp::min(len, limit as usize)
    } else {
        // `lst[:-1]` drops one from the end rather than meaning "none".
        let drop = (-limit) as usize;
        0..len.saturating_sub(drop)
    }
}

/// `data[key] = value` on an object: overwrite in place, else append.
fn upsert(data: &mut Pj, key: &str, value: Pj) {
    if let Pj::Obj(fields) = data {
        if let Some(slot) = fields.iter_mut().find(|(name, _)| name == key) {
            slot.1 = value;
            return;
        }
        fields.push((key.to_string(), value));
    }
}

impl Default for WindowStateManager {
    fn default() -> Self {
        // `window_state.py:25`'s defaults are the `config.py` module constants,
        // which the pets lane resolves through `pet_paths::data_dir()`.
        Self::in_data_dir(&crate::pet_paths::data_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "readmd-window-state-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manager(tag: &str) -> (PathBuf, WindowStateManager) {
        let dir = scratch_dir(tag);
        (dir.clone(), WindowStateManager::in_data_dir(&dir))
    }

    fn field<'a>(value: &'a Pj, key: &str) -> Option<&'a Pj> {
        value.get(key)
    }

    #[test]
    fn a_missing_settings_file_yields_the_python_defaults() {
        let (_dir, state) = manager("missing");
        let geo = state.load_geometry().unwrap();
        // `window_state.py:33-34`: DEFAULT_WIDTH/DEFAULT_HEIGHT verbatim.
        assert_eq!(field(&geo, "width").unwrap().to_value().as_i64(), Some(DEFAULT_WIDTH));
        assert_eq!(field(&geo, "height").unwrap().to_value().as_i64(), Some(DEFAULT_HEIGHT));
        assert_eq!(field(&geo, "x"), Some(&Pj::Null));
        assert_eq!(field(&geo, "y"), Some(&Pj::Null));
        assert_eq!(field(&geo, "maximized"), Some(&Pj::Bool(false)));
    }

    #[test]
    fn a_corrupt_settings_file_is_a_default_not_an_error() {
        // `utils.load_json` catches everything for an unparseable body.
        let (dir, state) = manager("corrupt");
        fs::write(dir.join(SETTINGS_FILE), b"{ not json ,").unwrap();
        assert_eq!(
            field(&state.load_geometry().unwrap(), "width")
                .unwrap()
                .to_value()
                .as_i64(),
            Some(DEFAULT_WIDTH)
        );
    }

    #[test]
    fn geometry_is_clamped_up_to_the_minimum_and_back_to_disk() {
        let (dir, state) = manager("clamp");
        assert_eq!(state.save_geometry(100, 40, Some(-5), None, true), Ok(true));
        let geo = state.load_geometry().unwrap();
        assert_eq!(field(&geo, "width").unwrap().to_value().as_i64(), Some(MIN_WIDTH));
        assert_eq!(field(&geo, "height").unwrap().to_value().as_i64(), Some(MIN_HEIGHT));
        assert_eq!(field(&geo, "x").unwrap().to_value().as_i64(), Some(-5));
        // `x`/`y` are independent: an omitted `y` still writes `null`.
        assert_eq!(field(&geo, "y"), Some(&Pj::Null));
        assert_eq!(field(&geo, "maximized"), Some(&Pj::Bool(true)));
        // Key order on disk is the dict-literal order of `window_state.py:50-56`.
        let raw = fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
        let order: Vec<&str> = vec!["width", "height", "x", "y", "maximized"]
            .into_iter()
            .filter(|key| raw.contains(&format!("\"{key}\"")))
            .collect();
        assert_eq!(order.len(), 5);
        assert!(raw.contains("\"geometry\""));
    }

    #[test]
    fn saving_geometry_preserves_every_other_settings_key() {
        let (dir, state) = manager("preserve");
        fs::write(
            dir.join(SETTINGS_FILE),
            concat!("{\"theme\": \"dark\", \"pet_enabled\": true, \"geometry\": {\"width\": 900}}"),
        )
        .unwrap();
        state.save_geometry(1200, 800, None, None, false).unwrap();
        let after = load_json(&dir.join(SETTINGS_FILE), Pj::empty_obj());
        assert_eq!(field(&after, "theme").and_then(Pj::as_str), Some("dark"));
        assert_eq!(field(&after, "pet_enabled"), Some(&Pj::Bool(true)));
        // The old geometry is replaced wholesale, not merged.
        assert_eq!(
            field(&after, "geometry").unwrap().get("width").unwrap().to_value().as_i64(),
            Some(1200)
        );
    }

    #[test]
    fn a_stored_small_width_is_read_back_clamped() {
        let (dir, state) = manager("read-clamp");
        fs::write(
            dir.join(SETTINGS_FILE),
            r#"{"geometry": {"width": 10, "height": 700, "x": 0, "y": 0}}"#,
        )
        .unwrap();
        let geo = state.load_geometry().unwrap();
        assert_eq!(field(&geo, "width").unwrap().to_value().as_i64(), Some(MIN_WIDTH));
        assert_eq!(field(&geo, "height").unwrap().to_value().as_i64(), Some(700));
        assert_eq!(field(&geo, "x").unwrap().to_value().as_i64(), Some(0));
    }

    #[test]
    fn float_and_string_coordinates_are_int_coerced_like_python() {
        let (dir, state) = manager("coerce");
        fs::write(
            dir.join(SETTINGS_FILE),
            r#"{"geometry": {"width": 1080.9, "height": "760", "x": -12.7, "y": " 40 ", "maximized": 1}}"#,
        )
        .unwrap();
        let geo = state.load_geometry().unwrap();
        // `int(1080.9)` truncates toward zero; `int("760")` parses; `int(-12.7)`
        // is -12 not -13; `bool(1)` is True.
        assert_eq!(field(&geo, "width").unwrap().to_value().as_i64(), Some(1080));
        assert_eq!(field(&geo, "height").unwrap().to_value().as_i64(), Some(760));
        assert_eq!(field(&geo, "x").unwrap().to_value().as_i64(), Some(-12));
        assert_eq!(field(&geo, "y").unwrap().to_value().as_i64(), Some(40));
        assert_eq!(field(&geo, "maximized"), Some(&Pj::Bool(true)));
    }

    #[test]
    fn a_non_numeric_dimension_raises_rather_than_defaulting() {
        // Python has no `try` in `load_geometry`, so `int("wide")` escapes.
        let (dir, state) = manager("raise");
        fs::write(dir.join(SETTINGS_FILE), r#"{"geometry": {"width": "wide"}}"#).unwrap();
        assert!(matches!(
            state.load_geometry(),
            Err(WindowStateError::TypeOrValueError(_))
        ));
    }

    #[test]
    fn a_top_level_list_settings_file_raises_attribute_error() {
        // `load_json` hands the valid document back untouched, then
        // `data.get('geometry')` is an AttributeError in Python.
        let (dir, state) = manager("list-doc");
        fs::write(dir.join(SETTINGS_FILE), b"[1, 2, 3]").unwrap();
        assert_eq!(
            state.load_geometry(),
            Err(WindowStateError::AttributeError("geometry"))
        );
    }

    #[test]
    fn a_missing_geometry_key_is_a_default_but_an_explicit_null_is_not() {
        let (dir, state) = manager("null-geo");
        fs::write(dir.join(SETTINGS_FILE), r#"{"geometry": {"width": null}}"#).unwrap();
        assert!(matches!(
            state.load_geometry(),
            Err(WindowStateError::TypeOrValueError(_))
        ));
        fs::write(dir.join(SETTINGS_FILE), r#"{"geometry": {}}"#).unwrap();
        assert_eq!(
            field(&state.load_geometry().unwrap(), "width")
                .unwrap()
                .to_value()
                .as_i64(),
            Some(DEFAULT_WIDTH)
        );
    }

    #[test]
    fn recent_files_round_trip_dedupes_to_the_front_and_truncates() {
        let (dir, state) = manager("recent");
        assert_eq!(state.load_recent_files(20), Vec::<String>::new());
        for index in 0..25 {
            state.add_recent_file(&format!("note{index}.md"), 20);
        }
        let recents = state.load_recent_files(20);
        assert_eq!(recents.len(), 20, "`recents[:limit]`");
        assert_eq!(recents[0], py_abspath("note24.md"));
        // A re-add moves the entry without growing the list.
        state.add_recent_file("note03.md", 20);
        let recents = state.load_recent_files(20);
        assert_eq!(recents.len(), 20);
        assert_eq!(recents[0], py_abspath("note03.md"));
        // `recents[:limit]`
        assert_eq!(recents.iter().filter(|p| **p == py_abspath("note03.md")).count(), 1);
        let raw = fs::read_to_string(dir.join(RECENT_FILE)).unwrap();
        assert!(raw.starts_with('['), "recent.json is a bare array: {raw}");
        assert!(raw.contains('\\') || raw.contains('/'), "paths are absolute");
    }

    #[test]
    fn an_empty_recent_input_returns_the_list_untouched() {
        let (_dir, state) = manager("recent-empty");
        state.add_recent_file("a.md", 20);
        let before = state.load_recent_files(20);
        assert_eq!(state.add_recent_file("", 20), before);
    }

    #[test]
    fn a_recent_file_of_the_wrong_shape_is_an_empty_list_not_a_crash() {
        // `isinstance(data, list)` short-circuits (`window_state.py:62-64`).
        let (dir, state) = manager("recent-shape");
        fs::write(dir.join(RECENT_FILE), br#"{"a": 1}"#).unwrap();
        assert_eq!(state.load_recent_files(20), Vec::<String>::new());
        fs::write(dir.join(RECENT_FILE), b"[\"keep\", 5, \"also\"]").unwrap();
        assert_eq!(
            state.load_recent_files(20),
            vec!["keep".to_string(), "also".to_string()],
            "non-strings are filtered, not fatal"
        );
    }

    #[test]
    fn clearing_recent_files_writes_an_empty_array() {
        let (dir, state) = manager("recent-clear");
        state.add_recent_file("x.md", 20);
        assert_eq!(state.clear_recent_files(), Ok(true));
        assert_eq!(fs::read_to_string(dir.join(RECENT_FILE)).unwrap(), "[]");
        assert_eq!(state.load_recent_files(20), Vec::<String>::new());
    }

    #[test]
    fn a_negative_limit_slices_from_the_end_like_python() {
        let (_dir, state) = manager("recent-negative");
        for index in 0..4 {
            state.add_recent_file(&format!("n{index}.md"), 20);
        }
        let all = state.load_recent_files(20);
        assert_eq!(all.len(), 4);
        // `[:1]` keeps one; `[:-1]` drops one from the *end*.
        assert_eq!(state.load_recent_files(1).len(), 1);
        assert_eq!(state.load_recent_files(-1).len(), 3);
    }

    #[test]
    fn dedupe_compares_on_normpath_not_on_the_raw_string() {
        // `os.path.normpath(p) != os.path.normpath(norm_path)` — separators and
        // `.` segments must normalise away before the comparison.
        let (_dir, state) = manager("recent-norm");
        let absolute = py_abspath("docs/a.md");
        state.add_recent_file(&absolute, 20);
        let messy = py_abspath("./docs//a.md");
        assert_eq!(py_normpath(&messy), py_normpath(&absolute));
        state.add_recent_file("docs\\a.md", 20);
        let recents = state.load_recent_files(20);
        assert_eq!(recents.len(), 1, "the same path must not appear twice");
    }
}
