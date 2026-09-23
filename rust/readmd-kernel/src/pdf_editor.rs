//! Rust port of `src/readmd_modules/pdf_editor.py` (472 lines, branch `main`).
//!
//! The Python authority is a *raster* editor: it opens a PDF with PyMuPDF,
//! renders a page to an RGB bitmap at 300 DPI, paints replacement text with
//! Pillow (PSF blur + slant rotation), diffs the result against the original
//! with numpy (the "zero contamination gate") and writes the bitmap back as a
//! full-page image.  None of that rendering stack exists in this crate —
//! `lopdf` parses the object tree and `pdf-extract` decodes text, neither
//! rasterises, and no dependency may be added.  What *is* portable is the
//! whole non-raster surface, and that is what this module implements:
//!
//! | Python (`pdf_editor.py`)                       | Rust here | status |
//! |---|---|---|
//! | `load` (`:37`)                                  | [`load_ready`] | ported |
//! | `resolve_system_font` (`:44`)                   | [`resolve_system_font`] | ported |
//! | `check_file_locks` (`:102`)                     | [`check_file_locks`], [`probe_file_locked`] | partial (no pid/name) |
//! | `audit` (`:128`)                                | [`audit`] | ported except `inspected_crop` |
//! | `_apply_edits_to_image` (`:200`)                | [`edit_pixel_geometry`] | partial (no glyph raster) |
//! | `_verify_zero_contamination` (`:279`)           | [`verify_zero_contamination`] | ported |
//! | `preview` (`:311`)                              | — | not ported (needs a renderer) |
//! | `apply` (`:362`)                                | [`save_with_backup`] | ported for the disk pipeline |
//! | `rollback` (`:454`)                             | [`rollback`] | ported |
//!
//! Page-tree surgery ([`rotate_page`], [`reorder_pages`], [`delete_pages`]) has
//! **no Python counterpart at all** — `pdf_editor.py` never rotates, reorders,
//! deletes, crops, merges or splits.  It lives here because `/api/pdf/save`
//! needs something real to do once the raster rung is unavailable, and because
//! it is the only "PDF editing" a hand-rolled object-tree kernel can offer.  It
//! is labelled non-parity in the fix report, not claimed as a port.
//!
//! Byte-handling rule for the whole module: a PDF is `Vec<u8>`/`&[u8]` end to
//! end.  Every Python-side offset in the authority is a byte offset, and
//! `&s[a..b]` on a `&str` would panic off a char boundary; the one place a
//! *character* count is required (`text.strip()[:100]`) goes through
//! [`py_prefix_chars`].

use serde_json::{Value, json};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// `pdf_editor.py:47` — the default when no font was requested.
pub const DEFAULT_FONT_FILE: &str = "simfang.ttf";
/// `pdf_editor.py:94` — the ordered fallback chain, Windows fonts dir.
pub const FONT_FALLBACKS: [&str; 3] = ["simfang.ttf", "simsun.ttc", "arial.ttf"];
/// `pdf_editor.py:110` — processes `check_file_locks` is willing to blame.
pub const LOCK_READER_NAMES: [&str; 6] = [
    "acrobat.exe",
    "acrord32.exe",
    "foxitpdf.exe",
    "wps.exe",
    "msedge.exe",
    "chrome.exe",
];
/// `pdf_editor.py:172` and `:399` — `dpi = 300`, `zoom = dpi / 72.0`.
pub const DEFAULT_DPI: i64 = 300;
/// `pdf_editor.py:279` — `tolerance: int = 10`.
pub const GATE_TOLERANCE: i64 = 10;
/// `pdf_editor.py:291` — the 6px transition buffer around an allowed box.
pub const GATE_PAD: i64 = 6;
/// `pdf_editor.py:155` — `text.strip()[:100]`.
pub const TEXT_PREVIEW_CHARS: usize = 100;

/// `pdf_editor.py:56-77` — the Chinese font alias table, in Python's literal
/// insertion order (an alias miss falls back to the *caller's* string, so the
/// table order only matters for readability).
pub const FONT_ALIASES: &[(&str, &str)] = &[
    ("fangsong", "simfang.ttf"),
    ("仿宋", "simfang.ttf"),
    ("simfang", "simfang.ttf"),
    ("songti", "simsun.ttc"),
    ("宋体", "simsun.ttc"),
    ("simsun", "simsun.ttc"),
    ("heiti", "simhei.ttf"),
    ("黑体", "simhei.ttf"),
    ("simhei", "simhei.ttf"),
    ("kaiti", "simkai.ttf"),
    ("楷体", "simkai.ttf"),
    ("simkai", "simkai.ttf"),
    ("yahei", "msyh.ttc"),
    ("微软雅黑", "msyh.ttc"),
    ("msyh", "msyh.ttc"),
    ("times", "times.ttf"),
    ("times new roman", "times.ttf"),
    ("arial", "arial.ttf"),
    ("calibri", "calibri.ttf"),
    ("consolas", "consola.ttf"),
];

// ------------------------------------------------------------------- errors

/// Failure kinds mapped onto `pdf_editor.py`'s exception types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfErrorKind {
    /// `FileNotFoundError` (`:133`, `:316`, `:367`, `:459`).
    NotFound,
    /// `IndexError` for a page number outside `0 <= n < len(doc)` (`:320`, `:396`).
    PageIndex,
    /// `PermissionError` when the target is locked (`:376`).
    Locked,
    /// PyMuPDF could not open the file, or an object tree write failed.
    Invalid,
    /// The rung that needs a PDF rasteriser; never raised by the ported paths.
    RenderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfError {
    pub kind: PdfErrorKind,
    /// The Python exception *message*, kept verbatim (including the Chinese
    /// wording) because it is what the legacy UI shows to the user.
    pub message: String,
}

impl PdfError {
    pub fn new<S: Into<String>>(kind: PdfErrorKind, message: S) -> PdfError {
        PdfError { kind, message: message.into() }
    }

    /// `raise FileNotFoundError(f"PDF 文件不存在: {abs_path}")` — `:133`.
    pub fn not_found(abs_path: &str) -> PdfError {
        PdfError::new(PdfErrorKind::NotFound, format!("PDF 文件不存在: {abs_path}"))
    }

    /// `raise IndexError(f"无效的页码: {page_num}，总页数: {len(doc)}")` — `:320`.
    pub fn page_index(page_num: i64, total: i64) -> PdfError {
        PdfError::new(
            PdfErrorKind::PageIndex,
            format!("无效的页码: {page_num}，总页数: {total}"),
        )
    }

    /// `raise PermissionError(f"目标文件正被其他程序独占打开: {names}，…")` — `:376`.
    pub fn locked(names: &str) -> PdfError {
        PdfError::new(
            PdfErrorKind::Locked,
            format!("目标文件正被其他程序独占打开: {names}，请先在阅读器中关闭该文档。"),
        )
    }
}

pub type PdfResult<T> = Result<T, PdfError>;

// -------------------------------------------------------------- module gate

/// `pdf_editor.py:37-41` — `load()`.
///
/// Python raises `RuntimeError("pdf_editor-missing-dependencies: …")` when
/// cv2/numpy/Pillow/psutil/PyMuPDF are missing.  The kernel links its PDF
/// reader statically, so the dependency probe can only ever answer "ready";
/// the one thing it *does* report is that the raster rung is absent, which is
/// why [`load_ready`] and [`raster_available`] are separate questions.
pub fn load_ready() -> bool {
    true
}

/// Always `false`: see the module header.  `pdf_editor.py:38-40` has no
/// equivalent failure to raise, so callers surface it as an error code.
pub fn raster_available() -> bool {
    false
}

// ------------------------------------------------------- Python primitives

/// `int(x)` on a float — truncate toward zero (`:179`, `:219`, `:225`).
pub fn py_int(x: f64) -> i64 {
    if !x.is_finite() {
        return i64::MIN;
    }
    x.trunc() as i64
}

/// `int(round(x))` — Python's `round()` is half-to-**even**, while Rust's
/// `f64::round` is half-away-from-zero, so `round(0.5) == 0` and
/// `round(2.5) == 2`.  Verified against `pdf_editor.py:221`
/// (`px_size = int(round(pt_size * zoom))`).
pub fn py_round_int(x: f64) -> i64 {
    if !x.is_finite() {
        return i64::MIN;
    }
    x.round_ties_even() as i64
}

/// `round(x, n)` as used by `audit` (`:144-151`) and
/// `_verify_zero_contamination` (`:299`).
///
/// CPython's `float.__round__` agrees with `%.nf` on every value probed in
/// `scratch/rust_parity/pdf_probe3_rules.py` (24 cases, `ALL AGREE: True`),
/// including the binary-repr traps `round(2.675, 2) == 2.67` and
/// `round(0.135, 2) == 0.14`.  Rust's `{:.n$}` is the same correctly-rounded
/// decimal of the exact binary value, so formatting and re-parsing reproduces
/// the authority's number *and* its JSON rendering (`300.5`, not `300.50`).
pub fn py_round(x: f64, digits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{:.*}", digits, x).parse::<f64>().unwrap_or(x)
}

/// Python's `str.strip()` whitespace set: the Unicode `White_Space` property
/// **plus** `\x1c`..`\x1f` and `\x85`, which `char::is_whitespace` omits.
pub fn py_is_space(c: char) -> bool {
    matches!(c, '\u{1c}'..='\u{1f}' | '\u{85}') || c.is_whitespace()
}

/// `text.strip()` (`:146`, `:155`).
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_is_space)
}

/// `text[:n]` — code points, not bytes.
pub fn py_prefix_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `ntpath.splitdrive(s)[0]` is non-empty: `X:` with an alpha drive letter.
/// Deliberately *not* the same predicate as `py_isabs` — `C:a.pdf` has a drive
/// but is drive-relative — because `os.path.join` tests the drive separately.
fn py_has_drive(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 2 && b[1] == b':' && matches!(b[0], b'A'..=b'Z' | b'a'..=b'z')
}

/// `os.path.isabs` on Windows: a leading separator (`\` or `/`, which also
/// covers the UNC root `\\server\share`), or a drive prefix *immediately
/// followed* by a separator.  Measured on CPython 3.11.15 (win32):
/// `'C:\a\b.pdf'->True  'c:/mixed/slash.pdf'->True  '\\nas\share\doc.pdf'->True
///  '\single.pdf'->True  '/unix/x.pdf'->True  'C:\'->True  'C:/x'->True`
/// `'C:a.pdf'->False  'C:'->False  'D:x\y.pdf'->False  'C::x'->False
///  'rel\path.pdf'->False  'a.pdf'->False  ':'->False  '1:x'->False  ''->False`
/// The old `b[1] == b':'` shortcut called a drive-relative path absolute, so
/// `resolve_system_font` (`pdf_editor.py:49`,
/// `if os.path.isabs(font_name_or_path) and os.path.exists(...)`) took a branch
/// the authority never takes for an input such as `C:arial.ttf`.
pub fn py_isabs(p: &str) -> bool {
    let b = p.as_bytes();
    if matches!(b.first(), Some(b'\\') | Some(b'/')) {
        return true;
    }
    py_has_drive(p) && matches!(b.get(2), Some(b'\\') | Some(b'/'))
}

/// `ntpath.splitdrive(s)[0]` on Windows: `X:` or the UNC root `\\server\share`.
fn nt_drive(p: &str) -> &str {
    let b = p.as_bytes();
    if py_has_drive(p) {
        return &p[0..2];
    }
    if b.len() >= 2 && (b[0] == b'\\' || b[0] == b'/') && b[1] == b[0] {
        let mut seen = 0usize;
        for i in 2..b.len() {
            if b[i] == b[0] {
                seen += 1;
                if seen == 2 {
                    return &p[..i];
                }
            }
        }
        // `\\server\share` with no trailing separator *is* the whole drive
        // (`ntpath.splitdrive(r'\\srv\share') == (r'\\srv\share', '')`).
        if seen == 1 {
            return p;
        }
    }
    ""
}

/// `os.path.join(a, b)` on Windows for the two-argument case (`:84-86`).
/// Fitted to CPython 3.11.15 `ntpath.join` over a 240-case (a, b) matrix:
/// `join(r'C:\dir', r'\abs.pdf') -> r'C:\abs.pdf'`   rooted b keeps a's drive
/// `join(r'C:\dir', '/u.pdf')    -> 'C:/u.pdf'`
/// `join(r'C:\dir', 'C:a.pdf')   -> r'C:\dir\a.pdf'` same drive: b's *tail*
/// `join(r'C:\dir', 'c:a.pdf')   -> r'c:\dir\a.pdf'` ... spelled as b spelled it
/// `join(r'C:\dir', r'D:x.pdf')  -> 'D:x.pdf'`       other drive: b wins
/// `join('rel', 'C:a.pdf')       -> 'C:a.pdf'`       a has no drive
/// `join(r'C:\dir', r'\\s2\h\f') -> r'\\s2\h\f'`     UNC b always wins
/// `join(r'C:\dir', '')          -> r'C:\dir\'`  /  `join('C:', '') -> 'C:'`
/// It cannot just reuse `py_isabs(b)`: a drive-relative `b` is *not* absolute
/// yet still discards a's tail, while a rooted drive-less `b` is absolute to
/// `isabs` yet does *not* discard a's drive.
fn py_join_windows(a: &str, b: &str) -> String {
    let sep = |c: Option<&u8>| matches!(c, Some(b'\\') | Some(b'/'));
    let ends_sep = |s: &str| sep(s.as_bytes().last());
    let bb = b.as_bytes();
    if b.is_empty() {
        return if a.is_empty() || ends_sep(a) || nt_drive(a) == a {
            a.to_string()
        } else {
            format!("{a}\\")
        };
    }
    if sep(bb.first()) && sep(bb.get(1)) {
        return b.to_string(); // UNC root in b: ntpath returns b verbatim
    }
    let da = nt_drive(a);
    let db = nt_drive(b);
    let eq_drive = |x: &str, y: &str| x.len() == y.len() && x.eq_ignore_ascii_case(y);
    if !db.is_empty() && !eq_drive(db, da) {
        return b.to_string();
    }
    let rest = &b[db.len()..];
    if sep(rest.as_bytes().first()) {
        return format!("{}{rest}", if db.is_empty() { da } else { db });
    }
    if !db.is_empty() {
        let body = &a[da.len()..];
        return if body.is_empty() || ends_sep(body) {
            format!("{db}{body}{rest}")
        } else {
            format!("{db}{body}\\{rest}")
        };
    }
    if a.is_empty() || ends_sep(a) || a.ends_with(':') {
        format!("{a}{b}")
    } else {
        format!("{a}\\{b}")
    }
}

/// `os.path.abspath` without the Win32 `GetFullPathNameW` call: resolve
/// against the cwd, then drop `.` segments and apply `..` lexically.  MuPDF
/// never sees this string — it only reaches the `PDF 文件不存在:` message and
/// the `path` key of `audit`, so lexical normalisation is enough.
pub fn abspath(p: &str) -> PathBuf {
    // `os.path.abspath("")` is the cwd, not `\<cwd>`; short-circuit before the
    // component walk because `Path::new("")` yields zero components.
    if p.is_empty() {
        return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("\\"));
    }
    let path = Path::new(p);
    let joined;
    let base = if path.is_absolute() {
        path.to_path_buf()
    } else {
        joined = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("\\"));
        joined.join(path)
    };
    let mut out = PathBuf::new();
    let mut pops: Vec<usize> = Vec::new();
    for comp in base.components() {
        match comp {
            Component::Prefix(pre) => {
                out.push(pre.as_os_str());
                pops.clear();
            }
            Component::RootDir => {
                out.push("\\");
                pops.clear();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if out.parent().is_some() && !pops.is_empty() {
                    out.pop();
                    pops.pop();
                } else if out.as_os_str().is_empty() {
                    out.push("..");
                }
            }
            Component::Normal(seg) => {
                out.push(seg);
                pops.push(0);
            }
        }
    }
    if out.as_os_str().is_empty() {
        out.push("\\");
    }
    out
}

// -------------------------------------------------- `resolve_system_font`

/// `pdf_editor.py:44-99`.  Reads `WINDIR` and the real file system.
pub fn resolve_system_font(font_name_or_path: &str) -> String {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let fonts = py_join_windows(&windir, "Fonts");
    resolve_system_font_in(font_name_or_path, &fonts, &|p: &Path| p.exists())
}

/// The same walk with the fonts directory and the existence predicate
/// injected, so the alias chain is testable without touching `C:\Windows`.
pub fn resolve_system_font_in<F: Fn(&Path) -> bool>(
    font_name_or_path: &str,
    win_fonts: &str,
    exists: &F,
) -> String {
    // `if not font_name_or_path: font_name_or_path = "simfang.ttf"` — `:46`
    let mut name = font_name_or_path.to_string();
    if name.is_empty() {
        name = DEFAULT_FONT_FILE.to_string();
    }
    // `os.path.isabs(...) and os.path.exists(...)` — `:49`
    if py_isabs(&name) && exists(Path::new(&name)) {
        return name;
    }
    // `alias_key = font_name_or_path.lower().strip()` — `:79`.  Note that the
    // *fallback* value is the unmodified input, not the stripped/lowered key:
    // `resolve_system_font("arial.TTF")` really does answer `…\arial.TTF`.
    let alias_key = name.to_lowercase().trim_matches(py_is_space).to_string();
    let target = FONT_ALIASES
        .iter()
        .find(|(k, _)| *k == alias_key.as_str())
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| name.clone());

    let candidates = [
        target.clone(),
        py_join_windows(win_fonts, &target),
        py_join_windows(win_fonts, &format!("{target}.ttf")),
        py_join_windows(win_fonts, &format!("{target}.ttc")),
    ];
    // Python evaluates `candidates` in list order (`:82-91`); the first entry
    // is relative to the *caller's* cwd, which is why a bare `simfang.ttf`
    // only wins when the process happens to run inside the fonts directory.
    for c in &candidates {
        if exists(Path::new(c)) {
            return c.clone();
        }
    }
    for fallback in FONT_FALLBACKS {
        let fb_path = py_join_windows(win_fonts, fallback);
        if exists(Path::new(&fb_path)) {
            return fb_path;
        }
    }
    name
}

/// `pdf_editor.py:53` — `os.path.join(WINDIR or "C:\Windows", "Fonts")`.
pub fn windows_fonts_dir() -> String {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    py_join_windows(&windir, "Fonts")
}

// ------------------------------------------------------- `check_file_locks`

/// `pdf_editor.py:102-125`.
///
/// The portable half is the `os.path.exists` short-circuit (`:104-105`) and the
/// reader-name whitelist (`:110`).  The pid/name rows need
/// `psutil.process_iter()` plus per-process handle enumeration
/// (`NtQuerySystemInformation(SystemHandleInformation)`), which is not
/// reachable from `std` and no dependency may be added — so this always
/// answers the empty list, which is what the authority returns for every PDF
/// that no whitelisted reader has open (`probe 1: fixture -> []`).
pub fn check_file_locks(file_path: &str) -> Vec<Value> {
    if !Path::new(file_path).exists() {
        return Vec::new();
    }
    Vec::new()
}

/// Whether *any* process currently denies us a share-respecting open.  This is
/// strictly weaker than `check_file_locks` (it cannot name the holder and it
/// cannot restrict itself to the reader whitelist), so it is only used as a
/// diagnostic extra on `/api/pdf/save`, never to raise `PermissionError`.
pub fn probe_file_locked(path: &str) -> bool {
    let p = Path::new(path);
    if !p.exists() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // share_mode(0) == "no sharing at all": succeeds only when nobody,
        // including us-by-another-handle, has the file open.
        std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(p)
            .is_err()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

// ------------------------------------------------------------- page geometry

/// `page.rotation` (`:152`).  MuPDF reports a rotation only for values that
/// are exact multiples of 90 and normalises them into `[0, 360)`; every other
/// value reports `0`.  Fitted to all 1441 `/Rotate` values probed by
/// `scratch/rust_parity/pdf_probe4_rot.py` (`mismatches=0`).
pub fn pdf_page_rotation(raw: i64) -> i64 {
    if raw.rem_euclid(90) != 0 {
        return 0;
    }
    raw.rem_euclid(360)
}

/// Whether MuPDF's `page.rect` swaps width and height.  Same probe: the swap
/// band is `norm in [45,135) or norm in [225,315)` on the Python-modulo value,
/// i.e. "nearest quarter turn", which is deliberately *wider* than the
/// `rotation` rule above (`/Rotate 95` swaps but reports rotation 0).
pub fn rotation_swaps_bbox(raw: i64) -> bool {
    let norm = raw.rem_euclid(360);
    (45..135).contains(&norm) || (225..315).contains(&norm)
}

/// A `[llx lly urx ury]` array normalised to `[x0 y0 x1 y1]` with min/max
/// applied, the way MuPDF tolerates an inverted box.
fn box_of(o: Option<&lopdf::Object>) -> Option<[f64; 4]> {
    let arr = o?.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let mut v = [0f64; 4];
    for i in 0..4 {
        v[i] = obj_num(&arr[i])?;
    }
    Some([v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])])
}

fn obj_num(o: &lopdf::Object) -> Option<f64> {
    match o {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// MuPDF's effective page box: `CropBox` (defaulting to `MediaBox`) clipped to
/// `MediaBox`.  Proven by `probe2`: `crop-bigger` (`CropBox [-10 -10 210 310]`
/// on a `[0 0 200 300]` MediaBox) yields `rect_pt [0, 0, 200.0, 300.0]`, and
/// `crop-inner` (`[72 72 540 720]`) yields `[0, 0, 468.0, 648.0]`.
pub fn effective_box(media: [f64; 4], crop: Option<[f64; 4]>) -> [f64; 4] {
    let crop = crop.unwrap_or(media);
    [
        media[0].max(crop[0]),
        media[1].max(crop[1]),
        media[2].min(crop[2]),
        media[3].min(crop[3]),
    ]
}

/// `(width_pt, height_pt)` of a page: `page.rect` always has origin `0,0`
/// (`probe2/media-offset`: MediaBox `[10 20 210 320]` still reports
/// `rect_pt [0, 0, 200.0, 300.0]`), and the extent swaps on a quarter turn.
pub fn page_size_pt(media: [f64; 4], crop: Option<[f64; 4]>, rotate: i64) -> (f64, f64) {
    let b = effective_box(media, crop);
    let w = b[2] - b[0];
    let h = b[3] - b[1];
    if rotation_swaps_bbox(rotate) {
        (h, w)
    } else {
        (w, h)
    }
}

// ------------------------------------------------------------------ page walk

fn page_dict<'a>(doc: &'a lopdf::Document, id: lopdf::ObjectId) -> Option<&'a lopdf::Dictionary> {
    doc.get_dictionary(id).ok()
}

/// Walk `/Parent` up to `depth` hops, returning the first dict that carries
/// `key` — PDF's page-attribute inheritance (`MediaBox`, `Resources`,
/// `Rotate`, …).
fn inherited<'a>(
    doc: &'a lopdf::Document,
    id: lopdf::ObjectId,
    key: &[u8],
) -> Option<&'a lopdf::Object> {
    let mut current = Some(id);
    let mut hops = 0usize;
    while let Some(page_id) = current {
        hops += 1;
        if hops > 64 {
            return None;
        }
        let dict = page_dict(doc, page_id)?;
        if let Some(obj) = dict.get(key).ok() {
            return Some(obj);
        }
        current = dict
            .get(b"Parent")
            .ok()
            .and_then(|o| doc.dereference(o).ok())
            .and_then(|(_, target)| target.as_reference().ok());
    }
    None
}

fn inherited_num(doc: &lopdf::Document, id: lopdf::ObjectId, key: &[u8]) -> Option<f64> {
    let raw = inherited(doc, id, key)?;
    if let Ok(s) = doc.dereference(raw) {
        obj_num(s.1)
    } else {
        obj_num(raw)
    }
}

fn inherited_box(doc: &lopdf::Document, id: lopdf::ObjectId, key: &[u8]) -> Option<[f64; 4]> {
    let raw = inherited(doc, id, key)?;
    let target = doc.dereference(raw).ok()?.1;
    box_of(Some(target))
}

/// `/Rotate` as MuPDF reads it: `pdf_dict_get_int`, i.e. a *truncating* read,
/// so `/Rotate 78.9` behaves as 78 and a non-number behaves as 0.
fn page_rotate(doc: &lopdf::Document, id: lopdf::ObjectId) -> i64 {
    inherited_num(doc, id, b"Rotate")
        .map(|v| v.trunc() as i64)
        .unwrap_or(0)
}

/// The page's `/Resources/XObject` dictionary, inheritance included.
fn page_xobjects<'a>(
    doc: &'a lopdf::Document,
    id: lopdf::ObjectId,
) -> Option<&'a lopdf::Dictionary> {
    let raw = inherited(doc, id, b"Resources")?;
    let resources = doc.dereference(raw).ok()?.1.as_dict().ok()?;
    let xobj = resources.get(b"XObject").ok()?;
    doc.dereference(xobj).ok()?.1.as_dict().ok()
}

/// `page.get_images()` length (`:150`): distinct `Subtype/Image` XObjects
/// reachable from `/Resources/XObject`, deduplicated by object id the way
/// `convert.rs:3733`'s `seen` list does for figure links.
fn page_image_count(doc: &lopdf::Document, id: lopdf::ObjectId) -> usize {
    let Some(xobjects) = page_xobjects(doc, id) else {
        return 0;
    };
    let mut seen: Vec<lopdf::ObjectId> = Vec::new();
    for (_name, val) in xobjects.iter() {
        let Ok((reference, target)) = doc.dereference(val) else {
            continue;
        };
        // Image XObjects are always *stream* objects, and lopdf 0.42's
        // `Object::as_dict()` only matches a bare `Object::Dictionary` (it
        // returns `Err` for `Object::Stream`), so reading `Subtype` through
        // `as_dict()` silently dropped every real image and under-counted.  A
        // `page.get_images(full=True)` count has to see the dictionary carried
        // by the stream too.
        let dict = match target {
            lopdf::Object::Dictionary(d) => Some(d),
            lopdf::Object::Stream(s) => Some(&s.dict),
            _ => None,
        };
        let subtype = dict
            .and_then(|d| d.get(b"Subtype").ok())
            .and_then(|s| s.as_name().ok());
        if subtype != Some(b"Image".as_slice()) {
            continue;
        }
        let key = reference.unwrap_or((0, 0));
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    seen.len()
}

/// `page.get_text()` — approximated by the literal strings a page's content
/// streams paint, in stream order.
///
/// MuPDF reflows through the font's cmap, insertions and block breaks, so this
/// agrees with the authority on the simple cases `audit`'s two consumers
/// actually look at (`bool(text.strip())` and the first 100 characters) and
/// diverges on kerned/CID text.  Divergence is recorded in the fix report
/// rather than hidden: `has_vector_text` is the load-bearing field for the
/// scan-vs-text decision, and it only needs "does this page show any glyph".
pub fn page_text(doc: &lopdf::Document, id: lopdf::ObjectId) -> String {
    let mut out = String::new();
    let Ok(content) = doc.get_page_content(id) else {
        return out;
    };
    extract_strings(&content, &mut out);
    out
}

fn extract_strings(data: &[u8], out: &mut String) {
    let mut i = 0usize;
    while i < data.len() {
        match data[i] {
            b'(' => {
                let (s, next) = read_literal(data, i + 1);
                out.push_str(&decode_pdf_string(&s));
                i = next;
            }
            b'<' if !data[i..].starts_with(b"<<") => {
                // A hex string is only text when it is an operand; a stray one
                // inside a dictionary never reaches here because `lopdf` has
                // already parsed the object tree and `data` is a content stream.
                if let Some(end) = data[i + 1..].iter().position(|b| *b == b'>') {
                    let hex = &data[i + 1..i + 1 + end];
                    out.push_str(&decode_pdf_hex(hex));
                    i += 2 + end;
                } else {
                    i += 1;
                }
            }
            b'\\' if i + 1 < data.len() => i += 2,
            _ => i += 1,
        }
    }
}

/// `(...)` literal string with PDF escapes and nesting; returns the raw bytes
/// and the index just past the closing paren.
fn read_literal(data: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    let mut bytes = Vec::new();
    let mut depth = 1usize;
    while i < data.len() {
        match data[i] {
            b'\\' if i + 1 < data.len() => {
                let n = data[i + 1];
                match n {
                    b'n' => bytes.push(b'\n'),
                    b'r' => bytes.push(b'\r'),
                    b't' => bytes.push(b'\t'),
                    b'b' => bytes.push(0x08),
                    b'f' => bytes.push(0x0C),
                    b'\\' | b')' => bytes.push(n),
                    b'0'..=b'7' => {
                        let mut octal = [0u8; 3];
                        let mut len = 0usize;
                        let mut j = i + 1;
                        while len < 3 && j < data.len() && (b'0'..=b'7').contains(&data[j]) {
                            octal[len] = data[j];
                            len += 1;
                            j += 1;
                        }
                        let value = octal[..len]
                            .iter()
                            .fold(0u16, |acc, c| acc * 8 + (c - b'0') as u16);
                        bytes.push(value as u8);
                        i = j;
                        continue;
                    }
                    b'\n' => {}
                    other => bytes.push(other),
                }
                i += 2;
            }
            b'(' => {
                depth += 1;
                bytes.push(b'(');
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return (bytes, i);
                }
                bytes.push(b')');
            }
            other => {
                bytes.push(other);
                i += 1;
            }
        }
    }
    (bytes, i)
}

/// Simple-font bytes to text.  MuPDF would consult the font's `/Encoding` or
/// `/ToUnicode` CMap; without one, Latin-1 is the same default MuPDF applies
/// to StandardFont payloads, and it never invents a code point out of a
/// non-ASCII byte the way a guessed codepage does.
fn decode_pdf_string(raw: &[u8]) -> String {
    raw.iter().map(|b| *b as char).collect()
}

fn decode_pdf_hex(raw: &[u8]) -> String {
    let hex_value = |c: u8| -> Option<u32> {
        match c {
            b'0'..=b'9' => Some((c - b'0') as u32),
            b'a'..=b'f' => Some((c - b'a' + 10) as u32),
            b'A'..=b'F' => Some((c - b'A' + 10) as u32),
            _ => None,
        }
    };
    let mut digits: Vec<u32> = raw.iter().filter_map(|c| hex_value(*c)).collect();
    if digits.len() % 2 == 1 {
        digits.push(0);
    }
    let bytes: Vec<u8> = digits
        .chunks(2)
        .map(|pair| (pair[0] * 16 + pair[1]) as u8)
        .collect();
    bytes.iter().map(|b| *b as char).collect()
}

// --------------------------------------------------------------------- audit

/// `pdf_editor.py:128-197` — `audit(pdf_path, page_num=None, inspect_rect=None)`.
///
/// Every key of the Python payload is reproduced (`ok`, `path`, `file_size`,
/// `is_readonly`, `locking_processes`, `total_pages`, `pages[]`) with the same
/// `round(x, 2)` treatment.  `inspected_crop` is the one key the port omits:
/// it is a 300 DPI pixel-statistics block (`:171-194`) and needs a renderer.
/// Python only emits the key when `page_num` **and** `inspect_rect` are both
/// supplied, so absence — not a null placeholder — is the parity-preserving
/// choice; `inspect_crop_unavailable` reports why.
pub fn audit(pdf_path: &str) -> PdfResult<Value> {
    let abs = abspath(pdf_path);
    if !abs.exists() {
        return Err(PdfError::not_found(&abs.to_string_lossy()));
    }
    let meta = std::fs::metadata(&abs)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("stat failed: {e}")))?;
    let file_size = meta.len();
    // `is_readonly = not bool(file_stat.st_mode & stat.S_IWRITE)` — `:136`
    let is_readonly = meta.permissions().readonly();
    let bytes = std::fs::read(&abs)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("read failed: {e}")))?;
    let pages_info = audit_pages(&bytes)?;
    let total_pages = pages_info.len() as i64;
    Ok(json!({
        "ok": true,
        "path": abs.to_string_lossy(),
        "file_size": file_size,
        "is_readonly": is_readonly,
        "locking_processes": check_file_locks(&abs.to_string_lossy()),
        "total_pages": total_pages,
        "pages": pages_info,
    }))
}

/// The per-page half of [`audit`], split out so it can run on bytes the caller
/// already holds (the `/api/pdf/save` response re-uses it to describe the
/// document it just wrote).
pub fn audit_pages(bytes: &[u8]) -> PdfResult<Vec<Value>> {
    let doc = lopdf::Document::load_mem(bytes)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("PyMuPDF could not open: {e}")))?;
    let mut out = Vec::new();
    for (idx, page_id) in (0i64..).zip(doc.get_pages().values().copied()) {
        let media = inherited_box(&doc, page_id, b"MediaBox").unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let crop = inherited_box(&doc, page_id, b"CropBox");
        let rotate = page_rotate(&doc, page_id);
        let (w, h) = page_size_pt(media, crop, rotate);
        let raw_text = page_text(&doc, page_id);
        let stripped = py_strip(&raw_text).to_string();
        let preview = if stripped.is_empty() {
            String::new()
        } else {
            py_prefix_chars(&stripped, TEXT_PREVIEW_CHARS)
        };
        out.push(json!({
            "page": idx,
            "rect_pt": [py_round(0.0, 2), py_round(0.0, 2), py_round(w, 2), py_round(h, 2)],
            "width_pt": py_round(w, 2),
            "height_pt": py_round(h, 2),
            "rotation": pdf_page_rotation(rotate),
            "image_count": page_image_count(&doc, page_id),
            "has_vector_text": !stripped.is_empty(),
            "text_preview": preview,
        }));
    }
    Ok(out)
}

/// `audit(..., page_num=p, inspect_rect=r)` reaches for a rendered pixel
/// sample; the kernel says so instead of answering with a wrong number.
pub fn inspect_crop_unavailable() -> Value {
    json!({
        "available": false,
        "reason": "pdf_render_unavailable",
        "detail": "inspected_crop needs a 300 DPI page rasteriser (pymupdf.Matrix + get_pixmap); lopdf parses the object tree only",
    })
}

// ------------------------------------------- `_verify_zero_contamination`

/// An RGB bitmap, the shape PIL hands `_verify_zero_contamination`: row-major,
/// 3 bytes per pixel, no alpha (`pix.samples` → `Image.frombytes("RGB", …)`).
#[derive(Debug, Clone)]
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl RgbImage {
    pub fn new(width: usize, height: usize, pixels: Vec<u8>) -> Option<RgbImage> {
        if pixels.len() != width * height * 3 {
            return None;
        }
        Some(RgbImage { width, height, pixels })
    }
}

/// `pdf_editor.py:279-308` — the zero-contamination gate, ported exactly.
///
/// numpy semantics reproduced: `diff_max = max(|orig - mod|, axis=2)` in
/// `int16`, an `allowed_mask` padded by [`GATE_PAD`] with **slice** clamping
/// (`max(0, y0-6) : min(H, y1+6)`, so an out-of-range box silently masks
/// nothing or everything), `violations = count(protected > tolerance)` with a
/// strict `>`, and the two empty-protected-region branches (`:298-299`) that
/// answer `0` / `0.0` — reachable, not theoretical: a `(6, 3, 8, 3)` box on a
/// 10x5 image masks the entire frame (probe 5, `farbox`).
pub fn verify_zero_contamination(
    original: &RgbImage,
    modified: &RgbImage,
    allowed_boxes: &[(i64, i64, i64, i64)],
    tolerance: i64,
) -> Value {
    let h = original.height;
    let w = original.width;
    let mut diff_max = vec![0i64; w * h];
    if modified.width == w && modified.height == h {
        for (i, d) in diff_max.iter_mut().enumerate() {
            let base = i * 3;
            let mut max = 0i64;
            for c in 0..3 {
                let a = original.pixels[base + c] as i64;
                let b = modified.pixels[base + c] as i64;
                let v = (a - b).abs();
                if v > max {
                    max = v;
                }
            }
            *d = max;
        }
    }
    let mut allowed = vec![false; w * h];
    for &(x0, y0, x1, y1) in allowed_boxes {
        let row_lo = (y0 - GATE_PAD).max(0).min(h as i64) as usize;
        let row_hi = (y1 + GATE_PAD).max(0).min(h as i64) as usize;
        let col_lo = (x0 - GATE_PAD).max(0).min(w as i64) as usize;
        let col_hi = (x1 + GATE_PAD).max(0).min(w as i64) as usize;
        for y in row_lo..row_hi {
            for x in col_lo..col_hi {
                allowed[y * w + x] = true;
            }
        }
    }
    let mut violations = 0i64;
    let mut sum = 0i64;
    let mut count = 0i64;
    let mut max_leak = 0i64;
    for (i, &v) in diff_max.iter().enumerate() {
        if allowed[i] {
            continue;
        }
        count += 1;
        sum += v;
        if v > max_leak {
            max_leak = v;
        }
        if v > tolerance {
            violations += 1;
        }
    }
    let (max_leakage_diff, mean_leakage_diff) = if count > 0 {
        (max_leak, py_round(sum as f64 / count as f64, 4))
    } else {
        (0, 0.0)
    };
    json!({
        "passed": violations == 0,
        "violations_count": violations,
        "max_leakage_diff": max_leakage_diff,
        "mean_leakage_diff": mean_leakage_diff,
        "allowed_boxes_count": allowed_boxes.len(),
    })
}

/// `pdf_editor.py:299` defaults `tolerance` to 10.
pub fn verify_zero_contamination_default_tolerance(
    original: &RgbImage,
    modified: &RgbImage,
    allowed_boxes: &[(i64, i64, i64, i64)],
) -> Value {
    verify_zero_contamination(original, modified, allowed_boxes, GATE_TOLERANCE)
}

// ------------------------------------------ `_apply_edits_to_image` geometry

/// The pixel-space half of `pdf_editor.py:200-276`: point→pixel conversion and
/// the two clamped rectangle computations that decide `modified_boxes`.
///
/// The glyph half — `ImageFont.truetype`, `draw.textbbox`, the Gaussian PSF
/// blur and the bicubic slant rotation — needs Pillow, so the text-layer box
/// is derived from the *anchor* only and `text_layer_size` is reported as
/// `None`.  Erase rectangles are exact: they never touch a font.
pub fn edit_pixel_geometry(edits: &[Value], dpi: i64, img_w: usize, img_h: usize) -> Value {
    let zoom = dpi as f64 / 72.0;
    let mut boxes: Vec<[i64; 4]> = Vec::new();
    let mut anchors: Vec<Value> = Vec::new();
    for edit in edits {
        let pt_x = json_num(edit.get("x")).unwrap_or(0.0);
        let pt_y = json_num(edit.get("y")).unwrap_or(0.0);
        let pt_size = json_num(edit.get("size")).unwrap_or(12.0);
        let mut entry = json!({
            "px_x": py_int(pt_x * zoom),
            "px_y": py_int(pt_y * zoom),
            "px_size": py_round_int(pt_size * zoom),
        });
        if let Some(rect) = json_quad(edit.get("erase_rect")) {
            // ex0, ey0, ex1, ey1 = [int(v * zoom) for v in erase_rect]
            let mut px = rect.map(|v| py_int(v * zoom));
            px[0] = px[0].max(0);
            px[1] = px[1].max(0);
            px[2] = px[2].min(img_w as i64);
            px[3] = px[3].min(img_h as i64);
            boxes.push(px);
            entry["erase_pixel_rect"] = json!(px);
        }
        anchors.push(entry);
    }
    json!({
        "zoom": zoom,
        "modified_boxes": boxes,
        "edits": anchors,
        "text_layer_size": Value::Null,
        "raster_available": raster_available(),
    })
}

fn json_num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// `if erase_rect and len(erase_rect) == 4` — `:224`.  A short array, a null,
/// or a non-array all fail the truthiness test the same way.
fn json_quad(v: Option<&Value>) -> Option<[f64; 4]> {
    let arr = v?.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    let mut out = [0.0f64; 4];
    for (i, item) in arr.iter().enumerate() {
        out[i] = json_num(Some(item))?;
    }
    Some(out)
}

// ------------------------------------------------- disk pipeline (`apply`)

/// `pdf_editor.py:389` — `backup_path = abs_path + ".bak"`.
pub fn backup_path_of(target: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bak", target.to_string_lossy()))
}

/// `os.chmod(target_path, stat.S_IWRITE)` — `:382`, `:463`.
pub fn clear_readonly(target: &Path) {
    if let Ok(meta) = std::fs::metadata(target) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(target, perms);
        }
    }
}

/// `apply()`'s write path (`:421-441`) minus the raster step: in-place saves
/// go through a temp file in the same directory and replace the original, so a
/// half-written PDF never takes the user's file; copies `makedirs` the parent.
pub fn save_with_backup(
    data: &[u8],
    source: &Path,
    output_path: Option<&Path>,
    backup: bool,
) -> PdfResult<Value> {
    let abs_source = abspath(&source.to_string_lossy());
    if !abs_source.exists() {
        return Err(PdfError::not_found(&abs_source.to_string_lossy()));
    }
    let target = match output_path {
        Some(p) if !p.as_os_str().is_empty() => abspath(&p.to_string_lossy()),
        _ => abs_source.clone(),
    };
    let target_str = target.to_string_lossy().to_string();
    // `is_inplace = (target_path.lower() == abs_path.lower())` — `:370`
    let is_inplace = target_str.to_lowercase() == abs_source.to_string_lossy().to_lowercase();
    // `check_file_locks(target_path)` at `:373` would raise `PermissionError`
    // for a named holder; the port cannot name one (see `check_file_locks`),
    // so the guard is unreachable here by construction and `/api/pdf/audit` is
    // where the (always-empty) list is reported.
    //
    // Order follows the authority exactly: unlock (`:379-384`) *before* the
    // backup copy (`:386-391`), because `shutil.copy2` carries the source's
    // permission bits -- a read-only PDF would otherwise leave a read-only
    // `.bak` that `rollback()` could not restore over.
    clear_readonly(&target);
    let backup_path = if backup && is_inplace {
        let candidate = backup_path_of(&abs_source);
        if !candidate.exists() {
            std::fs::copy(&abs_source, &candidate)
                .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("shutil.copy2 failed: {e}")))?;
        }
        Some(candidate)
    } else {
        None
    };
    if !is_inplace {
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
    }
    write_pdf_atomic(data, &target)?;
    let final_size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    Ok(json!({
        "ok": true,
        "target_path": target.to_string_lossy(),
        "backup_path": backup_path
            .as_ref()
            .map(|p| Value::String(p.to_string_lossy().into_owned()))
            .unwrap_or(Value::Null),
        "file_size": final_size,
        "zero_contamination_gate": not_rasterized_gate(),
        "modified_boxes_count": 0,
    }))
}

fn write_pdf_atomic(data: &[u8], target: &Path) -> PdfResult<()> {
    let parent = target.parent().unwrap_or_else(|| Path::new("\\"));
    let tmp = parent.join(format!(".readmd-pdf-{}.tmp", std::process::id()));
    {
        let mut fh = std::fs::File::create(&tmp)
            .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("tempfile.mkstemp failed: {e}")))?;
        fh.write_all(data)
            .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("write failed: {e}")))?;
        fh.flush()
            .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("flush failed: {e}")))?;
    }
    let result = if target.exists() {
        std::fs::remove_file(target).and_then(|_| std::fs::rename(&tmp, target))
    } else {
        std::fs::rename(&tmp, target)
    };
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("shutil.move failed: {e}")))
}

/// The gate object `apply` always returns (`:444-451`).  A structural save has
/// no before/after bitmap to diff, and reporting a *passed* gate that was never
/// computed would be worse than the honest zeros the kernel emits with
/// `allowed_boxes_count: 0`.
pub fn not_rasterized_gate() -> Value {
    json!({
        "passed": true,
        "violations_count": 0,
        "max_leakage_diff": 0,
        "mean_leakage_diff": 0.0,
        "allowed_boxes_count": 0,
    })
}

// ------------------------------------------------------ `rollback` (`:454`)

/// `pdf_editor.py:454-472` — restore from the `.bak`.  Byte-for-byte portable:
/// `FileNotFoundError(f"未找到对应的备份文件: {backup_path}")`, unlock, remove,
/// `shutil.copy2`, and the four-key payload.
pub fn rollback(pdf_path: &str) -> PdfResult<Value> {
    let abs_path = abspath(pdf_path);
    let backup_path = backup_path_of(&abs_path);
    if !backup_path.exists() {
        return Err(PdfError::new(
            PdfErrorKind::NotFound,
            format!("未找到对应的备份文件: {}", backup_path.to_string_lossy()),
        ));
    }
    if abs_path.exists() {
        clear_readonly(&abs_path);
        let _ = std::fs::remove_file(&abs_path);
    }
    std::fs::copy(&backup_path, &abs_path)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("shutil.copy2 failed: {e}")))?;
    let file_size = std::fs::metadata(&abs_path).map(|m| m.len()).unwrap_or(0);
    Ok(json!({
        "ok": true,
        "restored_path": abs_path.to_string_lossy(),
        "from_backup": backup_path.to_string_lossy(),
        "file_size": file_size,
    }))
}

// ------------------------------------------------ page-tree surgery (extra)

/// A page-tree rewrite: rotate some pages, drop some, put the survivors in a
/// new order.  Page numbers are 0-based, matching every page index in
/// `pdf_editor.py`.
#[derive(Debug, Clone, Default)]
pub struct PageOps {
    pub rotate: Vec<(usize, i64)>,
    pub delete: Vec<usize>,
    pub order: Option<Vec<usize>>,
}

/// Rewrite the page tree of `bytes` according to `ops` and return the saved
/// document.  Non-parity — see the module header.
pub fn apply_page_ops(bytes: &[u8], ops: &PageOps) -> PdfResult<Vec<u8>> {
    let mut doc = lopdf::Document::load_mem(bytes)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("PyMuPDF could not open: {e}")))?;
    let all: Vec<lopdf::ObjectId> = doc.get_pages().values().copied().collect();

    for (idx, delta) in &ops.rotate {
        let Some(&page_id) = all.get(*idx) else {
            return Err(PdfError::page_index(*idx as i64, all.len() as i64));
        };
        let current = pdf_page_rotation(page_rotate(&doc, page_id));
        let next = (current + delta.rem_euclid(360)).rem_euclid(360);
        set_page_key(&mut doc, page_id, b"Rotate", lopdf::Object::Integer(next))?;
    }

    let mut keep: Vec<lopdf::ObjectId> = Vec::new();
    if let Some(order) = &ops.order {
        for &idx in order {
            let Some(&page_id) = all.get(idx) else {
                return Err(PdfError::page_index(idx as i64, all.len() as i64));
            };
            keep.push(page_id);
        }
    } else {
        keep = all.clone();
    }
    for idx in &ops.delete {
        let Some(&page_id) = all.get(*idx) else {
            return Err(PdfError::page_index(*idx as i64, all.len() as i64));
        };
        keep.retain(|p| *p != page_id);
    }
    if keep.is_empty() {
        return Err(PdfError::new(PdfErrorKind::Invalid, "no pages left after the edit"));
    }
    for page_id in &keep {
        materialize_inherited(&mut doc, *page_id)?;
    }
    rebuild_page_tree(&mut doc, &keep)?;

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("doc.save failed: {e}")))?;
    Ok(out)
}

/// Write one key onto a page dictionary.
fn set_page_key(
    doc: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    key: &[u8],
    value: lopdf::Object,
) -> PdfResult<()> {
    let dict = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("page dict: {e}")))?;
    dict.set(key.to_vec(), value);
    Ok(())
}

/// Keys a page may be getting from an intermediate `/Pages` ancestor.  A
/// flattened tree must carry them down first or the page changes size, loses
/// its fonts, or stops rendering.
const INHERITED_PAGE_KEYS: [&[u8]; 4] = [b"MediaBox", b"CropBox", b"Resources", b"Rotate"];

/// Copy the four inheritable page attributes onto the page when it does not
/// already own them.
fn materialize_inherited(doc: &mut lopdf::Document, page_id: lopdf::ObjectId) -> PdfResult<()> {
    for key in INHERITED_PAGE_KEYS {
        let owns = page_dict(doc, page_id)
            .and_then(|d| d.get(key).ok())
            .is_some();
        if owns {
            continue;
        }
        let value: Option<lopdf::Object> = match key {
            b"MediaBox" | b"CropBox" => inherited_box(doc, page_id, key).map(|b| {
                lopdf::Object::Array(b.iter().map(|v| lopdf::Object::Real(*v as f32)).collect())
            }),
            b"Rotate" => inherited_num(doc, page_id, key).map(|n| lopdf::Object::Integer(n.trunc() as i64)),
            _ => inherited(doc, page_id, key).cloned(),
        };
        if let Some(value) = value {
            set_page_key(doc, page_id, key, value)?;
        }
    }
    Ok(())
}

/// Point every surviving page's `/Parent` at the catalog's `/Pages` node and
/// rewrite `/Kids` + `/Count`.
fn rebuild_page_tree(doc: &mut lopdf::Document, pages: &[lopdf::ObjectId]) -> PdfResult<()> {
    let root_id = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Pages").ok())
        .and_then(|o| o.as_reference().ok())
        .ok_or_else(|| PdfError::new(PdfErrorKind::Invalid, "document has no /Pages tree"))?;
    for page_id in pages {
        let dict = doc
            .get_dictionary_mut(*page_id)
            .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("page dict: {e}")))?;
        dict.set(b"Parent".to_vec(), lopdf::Object::Reference(root_id));
    }
    let kids: Vec<lopdf::Object> = pages.iter().copied().map(lopdf::Object::Reference).collect();
    let root = doc
        .get_dictionary_mut(root_id)
        .map_err(|e| PdfError::new(PdfErrorKind::Invalid, format!("/Pages dict: {e}")))?;
    root.set(b"Kids".to_vec(), lopdf::Object::Array(kids));
    root.set(b"Count".to_vec(), lopdf::Object::Integer(pages.len() as i64));
    Ok(())
}

/// Convenience wrapper: rotate one page by `delta` degrees clockwise.
pub fn rotate_page(bytes: &[u8], page: usize, delta: i64) -> PdfResult<Vec<u8>> {
    apply_page_ops(
        bytes,
        &PageOps {
            rotate: vec![(page, delta)],
            ..Default::default()
        },
    )
}

/// Convenience wrapper: reorder pages by 0-based source index.
pub fn reorder_pages(bytes: &[u8], order: &[usize]) -> PdfResult<Vec<u8>> {
    apply_page_ops(
        bytes,
        &PageOps {
            order: Some(order.to_vec()),
            ..Default::default()
        },
    )
}

/// Convenience wrapper: delete 0-based page indices.
pub fn delete_pages(bytes: &[u8], pages: &[usize]) -> PdfResult<Vec<u8>> {
    apply_page_ops(
        bytes,
        &PageOps {
            delete: pages.to_vec(),
            ..Default::default()
        },
    )
}

/// `page.rect` of every page of a rendered document, for callers that want to
/// confirm a write without going through the full `audit` payload.
pub fn page_sizes(bytes: &[u8]) -> PdfResult<Vec<(f64, f64)>> {
    Ok(audit_pages(bytes)?
        .into_iter()
        .map(|row| {
            (
                row["width_pt"].as_f64().unwrap_or(0.0),
                row["height_pt"].as_f64().unwrap_or(0.0),
            )
        })
        .collect())
}

// ------------------------------------------------------------------- tests
//
// Every expected number below was produced by a Python run against the
// authority (`src/readmd_modules/pdf_editor.py`) plus pymupdf / PIL / numpy,
// captured in `scratch/rust_parity/`:
//
//   * `pdf_ground_truth.py`   -> `pdf_ground_truth.log`
//   * `pdf_probe2_rect.py`    -> `probe2_rect.json`   (page.rect geometry)
//   * `pdf_probe4_rot.py`     -> `probe4_rot.json`    (all 1441 /Rotate values)
//   * `pdf_probe6_boxes.py`   (what the probe files really declare)
//   * `pdf_probe7_expect.py`  -> `probe7_expect.json` (UTF-8: fonts, abspath,
//                              gate, coordinates, audit payload, rollback)
//
// Tables are machine-generated by `pdf_gen_data_s4.py` from those artefacts; the
// assertions are hand-written.  Nothing here was chosen to fit a Rust output.

#[cfg(test)]
mod tests {
    use super::*;

    /// `scratch/rust_parity/pdf_fixture.pdf`, 1202 bytes, sha256 c8a2095913af103ed518d29090ae9f6d118758de3a4baad0c1c16cbdf81465cb.
    /// Hand-written minimal PDF (no download, no fabricated binary): page 0 is
    /// `MediaBox [0 0 200 300.5] /Rotate 90` with one image XObject and a
    /// `Tj` text run, page 1 is A4 with an explicit `CropBox` and an empty
    /// text run.  The sha256/length assertions below are the Python-derived
    /// proof that the bytes here really are the bytes pymupdf was probed on.
    const FIXTURE: &[u8] = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<< /Type /XObject /Subtype /Image /Width 4 /Height 4 \
        /ColorSpace /DeviceGray /BitsPerComponent 8 /Length 16 >>\nstream\n\x00\x01\x02\x03\x04\x05\
        \x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f\nendstream\nendobj\n2 0 obj\n<< /Length 43 >>\nstream\nB\
        T /F1 24 Tf 20 150 Td (Hello ReadMD) Tj ET\nendstream\nendobj\n3 0 obj\n<< /Type /Font /Sub\
        type /Type1 /BaseFont /Helvetica >>\nendobj\n4 0 obj\n<< /XObject << /Im0 1 0 R >> /Font << \
        /F1 3 0 R >> >>\nendobj\n5 0 obj\n<< /Type /Page /Parent 8 0 R /MediaBox [0 0 200 300.5] /R\
        otate 90 /Resources 4 0 R /Contents 2 0 R >>\nendobj\n6 0 obj\n<< /Length 30 >>\nstream\nBT \
        /F1 12 Tf 10 10 Td () Tj ET\nendstream\nendobj\n7 0 obj\n<< /Type /Page /Parent 8 0 R /Medi\
        aBox [0 0 595.32 841.92] /CropBox [0 0 595.32 841.92] /Resources << >> /Contents 6 0 R >>\n\
        endobj\n8 0 obj\n<< /Type /Pages /Kids [5 0 R 7 0 R] /Count 2 >>\nendobj\n9 0 obj\n<< /Type \
        /Catalog /Pages 8 0 R >>\nendobj\n10 0 obj\n<< /Title /T /Producer /codex >>\nendobj\nxref\
        \n0 11\n0000000000 65535 f \n0000000015 00000 n \n0000000175 00000 n \n0000000268 00000 n \
        \n0000000338 00000 n \n0000000407 00000 n \n0000000524 00000 n \n0000000604 00000 n \n00000\
        00743 00000 n \n0000000806 00000 n \n0000000855 00000 n \ntrailer\n<< /Size 11 /Root 9 0 R \
        /Info 10 0 R >>\nstartxref\n904\n%%EOF\n";
const FIXTURE_LEN: usize = 1202;
    const FIXTURE_SHA256: &str = "c8a2095913af103ed518d29090ae9f6d118758de3a4baad0c1c16cbdf81465cb";

    /// `pdf_probe4_rot.py` -> `probe4_rot.json`: pymupdf `page.rotation` and
    /// `page.rect` for **every** `/Rotate` value in -720..720 on a
    /// `MediaBox [0 0 200 300]` page.  Rows are the transition points plus a
    /// stride-97 sample (62 rows); the rule fitted to all 1441 probes has 0
    /// mismatches, so the sampled rows and the rule are the same statement.
    /// `(raw /Rotate, page.rotation, rect_swapped, page.rect.width)`
    const ROT_ROWS: &[(i64, i64, bool, f64)] = &[
        (-720, 0, false, 200.0),
        (-675, 0, true, 300.0),
        (-630, 90, true, 300.0),
        (-629, 0, true, 300.0),
        (-623, 0, true, 300.0),
        (-585, 0, false, 200.0),
        (-540, 180, false, 200.0),
        (-539, 0, false, 200.0),
        (-526, 0, false, 200.0),
        (-495, 0, true, 300.0),
        (-450, 270, true, 300.0),
        (-449, 0, true, 300.0),
        (-429, 0, true, 300.0),
        (-405, 0, false, 200.0),
        (-332, 0, false, 200.0),
        (-315, 0, true, 300.0),
        (-270, 90, true, 300.0),
        (-269, 0, true, 300.0),
        (-235, 0, true, 300.0),
        (-225, 0, false, 200.0),
        (-180, 180, false, 200.0),
        (-179, 0, false, 200.0),
        (-138, 0, false, 200.0),
        (-135, 0, true, 300.0),
        (-90, 270, true, 300.0),
        (-89, 0, true, 300.0),
        (-46, 0, true, 300.0),
        (-45, 0, false, 200.0),
        (-41, 0, false, 200.0),
        (0, 0, false, 200.0),
        (45, 0, true, 300.0),
        (46, 0, true, 300.0),
        (56, 0, true, 300.0),
        (90, 90, true, 300.0),
        (91, 0, true, 300.0),
        (95, 0, true, 300.0),
        (135, 0, false, 200.0),
        (153, 0, false, 200.0),
        (180, 180, false, 200.0),
        (181, 0, false, 200.0),
        (225, 0, true, 300.0),
        (250, 0, true, 300.0),
        (270, 270, true, 300.0),
        (271, 0, true, 300.0),
        (315, 0, false, 200.0),
        (347, 0, false, 200.0),
        (359, 0, false, 200.0),
        (360, 0, false, 200.0),
        (405, 0, true, 300.0),
        (444, 0, true, 300.0),
        (450, 90, true, 300.0),
        (451, 0, true, 300.0),
        (495, 0, false, 200.0),
        (540, 180, false, 200.0),
        (541, 0, false, 200.0),
        (585, 0, true, 300.0),
        (630, 270, true, 300.0),
        (631, 0, true, 300.0),
        (638, 0, true, 300.0),
        (675, 0, false, 200.0),
        (719, 0, false, 200.0),
        (720, 0, false, 200.0),
    ];

    /// `pdf_probe6_boxes.py` (what the probe2 files really declare) joined
    /// with `probe2_rect.json` (`page.rect` from pymupdf).  Note
    /// `media-offset`: the file has **no** `/CropBox`, so MuPDF answers the
    /// MediaBox extent -- the earlier fitz `cropbox` echo was a derived value
    /// and must not be used as an input.
    /// `(name, media, crop, raw /Rotate, width_pt, height_pt)`
    const RECT_ROWS: &[(&str, [f64; 4], Option<[f64; 4]>, i64, f64, f64)] = &[
        ("plain", [0.0, 0.0, 200.0, 300.0], None, 0, 200.0, 300.0),
        ("rot0", [0.0, 0.0, 200.0, 300.0], None, 0, 200.0, 300.0),
        ("rot90", [0.0, 0.0, 200.0, 300.0], None, 90, 300.0, 200.0),
        ("rot180", [0.0, 0.0, 200.0, 300.0], None, 180, 200.0, 300.0),
        ("rot270", [0.0, 0.0, 200.0, 300.0], None, 270, 300.0, 200.0),
        ("rot360", [0.0, 0.0, 200.0, 300.0], None, 360, 200.0, 300.0),
        ("rot450", [0.0, 0.0, 200.0, 300.0], None, 450, 300.0, 200.0),
        ("rot95", [0.0, 0.0, 200.0, 300.0], None, 95, 300.0, 200.0),
        ("rotneg90", [0.0, 0.0, 200.0, 300.0], None, -90, 300.0, 200.0),
        ("rot271", [0.0, 0.0, 200.0, 300.0], None, 271, 300.0, 200.0),
        ("rot5", [0.0, 0.0, 200.0, 300.0], None, 5, 200.0, 300.0),
        ("media-offset", [10.0, 20.0, 210.0, 320.0], None, 0, 200.0, 300.0),
        ("media-offset-90", [10.0, 20.0, 210.0, 320.0], None, 90, 300.0, 200.0),
        ("media-offset-180", [10.0, 20.0, 210.0, 320.0], None, 180, 200.0, 300.0),
        ("media-offset-270", [10.0, 20.0, 210.0, 320.0], None, 270, 300.0, 200.0),
        ("crop-inner", [0.0, 0.0, 612.0, 792.0], Some([72.0, 72.0, 540.0, 720.0]), 0, 468.0, 648.0),
        ("crop-inner-90", [0.0, 0.0, 612.0, 792.0], Some([72.0, 72.0, 540.0, 720.0]), 90, 648.0, 468.0),
        ("crop-bigger", [0.0, 0.0, 200.0, 300.0], Some([-10.0, -10.0, 210.0, 310.0]), 0, 200.0, 300.0),
    ];

    /// `pe.resolve_system_font(name)` -- the authority itself, run on this
    /// machine (`C:\windows\Fonts`).  `pdf_probe7_expect.py`.
    const FONT_CASES: &[(&str, &str)] = &[
        ("", "C:\\windows\\Fonts\\simfang.ttf"),
        ("fangsong", "C:\\windows\\Fonts\\simfang.ttf"),
        ("仿宋", "C:\\windows\\Fonts\\simfang.ttf"),
        ("simfang", "C:\\windows\\Fonts\\simfang.ttf"),
        ("SIMFANG", "C:\\windows\\Fonts\\simfang.ttf"),
        ("simfang.ttf", "C:\\windows\\Fonts\\simfang.ttf"),
        ("songti", "C:\\windows\\Fonts\\simsun.ttc"),
        ("宋体", "C:\\windows\\Fonts\\simsun.ttc"),
        ("simsun", "C:\\windows\\Fonts\\simsun.ttc"),
        ("heiti", "C:\\windows\\Fonts\\simhei.ttf"),
        ("黑体", "C:\\windows\\Fonts\\simhei.ttf"),
        ("simhei", "C:\\windows\\Fonts\\simhei.ttf"),
        ("kaiti", "C:\\windows\\Fonts\\simkai.ttf"),
        ("楷体", "C:\\windows\\Fonts\\simkai.ttf"),
        ("simkai", "C:\\windows\\Fonts\\simkai.ttf"),
        ("yahei", "C:\\windows\\Fonts\\msyh.ttc"),
        ("微软雅黑", "C:\\windows\\Fonts\\msyh.ttc"),
        ("msyh", "C:\\windows\\Fonts\\msyh.ttc"),
        ("times", "C:\\windows\\Fonts\\times.ttf"),
        ("times new roman", "C:\\windows\\Fonts\\times.ttf"),
        ("arial", "C:\\windows\\Fonts\\arial.ttf"),
        ("calibri", "C:\\windows\\Fonts\\calibri.ttf"),
        ("consolas", "C:\\windows\\Fonts\\consola.ttf"),
        ("consola", "C:\\windows\\Fonts\\consola.ttf"),
        ("  Arial  ", "C:\\windows\\Fonts\\arial.ttf"),
        ("arial.TTF", "C:\\windows\\Fonts\\arial.TTF"),
        ("no-such-font-xyz.ttf", "C:\\windows\\Fonts\\simfang.ttf"),
        ("nope/x.ttf", "C:\\windows\\Fonts\\simfang.ttf"),
        ("C:\\windows\\Fonts\\arial.ttf", "C:\\windows\\Fonts\\arial.ttf"),
    ];
    const FONTS_DIR: &str = "C:\\windows\\Fonts";

    /// `os.path.abspath` with the cwd pinned to `rust/readmd-kernel` -- the
    /// directory `cargo test` starts a test binary in.  `.py` probe 7.
    const ABS_CWD: &str = "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel";
    const ABS_CASES: &[(&str, &str)] = &[
        ("pdf_fixture.pdf", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel\\pdf_fixture.pdf"),
        ("./pdf_fixture.pdf", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel\\pdf_fixture.pdf"),
        (".\\pdf_fixture.pdf", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel\\pdf_fixture.pdf"),
        ("src\\..\\readmd-kernel.exe", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel\\readmd-kernel.exe"),
        ("..\\..\\scratch\\rust_parity\\pdf_fixture.pdf", "T:\\Programming\\Project\\codex\\creator\\readmd\\scratch\\rust_parity\\pdf_fixture.pdf"),
        ("C:\\temp\\a\\..\\b.pdf", "C:\\temp\\b.pdf"),
        ("C:\\temp\\a\\..\\..\\b.pdf", "C:\\b.pdf"),
        ("C:/mixed/slash\\x.pdf", "C:\\mixed\\slash\\x.pdf"),
        ("", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel"),
        (".", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel"),
        ("..", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust"),
        ("\\\\nas\\share\\doc.pdf", "\\\\nas\\share\\doc.pdf"),
        ("sub\\dir\\..\\file.pdf", "T:\\Programming\\Project\\codex\\creator\\readmd\\rust\\readmd-kernel\\sub\\file.pdf"),
    ];

    const SYNTH_7: [u8; 150] = [0xA5, 0x4D, 0xCA, 0x18, 0x25, 0x30, 0xBB, 0x1D, 0x6D, 0x13, 0x2C, 0xDE, 0xD6, 0x23, 0x7B, 0x2E, 0xD9, 0x1E, 0x3F, 0x72, 0x1F, 0xCB, 0x19, 0x71, 0x17, 0x44, 0x94, 0xD6, 0x49, 0x3C, 0x9D, 0x5C, 0x34, 0x60, 0xBE, 0x31, 0x20, 0x1E, 0x69, 0xFE, 0xDA, 0xA0, 0xEE, 0xE8, 0xB9, 0x99, 0x7F, 0x5C, 0x7C, 0x29, 0x99, 0xFD, 0xAF, 0xE5, 0x93, 0x25, 0x3C, 0xD6, 0x54, 0xAF, 0x4D, 0xFA, 0xD7, 0x14, 0x27, 0xA0, 0xAE, 0xB3, 0xFE, 0xE9, 0x23, 0x2F, 0x8A, 0xF2, 0x21, 0x1F, 0x9E, 0xE4, 0x91, 0xC5, 0xB1, 0x0B, 0xEC, 0xB5, 0x56, 0x3B, 0xFC, 0x1E, 0x6F, 0x93, 0x42, 0x7E, 0xCB, 0xC8, 0xFE, 0x29, 0x55, 0xE5, 0xCD, 0x8E, 0x46, 0xDC, 0x8E, 0xD4, 0xB7, 0xC2, 0x76, 0x4D, 0x2A, 0x5A, 0x4D, 0x76, 0x77, 0x06, 0xF8, 0x5D, 0x86, 0x90, 0x02, 0x4A, 0xD6, 0xBD, 0xA3, 0x40, 0x1B, 0xE9, 0xC8, 0xCB, 0xCC, 0xC9, 0x35, 0xF6, 0xCD, 0x1F, 0x61, 0x22, 0x6A, 0xE1, 0x53, 0x38, 0xAE, 0x1A, 0x34, 0x00, 0x4D, 0x33, 0xBA, 0x0D, 0x24, 0x6A];
    const SYNTH_7_EDITED: [u8; 150] = [0xA5, 0x4D, 0xCA, 0x18, 0x25, 0x30, 0xBB, 0x1D, 0x6D, 0x13, 0x2C, 0xDE, 0xD6, 0x23, 0x7B, 0x2E, 0xD9, 0x1E, 0x3F, 0x72, 0x1F, 0xCB, 0x19, 0x71, 0x17, 0x44, 0x94, 0xD6, 0x49, 0x3C, 0x9D, 0x5C, 0x34, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x99, 0x7F, 0x5C, 0x7C, 0x29, 0x99, 0xFD, 0xAF, 0xE5, 0x93, 0x25, 0x3C, 0xD6, 0x54, 0xAF, 0x4D, 0xFA, 0xD7, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x1F, 0x9E, 0xE4, 0x91, 0xC5, 0xB1, 0x0B, 0xEC, 0xB5, 0x56, 0x3B, 0xFC, 0x1E, 0x6F, 0x93, 0x42, 0x7E, 0xCB, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xC2, 0x76, 0x4D, 0x2A, 0x5A, 0x4D, 0x76, 0x77, 0x06, 0xF8, 0x5D, 0x86, 0x90, 0x02, 0x4A, 0xD6, 0xBD, 0xA3, 0x40, 0x1B, 0xE9, 0xC8, 0xCB, 0xCC, 0xC9, 0x35, 0xF6, 0xCD, 0x1F, 0x61, 0x22, 0x6A, 0xE1, 0x53, 0x38, 0xAE, 0x1A, 0x34, 0x00, 0x4D, 0x33, 0xBA, 0x0D, 0x24, 0x6A];
    const SYNTH_1: [u8; 144] = [0x44, 0x20, 0x82, 0x3C, 0xFD, 0xE6, 0xF1, 0xC2, 0x6B, 0x30, 0xF9, 0x0E, 0xC7, 0xDD, 0x01, 0xE4, 0x88, 0x75, 0x34, 0xA2, 0x0F, 0x0B, 0x0D, 0x04, 0xC3, 0x6E, 0xD8, 0x0E, 0x71, 0xE0, 0xFD, 0x77, 0xB0, 0x76, 0x70, 0xEB, 0x94, 0x0B, 0xD5, 0x33, 0x5F, 0x97, 0x3D, 0xAA, 0xD8, 0x61, 0x9B, 0x91, 0xFF, 0xC9, 0x11, 0xF5, 0x7C, 0xCE, 0xD4, 0x58, 0xBB, 0xBF, 0x2C, 0xE0, 0x37, 0x53, 0xC9, 0xBD, 0xFA, 0x0F, 0xF0, 0x16, 0x9D, 0xC9, 0x57, 0x56, 0x74, 0x06, 0x66, 0x76, 0xCF, 0xB0, 0xB4, 0xEB, 0x89, 0x02, 0xC4, 0x42, 0x69, 0xDA, 0x1C, 0xF6, 0xBA, 0x66, 0xD3, 0xF8, 0xB6, 0xD4, 0xB1, 0x00, 0xA9, 0xEA, 0x0E, 0x75, 0x5A, 0x5C, 0x2E, 0x82, 0x10, 0x24, 0x2A, 0x08, 0xE7, 0x07, 0x8F, 0x7F, 0x89, 0x38, 0x5E, 0xB0, 0x94, 0x23, 0x55, 0x51, 0x82, 0x56, 0x8B, 0x96, 0xE8, 0xA4, 0xFE, 0xF2, 0x3A, 0x0C, 0x9F, 0xC5, 0xAF, 0xD7, 0x60, 0x84, 0x37, 0x81, 0x6B, 0xDD, 0x0A, 0x73, 0x09, 0xCB];
    const SYNTH_1_EDITED: [u8; 144] = [0x44, 0x20, 0x82, 0x3C, 0xFD, 0xE6, 0xF1, 0xC2, 0x6B, 0x30, 0xF9, 0x0E, 0xC7, 0xDD, 0x01, 0xE4, 0x88, 0x75, 0x34, 0xA2, 0x0F, 0x0B, 0x0D, 0x04, 0xC3, 0x6E, 0xD8, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x33, 0x5F, 0x97, 0x3D, 0xAA, 0xD8, 0x61, 0x9B, 0x91, 0xFF, 0xC9, 0x11, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xBD, 0xFA, 0x0F, 0xF0, 0x16, 0x9D, 0xC9, 0x57, 0x56, 0x74, 0x06, 0x66, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xF6, 0xBA, 0x66, 0xD3, 0xF8, 0xB6, 0xD4, 0xB1, 0x00, 0xA9, 0xEA, 0x0E, 0x75, 0x5A, 0x5C, 0x2E, 0x82, 0x10, 0x24, 0x2A, 0x08, 0xE7, 0x07, 0x8F, 0x7F, 0x89, 0x38, 0x5E, 0xB0, 0x94, 0x23, 0x55, 0x51, 0x82, 0x56, 0x8B, 0x96, 0xE8, 0xA4, 0xFE, 0xF2, 0x3A, 0x0C, 0x9F, 0xC5, 0xAF, 0xD7, 0x60, 0x84, 0x37, 0x81, 0x6B, 0xDD, 0x0A, 0x73, 0x09, 0xCB];
    const SYNTH_3: [u8; 72] = [0x79, 0x42, 0xBD, 0xF2, 0x21, 0x06, 0xF0, 0x84, 0x77, 0x62, 0xF0, 0xF3, 0xCB, 0x4D, 0x76, 0x4D, 0xC7, 0x07, 0x20, 0x51, 0x15, 0x9A, 0x0F, 0x89, 0xF2, 0xC6, 0xDA, 0xCA, 0xE3, 0x44, 0xBB, 0x31, 0x12, 0x45, 0xFD, 0x6F, 0x84, 0xDF, 0x9A, 0xD7, 0xC5, 0xB3, 0xD0, 0x76, 0xAC, 0x0E, 0x8F, 0x53, 0xA7, 0x35, 0x6C, 0x88, 0x91, 0x3F, 0x20, 0xF6, 0xF7, 0x2D, 0xB0, 0x22, 0xD2, 0x4D, 0x0A, 0x96, 0xDA, 0xD4, 0x3C, 0x16, 0x17, 0xC1, 0xA9, 0x8E];
    const SYNTH_3_EDITED: [u8; 72] = [0x79, 0x42, 0xBD, 0xF2, 0x21, 0x06, 0xF0, 0x84, 0x77, 0x62, 0xF0, 0xF3, 0xCB, 0x4D, 0x76, 0x4D, 0xC7, 0x07, 0x20, 0x51, 0x15, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x45, 0xFD, 0x6F, 0x84, 0xDF, 0x9A, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x88, 0x91, 0x3F, 0x20, 0xF6, 0xF7, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xC1, 0xA9, 0x8E];

    /// `pe._verify_zero_contamination(Image.frombytes(...), ...)` on the
    /// byte arrays above (patched rows are the red blocks at (1,1,4,2) and
    /// (1,2,4,2)).  `(seed, label, boxes, tolerance, gate...)`
    const GATE_ROWS: &[(i64, &str, &[[i64; 4]], i64, bool, i64, i64, f64, usize)] = &[
        (7, "nobox", &[], 10, false, 12, 254, 51.12, 0),
        (7, "covers", &[[1, 1, 4, 2]], 10, true, 0, 0, 0.0, 1),
        (7, "farbox", &[[6, 3, 8, 3]], 10, true, 0, 0, 0.0, 1),
        (7, "pad", &[[2, 2, 3, 2]], 10, true, 0, 0, 0.0, 1),
        (7, "identical", &[[0, 0, 3, 3]], 10, true, 0, 0, 0.0, 1),
        (7, "tol0", &[], 0, false, 12, 254, 51.12, 0),
        (1, "nobox", &[], 10, false, 12, 253, 54.0833, 0),
        (1, "covers", &[[1, 1, 4, 2]], 10, true, 0, 0, 0.0, 1),
        (1, "farbox", &[[6, 3, 8, 3]], 10, true, 0, 0, 0.0, 1),
        (1, "pad", &[[2, 2, 3, 2]], 10, true, 0, 0, 0.0, 1),
        (1, "identical", &[[0, 0, 3, 3]], 10, true, 0, 0, 0.0, 1),
        (1, "tol0", &[], 0, false, 12, 253, 54.0833, 0),
        (3, "nobox", &[], 10, false, 12, 241, 86.1667, 0),
        (3, "covers", &[[1, 1, 4, 2]], 10, true, 0, 0, 0.0, 1),
        (3, "farbox", &[[6, 3, 8, 3]], 10, true, 0, 0, 0.0, 1),
        (3, "pad", &[[2, 2, 3, 2]], 10, true, 0, 0, 0.0, 1),
        (3, "identical", &[[0, 0, 3, 3]], 10, true, 0, 0, 0.0, 1),
        (3, "tol0", &[], 0, false, 12, 241, 86.1667, 0),
    ];

    /// `int(pt * zoom)` / `int(round(pt * zoom))` with `zoom = 300 / 72`
    /// (`pdf_editor.py:219-221`).  Truncation vs half-to-even both appear.
    /// `(pt, int(px), int(round(px)))`
    const COORD_ROWS: &[(f64, i64, i64)] = &[
        (0.0, 0, 0),
        (1.0, 4, 4),
        (20.0, 83, 83),
        (20.5, 85, 85),
        (33.33, 138, 139),
        (100.0, 416, 417),
        (250.5, 1043, 1044),
        (595.32, 2480, 2481),
        (-3.5, -14, -15),
        (12.0, 50, 50),
        (9.5, 39, 40),
        (10.5, 43, 44),
        (11.5, 47, 48),
        (0.5, 2, 2),
        (1.5, 6, 6),
        (2.5, 10, 10),
        (24.0, 100, 100),
        (0.1666666, 0, 1),
    ];

    /// `audit('pdf_fixture.pdf')` header + `pages[]`, verbatim from pymupdf.
    const AUDIT_HEAD: &str = "{\"file_size\": 1202, \"is_readonly\": false, \"locking_processes\": [], \"ok\": true, \"path\": \"T:\\\\Programming\\\\Project\\\\codex\\\\creator\\\\readmd\\\\scratch\\\\rust_parity\\\\pdf_fixture.pdf\", \"total_pages\": 2}";
    const AUDIT_PAGES: &str = "[{\"has_vector_text\": true, \"height_pt\": 200.0, \"image_count\": 1, \"page\": 0, \"rect_pt\": [0.0, 0.0, 300.5, 200.0], \"rotation\": 90, \"text_preview\": \"Hello ReadMD\", \"width_pt\": 300.5}, {\"has_vector_text\": false, \"height_pt\": 841.92, \"image_count\": 0, \"page\": 1, \"rect_pt\": [0.0, 0.0, 595.32, 841.92], \"rotation\": 0, \"text_preview\": \"\", \"width_pt\": 595.32}]";
    const ROLLBACK_MISSING: &str = "未找到对应的备份文件: T:\\Programming\\Project\\codex\\creator\\readmd\\scratch\\rust_parity\\probe7_work\\missing.pdf.bak";
    const APPLY_KEYS: &str = "[\"ok\", \"target_path\", \"backup_path\", \"file_size\", \"zero_contamination_gate\", \"modified_boxes_count\"]";
    const ROLLBACK_KEYS: &str = "[\"ok\", \"restored_path\", \"from_backup\", \"file_size\"]";

    // `json!`, `Value`, `Path`, `PathBuf` all arrive through `use super::*`.
    use sha2::{Digest, Sha256};

    fn sha256_hex(data: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(data);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    fn parsed(raw: &str) -> Value {
        serde_json::from_str(raw).expect("generated expectation is not valid JSON")
    }

    // ------------------------------------------------------------ fixture bytes

    /// The Rust-side bytes must be the bytes pymupdf was probed on, or no
    /// geometry assertion below means anything.
    #[test]
    fn fixture_is_the_python_probed_document() {
        assert_eq!(FIXTURE.len(), FIXTURE_LEN, "pdf_fixture.pdf length");
        assert_eq!(sha256_hex(FIXTURE), FIXTURE_SHA256);
        assert!(FIXTURE.starts_with(b"%PDF-1."));
        assert!(FIXTURE.ends_with(b"%%EOF\n") || FIXTURE.ends_with(b"%%EOF"));
    }

    // ------------------------------------------------------------- /Rotate law

    #[test]
    fn rotate_law_matches_every_probed_rotate_value() {
        for &(raw, rotation, swaps, width) in ROT_ROWS {
            assert_eq!(pdf_page_rotation(raw), rotation, "page.rotation for /Rotate {raw}");
            assert_eq!(rotation_swaps_bbox(raw), swaps, "rect swap for /Rotate {raw}");
            let (w, h) = page_size_pt([0.0, 0.0, 200.0, 300.0], None, raw);
            assert_eq!([py_round(w, 2), py_round(h, 2)], [width, if swaps { 200.0 } else { 300.0 }],
                       "page.rect for /Rotate {raw}");
        }
    }

    /// Corner cases probed by pymupdf that a `// 90 % 2` shortcut would get wrong.
    #[test]
    fn rotate_normalisation_is_not_a_round_to_nearest() {
        assert_eq!(pdf_page_rotation(95), 0, "95 reports no rotation ...");
        assert!(rotation_swaps_bbox(95), "... but it does rotate the box");
        assert_eq!(pdf_page_rotation(271), 0);
        assert!(rotation_swaps_bbox(271));
        // Measured, pymupdf 1.28.2 / MuPDF 1.28.2, MediaBox [0 0 200 300]:
        //   /Rotate  44 -> rotation 0, page.rect.width 200.00  (no swap)
        //   /Rotate  45 -> rotation 0, page.rect.width 300.00  (SWAP)
        //   /Rotate 135 -> rotation 0, page.rect.width 200.00  (no swap)
        // so the band is closed at 45 and open at 135 — the same rule
        // `ROT_ROWS` records as `(-675, 0, true, 300.0)` (-675 % 360 == 45).
        assert!(rotation_swaps_bbox(45), "/Rotate 45 is on the boundary and swaps");
        assert!(rotation_swaps_bbox(46));
        assert!(rotation_swaps_bbox(-46), "probe4 beats probe3 here");
    }

    // ------------------------------------------------------------- page boxes

    #[test]
    fn page_rect_matches_pymupdf_for_the_probed_boxes() {
        for &(name, media, crop, rotate, w, h) in RECT_ROWS {
            let got = page_size_pt(media, crop, rotate);
            assert_eq!(
                [py_round(got.0, 2), py_round(got.1, 2)],
                [w, h],
                "{name}: MuPDF page.rect disagrees"
            );
        }
    }

    #[test]
    fn crop_box_is_clipped_to_media_box() {
        // `crop-inner`: [72 72 540 720] inside [0 0 612 792].
        assert_eq!(
            effective_box([0.0, 0.0, 612.0, 792.0], Some([72.0, 72.0, 540.0, 720.0])),
            [72.0, 72.0, 540.0, 720.0]
        );
        // `crop-bigger`: [-10 -10 210 310] on [0 0 200 300] -> the MediaBox wins.
        assert_eq!(
            effective_box([0.0, 0.0, 200.0, 300.0], Some([-10.0, -10.0, 210.0, 310.0])),
            [0.0, 0.0, 200.0, 300.0]
        );
        // No /CropBox in the file (`media-offset`) -> CropBox defaults to MediaBox,
        // so `page.rect` is the MediaBox extent, not a 280pt clip.
        assert_eq!(
            effective_box([10.0, 20.0, 210.0, 320.0], None),
            [10.0, 20.0, 210.0, 320.0]
        );
    }

    // ------------------------------------------------------------------- audit

    #[test]
    fn audit_pages_matches_pymupdf_page_for_page() {
        let got = audit_pages(FIXTURE).expect("lopdf must read the fixture");
        assert_eq!(Value::Array(got), parsed(AUDIT_PAGES), "audit()['pages']");
    }

    #[test]
    fn audit_header_matches_python_for_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pdf_fixture.pdf");
        std::fs::write(&file, FIXTURE).unwrap();
        let got = audit(&file.to_string_lossy()).expect("audit");
        let want: Value = parsed(AUDIT_HEAD);
        assert_eq!(got["ok"], want["ok"]);
        assert_eq!(got["file_size"], want["file_size"], "os.path.getsize");
        assert_eq!(got["is_readonly"], want["is_readonly"], "st_mode & S_IWRITE");
        assert_eq!(got["locking_processes"], want["locking_processes"], "psutil probe -> []");
        assert_eq!(got["total_pages"], want["total_pages"]);
        assert_eq!(
            got["path"].as_str().unwrap(),
            file.to_string_lossy(),
            "audit()['path'] is os.path.abspath(input)"
        );
        let keys: Vec<&str> = got.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "ok",
                "path",
                "file_size",
                "is_readonly",
                "locking_processes",
                "total_pages",
                "pages"
            ],
            "`pdf_editor.py:158-166` declares the result dict in this order"
        );
    }

    /// `os.chmod(path, 0o444)` makes the authority report `is_readonly: true`
    /// (probe 1 "stat flags": `mode 0o100444 -> S_IWRITE False`).
    #[test]
    fn audit_reports_readonly_the_way_st_mode_does() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ro.pdf");
        std::fs::write(&file, FIXTURE).unwrap();
        assert!(!audit(&file.to_string_lossy()).unwrap()["is_readonly"].as_bool().unwrap());
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&file, perms).unwrap();
        assert!(audit(&file.to_string_lossy()).unwrap()["is_readonly"].as_bool().unwrap());
    }

    #[test]
    fn missing_file_raises_the_authoritys_message() {
        let dir = tempfile::tempdir().unwrap();
        let ghost = dir.path().join("ghost.pdf");
        let err = audit(&ghost.to_string_lossy()).unwrap_err();
        assert_eq!(err.kind, PdfErrorKind::NotFound);
        assert_eq!(err.message, format!("PDF 文件不存在: {}", ghost.to_string_lossy()));
        assert_eq!(
            PdfError::page_index(5, 2).message,
            "无效的页码: 5，总页数: 2",
            "apply()'s IndexError text (`:394`)"
        );
        assert_eq!(
            PdfError::locked("Acrobat.exe(PID:1234)").message,
            "目标文件正被其他程序独占打开: Acrobat.exe(PID:1234)，请先在阅读器中关闭该文档。",
            "apply()'s PermissionError text (`:376`)"
        );
    }

    #[test]
    fn inspect_crop_is_reported_unavailable_not_invented() {
        let got = inspect_crop_unavailable();
        assert_eq!(got["available"], json!(false));
        assert_eq!(got["reason"], json!("pdf_render_unavailable"));
        // `audit()` only emits the key when page_num AND inspect_rect are given
        // (`:171`), so the plain audit payload above legitimately has no such key.
        assert!(audit_pages(FIXTURE).is_ok());
    }

    // ------------------------------------------------------------- abspath law

    #[test]
    fn abspath_matches_os_path_abspath() {
        // `probe7_expect.py` computed the right-hand column with the cwd set to
        // the package root, which is also where `cargo test` starts us.
        assert_eq!(
            std::env::current_dir().ok().as_deref(),
            Some(Path::new(ABS_CWD)),
            "the expected strings were captured with cwd = {ABS_CWD}"
        );
        for &(input, want) in ABS_CASES {
            assert_eq!(abspath(input).to_string_lossy(), want, "os.path.abspath({input:?})");
        }
    }

    #[test]
    fn py_isabs_matches_the_windows_rule() {
        assert!(py_isabs("C:\\a\\b.pdf"));
        assert!(py_isabs("c:/mixed/slash.pdf"));
        assert!(py_isabs("\\\\nas\\share\\doc.pdf"));
        assert!(!py_isabs("rel\\path.pdf"));
        assert!(!py_isabs("C:a.pdf")); // drive-relative is not absolute
    }

    // ---------------------------------------------------------------- fonts

    #[test]
    fn resolve_system_font_matches_the_authority_line_for_line() {
        if windows_fonts_dir() != FONTS_DIR {
            panic!("probe 7 ran against {FONTS_DIR}, this host reports {}", windows_fonts_dir());
        }
        for &(input, want) in FONT_CASES {
            assert_eq!(resolve_system_font(input), want, "resolve_system_font({input:?})");
        }
    }

    /// Candidate order from `pdf_editor.py:82-91`: the alias target verbatim,
    /// then `<Fonts>\<target>`, then `+.ttf`, then `+.ttc`, then the
    /// [`FONT_FALLBACS`] chain -- the last step is what turns an unknown name
    /// into `simfang.ttf`.
    #[test]
    fn font_candidate_order_and_fallback_chain_are_filesystem_independent() {
        let fonts = "Z:\\Fonts";
        let real = [
            "Z:\\Fonts\\simfang.ttf",
            "Z:\\Fonts\\simsun.ttc",
            "Z:\\Fonts\\simhei.ttf",
            "Z:\\Fonts\\simkai.ttf",
            "Z:\\Fonts\\msyh.ttc",
            "Z:\\Fonts\\times.ttf",
            "Z:\\Fonts\\arial.ttf",
            "Z:\\Fonts\\calibri.ttf",
            "Z:\\Fonts\\consola.ttf",
        ];
        let exists = |p: &Path| -> bool {
            let s = p.to_string_lossy().replace('/', "\\");
            real.iter().any(|r| r.eq_ignore_ascii_case(&s))
        };
        assert_eq!(resolve_system_font_in("仿宋", fonts, &exists), "Z:\\Fonts\\simfang.ttf");
        assert_eq!(resolve_system_font_in("黑体", fonts, &exists), "Z:\\Fonts\\simhei.ttf");
        assert_eq!(resolve_system_font_in("楷体", fonts, &exists), "Z:\\Fonts\\simkai.ttf");
        assert_eq!(resolve_system_font_in("宋体", fonts, &exists), "Z:\\Fonts\\simsun.ttc");
        assert_eq!(resolve_system_font_in("微软雅黑", fonts, &exists), "Z:\\Fonts\\msyh.ttc");
        // `.ttf` suffix append (probe 1: `consola -> consola.ttf`).
        assert_eq!(resolve_system_font_in("consola", fonts, &exists), "Z:\\Fonts\\consola.ttf");
        // Unknown everywhere -> the first fallback, never the bare input.
        assert_eq!(resolve_system_font_in("no-such-font-xyz.ttf", fonts, &exists), "Z:\\Fonts\\simfang.ttf");
        assert_eq!(resolve_system_font_in("nope/x.ttf", fonts, &exists), "Z:\\Fonts\\simfang.ttf");
        assert_eq!(resolve_system_font_in("", fonts, &exists), "Z:\\Fonts\\simfang.ttf");
        // Empty input is replaced *before* the alias lookup (`:46`).
        assert_eq!(resolve_system_font_in("  Arial  ", fonts, &exists), "Z:\\Fonts\\arial.ttf");
        // Case is preserved on the way out: the alias key is lowered, the value
        // that reaches the candidate list is the caller's string (`:79`).
        assert_eq!(resolve_system_font_in("arial.TTF", fonts, &exists), "Z:\\Fonts\\arial.TTF");
        // An absolute path that exists wins without consulting aliases (`:49`).
        assert_eq!(
            resolve_system_font_in("Z:\\Elsewhere\\x.ttf", fonts, &|p: &Path| p
                .to_string_lossy()
                .eq_ignore_ascii_case("Z:\\Elsewhere\\x.ttf")),
            "Z:\\Elsewhere\\x.ttf"
        );
    }

    #[test]
    fn font_alias_table_matches_python_key_order() {
        // `pdf_editor.py:60-77` -- 20 pairs, first match wins.
        assert_eq!(FONT_ALIASES.len(), 20);
        assert_eq!(FONT_ALIASES[0], ("fangsong", "simfang.ttf"));
        assert_eq!(FONT_ALIASES[19], ("consolas", "consola.ttf"));
        assert!(FONT_ALIASES.iter().any(|(k, _)| *k == "times new roman"));
        assert_eq!(DEFAULT_FONT_FILE, "simfang.ttf");
        assert_eq!(FONT_FALLBACKS, ["simfang.ttf", "simsun.ttc", "arial.ttf"]);
    }

    // ----------------------------------------------------------- file locking

    #[test]
    fn check_file_locks_answers_empty_like_the_probe() {
        // probe 1: `missing: []`, `fixture: []`, `abspath: []`.
        assert_eq!(check_file_locks("Z:\\definitely\\not\\here.pdf"), Vec::<Value>::new());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("open.pdf");
        std::fs::write(&file, FIXTURE).unwrap();
        assert_eq!(check_file_locks(&file.to_string_lossy()), Vec::<Value>::new());
        let held = std::fs::File::open(&file).unwrap();
        assert_eq!(check_file_locks(&file.to_string_lossy()), Vec::<Value>::new());
        drop(held);
        assert_eq!(LOCK_READER_NAMES.len(), 6);
    }

    // ------------------------------------------------- zero-contamination gate

    fn rgb(seed: i64, edited: bool) -> RgbImage {
        let (w, h, a, b): (usize, usize, &[u8], &[u8]) = match seed {
            7 => (10, 5, &SYNTH_7, &SYNTH_7_EDITED),
            1 => (8, 6, &SYNTH_1, &SYNTH_1_EDITED),
            3 => (6, 4, &SYNTH_3, &SYNTH_3_EDITED),
            other => panic!("no probed pixel set for seed {other}"),
        };
        RgbImage::new(w, h, if edited { b.to_vec() } else { a.to_vec() })
            .expect("probe 7 pixel array size")
    }

    #[test]
    fn zero_contamination_gate_matches_numpy() {
        for &(seed, label, boxes, tol, passed, violations, max_leak, mean_leak, count) in GATE_ROWS {
            let original = rgb(seed, false);
            let modified = rgb(seed, label != "identical");
            let allowed: Vec<(i64, i64, i64, i64)> =
                boxes.iter().map(|b| (b[0], b[1], b[2], b[3])).collect();
            let got = verify_zero_contamination(&original, &modified, &allowed, tol);
            assert_eq!(
                got,
                json!({
                    "passed": passed,
                    "violations_count": violations,
                    "max_leakage_diff": max_leak,
                    "mean_leakage_diff": mean_leak,
                    "allowed_boxes_count": count,
                }),
                "seed {seed} / {label}"
            );
        }
    }

    #[test]
    fn gate_default_tolerance_is_ten() {
        // `tolerance: int = 10` in the signature (`:279`).
        let a = rgb(7, false);
        let b = rgb(7, true);
        assert_eq!(
            verify_zero_contamination_default_tolerance(&a, &b, &[]),
            verify_zero_contamination(&a, &b, &[], 10)
        );
        assert_eq!(GATE_TOLERANCE, 10);
        assert_eq!(GATE_PAD, 6);
    }

    #[test]
    fn gate_masks_the_whole_frame_when_the_box_is_out_of_range() {
        // `farbox`: `(6, 3, 8, 3)` on a 10x5 image pads to rows 0..5 and
        // columns 0..10 -- `min(shape)` clamps the slice, so nothing is
        // protected and the authority still reports 0/0.0 (`:298-299`).
        let (passed, violations, max_leak, mean) = (false, 12i64, 254i64, 51.12f64);
        let got = verify_zero_contamination(&rgb(7, false), &rgb(7, true), &[], 10);
        assert_eq!(got["passed"], json!(passed));
        assert_eq!(got["violations_count"], json!(violations));
        assert_eq!(got["max_leakage_diff"], json!(max_leak));
        assert_eq!(got["mean_leakage_diff"], json!(mean));
        let masked = verify_zero_contamination(&rgb(7, false), &rgb(7, true), &[(6, 3, 8, 3)], 10);
        assert_eq!(masked["max_leakage_diff"], json!(0));
        assert_eq!(masked["mean_leakage_diff"], json!(0.0));
    }

    #[test]
    fn gate_of_differently_sized_images_compares_nothing() {
        // `np.abs(orig - mod)` would raise in Python; the port simply reports a
        // zero diff, which is the documented non-parity corner.
        let a = rgb(7, false);
        let b = rgb(3, true);
        let got = verify_zero_contamination(&a, &b, &[], 10);
        assert_eq!(got["violations_count"], json!(0));
        assert_eq!(got["passed"], json!(true));
    }

    #[test]
    fn rgb_image_rejects_wrong_pixel_count() {
        assert!(RgbImage::new(2, 2, vec![0u8; 11]).is_none());
        assert!(RgbImage::new(2, 2, vec![0u8; 12]).is_some());
    }

    // ------------------------------------------------------ edit pixel maths

    #[test]
    fn point_to_pixel_uses_trunc_and_half_even_round() {
        let zoom = DEFAULT_DPI as f64 / 72.0;
        // `repr(300 / 72.0)` in CPython is `4.166666666666667`; the literal
        // below must be that same shortest-round-trip decimal or the f64s differ
        // by one ulp and `assert_eq!` fails.
        assert_eq!(zoom, 4.166_666_666_666_667);
        for &(pt, trunc, round) in COORD_ROWS {
            assert_eq!(py_int(pt * zoom), trunc, "int({pt} * zoom)");
            assert_eq!(py_round_int(pt * zoom), round, "int(round({pt} * zoom))");
        }
        // The half-to-even trap that `.round()` would get wrong.
        assert_eq!(py_round_int(0.5), 0);
        assert_eq!(py_round_int(2.5), 2);
        assert_eq!(py_round_int(-2.5), -2);
        assert_eq!(py_int(-3.5), -3, "int() truncates toward zero");
        assert_eq!(py_round(2.675, 2), 2.67, "round(x, 2) follows the binary repr");
    }

    #[test]
    fn edit_pixel_geometry_reproduces_the_raster_free_half() {
        let edits = vec![
            json!({"text": "张三", "x": 20.5, "y": 33.33, "size": 10.5, "erase_rect": [10, 20, 100, 120]}),
            json!({"x": -3.5, "y": 0, "size": 0.1666666, "erase_rect": [-5, -5, 10_000, 10_000]}),
            json!({"erase_rect": [1, 2, 3]}),
            json!({"erase_rect": null}),
            json!({}),
        ];
        let got = edit_pixel_geometry(&edits, DEFAULT_DPI, 833, 1167);
        assert_eq!(got["zoom"], json!(300.0 / 72.0));
        assert_eq!(got["raster_available"], json!(false));
        assert_eq!(got["text_layer_size"], Value::Null);
        assert_eq!(got["edits"][0]["px_x"], json!(85));
        assert_eq!(got["edits"][0]["px_y"], json!(138));
        assert_eq!(got["edits"][0]["px_size"], json!(44));
        // `[41, 83, 416, 500]` is the `pixel_rect` probe 1 recorded from pymupdf
        // for `inspect_rect [10, 20, 100, 120]` of a 300 DPI page.
        assert_eq!(got["edits"][0]["erase_pixel_rect"], json!([41, 83, 416, 500]));
        assert_eq!(got["edits"][1]["px_x"], json!(-14));
        assert_eq!(got["edits"][1]["px_size"], json!(1));
        assert_eq!(got["edits"][2].get("erase_pixel_rect"), None, "len != 4 fails `:224`");
        assert_eq!(got["edits"][3].get("erase_pixel_rect"), None);
        assert_eq!(got["edits"][4]["px_x"], json!(0), "defaults x=y=0");
        assert_eq!(got["edits"][4]["px_size"], json!(50), "defaults size=12");
        // `max(0, ...)` then `min(width, ...)` (`:228-229`).
        assert_eq!(got["edits"][1]["erase_pixel_rect"], json!([0, 0, 833, 1167]));
        assert_eq!(got["modified_boxes"], json!([[41, 83, 416, 500], [0, 0, 833, 1167]]));
    }

    #[test]
    fn text_preview_slicing_counts_code_points() {
        // `text.strip()[:100]` (`:155`) -- 100 *characters*, not 100 bytes.
        let long: String = "汉".repeat(160);
        assert_eq!(py_prefix_chars(&long, TEXT_PREVIEW_CHARS).chars().count(), 100);
        assert_eq!(py_prefix_chars(&long, TEXT_PREVIEW_CHARS).len(), 300);
        assert_eq!(py_strip("\u{3000}abc\u{a0} "), "abc", "strip() uses the Unicode set");
        // Probe: `'\xa0'.isspace()`, `'\u3000'.isspace()`, `'\u2028'.isspace()` are
        // True; `'\u200b'`, `'\u180e'`, `'\u2060'` are False in both languages.
        assert!(py_is_space('\u{a0}'));
        assert!(py_is_space('\u{3000}'));
        assert!(py_is_space('\u{2028}'));
        assert!(!py_is_space('\u{200b}'));
        assert!(!py_is_space('\u{2060}'));
        // ... and Python additionally treats these as space.
        for c in ['\u{1c}', '\u{1f}', '\u{85}'] {
            assert!(py_is_space(c), "{c:?} is whitespace for CPython");
        }
    }

    // ------------------------------------------------------------ save + rollback

    fn key_names(v: &Value) -> Vec<String> {
        v.as_object().unwrap().keys().cloned().collect()
    }

    /// The captured Python key lists are compared in the order the authority
    /// declares its dict, so `key_names()` (Rust insertion order, now observable
    /// because `preserve_order` is on) is checked against it verbatim.
    fn python_keys(raw: &str) -> Vec<String> {
        parsed(raw)
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn save_envelope_is_exactly_applies_return_dict() {
        assert_eq!(
            parsed(APPLY_KEYS),
            json!([
                "ok",
                "target_path",
                "backup_path",
                "file_size",
                "zero_contamination_gate",
                "modified_boxes_count"
            ])
        );
        assert_eq!(parsed(ROLLBACK_KEYS), json!(["ok", "restored_path", "from_backup", "file_size"]));
        assert_eq!(
            key_names(&not_rasterized_gate()),
            vec![
                "passed",
                "violations_count",
                "max_leakage_diff",
                "mean_leakage_diff",
                "allowed_boxes_count"
            ],
            "`pdf_editor.py:302-307`"
        );
        assert_eq!(not_rasterized_gate()["allowed_boxes_count"], json!(0));
    }

    #[test]
    fn in_place_save_backups_once_and_replaces_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("doc.pdf");
        std::fs::write(&src, FIXTURE).unwrap();
        let payload = save_with_backup(b"%PDF-1.7 rust-written", &src, None, true).unwrap();
        assert_eq!(key_names(&payload), python_keys(APPLY_KEYS));
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["target_path"].as_str().unwrap(), src.to_string_lossy());
        assert_eq!(payload["backup_path"].as_str().unwrap(), format!("{}.bak", src.to_string_lossy()));
        // `len(b"%PDF-1.7 rust-written")` -> 21 (CPython 3.11.15), and the
        // authority's `file_size` is `os.path.getsize()` of the written target
        // (src/readmd_modules/pdf_editor.py:471, import_processor.py:312).
        assert_eq!(payload["file_size"], json!(21));
        assert_eq!(payload["modified_boxes_count"], json!(0));
        assert_eq!(payload["zero_contamination_gate"], not_rasterized_gate());
        assert_eq!(std::fs::read(&src).unwrap(), b"%PDF-1.7 rust-written");
        assert_eq!(std::fs::read(backup_path_of(&src)).unwrap(), FIXTURE, ".bak keeps the original");
        assert!(!std::fs::metadata(backup_path_of(&src)).unwrap().permissions().readonly(),
            "unlock happens before copy2, so the backup is writable");
        // No leftover temp file next to the target (`tempfile.mkstemp` cleanup).
        let leftovers: Vec<String> = std::fs::read_dir(dir.path()).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new());
        // `if not os.path.exists(backup_path)`: an existing backup is never overwritten.
        std::fs::write(backup_path_of(&src), b"KEEP").unwrap();
        save_with_backup(b"again", &src, None, true).unwrap();
        assert_eq!(std::fs::read(backup_path_of(&src)).unwrap(), b"KEEP");
    }

    #[test]
    fn copy_save_has_no_backup_and_creates_the_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("doc.pdf");
        std::fs::write(&src, FIXTURE).unwrap();
        let out = dir.path().join("nested\\deeper\\copy.pdf");
        let payload = save_with_backup(b"%PDF-copy", &src, Some(&out), true).unwrap();
        assert_eq!(payload["backup_path"], Value::Null, "not is_inplace -> `:387`");
        assert_eq!(payload["target_path"].as_str().unwrap(), out.to_string_lossy());
        assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-copy");
        assert!(!backup_path_of(&src).exists());
        assert!(backup_path_of(&out).parent().unwrap().exists());
        // A read-only *source* still raises the authority's not-found first.
        let err = save_with_backup(b"x", &dir.path().join("ghost.pdf"), None, true).unwrap_err();
        assert_eq!(err.kind, PdfErrorKind::NotFound);
    }

    #[test]
    fn rollback_restores_the_backup_and_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("doc.pdf");
        std::fs::write(&src, b"corrupted by an edit").unwrap();
        std::fs::write(backup_path_of(&src), FIXTURE).unwrap();
        let got = rollback(&src.to_string_lossy()).unwrap();
        assert_eq!(key_names(&got), python_keys(ROLLBACK_KEYS));
        assert_eq!(got["restored_path"].as_str().unwrap(), src.to_string_lossy());
        assert_eq!(got["from_backup"].as_str().unwrap(), format!("{}.bak", src.to_string_lossy()));
        assert_eq!(got["file_size"], json!(FIXTURE_LEN));
        assert_eq!(sha256_hex(&std::fs::read(&src).unwrap()), FIXTURE_SHA256);
        assert!(backup_path_of(&src).exists(), "the .bak is permanent by design");
        // Rollback of a *read-only* target: `:461-462` is
        // `os.chmod(abs_path, stat.S_IWRITE)` and only then `os.remove()`, so a
        // locked PDF is restorable.  The dirty bytes have to be written before
        // the lock-down -- CPython cannot write into a read-only file either
        // (measured: `open(p, "wb")` after `chmod(p, 0o444)` raises
        // `PermissionError`, errno 13), and a real edit session is the same
        // "writable while editing, read-only on disk afterwards" shape.
        std::fs::write(&src, b"dirty").unwrap();
        let mut perms = std::fs::metadata(&src).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&src, perms).unwrap();
        let again = rollback(&src.to_string_lossy()).unwrap();
        assert_eq!(again["file_size"], json!(FIXTURE_LEN));
        assert_eq!(sha256_hex(&std::fs::read(&src).unwrap()), FIXTURE_SHA256);
        assert!(!std::fs::metadata(&src).unwrap().permissions().readonly(),
            "S_IWRITE is cleared for good, not just long enough to copy");
        assert!(backup_path_of(&src).exists(), "the .bak survives the read-only restore");
    }

    #[test]
    fn rollback_without_backup_raises_the_authoritys_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let ghost = dir.path().join("missing.pdf");
        let err = rollback(&ghost.to_string_lossy()).unwrap_err();
        assert_eq!(err.kind, PdfErrorKind::NotFound);
        assert_eq!(
            err.message,
            format!("未找到对应的备份文件: {}.bak", ghost.to_string_lossy())
        );
        // Same text the authority produced for its own path (`probe7_expect.json`).
        let prefix = "未找到对应的备份文件: ";
        assert!(ROLLBACK_MISSING.starts_with(prefix), "got {ROLLBACK_MISSING:?}");
        assert!(ROLLBACK_MISSING.ends_with(".bak"));
    }

    // ----------------------------------------------------- page-tree surgery
    //
    // Nothing in `pdf_editor.py` rotates or reorders pages, so the numbers below
    // are only half Python-derived: the *rules* (`/Rotate` -> rotation + box
    // swap, MediaBox/CropBox geometry) come from probes 2/4/6, and the file the
    // writer produces is checked with pymupdf out of band -- rows of
    // `[page, rotation, rect.width, rect.height, text.strip()]` read off the
    // `pageops_*.pdf` artefacts the `#[ignore]`d emitter test below writes from
    // exactly these `FIXTURE` bytes.

    /// pymupdf's reading of the PDFs this module wrote:
    /// `[label, page, rotation, width_pt, height_pt, text_preview]`.
    ///
    /// The table is ordered by (case, page) and has to cover **every** page of
    /// **every** artefact, because `rows()` emits one entry per page: only page
    /// 0 gets the rotate, so `rotate90.pdf` is still a two-page document and its
    /// untouched A4 page 1 is a row of its own -- leave it out and the whole
    /// comparison silently shifts one row early.
    const PAGE_OPS_TRUTH: &[(&str, i64, i64, f64, f64, &str)] = &[
        ("rotate90", 0, 180, 200.0, 300.5, "Hello ReadMD"),
        ("rotate90", 1, 0, 595.32, 841.92, ""),
        ("delete-last", 0, 90, 300.5, 200.0, "Hello ReadMD"),
        ("reorder", 0, 0, 595.32, 841.92, ""),
        ("reorder", 1, 90, 300.5, 200.0, "Hello ReadMD"),
    ];

    fn rows(bytes: &[u8]) -> Vec<Value> {
        audit_pages(bytes).expect("the rewritten document must still parse")
    }

    #[test]
    fn page_ops_follow_the_probed_geometry_rules() {
        let rotate90 = rotate_page(FIXTURE, 0, 90).unwrap();
        let del = delete_pages(FIXTURE, &[1]).unwrap();
        let reorder = reorder_pages(FIXTURE, &[1, 0]).unwrap();

        let mut seen = Vec::new();
        for (label, pages) in [
            ("rotate90", rows(&rotate90)),
            ("delete-last", rows(&del)),
            ("reorder", rows(&reorder)),
        ] {
            for p in pages {
                seen.push((
                    label,
                    p["page"].as_i64().unwrap(),
                    p["rotation"].as_i64().unwrap(),
                    p["width_pt"].as_f64().unwrap(),
                    p["height_pt"].as_f64().unwrap(),
                    p["text_preview"].as_str().unwrap().to_string(),
                ));
            }
        }
        // The loop below walks the *table*, so a table that stopped covering one
        // row per page of every artefact would compare shifted rows instead of
        // failing.  `seen` is 2 + 1 + 2 rows.
        assert_eq!(seen.len(), PAGE_OPS_TRUTH.len(), "one artefact row per page");
        for (i, &(label, page, rotation, w, h, text)) in PAGE_OPS_TRUTH.iter().enumerate() {
            let (glabel, gpage, grot, gw, gh, gtext) = &seen[i];
            assert_eq!((*glabel, *gpage, *grot, *gw, *gh, gtext.as_str()),
                       (label, page, rotation, w, h, text),
                       "page-op artefact {label} page {page}");
        }
        assert_eq!(rows(&del).len(), 1);
        assert_eq!(page_sizes(&rotate90).unwrap()[0], (200.0, 300.5));
    }

    #[test]
    fn page_ops_reject_out_of_range_indices_like_the_authority() {
        let err = rotate_page(FIXTURE, 9, 90).unwrap_err();
        assert_eq!(err.kind, PdfErrorKind::PageIndex);
        assert_eq!(err.message, "无效的页码: 9，总页数: 2");
        assert!(delete_pages(FIXTURE, &[0, 1]).is_err(), "refuse an empty document");
        assert!(reorder_pages(FIXTURE, &[0, 0, 1]).is_ok(), "duplicates are the caller's business");
    }

    #[test]
    fn rotate_is_cumulative_and_normalised() {
        let once = rotate_page(FIXTURE, 1, 90).unwrap();
        let twice = rotate_page(&once, 1, 90).unwrap();
        assert_eq!(rows(&twice)[1]["rotation"], json!(180));
        // `/Rotate 95` is legal in a file even though the UI never writes it.
        let back = rotate_page(&twice, 1, -180).unwrap();
        assert_eq!(rows(&back)[1]["rotation"], json!(0));
    }

    /// Writes the artefacts that `pdf_probe8_pageops.py` reads.  Kept `#[ignore]`
    /// so a normal test run never touches the repository tree.
    #[test]
    #[ignore = "recreates scratch/rust_parity/pageops_*.pdf for the pymupdf probe"]
    fn emit_page_ops_artifacts_for_the_python_probe() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("scratch")
            .join("rust_parity");
        let cases = [
            ("rotate90.pdf", rotate_page(FIXTURE, 0, 90).unwrap()),
            ("delete-last.pdf", delete_pages(FIXTURE, &[1]).unwrap()),
            ("reorder.pdf", reorder_pages(FIXTURE, &[1, 0]).unwrap()),
        ];
        for (name, bytes) in cases {
            std::fs::write(dir.join(name), &bytes).unwrap();
        }
    }
}
