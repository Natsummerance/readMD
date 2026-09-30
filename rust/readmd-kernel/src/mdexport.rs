//! Pure Rust Document Export Engine (HTML, PDF, DOCX, EPUB, LaTeX/TeX)
//!
//! Replaces legacy Python export modules (src/readmd_modules/mdexport/ and texmd.py).
//! Zero external Python runtime dependency, 100% pure native Rust implementation.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use base64::Engine;

// ============================================================================
// Types & Result Model
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub ok: bool,
    pub path: Option<String>,
    pub size: Option<u64>,
    pub warns: Option<Vec<String>>,
    pub error: Option<String>,
    pub canceled: Option<bool>,
}

pub struct ZipEntry {
    pub path: String,
    pub data: Vec<u8>,
    pub compress: bool,
}

// ============================================================================
// Minimal Pure Rust ZIP Writer (using flate2)
//
// Byte layout mirrors CPython `zipfile` so the archives written by the export
// engines are structurally identical to the Python reference product:
//   * local file header  : sig, version-needed 20, flags, method, dos time/date,
//                          crc32, comp size, uncomp size, name len, extra len 0
//   * central directory  : version-made-by 20 (create_system 0 = DOS/Windows),
//                          version-needed 20, same flags/time/crc/sizes,
//                          comment len 0, disk 0, internal attrs 0,
//                          external attrs 0o600 << 16 (what `writestr` sets)
//   * end of central dir : single-disk record, no comment
//   * the UTF-8 name flag (0x0800) is only set for non-ASCII entry names, and
//     no data descriptor is ever written (`writestr` knows the sizes upfront).
// ============================================================================

/// ZIP general purpose bit flag for an entry name, matching the flags half of
/// CPython's `zipfile._encodeFilenameFlags`: 0x0800 only when the name is not
/// representable in the code page (here: not ASCII).
fn zip_flags(name: &str) -> u16 {
    if name.is_ascii() {
        0
    } else {
        0x0800
    }
}

/// MS-DOS timestamp pair for `datetime`, mirroring `zipfile`'s encoding.
pub fn dos_datetime(year: u16, month: u16, day: u16, hour: u16, min: u16, sec: u16) -> (u16, u16) {
    let y = year.max(1980);
    let date = (y - 1980) << 9 | month << 5 | day;
    let time = hour << 11 | min << 5 | (sec / 2);
    (time, date)
}

/// Current wall-clock as a DOS (time, date) pair. Python uses `time.localtime()`;
/// the Rust kernel has no tz database, so it stamps UTC (same field layout).
fn dos_now() -> (u16, u16) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Civil-days algorithm from `time` crate's documented days-from-civil inverse.
    let days = (secs / 86400) as i64;
    let tod = (secs % 86400) as u32;
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097).max(0);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).max(0) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let (y, m) = if m <= 2 { (y + 1, m) } else { (y, m) };
    dos_datetime(
        y as u16,
        m as u16,
        d as u16,
        (tod / 3600) as u16,
        ((tod % 3600) / 60) as u16,
        (tod % 60) as u16,
    )
}

pub fn write_zip(entries: &[ZipEntry]) -> Result<Vec<u8>, String> {
    let (dos_time, dos_date) = dos_now();
    write_zip_at(entries, dos_time, dos_date)
}

/// ZIP writer with an explicit DOS timestamp so tests stay byte-deterministic.
pub fn write_zip_at(entries: &[ZipEntry], dos_time: u16, dos_date: u16) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut cd_records: Vec<(String, u16, u32, u32, u32, u32)> = Vec::new();

    for entry in entries {
        let local_header_offset = out.len() as u32;
        let mut crc = flate2::Crc::new();
        crc.update(&entry.data);
        let crc32 = crc.sum();
        let uncompressed_size = entry.data.len() as u32;

        let (method, compressed_data) = if entry.compress {
            // Python's ZipFile default compresslevel is zlib's default (-1 => level 6).
            // `writestr` keeps method=8 even for an empty member: CPython 3.11 emits a
            // 2-byte raw-deflate stream (`03 00`), never a STORED fallback.
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
            encoder.write_all(&entry.data).map_err(|e| e.to_string())?;
            let res = encoder.finish().map_err(|e| e.to_string())?;
            (8u16, res)
        } else {
            (0u16, entry.data.clone())
        };
        let compressed_size = compressed_data.len() as u32;

        let filename_bytes = entry.path.as_bytes();
        let filename_len = filename_bytes.len() as u16;
        let flags = zip_flags(&entry.path);

        // Local file header (30 bytes + filename)
        out.extend_from_slice(&0x04034b50u32.to_le_bytes()); // signature
        out.extend_from_slice(&20u16.to_le_bytes());        // version needed to extract
        out.extend_from_slice(&flags.to_le_bytes());        // general purpose bit flag
        out.extend_from_slice(&method.to_le_bytes());        // compression method
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&crc32.to_le_bytes());
        out.extend_from_slice(&compressed_size.to_le_bytes());
        out.extend_from_slice(&uncompressed_size.to_le_bytes());
        out.extend_from_slice(&filename_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());          // extra length
        out.extend_from_slice(filename_bytes);
        out.extend_from_slice(&compressed_data);

        cd_records.push((
            entry.path.clone(),
            method,
            crc32,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        ));
    }

    let cd_offset = out.len() as u32;
    for (name, method, crc32, comp_size, uncomp_size, local_offset) in &cd_records {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u16;
        let flags = zip_flags(name);

        // Central directory header (46 bytes + filename)
        out.extend_from_slice(&0x02014b50u32.to_le_bytes()); // signature
        out.extend_from_slice(&20u16.to_le_bytes());         // version made by (DOS, 2.0)
        out.extend_from_slice(&20u16.to_le_bytes());         // version needed
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&crc32.to_le_bytes());
        out.extend_from_slice(&comp_size.to_le_bytes());
        out.extend_from_slice(&uncomp_size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());          // extra length
        out.extend_from_slice(&0u16.to_le_bytes());          // comment length
        out.extend_from_slice(&0u16.to_le_bytes());          // disk number start
        out.extend_from_slice(&0u16.to_le_bytes());          // internal attributes
        out.extend_from_slice(&(0o600u32 << 16).to_le_bytes()); // external attributes
        out.extend_from_slice(&local_offset.to_le_bytes());
        out.extend_from_slice(name_bytes);
    }
    let cd_size = (out.len() as u32) - cd_offset;

    // End of central directory record (22 bytes)
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());          // disk number
    out.extend_from_slice(&0u16.to_le_bytes());          // start disk
    out.extend_from_slice(&(cd_records.len() as u16).to_le_bytes()); // entries on this disk
    out.extend_from_slice(&(cd_records.len() as u16).to_le_bytes()); // total entries
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());          // comment len

    Ok(out)
}

// ============================================================================
// Image Resolution & Embedding — `mdexport/__init__.py:33-74` (`ImageResolver`)
//
// The HTML branch of `mdexport/__init__.py` (`export()`, lines 121-132) is the
// only exporter that rewrites the markdown before it reaches the renderer:
// `re.sub(r'!\[([^\]]*)\]\(([^)\s]+)\)', embed_image, content)`.  Every image
// it cannot embed is left byte-identical, and every image it *skips* appends a
// warning to the `warns` list the API returns — so both the document text and
// the warning list are observable output.
// ============================================================================

/// `mimetypes.guess_type()` restricted to the `image/*` half of CPython 3.11's
/// portable table (`Lib/mimetypes.py` `_types_map_default`).  Only that half can
/// matter to `embed_image`: any result that does not start with `image/` (and
/// `None`, which becomes `'application/octet-stream'` at
/// `mdexport/__init__.py:127`) makes it return the original `![...](...)` text
/// untouched.
///
/// Known, unavoidable environment divergence: on Windows `mimetypes` merges
/// `HKCR\<.ext>\Content Type` over this table at import time, so the reference
/// machine resolves `guess_type('x.webp') -> ''` (blank registry value, i.e. it
/// silently *skips* webp) and `'.ico' -> 'image/x-icon'`, while the portable
/// stdlib table says `image/webp` / `image/vnd.microsoft.icon`.  A Rust kernel
/// has no registry to consult, so the stdlib table is used.
const MIME_IMAGE_TYPES: [(&str, &str); 23] = [
    (".avif", "image/avif"),
    (".bmp", "image/bmp"),
    (".gif", "image/gif"),
    (".heic", "image/heic"),
    (".heif", "image/heif"),
    (".ico", "image/vnd.microsoft.icon"),
    (".ief", "image/ief"),
    (".jpe", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".jpg", "image/jpeg"),
    (".pbm", "image/x-portable-bitmap"),
    (".pgm", "image/x-portable-graymap"),
    (".png", "image/png"),
    (".pnm", "image/x-portable-anymap"),
    (".ppm", "image/x-portable-pixmap"),
    (".ras", "image/x-cmu-raster"),
    (".rgb", "image/x-rgb"),
    (".svg", "image/svg+xml"),
    (".tif", "image/tiff"),
    (".tiff", "image/tiff"),
    (".xbm", "image/x-xbitmap"),
    (".xpm", "image/x-xpixmap"),
    (".xwd", "image/x-xwindowdump"),
];

/// `MimeTypes.encodings_map` (`mimetypes.py`), looked up **case-sensitively**.
/// `guess_type` strips one of these first and then types the remaining
/// extension, so `photo.png.gz` is reported as `image/png` and embedded.
const MIME_ENCODING_SUFFIXES: [&str; 5] = [".Z", ".br", ".bz2", ".gz", ".xz"];

/// `mimetypes.guess_type(path)[0]` for the image half of the table.  The
/// extension is `posixpath.splitext` of the drive-stripped path: everything
/// after the last `.` unless the path ends with a `.`, which yields `''`
/// (`guess_type` → `None` → the caller substitutes `application/octet-stream`).
fn mime_image_of(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let mut body = name.as_str();
    // `posixpath.splitext` + the `encodings_map` rung of `guess_type`.
    match body.rfind('.') {
        Some(dot) if dot + 1 < body.len() => {
            let ext = &body[dot..];
            if MIME_ENCODING_SUFFIXES.contains(&ext) {
                body = &body[..dot];
            }
        }
        _ => {}
    }
    let dot = body.rfind('.')?;
    if dot + 1 >= body.len() {
        return None; // trailing '.' -> splitext gives an empty extension
    }
    let ext = body[dot..].to_lowercase();
    MIME_IMAGE_TYPES
        .iter()
        .find(|(e, _)| *e == ext.as_str())
        .map(|(_, mime)| *mime)
}

/// `os.path.isabs` with `ntpath` semantics: a leading separator or a drive.
/// (`posixpath.isabs` is the leading-separator half of the same test.)
fn py_isabs(p: &str) -> bool {
    let b = p.as_bytes();
    b.starts_with(b"\\") || b.starts_with(b"/")
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

/// `os.path.join(a, b)` for the two-argument form in `ImageResolver.resolve`.
fn py_join(a: &str, b: &str) -> String {
    if b.is_empty() {
        return a.to_string();
    }
    if py_isabs(b) {
        return b.to_string();
    }
    if a.is_empty() {
        return b.to_string();
    }
    if a.ends_with('\\') || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}\\{b}")
    }
}

/// `src[:80]` — CPython slices by code point, not by byte.
fn py_head80(s: &str) -> String {
    s.chars().take(80).collect()
}

/// What `ImageResolver.resolve` hands back: either a real file on disk, or the
/// blob a `data:` URL decoded to.  CPython writes that blob into the export
/// `tmpdir` and returns the temp path; the file is pure detour (its name is a
/// sha256 hex + the subtype, and only its extension and size are read back), so
/// the bytes are carried directly.
#[derive(Clone)]
enum Resolved {
    File(PathBuf),
    Inline { mime: String, data: Vec<u8> },
}

/// `mdexport/__init__.py:42-74` — `ImageResolver.resolve` for the HTML branch
/// (`tmpdir` is always set there, because `export()` creates it at line 97).
struct HtmlImageResolver {
    base_dir: String,
    /// `self._cache`.  Only the *local file* outcome (including the negative
    /// one) and a *successful* `data:` decode are stored, so a failed `data:`
    /// decode warns once per occurrence while a missing file warns once per
    /// source string.
    cache: Vec<(String, Option<Resolved>)>,
}

impl HtmlImageResolver {
    fn new(base_dir: &str) -> Self {
        HtmlImageResolver { base_dir: base_dir.to_string(), cache: Vec::new() }
    }

    /// `if src in self._cache: return self._cache[src]` — a hit short-circuits,
    /// so the stored value is handed back as-is (and `None` is a *silent*
    /// return, not a warning).
    fn cached(&self, src: &str) -> Option<Option<Resolved>> {
        self.cache.iter().find(|(k, _)| k == src).map(|(_, v)| v.clone())
    }

    fn remember(&mut self, src: &str, value: Option<Resolved>) {
        match self.cache.iter_mut().find(|(k, _)| k == src) {
            Some(slot) => slot.1 = value,
            None => self.cache.push((src.to_string(), value)),
        }
    }

    fn resolve(&mut self, src: &str, warns: &mut Vec<String>) -> Option<Resolved> {
        // `src = (src or '').strip()` — CPython's `str.strip()` whitespace set
        // covers `\x1c`-`\x1f` on top of Unicode `White_Space`.
        let src = py_strip(src);
        if src.is_empty() {
            return None;
        }

        if src.starts_with("data:image/") {
            if let Some(hit) = self.cached(src) {
                return hit;
            }
            if let Some(target) = decode_data_image(src) {
                self.remember(src, Some(target.clone()));
                return Some(target);
            }
            // CPython stores nothing here and falls through to the `data:`
            // guard below, so this branch must not touch the cache.
        }

        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            warns.push(format!("远程/内联图片不支持嵌入，已跳过：{}", py_head80(src)));
            return None;
        }

        if let Some(hit) = self.cached(src) {
            return hit;
        }

        let decoded = percent_encoding::percent_decode_str(src).decode_utf8_lossy().to_string();
        let cand = if py_isabs(&decoded) {
            decoded
        } else {
            py_join(&self.base_dir, &decoded)
        };
        let cand = crate::convert::py_normpath(&cand);
        let path = PathBuf::from(&cand);
        if path.is_file() {
            let hit = Resolved::File(path);
            self.remember(src, Some(hit.clone()));
            return Some(hit);
        }
        warns.push(format!("图片不存在，已跳过：{src}"));
        self.remember(src, None);
        None
    }
}

/// `re.fullmatch(r'data:image/(png|jpeg|gif|webp);base64,([A-Za-z0-9+/=\s]+)',
/// src)` + `len(match[2]) <= 32 * 1024 * 1024` + `base64.b64decode(match[2],
/// validate=True)` (`mdexport/__init__.py:49-58`).  The subtype becomes the temp
/// file's extension, and `guess_type` maps those four back to `image/<subtype>`
/// unchanged, so it is carried as the mime directly.
///
/// A Python `\s` class inside the pattern matches `U+001C`-`U+001F` as well
/// (`Py_UNICODE_ISSPACE`), which Rust's `\s` (Unicode `White_Space`) does not —
/// hence the explicit range.  Any whitespace still reaches `a2b_base64` and
/// fails its strict-mode check there, exactly like CPython.
fn decode_data_image(src: &str) -> Option<Resolved> {
    static DATA_IMAGE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = DATA_IMAGE_RE.get_or_init(|| {
        regex::Regex::new(r"^data:image/(png|jpeg|gif|webp);base64,([A-Za-z0-9+/=\s\x1c-\x1f]+)$")
            .expect("data image regex")
    });
    let caps = re.captures(src)?;
    if caps[2].chars().count() > 32 * 1024 * 1024 {
        return None;
    }
    let data = a2b_base64_strict(&caps[2])?;
    Some(Resolved::Inline { mime: format!("image/{}", &caps[1]), data })
}

/// `binascii.a2b_base64(data, strict_mode=True)` (Modules/binascii.c, CPython
/// 3.11) ported statement for statement, so every strict-mode rejection —
/// leading padding, discontinuous padding, excess data after padding, a lone
/// extra data character, missing padding, non-alphabet characters — fails the
/// same way Python does and the caller falls back to warning + leaving the
/// `![...](...)` text untouched.
fn a2b_base64_strict(text: &str) -> Option<Vec<u8>> {
    fn digit(ch: u8) -> Option<u8> {
        match ch {
            b'A'..=b'Z' => Some(ch - b'A'),
            b'a'..=b'z' => Some(ch - b'a' + 26),
            b'0'..=b'9' => Some(ch - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    if !bytes.is_empty() && bytes[0] == b'=' {
        return None; // Leading padding not allowed
    }
    let mut out: Vec<u8> = Vec::new();
    let mut quad_pos = 0usize;
    let mut leftchar = 0u8;
    let mut pads = 0i32;
    let mut padding_started = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'=' {
            padding_started = true;
            if quad_pos >= 2 && quad_pos as i32 + { pads += 1; pads } >= 4 {
                // A pad sequence ends the scan; strict mode rejects trailing
                // garbage after it.
                if i + 1 < bytes.len() {
                    return None; // Excess data after padding
                }
                return Some(out);
            }
            i += 1;
            continue;
        }
        let Some(val) = digit(ch) else {
            return None; // Only base64 data is allowed
        };
        if padding_started {
            return None; // Discontinuous padding not allowed
        }
        pads = 0;
        match quad_pos {
            0 => {
                quad_pos = 1;
                leftchar = val;
            }
            1 => {
                quad_pos = 2;
                out.push((leftchar << 2) | (val >> 4));
                leftchar = val & 0x0f;
            }
            2 => {
                quad_pos = 3;
                out.push((leftchar << 4) | (val >> 2));
                leftchar = val & 0x03;
            }
            _ => {
                quad_pos = 0;
                out.push((leftchar << 6) | val);
                leftchar = 0;
            }
        }
        i += 1;
    }
    if quad_pos != 0 {
        return None; // quad_pos == 1: invalid length, else Incorrect padding
    }
    Some(out)
}

/// `mdexport/__init__.py:124-131` — the `re.sub` callback `embed_image`.
fn embed_images_for_html(content: &str, base_dir: &str, warns: &mut Vec<String>) -> String {
    static IMG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = IMG_RE.get_or_init(|| {
        // Python's `[^)\s]` also excludes `U+001C`-`U+001F` (they are `\s` for
        // `Py_UNICODE_ISSPACE`), so the negated class has to list them.
        regex::Regex::new(r"!\[([^\]]*)\]\(([^)\s\x1c-\x1f]+)\)").expect("export image regex")
    });
    let mut resolver = HtmlImageResolver::new(base_dir);
    re.replace_all(content, |caps: &regex::Captures| {
        let whole = caps[0].to_string();
        let alt = &caps[1];
        let Some(target) = resolver.resolve(&caps[2], warns) else {
            return whole;
        };
        let (mime, data) = match target {
            Resolved::File(p) => {
                // `guess_type` first, then the 24 MiB size cap, then the read:
                // in that order, so an oversized or non-image file is never
                // opened.
                let Some(m) = mime_image_of(&p) else { return whole };
                match fs::metadata(&p) {
                    Ok(md) if md.len() <= 24 * 1024 * 1024 => match fs::read(&p) {
                        Ok(bytes) => (m.to_string(), bytes),
                        Err(_) => return whole,
                    },
                    _ => return whole,
                }
            }
            Resolved::Inline { mime, data } => {
                if data.len() > 24 * 1024 * 1024 {
                    return whole;
                }
                (mime, data)
            }
        };
        // `if not mime.startswith('image/')`: only reachable for the temp-file
        // path of a `data:` URL, whose subtype is always `image/*` — kept for
        // the file branch's table values that could one day be non-image.
        if !mime.starts_with("image/") {
            return whole;
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
        format!("![{alt}](data:{mime};base64,{encoded})")
    })
    .to_string()
}

// ============================================================================
// HTML Export — `mdexport/html_render.py`
// ============================================================================

/// `html_render._read_asset` (`html_render.py:22-31`) — `assets_dir/name`, then
/// `assets_dir/vendor/name`.  The first path that *exists* decides the result:
/// `os.path.exists` is true for directories and broken links too, and the
/// `except Exception: return ''` inside the loop returns rather than falling
/// through to the second base.
fn read_asset_file(assets_dir: &Path, name: &str) -> String {
    for base in [assets_dir.to_path_buf(), assets_dir.join("vendor")] {
        let p = base.join(name);
        if p.exists() {
            return fs::read_to_string(&p).unwrap_or_default();
        }
    }
    String::new()
}

/// `html_render._FONT_MAP` — the key is already whitelisted by `styles._font`
/// (`styles.py:259-261`), so `_FONT_MAP.get` never reaches its default.
const FONT_MAP: [(&str, &str); 6] = [
    ("MicrosoftYaHei", "\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif"),
    ("SimHei", "\"SimHei\", \"黑体\", sans-serif"),
    ("SimSun", "\"SimSun\", \"宋体\", serif"),
    ("KaiTi", "\"KaiTi\", \"楷体\", serif"),
    ("DengXian", "\"DengXian\", \"等线\", sans-serif"),
    ("Arial", "Arial, sans-serif"),
];

/// `_FONT_MAP.get(ty['font'], '"Microsoft YaHei", sans-serif')` (`html_render.py:37`).
fn font_stack(name: &str) -> String {
    FONT_MAP
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "\"Microsoft YaHei\", sans-serif".to_string())
}

/// `styles._ALIGNS` / `styles._FONTS` / `styles._MONO` (`styles.py:112-114`).
const ALIGNS: [&str; 4] = ["left", "center", "right", "justify"];
const TYPO_FONTS: [&str; 6] = ["MicrosoftYaHei", "SimHei", "SimSun", "KaiTi", "DengXian", "Arial"];
const MONO_FONTS: [&str; 3] = ["Consolas", "Courier New", "SimHei"];
const PAGE_SIZES: [&str; 7] = ["A3", "A4", "A5", "B5", "Letter", "Legal", "Custom"];
const ORIENTATIONS: [&str; 2] = ["portrait", "landscape"];

/// CPython `'%g' % value` (default precision 6): the decimal exponent comes from
/// the 6-significant-digit rounding, the value is printed in fixed form with
/// `6 - 1 - exp` fraction digits when `-4 <= exp < 6` and in exponential form
/// otherwise, and trailing zeros are stripped from the fraction in both cases.
fn css_g(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let sci = format!("{:.5e}", value);
    let (mantissa, exp_str) = sci.split_once('e').expect("rust exponent form always has 'e'");
    let exp: i32 = exp_str.parse().expect("rust exponent digits");
    if exp < -4 || exp >= 6 {
        let digits = exp.unsigned_abs();
        return format!("{}e{}{:02}", trim_fraction(mantissa), if exp < 0 { '-' } else { '+' }, digits);
    }
    let places = (5 - exp).max(0) as usize;
    trim_fraction(&format!("{:.places$}", value))
}

/// CPython `'%.2f' % value`.
fn css_f2(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    format!("{:.2}", value)
}

/// Drop trailing zeros (and then a bare `.`) from a mantissa, as C's `%g` does.
fn trim_fraction(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    let trimmed = text.trim_end_matches('0');
    trimmed.strip_suffix('.').map(|s| s.to_string()).unwrap_or_else(|| trimmed.to_string())
}

/// `float(value)` for a JSON value: numbers pass through, `True`/`False` become
/// `1.0`/`0.0` (`bool` is an `int` subclass), strings go through CPython's float
/// grammar, and everything else raises `TypeError`.  `_clamp` maps all of those
/// to the field default, so `None` here means "use the default".
fn py_float_of(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n.as_f64(),
        Value::String(s) => py_float_str(s),
        _ => None,
    }
}

/// `float(str)`.  Measured on the reference interpreter: `float()` trims exactly
/// the Unicode `White_Space` set **minus** `U+001C`-`U+001F`
/// (`float('\x1c1')` raises while `float('\x851')`, `float('\xa01')`,
/// `float('\u16801')` and `float('\u20291')` succeed) — i.e. precisely Rust's
/// `char::is_whitespace`, so this scanner deliberately does *not* use
/// `py_isspace`.  `_` is allowed only between digits.  The mantissa may carry
/// exactly one leading sign — measured `float('+.25') == 0.25`,
/// `float('-1.') == -1.0`, `float('-.5') == -0.5`, while `float('+-1')` and
/// `float('+ 1')` raise, which the single `strip_prefix` reproduces because a
/// second sign then fails the digit scan.
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

/// `styles._clamp(v, lo, hi, default)` (`styles.py:132-139`): unparsable or
/// non-finite values take `default`, everything else clamps.
fn style_clamp(value: Option<&Value>, lo: f64, hi: f64, default: f64) -> f64 {
    match value.map(py_float_of) {
        Some(Some(f)) if f.is_finite() => hi.min(lo.max(f)),
        _ => default,
    }
}

/// `styles._hex(v, default)` (`styles.py:248-256`):
/// `re.match(r'^#[0-9a-fA-F]{6}$', v)`.  `re.match`'s `$` also matches before a
/// single trailing newline, so `'#ffffff\n'` is accepted and then reaches the
/// CSS verbatim — that is what the Python authority emits, so this port keeps it.
fn style_hex(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(s)) if is_hex_color(s) => s.clone(),
        _ => default.to_string(),
    }
}

fn is_hex_color(s: &str) -> bool {
    let body = s.strip_suffix('\n').unwrap_or(s);
    let Some(digits) = body.strip_prefix('#') else {
        return false;
    };
    digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `x if x in ALLOWED else fallback` — the membership tests at `styles.py:151`,
/// `:153`, `:162`, `:167`, `:178`, `:193`, `:199`, `:215`, `:225`.
fn style_choice(value: Option<&Value>, allowed: &[&str], fallback: &str) -> String {
    match value {
        Some(Value::String(s)) if allowed.contains(&s.as_str()) => s.clone(),
        _ => fallback.to_string(),
    }
}

/// `bool(x)` where a missing key means "the schema default".
fn style_bool(sec: Option<&serde_json::Map<String, Value>>, key: &str, default: bool) -> bool {
    match val_at(sec, key) {
        None => default,
        Some(v) => py_truthy(v),
    }
}

/// `a.get(key)` after `deep_merge`: `styles.deep_merge` *skips* `None`
/// overrides (`styles.py:123-124`), so a JSON `null` is indistinguishable from
/// an absent key.
fn val_at<'a>(sec: Option<&'a serde_json::Map<String, Value>>, key: &str) -> Option<&'a Value> {
    sec?.get(key).filter(|v| !v.is_null())
}

/// The `isinstance(default, dict) and not isinstance(s.get(key), dict)` reset at
/// `styles.py:145-147` plus the per-section `if not isinstance(..., dict)` at
/// `styles.py:172-173`: a section that is present but not an object behaves
/// exactly like the untouched schema default, i.e. like an absent section here.
fn map_at<'a>(options: &'a Value, key: &str) -> Option<&'a serde_json::Map<String, Value>> {
    options.get(key).filter(|v| !v.is_null()).and_then(|v| v.as_object())
}

/// The same reset one level down, for `style['headings']['h%d']`.
fn heading_at<'a>(options: &'a Value, name: &str) -> Option<&'a serde_json::Map<String, Value>> {
    map_at(options, "headings").and_then(|h| h.get(name)).and_then(|v| v.as_object())
}

/// One `DEFAULT_STYLE['headings']['h%d']` row (`styles.py:34-39`).
struct HeadingDefault {
    size: f64,
    color: &'static str,
    before: f64,
    after: f64,
}

/// `_build_css` reads `style['headings']['h%d']` out of the *sanitized* dict, so
/// the schema defaults have to be spelled out here — `styles.sanitize` only ever
/// applies itself on top of them.
const HEADING_DEFAULTS: [HeadingDefault; 6] = [
    HeadingDefault { size: 20.0, color: "#1a1a1a", before: 18.0, after: 10.0 },
    HeadingDefault { size: 16.0, color: "#1f2937", before: 14.0, after: 8.0 },
    HeadingDefault { size: 14.0, color: "#2d3748", before: 12.0, after: 6.0 },
    HeadingDefault { size: 12.0, color: "#374151", before: 10.0, after: 6.0 },
    HeadingDefault { size: 11.0, color: "#4a5568", before: 8.0, after: 4.0 },
    HeadingDefault { size: 10.5, color: "#4a5568", before: 8.0, after: 4.0 },
];

/// `styles.page_dimensions(style)` (`styles.py:235-239`).  `page['size']` is
/// already whitelisted against `PAGE_SIZES`, and only `Custom` uses the numeric
/// width/height.
fn page_dimensions(size: &str, orientation: &str, width: f64, height: f64) -> (f64, f64) {
    let (w, h) = if size == "Custom" {
        (width, height)
    } else {
        match size {
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

/// `html_render._build_css(style)` (`html_render.py:34-88`), where `style` is
/// `styles.sanitize(options)`.  `sanitize` is a pure default-fill + clamp and
/// `_build_css` reads only the keys reproduced below, so the raw API `options`
/// can be passed straight through; the 26 machine-generated goldens in
/// `test_build_export_css_matches_python_goldens` pin the equivalence.
fn build_export_css(options: &Value) -> String {
    // `th = _THEME.get(style.get('htmlTheme'), _THEME['light'])`; only `bg`/`fg`
    // are ever read from the theme (`codeBg`/`quoteBg` are dead entries — `pre`
    // and `blockquote` take theirs from `style['code']['bg']` / `style['quote']['bg']`).
    // `styles.sanitize` (`styles.py:215`) has already forced the value into
    // `light|dark|sepia`, hence the `unwrap_or("light")`.
    let (bg, fg) = match options
        .get("htmlTheme")
        .and_then(|v| v.as_str())
        .unwrap_or("light")
    {
        "dark" => ("#14161a", "#d6d9de"),
        "sepia" => ("#faf4e7", "#3b2f1d"),
        _ => ("#ffffff", "#262626"),
    };

    let typo = map_at(options, "typography");
    let font = font_stack(&style_choice(val_at(typo, "font"), &TYPO_FONTS, "MicrosoftYaHei"));
    let ty_size = style_clamp(val_at(typo, "size"), 8.0, 20.0, 11.0);
    let ty_line = style_clamp(val_at(typo, "lineHeight"), 1.0, 2.5, 1.6);
    let ty_align = style_choice(val_at(typo, "align"), &ALIGNS, "left");
    let ty_spacing = style_clamp(val_at(typo, "spacing"), 0.0, 30.0, 6.0);
    let ty_indent = style_clamp(val_at(typo, "firstLineIndent"), 0.0, 30.0, 0.0);

    let mut css: Vec<String> = Vec::new();
    css.push(format!(":root {{ --bg:{bg}; --fg:{fg}; }}"));
    css.push("* { box-sizing: border-box; }".to_string());
    css.push(format!(
        "body {{ margin:0; background:var(--bg); color:{fg}; font-family:{font}; font-size:{}px; line-height:{}; padding:24px 16px 64px; }}",
        css_g(ty_size),
        css_g(ty_line)
    ));
    css.push("#content { max-width:820px; margin:0 auto; word-wrap:break-word; }".to_string());

    for (idx, d) in HEADING_DEFAULTS.iter().enumerate() {
        let name = format!("h{}", idx + 1);
        let h = heading_at(options, &name);
        let size = style_clamp(val_at(h, "size"), 8.0, 40.0, d.size);
        let color = match val_at(h, "color") {
            None => d.color.to_string(),
            Some(v) => style_hex(Some(v), "#1a1a1a"),
        };
        let weight = if style_bool(h, "bold", true) { "bold" } else { "normal" };
        let align = style_choice(val_at(h, "align"), &ALIGNS, "left");
        // `_clamp`'s own default (10 / 6) differs from the schema default, so an
        // invalid value falls to 10 / 6 while an absent one falls to h1..h6's.
        let before = match val_at(h, "before") {
            None => d.before,
            Some(v) => style_clamp(Some(v), 0.0, 60.0, 10.0),
        };
        let after = match val_at(h, "after") {
            None => d.after,
            Some(v) => style_clamp(Some(v), 0.0, 40.0, 6.0),
        };
        css.push(format!(
            "{name} {{ font-size:{}pt; color:{color}; font-weight:{weight}; text-align:{align}; margin-top:{}pt; margin-bottom:{}pt; line-height:1.35; }}",
            css_g(size),
            css_g(before),
            css_g(after)
        ));
    }

    let tb = map_at(options, "table");
    let width_pct = style_clamp(val_at(tb, "widthPct"), 50.0, 100.0, 100.0);
    let cell_size = style_clamp(val_at(tb, "cellSize"), 7.0, 16.0, 10.0);
    css.push(format!(
        "table {{ border-collapse:collapse; width:{}%; margin:8px auto; font-size:{}pt; }}",
        css_g(width_pct),
        css_g(cell_size)
    ));
    let border_w = style_clamp(val_at(tb, "borderWidth"), 0.0, 3.0, 0.75);
    let border_c = style_hex(val_at(tb, "borderColor"), "#c8cdd4");
    let cell_pad_s = css_g(style_clamp(val_at(tb, "cellPadding"), 0.0, 20.0, 6.0));
    let tb_align = style_choice(val_at(tb, "align"), &ALIGNS, "left");
    css.push(format!(
        "th, td {{ border:{}px solid {border_c}; padding:{cell_pad_s}px {cell_pad_s}px; text-align:{tb_align}; }}",
        css_f2(border_w)
    ));
    let header_bg = style_hex(val_at(tb, "headerBg"), "#3b6ef5");
    let header_color = style_hex(val_at(tb, "headerColor"), "#ffffff");
    let header_weight = if style_bool(tb, "headerBold", true) { "bold" } else { "normal" };
    css.push(format!("th {{ background:{header_bg}; color:{header_color}; font-weight:{header_weight}; }}"));
    if style_bool(tb, "banded", true) {
        let band = style_hex(val_at(tb, "bandColor"), "#f3f5f9");
        css.push(format!("tbody tr:nth-child(even) {{ background:{band}; }}"));
    }

    let code = map_at(options, "code");
    let code_bg = style_hex(val_at(code, "bg"), "#f5f6f8");
    let code_color = style_hex(val_at(code, "color"), "#2f3b4a");
    let code_border_w = style_clamp(val_at(code, "borderWidth"), 0.0, 3.0, 0.5);
    let code_border_c = style_hex(val_at(code, "borderColor"), "#dfe3e8");
    let code_radius = if style_bool(code, "rounded", true) { "8px" } else { "0" };
    let code_font = style_choice(val_at(code, "font"), &MONO_FONTS, "Consolas");
    let code_size = style_clamp(val_at(code, "size"), 6.0, 16.0, 9.5);
    css.push(format!(
        "pre {{ background:{code_bg}; color:{code_color}; border:{}px solid {code_border_c}; border-radius:{code_radius}; padding:12px 14px; overflow:auto; font-family:{code_font}, Consolas, monospace; font-size:{}pt; line-height:1.5; }}",
        css_f2(code_border_w),
        css_g(code_size)
    ));
    css.push(format!("code {{ font-family:{code_font}, Consolas, monospace; }}"));
    css.push(format!(
        ":not(pre) > code {{ background:{code_bg}; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }}"
    ));

    let quote = map_at(options, "quote");
    let quote_bg = style_hex(val_at(quote, "bg"), "#f3f6ff");
    let quote_color = style_hex(val_at(quote, "color"), "#4a5568");
    let quote_bar = style_hex(val_at(quote, "barColor"), "#3b6ef5");
    css.push(format!(
        "blockquote {{ margin:8px 0; padding:8px 14px; background:{quote_bg}; color:{quote_color}; border-left:4px solid {quote_bar}; }}"
    ));
    let link_color = style_hex(val_at(map_at(options, "link"), "color"), "#2b6cb0");
    css.push(format!("a {{ color:{link_color}; }}"));
    let hr_color = style_hex(val_at(map_at(options, "hr"), "color"), "#d8dce2");
    css.push(format!("hr {{ border:none; border-top:1px solid {hr_color}; margin:16px 0; }}"));
    css.push("img { max-width:100%; height:auto; }".to_string());
    let img_w = style_clamp(val_at(map_at(options, "images"), "widthPct"), 10.0, 100.0, 92.0);
    css.push(format!(
        "p > img:only-child {{ display:block; max-width:{}%; margin:0 auto; object-fit:contain; }}",
        css_g(img_w)
    ));
    css.push("li.task-list-item { list-style:none; margin-left:-20px; }".to_string());
    css.push("blockquote p, blockquote li { margin:4px 0; }".to_string());

    let page = map_at(options, "page");
    let p_width = style_clamp(val_at(page, "width"), 80.0, 600.0, 210.0);
    let p_height = style_clamp(val_at(page, "height"), 80.0, 600.0, 297.0);
    let p_size = style_choice(val_at(page, "size"), &PAGE_SIZES, "A4");
    let p_orient = style_choice(val_at(page, "orientation"), &ORIENTATIONS, "portrait");
    let mut m_top = style_clamp(val_at(page, "marginTop"), 0.0, 60.0, 20.0);
    let mut m_right = style_clamp(val_at(page, "marginRight"), 0.0, 60.0, 18.0);
    let mut m_bottom = style_clamp(val_at(page, "marginBottom"), 0.0, 60.0, 20.0);
    let mut m_left = style_clamp(val_at(page, "marginLeft"), 0.0, 60.0, 18.0);

    let (width, height) = page_dimensions(&p_size, &p_orient, p_width, p_height);
    // `styles.sanitize` rescales the two margin pairs *last* (`styles.py:227-231`),
    // before `_build_css` ever sees the dict, so the `@page` rule below must use
    // the rescaled numbers.  `p[a] * (limit - 30) / total` keeps CPython's
    // left-to-right float order.
    let limit_w = width - 30.0;
    let total_lr = m_left + m_right;
    if total_lr > limit_w {
        m_left = m_left * limit_w / total_lr;
        m_right = m_right * limit_w / total_lr;
    }
    let limit_h = height - 30.0;
    let total_tb = m_top + m_bottom;
    if total_tb > limit_h {
        m_top = m_top * limit_h / total_tb;
        m_bottom = m_bottom * limit_h / total_tb;
    }
    css.push(format!(
        "@page {{ size:{}mm {}mm; margin:{}mm {}mm {}mm {}mm; }}",
        css_g(width),
        css_g(height),
        css_g(m_top),
        css_g(m_right),
        css_g(m_bottom),
        css_g(m_left)
    ));
    css.push(format!(
        "body {{ font-size:{}pt; }} p {{ text-align:{ty_align}; text-indent:{}mm; margin-bottom:{}pt; }}",
        css_g(ty_size),
        css_g(ty_indent),
        css_g(ty_spacing)
    ));
    css.push("h1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }".to_string());
    for idx in 1..=6usize {
        if style_bool(heading_at(options, &format!("h{idx}")), "pageBreakBefore", false) {
            css.push(format!("h{idx} {{ break-before:page; }}"));
        }
    }
    css.push("@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }".to_string());
    css.push(".readmd-pagebreak { break-after:page; page-break-after:always; }".to_string());
    css.join("\n")
}

/// `html_render._esc_attr` (`html_render.py:116-117`) — note it escapes `"` but
/// *not* `'`, unlike `html.escape(s, quote=True)`.
fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `json.dumps(text, ensure_ascii=False).replace('<', '\\u003c')`
/// (`html_render.py:100` and `:103`).  The payload lands inside
/// `<script type="application/json">`, so the escape has to be the six JSON
/// characters `\u003c`; Rust's brace form `\u{003c}` is a `JSON.parse`
/// SyntaxError.
fn json_dump_escaped(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string()).replace('<', "\\u003c")
}

/// `str(meta.get(k) or '')[:120]` — `styles.sanitize` normalises the PDF
/// metadata to a truncated string (`styles.py:217-219`), and `render` then reads
/// `style['meta'].get('title')` (`html_render.py:102`).
fn meta_text(options: &Value, section: &str, key: &str) -> String {
    let sec = map_at(options, section);
    let raw = match val_at(sec, key) {
        Some(v) if py_truthy(v) => py_str(v),
        _ => String::new(),
    };
    raw.chars().take(120).collect()
}

/// `html_render._TEMPLATE` — the runtime bytes of the Python authority's
/// template, transcribed by `scratch/rust_parity/md_s2_gen.py`.  The
/// placeholders are substituted in exactly the order `render()` chains them.
/// `_TEMPLATE` — 1954 chars of the Python authority's runtime bytes.
const EXPORT_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="ReadMD">
<meta name="keywords" content="__KEYWORDS__">
<title>__TITLE__</title>
<style>
__CSS__
</style>
</head>
<body>
<article id="content" class="readmd-export"></article>
<script type="application/json" id="md-source">__MD__</script>
<script>
window.MathJax = {
  tex: { inlineMath: [['$', '$'], ['\\(', '\\)']], displayMath: [['$$', '$$'], ['\\[', '\\]']] },
  svg: { fontCache: 'global' },
  options: { skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'] }
};
</script>
<script>
__MARKED__
</script>
<script>
__MATHJAX__
</script>
<script>
(function () {
  var md = JSON.parse(document.getElementById('md-source').textContent);
  var html;
  try { html = marked.parse(md, { gfm: true, breaks: true }); }
  catch (e) { html = '<p>渲染失败：' + e.message + '</p>'; }
  document.getElementById('content').innerHTML = html;
  var root = document.getElementById('content');
  var highlights = __HIGHLIGHTS__;
  root.querySelectorAll('pre > code').forEach(function (code) {
    var key = code.textContent.replace(/
+$/, '');
    if (Object.prototype.hasOwnProperty.call(highlights, key)) code.innerHTML = highlights[key];
  });
  var walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);
  var breaks = [], node;
  while ((node = walker.nextNode())) {
    if (/^page-?break$/.test(node.textContent.trim())) breaks.push(node);
  }
  root.querySelectorAll('p').forEach(function (p) {
    if (p.textContent.trim() === '\\newpage') breaks.push(p);
  });
  breaks.forEach(function (node) {
    var div = document.createElement('div');
    div.className = 'readmd-pagebreak'; node.replaceWith(div);
  });
  if (window.MathJax && MathJax.typesetPromise) {
    try { MathJax.typesetPromise().catch(function () {}); } catch (e) {}
  }
})();
</script>
</body>
</html>
"#;

/// `mdexport/html_render.render` (`html_render.py:91-113`) plus the `html`
/// branch of `mdexport/__init__.py:export` (lines 121-132).
pub fn export_html(
    content: &str,
    base_dir: &str,
    out_path: &str,
    options: &Value,
    source_name: &str,
    assets_dir: &Path,
) -> Result<ExportResult, String> {
    let mut warns: Vec<String> = Vec::new();
    // `__init__.py:131` runs before `html_render.render`, so image warnings
    // precede the asset warnings.
    let embedded_md = embed_images_for_html(content, base_dir, &mut warns);

    let marked_js = read_asset_file(assets_dir, "marked.min.js");
    let mathjax_js = read_asset_file(assets_dir, "mathjax/tex-svg.js");
    if marked_js.is_empty() {
        warns.push("marked.min.js 未找到，HTML 导出可能无法渲染".to_string());
    }
    if mathjax_js.is_empty() {
        warns.push("MathJax 未找到，公式可能无法渲染".to_string());
    }

    let css = build_export_css(options);
    // `plugin_runtime.highlighted_blocks` / `.keywords` are optional-plugin
    // bridges: `run_plugin('pygments', ..., default={})` and
    // `run_plugin('jieba', ..., default='')` return their default whenever the
    // plugin is not enabled, and neither `pygments` nor `jieba` is in
    // `plugin_manager.DEFAULT_ENABLED` (`plugin_manager.py:43`).  Same
    // disabled-plugin path as `texmd::latex_label`.
    let highlights = "{}".to_string();
    let keyword_text = "";
    let title = {
        let meta_title = meta_text(options, "meta", "title");
        if !meta_title.is_empty() {
            meta_title
        } else if !source_name.is_empty() {
            source_name.to_string()
        } else {
            "ReadMD 导出".to_string()
        }
    };
    let md_esc = json_dump_escaped(&embedded_md);

    let html = EXPORT_TEMPLATE
        .replace("__TITLE__", &esc_attr(&title))
        .replace("__CSS__", &css)
        .replace("__KEYWORDS__", &esc_attr(keyword_text))
        .replace("__HIGHLIGHTS__", &highlights)
        .replace("__MARKED__", &marked_js)
        .replace("__MATHJAX__", &mathjax_js)
        .replace("__MD__", &md_esc);

    if let Some(parent) = Path::new(out_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    // `render()` writes with `newline='\n'`, so no translation happens and the
    // string bytes are the file bytes.
    fs::write(out_path, html.as_bytes()).map_err(|e| format!("Failed to write HTML: {}", e))?;
    let size = fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);

    Ok(ExportResult {
        ok: true,
        path: Some(out_path.to_string()),
        size: Some(size),
        warns: Some(warns),
        error: None,
        canceled: Some(false),
    })
}

// ============================================================================
// LaTeX Export
// ============================================================================

/// LaTeX export built on the shared AST ([`crate::md_ast`]) and
/// [`crate::latex_writer`].
///
/// Local images are copied next to the `.tex` into `<stem>.assets/` so the
/// output directory compiles as-is; remote / missing images are reported in
/// `warns`.  The `.tex` itself is written atomically.
pub fn export_tex(
    content: &str,
    base_dir: &str,
    out_path: &str,
    options: &Value,
    source_name: &str,
) -> Result<ExportResult, String> {
    let doc = crate::md_ast::parse(content);
    let opt_str = |ptrs: &[&str]| -> Option<String> {
        ptrs.iter()
            .filter_map(|p| options.pointer(p).and_then(|v| v.as_str()))
            .map(|s| s.trim().to_string())
            .find(|s| !s.is_empty())
    };
    let title = opt_str(&["/meta/title", "/tex/title", "/latex/title"])
        .or_else(|| doc.meta("title"))
        .or_else(|| Some(source_name.trim().to_string()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "Document".to_string());
    let author = opt_str(&["/meta/author", "/tex/author", "/latex/author"])
        .or_else(|| doc.meta("author"))
        .unwrap_or_default();
    let tex_bool = |key: &str| -> Option<bool> {
        options
            .pointer(&format!("/tex/{key}"))
            .or_else(|| options.pointer(&format!("/latex/{key}")))
            .and_then(|v| v.as_bool())
    };
    let defaults = crate::latex_writer::LatexOptions::default();
    let lopts = crate::latex_writer::LatexOptions {
        title,
        author,
        date: doc.meta("date"),
        doc_class: opt_str(&["/tex/docClass", "/latex/docClass"]).unwrap_or(defaults.doc_class),
        font_size: opt_str(&["/tex/fontSize", "/latex/fontSize"]).unwrap_or(defaults.font_size),
        paper: opt_str(&["/tex/paperSize", "/latex/paperSize"]).unwrap_or(defaults.paper),
        margin: opt_str(&["/tex/margin", "/latex/margin"]).unwrap_or(defaults.margin),
        use_ctex: tex_bool("useCtex"),
        toc: tex_bool("toc").unwrap_or(false),
    };

    // `<stem>.assets/` beside the output, created only if an image is copied.
    let out = Path::new(out_path);
    let stem = out.file_stem().and_then(|s| s.to_str()).unwrap_or("document").to_string();
    let assets_name = format!("{stem}.assets");
    let assets_dir = out.parent().unwrap_or_else(|| Path::new(".")).join(&assets_name);
    let resolver = std::cell::RefCell::new(HtmlImageResolver::new(base_dir));
    let warns = std::cell::RefCell::new(Vec::<String>::new());
    let copied = std::cell::RefCell::new(Vec::<(String, String)>::new());
    let images = |src: &str| -> Option<String> {
        if let Some((_, rel)) = copied.borrow().iter().find(|(s, _)| s == src) {
            return Some(rel.clone());
        }
        let hit = resolver.borrow_mut().resolve(src, &mut warns.borrow_mut())?;
        let (ext, bytes) = match hit {
            Resolved::File(p) => {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
                match fs::read(&p) {
                    Ok(b) => (ext, b),
                    Err(e) => {
                        warns.borrow_mut().push(format!("图片读取失败，已跳过：{src}（{e}）"));
                        return None;
                    }
                }
            }
            Resolved::Inline { mime, data } => {
                (mime.rsplit('/').next().unwrap_or("png").replace("jpeg", "jpg"), data)
            }
        };
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "pdf" | "eps") {
            warns.borrow_mut().push(format!("LaTeX 不支持该图片格式（{ext}），已跳过：{}", py_head80(src)));
            return None;
        }
        let n = copied.borrow().len() + 1;
        let name = format!("img{n}.{ext}");
        if let Err(e) = fs::create_dir_all(&assets_dir).and_then(|_| fs::write(assets_dir.join(&name), &bytes)) {
            warns.borrow_mut().push(format!("图片复制失败，已跳过：{src}（{e}）"));
            return None;
        }
        let rel = format!("{assets_name}/{name}");
        copied.borrow_mut().push((src.to_string(), rel.clone()));
        Some(rel)
    };
    let tex = crate::latex_writer::render_document(&doc, &lopts, &images);

    crate::content::write_bytes_atomic(out, tex.as_bytes())
        .map_err(|e| format!("Failed to write TeX: {}", e))?;
    let size = fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);

    Ok(ExportResult {
        ok: true,
        path: Some(out_path.to_string()),
        size: Some(size),
        warns: Some(warns.into_inner()),
        error: None,
        canceled: Some(false),
    })
}

// ============================================================================
// EPUB 3 Export — faithful port of src/readmd_modules/mdexport/{parser,
// xhtml_render,epub_render}.py. The Python API path (`_api_export_epub` ->
// `epub_render.build_epub`) never passes `resolve`, so no images are ever
// packaged; `<img>` stays a broken relative reference, exactly like Python.
// ============================================================================

/// `html.escape(s, quote=...)` — `&` first, then `<`/`>`, then quotes.
pub(crate) fn html_escape(s: &str, quote: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quote => out.push_str("&quot;"),
            '\'' if quote => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Python truthiness for JSON values (`0`, `""`, `false`, `null`, `[]`, `{}` are falsy).
pub(crate) fn py_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `str(value)` / f-string interpolation of a JSON value, Python-style.
pub(crate) fn py_str(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => {
            let parts: Vec<String> = o
                .iter()
                .map(|(k, val)| format!("{}: {}", py_repr(&Value::String(k.clone())), py_repr(val)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn py_repr(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        other => py_str(other),
    }
}

/// Python f-string number rendering: `11` stays `11`, `11.0` becomes `11.0`.
fn py_number_text(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    match n.as_f64() {
        Some(f) => {
            let s = f.to_string();
            if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
                s
            } else {
                format!("{s}.0")
            }
        }
        None => n.to_string(),
    }
}

/// `x = opts.get(key) or {}` followed by `x.get(...)` in Python.
///
/// A truthy non-dict value makes Python raise `AttributeError`, which the API
/// layer turns into `500 export_failed`; falsy/absent collapses to `{}`.
fn sub_dict(opts: &Value, key: &str) -> Result<Value, String> {
    match opts.get(key) {
        None => Ok(Value::Object(serde_json::Map::new())),
        Some(v) if !py_truthy(v) => Ok(Value::Object(serde_json::Map::new())),
        Some(v @ Value::Object(_)) => Ok(v.clone()),
        Some(_) => Err(EPI_TYPE_ERROR.to_string()),
    }
}

/// `opts.get(name, {}).get(key)` as used inside a Python `or` chain.
///
/// A missing `name` behaves like `{}`; a present-but-null (or non-dict) value
/// raises `AttributeError`, which callers must only surface when they actually
/// reach this term of the chain.
fn nested_get(opts: &Value, name: &str, key: &str) -> Result<Value, String> {
    match opts.get(name) {
        None => Ok(Value::Null),
        Some(v @ Value::Object(_)) => Ok(v.get(key).cloned().unwrap_or(Value::Null)),
        Some(_) => Err(EPI_TYPE_ERROR.to_string()),
    }
}

/// `epub_render.build_epub_css`
pub(crate) fn build_epub_css(opts: &Value) -> Result<String, String> {
    let epub_opts = sub_dict(opts, "epub")?;
    let pick = |key: &str, fallback: Value| -> Value {
        match epub_opts.get(key) {
            Some(v) if py_truthy(v) => v.clone(),
            _ => fallback,
        }
    };

    // font_size = epub_opts.get('fontSize') or opts.get('typography', {}).get('size') or 11
    let font_size = match epub_opts.get("fontSize") {
        Some(v) if py_truthy(v) => v.clone(),
        _ => {
            let size = nested_get(opts, "typography", "size")?;
            if py_truthy(&size) {
                size
            } else {
                Value::from(11)
            }
        }
    };
    // line_height = epub_opts.get('lineHeight') or opts.get('typography', {}).get('lineHeight') or 1.8
    let line_height = match epub_opts.get("lineHeight") {
        Some(v) if py_truthy(v) => v.clone(),
        _ => {
            let lh = nested_get(opts, "typography", "lineHeight")?;
            if py_truthy(&lh) {
                lh
            } else {
                Value::from(1.8)
            }
        }
    };
    let margin_v = pick("marginV", Value::from(5));
    let margin_h = pick("marginH", Value::from(8));
    let font_family = py_str(&pick(
        "fontFamily",
        Value::String("-apple-system, BlinkMacSystemFont, \"PingFang SC\", \"Microsoft YaHei\", serif".to_string()),
    ));

    let num = |v: &Value| match v {
        Value::Number(n) => py_number_text(n),
        other => py_str(other),
    };

    Ok(format!(
        r#"@charset "utf-8";
body {{
    font-family: {font_family};
    font-size: {fs}pt;
    margin: {mv}% {mh}%;
    line-height: {lh};
    color: #1a1a1a;
}}
h1, h2, h3, h4, h5, h6 {{
    font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif;
    font-weight: 600;
    line-height: 1.4;
    color: #0f172a;
    page-break-after: avoid;
}}
h1 {{ font-size: 1.8em; margin-top: 1.5em; border-bottom: 1px solid #e2e8f0; padding-bottom: 0.3em; }}
h2 {{ font-size: 1.4em; margin-top: 1.3em; }}
p {{ margin: 0.8em 0; text-align: justify; }}
pre, code {{
    font-family: "Courier New", Courier, monospace;
    background-color: #f1f5f9;
    font-size: 0.9em;
}}
pre {{
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
    border: 1px solid #e2e8f0;
}}
blockquote {{
    margin: 1em 0;
    padding-left: 1em;
    border-left: 4px solid #3b82f6;
    color: #475569;
}}
table {{
    width: 100%;
    border-collapse: collapse;
    margin: 1.2em 0;
}}
th, td {{
    border: 1px solid #cbd5e1;
    padding: 8px 10px;
    text-align: left;
}}
th {{ background-color: #f8fafc; }}
"#,
        fs = num(&font_size),
        mv = num(&margin_v),
        mh = num(&margin_h),
        lh = num(&line_height),
    ))
}

/// Build the OCF container; returns its bytes and the image warnings.
/// `base_dir` resolves relative image paths (empty = working directory).
#[allow(clippy::too_many_arguments)]
pub fn epub_build_bytes(
    markdown: &Value,
    base_dir: &str,
    options: &Value,
    title: &str,
    author: &str,
    language: &str,
    book_uuid: &str,
    mod_time: &str,
) -> Result<(Vec<u8>, Vec<String>), String> {
    // `markdown_content.splitlines()` requires a str: `None` (JSON null) and
    // every other non-str raise AttributeError in Python -> `export_failed`.
    let markdown = match markdown {
        Value::String(s) => s.clone(),
        _ => return Err(EPI_TYPE_ERROR.to_string()),
    };
    let opts = match options {
        Value::Object(_) => options.clone(),
        Value::Null => Value::Object(serde_json::Map::new()),
        _ => return Err(EPI_TYPE_ERROR.to_string()),
    };
    let epub_opts = sub_dict(&opts, "epub")?;

    // book_title = epub_opts.get('title') or opts.get('meta', {}).get('title') or title or "ReadMD 电子书"
    let mut book_title_v = epub_opts.get("title").filter(|v| py_truthy(v)).cloned().unwrap_or(Value::Null);
    if !py_truthy(&book_title_v) {
        book_title_v = nested_get(&opts, "meta", "title")?;
    }
    if !py_truthy(&book_title_v) {
        book_title_v = Value::String(title.to_string());
    }
    if !py_truthy(&book_title_v) {
        book_title_v = Value::String("ReadMD 电子书".to_string());
    }
    let book_title = match book_title_v {
        Value::String(s) => s,
        _ => return Err(EPI_TYPE_ERROR.to_string()),
    };

    let mut book_author_v = epub_opts.get("author").filter(|v| py_truthy(v)).cloned().unwrap_or(Value::Null);
    if !py_truthy(&book_author_v) {
        book_author_v = nested_get(&opts, "meta", "author")?;
    }
    if !py_truthy(&book_author_v) {
        book_author_v = Value::String(author.to_string());
    }
    if !py_truthy(&book_author_v) {
        book_author_v = Value::String("ReadMD Author".to_string());
    }
    let book_author = match book_author_v {
        Value::String(s) => s,
        _ => return Err(EPI_TYPE_ERROR.to_string()),
    };

    let book_lang = epub_opts
        .get("language")
        .filter(|v| py_truthy(v))
        .map(py_str)
        .or_else(|| {
            let param = Value::String(language.to_string());
            if py_truthy(&param) { Some(py_str(&param)) } else { None }
        })
        .unwrap_or_else(|| "zh-CN".to_string());
    let book_publisher = match epub_opts.get("publisher").filter(|v| py_truthy(v)) {
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err(EPI_TYPE_ERROR.to_string()),
        None => "ReadMD".to_string(),
    };
    let book_isbn = match epub_opts.get("isbn").filter(|v| py_truthy(v)) {
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err(EPI_TYPE_ERROR.to_string()),
        None => String::new(),
    };
    let split_level = epub_opts
        .get("splitLevel")
        .filter(|v| py_truthy(v))
        .map(py_str)
        .unwrap_or_else(|| "h1".to_string());

    let custom_css = format!("{}\n{}", build_epub_css(&opts)?, crate::epub_writer::EXTRA_CSS);

    // Local images are packaged once each as `OEBPS/images/img_N.ext`.
    let mut warns: Vec<String> = Vec::new();
    let mut resolver = HtmlImageResolver::new(base_dir);
    let mut packaged: Vec<(String, String, Vec<u8>)> = Vec::new(); // (href, media type, bytes)
    let mut by_src: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    let doc = crate::md_ast::parse(&markdown);
    let chapters = {
        let mut map = |src: &str| -> Option<String> {
            if let Some(hit) = by_src.get(src) {
                return hit.clone();
            }
            let got = match resolver.resolve(src, &mut warns) {
                Some(Resolved::File(p)) => {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    match (epub_image_media_type(&ext), fs::read(&p)) {
                        (Some(mt), Ok(data)) => Some((ext, mt.to_string(), data)),
                        (None, _) => {
                            warns.push(format!("EPUB 不支持该图片格式，已跳过：{src}"));
                            None
                        }
                        (_, Err(e)) => {
                            warns.push(format!("图片读取失败，已跳过：{src}（{e}）"));
                            None
                        }
                    }
                }
                Some(Resolved::Inline { mime, data }) => {
                    let ext = mime.rsplit('/').next().unwrap_or("png").replace("jpeg", "jpg");
                    epub_image_media_type(&ext).map(|mt| (ext.clone(), mt.to_string(), data))
                }
                None => None,
            };
            let href = got.map(|(ext, mt, data)| {
                let href = format!("images/img_{}.{}", packaged.len() + 1, ext);
                packaged.push((href.clone(), mt, data));
                href
            });
            by_src.insert(src.to_string(), href.clone());
            href
        };
        crate::epub_writer::render_chapters(&doc, &split_level, &mut map)
    };
    let mut chapters = chapters;
    if chapters.is_empty() {
        chapters.push(crate::epub_writer::Chapter { title: book_title.clone(), body: String::new(), has_math: false });
    }
    // A document without any split heading is one chapter named after the book.
    if chapters.len() == 1 && chapters[0].title == "序言" {
        chapters[0].title = book_title.clone();
    }
    let math_chapters: Vec<bool> = chapters.iter().map(|c| c.has_math).collect();

    let mut chapter_files: Vec<(String, String, String, String)> = Vec::new();
    for (idx, chap) in chapters.iter().enumerate() {
        let chap_title = &chap.title;
        let chap_id = format!("chap_{}", idx + 1);
        let chap_filename = format!("chapter_{}.xhtml", idx + 1);
        let chap_body = chap.body.clone();
        let chap_xhtml = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
    <meta charset="utf-8"/>
    <title>{title}</title>
    <link rel="stylesheet" type="text/css" href="style.css"/>
</head>
<body>
    {body}
</body>
</html>"#,
            lang = book_lang,
            title = html_escape(chap_title, false),
            body = chap_body,
        );
        chapter_files.push((chap_id, chap_filename, chap_title.clone(), chap_xhtml));
    }

    let nav_items = chapter_files
        .iter()
        .map(|(_, fn_, t, _)| format!("        <li><a href=\"{f}\">{t}</a></li>", f = fn_, t = html_escape(t, false)))
        .collect::<Vec<_>>()
        .join("\n");
    let nav_xhtml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
    <meta charset="utf-8"/>
    <title>目录</title>
    <link rel="stylesheet" type="text/css" href="style.css"/>
</head>
<body>
    <nav epub:type="toc" id="toc">
        <h1>目录</h1>
        <ol>
{items}
        </ol>
    </nav>
</body>
</html>"#,
        lang = book_lang,
        items = nav_items,
    );

    let ncx_points = chapter_files
        .iter()
        .enumerate()
        .map(|(i, (_, fn_, t, _))| {
            format!(
                "    <navPoint id=\"navPoint-{n}\" playOrder=\"{n}\">\n        <navLabel><text>{t}</text></navLabel>\n        <content src=\"{f}\"/>\n    </navPoint>",
                n = i + 1,
                t = html_escape(t, false),
                f = fn_,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let toc_ncx = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
    <head>
        <meta name="dtb:uid" content="urn:uuid:{uid}"/>
        <meta name="dtb:depth" content="1"/>
        <meta name="dtb:totalPageCount" content="0"/>
        <meta name="dtb:maxPageNumber" content="0"/>
    </head>
    <docTitle><text>{title}</text></docTitle>
    <navMap>
{points}
    </navMap>
</ncx>"#,
        uid = book_uuid,
        title = html_escape(&book_title, false),
        points = ncx_points,
    );

    let mut manifest_items: Vec<String> = vec![
        "<item id=\"style\" href=\"style.css\" media-type=\"text/css\"/>".to_string(),
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>".to_string(),
        "<item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>".to_string(),
    ];
    for (i, (cid, fn_, _, _)) in chapter_files.iter().enumerate() {
        let props = if math_chapters.get(i).copied().unwrap_or(false) { " properties=\"mathml\"" } else { "" };
        manifest_items.push(format!(
            "<item id=\"{cid}\" href=\"{f}\" media-type=\"application/xhtml+xml\"{props}/>",
            cid = cid,
            f = fn_
        ));
    }
    for (i, (href, mt, _)) in packaged.iter().enumerate() {
        manifest_items.push(format!("<item id=\"img_{}\" href=\"{href}\" media-type=\"{mt}\"/>", i + 1));
    }
    let mut spine_items = vec!["<itemref idref=\"nav\"/>".to_string()];
    for (cid, _, _, _) in &chapter_files {
        spine_items.push(format!("<itemref idref=\"{cid}\"/>"));
    }
    let publisher_tag = if book_publisher.is_empty() {
        String::new()
    } else {
        format!("<dc:publisher>{}</dc:publisher>", html_escape(&book_publisher, false))
    };
    let isbn_tag = if book_isbn.is_empty() {
        String::new()
    } else {
        format!("<dc:identifier id=\"ISBN\">{}</dc:identifier>", html_escape(&book_isbn, false))
    };
    let content_opf = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="3.0">
    <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
        <dc:identifier id="BookId">urn:uuid:{uid}</dc:identifier>
        {isbn}
        <dc:title>{title}</dc:title>
        <dc:creator>{author}</dc:creator>
        {publisher}
        <dc:language>{lang}</dc:language>
        <meta property="dcterms:modified">{mod_time}</meta>
    </metadata>
    <manifest>
        {manifest}
    </manifest>
    <spine toc="ncx">
        {spine}
    </spine>
</package>"#,
        uid = book_uuid,
        isbn = isbn_tag,
        title = html_escape(&book_title, false),
        author = html_escape(&book_author, false),
        publisher = publisher_tag,
        lang = book_lang,
        mod_time = mod_time,
        manifest = manifest_items.join("\n"),
        spine = spine_items.join("\n"),
    );

    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
    </rootfiles>
</container>"#;

    let mut entries: Vec<ZipEntry> = Vec::new();
    entries.push(ZipEntry {
        path: "mimetype".to_string(),
        data: b"application/epub+zip".to_vec(),
        compress: false,
    });
    entries.push(ZipEntry {
        path: "META-INF/container.xml".to_string(),
        data: container_xml.as_bytes().to_vec(),
        compress: true,
    });
    entries.push(ZipEntry { path: "OEBPS/content.opf".to_string(), data: content_opf.into_bytes(), compress: true });
    entries.push(ZipEntry { path: "OEBPS/nav.xhtml".to_string(), data: nav_xhtml.into_bytes(), compress: true });
    entries.push(ZipEntry { path: "OEBPS/toc.ncx".to_string(), data: toc_ncx.into_bytes(), compress: true });
    entries.push(ZipEntry { path: "OEBPS/style.css".to_string(), data: custom_css.into_bytes(), compress: true });
    for (_, fn_, _, xhtml) in chapter_files {
        entries.push(ZipEntry { path: format!("OEBPS/{fn_}"), data: xhtml.into_bytes(), compress: true });
    }
    for (href, _, data) in packaged {
        // Already-compressed formats are stored.
        let compress = href.ends_with(".svg");
        entries.push(ZipEntry { path: format!("OEBPS/{href}"), data, compress });
    }
    Ok((write_zip(&entries)?, warns))
}

fn epub_image_media_type(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// Marker for "the Python interpreter would have raised TypeError/AttributeError".
pub(crate) const EPI_TYPE_ERROR: &str = "__python_type_error__";

/// `epub_render.export_epub` / `build_epub` — write the container to disk.
/// Signature kept for the Rust-only `/api/export` dispatcher.
pub fn export_epub(
    content: &str,
    base_dir: &str,
    out_path: &str,
    options: &Value,
    source_name: &str,
) -> Result<ExportResult, String> {
    let title = options
        .pointer("/epub/title")
        .or_else(|| options.pointer("/meta/title"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(source_name);
    let title = if title.is_empty() { "ReadMD 电子书" } else { title };
    let author = options
        .pointer("/epub/author")
        .or_else(|| options.pointer("/meta/author"))
        .and_then(|v| v.as_str())
        .unwrap_or("ReadMD Author");
    let lang = options
        .pointer("/epub/language")
        .and_then(|v| v.as_str())
        .unwrap_or("zh-CN");
    let (bytes, warns) = epub_build_bytes(
        &Value::String(content.to_string()),
        base_dir,
        options,
        title,
        author,
        lang,
        &uuid::Uuid::new_v4().to_string(),
        &now_iso_z(),
    )?;
    if let Some(parent) = Path::new(out_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(out_path, &bytes).map_err(|e| format!("Failed to write EPUB: {}", e))?;
    Ok(ExportResult {
        ok: true,
        path: Some(out_path.to_string()),
        size: Some(bytes.len() as u64),
        warns: Some(warns),
        error: None,
        canceled: Some(false),
    })
}

/// `datetime.datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ')`
pub fn now_iso_z() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        (y + 1, m as u32, d as u32)
    } else {
        (y, m as u32, d as u32)
    }
}

// ============================================================================
// DOCX Export (Native OpenXML Generator with Math OMML)
// ============================================================================

/// DOCX export built on the shared AST ([`crate::md_ast`]) and
/// [`crate::docx_writer`], applying the sanitized export style.
pub fn export_docx(
    content: &str,
    base_dir: &str,
    out_path: &str,
    options: &Value,
    source_name: &str,
) -> Result<ExportResult, String> {
    let doc = crate::md_ast::parse(content);
    let style = crate::export_styles::sanitize_options(Some(options));
    let resolver = std::cell::RefCell::new(HtmlImageResolver::new(base_dir));
    let warns = std::cell::RefCell::new(Vec::<String>::new());
    let images = |src: &str| -> Option<(String, Vec<u8>)> {
        let hit = resolver.borrow_mut().resolve(src, &mut warns.borrow_mut())?;
        match hit {
            Resolved::File(p) => {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
                match fs::read(&p) {
                    Ok(b) => Some((ext, b)),
                    Err(e) => {
                        warns.borrow_mut().push(format!("图片读取失败，已跳过：{src}（{e}）"));
                        None
                    }
                }
            }
            Resolved::Inline { mime, data } => Some((mime.rsplit('/').next().unwrap_or("png").to_string(), data)),
        }
    };
    let pkg = crate::docx_writer::build(&doc, &style, source_name, &images);
    let entries: Vec<ZipEntry> = pkg
        .parts
        .into_iter()
        .map(|(path, data)| {
            let compress = !path.starts_with("word/media/");
            ZipEntry { path, data, compress }
        })
        .collect();
    let zip_bytes = write_zip(&entries)?;
    crate::content::write_bytes_atomic(Path::new(out_path), &zip_bytes)
        .map_err(|e| format!("Failed to write DOCX: {}", e))?;
    let mut all_warns = warns.into_inner();
    all_warns.extend(pkg.warns);
    Ok(ExportResult {
        ok: true,
        path: Some(out_path.to_string()),
        size: Some(zip_bytes.len() as u64),
        warns: Some(all_warns),
        error: None,
        canceled: Some(false),
    })
}

// ============================================================================
// PDF Export (Native `pdf_render` Pipeline)
// ============================================================================
//
// The Python product's PDF branch is `mdexport/__init__.py:115-117`:
//
//     stage = 'render'
//     if fmt == 'pdf':
//         from . import pdf_render
//         pdf_render.render(blocks, output_tmp, style, tmpdir, resolve, warns)
//
// with `blocks = _parser.parse(content)` (`:90`) and `style =
// _styles.sanitize(options)` (`:88`).  That dispatch is *unconditional*:
// `mdexport/` carries no second PDF implementation and no browser fallback of
// any kind, so the faithful mirror is "always native" and `export_document`
// keeps calling this function for the `"pdf"` format arm.
//
// What used to sit here was a browser locator plus a `--print-to-pdf` subprocess
// aimed at an installed Chromium-based browser.  That was not the Python lane:
// it made an external program a hard requirement of "Export PDF", it printed the
// *HTML* export — whose body is generated client-side by inlined marked.js and
// whose math is typeset by inlined MathJax (`mdexport.rs`, `export_html`) —
// through a browser layout engine instead of the reportlab flowable pipeline,
// and it produced none of the cover page, table of contents, document outline,
// page decoration or `/Title` metadata that `pdf_render` emits.  All of it is
// gone: this lane composes the document in-process, spawns nothing, and adds no
// dependency that is not already in `Cargo.toml`.
//
// Capability gaps this switch leaves open are recorded in
// `scratch/rust_parity/native_pdf_s1/REPORT.md`; each one is made *loud* below
// (a `warns` entry with the authority's own wording) rather than silently
// degraded, and none of them re-introduces a browser.

/// `hashlib.sha256(blob).hexdigest()` (`mdexport/__init__.py:56`) — the name
/// CPython gives a materialised `data:` image inside the export `tmpdir`.  Only
/// the extension and the bytes are read back, but keeping the digest preserves
/// CPython's de-duplication: one blob, one file.
fn pdf_digest_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// CPython's `tempfile.mkdtemp` / `mkstemp` 8-character random tail, for the
/// `readmd-export-XXXXXXXX` scratch directory and the
/// `.<name>.readmd-XXXXXXXX.pdf` staging file.  `rand` is not a dependency of
/// this crate, so the tail is clock + a process counter driven through an LCG;
/// uniqueness per call is all the name has to provide.
fn pdf_random_tail() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        .wrapping_add(SEQ.fetch_add(1, Ordering::Relaxed).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut state = seed;
    let mut out = String::with_capacity(8);
    for _ in 0..8 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        out.push(ALPHABET[((state >> 33) as usize) % ALPHABET.len()] as char);
    }
    out
}

/// The warning half of `formula.prepare` (`formula.py:132-213`), which this lane
/// cannot share with the rasterising half because there is no rasteriser.
///
/// Python appends `'公式无法渲染，已按文本保留：%s' % latex[:60]` for *every* math
/// node it has to leave as text — the cache in `prepare` only dedupes the
/// rendering work, never the warning — and `_walk` reaches them through
/// paragraph/heading text, table cells, list items, nested quote blocks and
/// display blocks.  Walking the AST built above covers exactly those positions
/// (arrays are visited in document order), so the string, the per-node count and
/// the ordering all match the authority.
fn pdf_math_fallback_warns(blocks: &Value) -> Vec<String> {
    let mut out = Vec::new();
    pdf_math_warn_walk(blocks, &mut out);
    out
}

fn pdf_math_warn_walk(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(a) => {
            for nd in a {
                pdf_math_warn_walk(nd, out);
            }
        }
        Value::Object(map) => {
            // `b['type'] == 'math' and b.get('display')` (`formula.py:191`) for
            // the display block, `nd.get('t') == 'math'` (`:141`) for inline.
            let is_block_math = map.get("type").and_then(|t| t.as_str()) == Some("math")
                && map.get("display").and_then(|d| d.as_bool()) == Some(true);
            let is_inline_math = map.get("t").and_then(|t| t.as_str()) == Some("math");
            let latex = map
                .get("latex")
                .and_then(|l| l.as_str())
                .unwrap_or_default()
                .to_string();
            // Formulas the vector layout handles are drawn, not left as text.
            let laid_out = (is_block_math || is_inline_math)
                && crate::math_layout::layout(&latex, is_block_math, 10.0).is_some();
            if (is_block_math || is_inline_math) && !laid_out {
                // `latex[:60]` — CPython slices by code point, not by byte.
                let head: String = latex.chars().take(60).collect();
                out.push(format!("公式无法渲染，已按文本保留：{head}"));
            }
            for child in map.values() {
                pdf_math_warn_walk(child, out);
            }
        }
        _ => {}
    }
}

/// `mdexport/__init__.py:33-74` — `ImageResolver`, narrowed to what the
/// renderer's `resolve` callback answers: a path, or nothing.
///
/// `HtmlImageResolver` above cannot be reused as-is: `pdf_render` wants an
/// `Fn(&str) -> Option<String>`, so the cache and the warnings need interior
/// mutability, and CPython materialises a `data:` blob into `tmpdir` and hands
/// back that file rather than carrying bytes.  The lookup order, the cache
/// policy (a failed `data:` decode is *not* cached, so it warns again next
/// time), the percent-decoding and the two warning strings mirror the Python and
/// the HTML lane.
struct PdfImageResolver {
    base_dir: String,
    tmpdir: String,
    /// `self._cache`.
    cache: std::cell::RefCell<Vec<(String, Option<String>)>>,
    warns: std::cell::RefCell<Vec<String>>,
}

impl PdfImageResolver {
    fn new(base_dir: &str, tmpdir: &str) -> PdfImageResolver {
        PdfImageResolver {
            base_dir: base_dir.to_string(),
            tmpdir: tmpdir.to_string(),
            cache: std::cell::RefCell::new(Vec::new()),
            warns: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn cached(&self, src: &str) -> Option<Option<String>> {
        self.cache.borrow().iter().find(|(k, _)| k == src).map(|(_, v)| v.clone())
    }

    fn remember(&self, src: &str, value: Option<String>) {
        let mut cache = self.cache.borrow_mut();
        match cache.iter_mut().find(|(k, _)| k == src) {
            Some(slot) => slot.1 = value,
            None => cache.push((src.to_string(), value)),
        }
    }

    fn warn(&self, message: String) {
        self.warns.borrow_mut().push(message);
    }

    fn resolve_src(&self, src: &str) -> Option<String> {
        // `src = (src or '').strip()`
        let src = py_strip(src).to_string();
        if src.is_empty() {
            return None;
        }

        if src.starts_with("data:image/") {
            if let Some(hit) = self.cached(&src) {
                return hit;
            }
            if let Some(target) = decode_data_image(&src) {
                if let Resolved::Inline { mime, data } = target {
                    let subtype = mime.rsplit('/').next().unwrap_or("png").to_string();
                    let path = Path::new(&self.tmpdir)
                        .join(format!("{}.{}", pdf_digest_hex(&data), subtype));
                    if !path.is_file() {
                        let _ = fs::create_dir_all(&self.tmpdir);
                        let _ = fs::write(&path, &data);
                    }
                    let hit = path.to_string_lossy().to_string();
                    self.remember(&src, Some(hit.clone()));
                    return Some(hit);
                }
            }
            // A `data:` that will not decode is *not* cached, and falls through
            // to the remote/data guard below, exactly like CPython.
        }

        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            self.warn(format!("远程/内联图片不支持嵌入，已跳过：{}", py_head80(&src)));
            return None;
        }

        if let Some(hit) = self.cached(&src) {
            return hit;
        }

        let decoded = percent_encoding::percent_decode_str(&src).decode_utf8_lossy().to_string();
        let cand = if py_isabs(&decoded) {
            decoded
        } else {
            py_join(&self.base_dir, &decoded)
        };
        let cand = crate::convert::py_normpath(&cand);
        if Path::new(&cand).is_file() {
            self.remember(&src, Some(cand.clone()));
            return Some(cand);
        }
        self.warn(format!("图片不存在，已跳过：{src}"));
        self.remember(&src, None);
        None
    }

    /// `shutil.rmtree`-equivalent collection point: the renderer has finished
    /// with the closure by the time this runs.
    fn take_warns(&self) -> Vec<String> {
        self.warns.take()
    }
}

pub fn export_pdf(
    content: &str,
    base_dir: &str,
    out_path: &str,
    options: &Value,
    source_name: &str,
    assets_dir: &Path,
) -> Result<ExportResult, String> {
    // Neither is part of the PDF contract any more: nothing is rendered to HTML
    // first, so there is no document to inline marked.js/MathJax assets into and
    // no `<title>` to name after the source file.  The signature is kept stable
    // because `export_document` (the `"pdf"` arm below) and `server.rs`'s
    // `/api/export` both pass all six arguments.
    let _ = (source_name, assets_dir);

    // `blocks = _parser.parse(content)` / `style = _styles.sanitize(options)`
    // (`mdexport/__init__.py:88-90`).  `export_styles::sanitize_options` is the
    // port of `styles.sanitize` and, like the authority, hands back the fully
    // merged + clamped dictionary `pdf_render` reads its page size, margins,
    // fonts, colors and table geometry out of — so `page.size` /
    // `page.orientation` / `page.margin{Top,Right,Bottom,Left}` keep driving the
    // output exactly as the `@page` rule they drove through the browser did
    // (`html_render.py:81`, `pdf_render::page_dimensions`).
    // The shared AST carries fence languages, table alignment, task markers,
    // list starts and nesting, which the old line parser dropped.
    let blocks = crate::md_ast::to_render_json(&crate::md_ast::parse(content).blocks);
    let style = crate::export_styles::sanitize_options(Some(options));

    // `tmpdir = tempfile.mkdtemp(prefix='readmd-export-')`, dropped in `finally`.
    let tmpdir_path = std::env::temp_dir().join(format!("readmd-export-{}", pdf_random_tail()));
    fs::create_dir_all(&tmpdir_path).map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let tmpdir = tmpdir_path.to_string_lossy().to_string();

    // `resolve = ImageResolver(base_dir, warns, tmpdir).resolve`
    let resolver = PdfImageResolver::new(base_dir, &tmpdir);
    let resolve = |src: &str| -> Option<String> { resolver.resolve_src(src) };

    // `fd, output_tmp = tempfile.mkstemp(prefix='.<name>.readmd-',
    // suffix=EXTS['pdf'], dir=output_dir)` then `os.replace(output_tmp,
    // out_path)`: the caller's file is only replaced once a complete, non-empty
    // PDF exists, and a failed render leaves whatever was there untouched.
    let out = Path::new(out_path);
    let out_dir = match out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let out_name = out.file_name().and_then(|s| s.to_str()).unwrap_or("document.pdf");
    let stage_path = out_dir
        .join(format!(".{}.readmd-{}.pdf", out_name, pdf_random_tail()))
        .to_string_lossy()
        .to_string();

    // `stage = 'formula'` warnings, ahead of the render exactly as in CPython,
    // where `formula.prepare(blocks, style, warns)` runs before `pdf_render`.
    let mut warns = pdf_math_fallback_warns(&blocks);
    if let Err(e) =
        crate::pdf_render::render(&blocks, &stage_path, &style, &tmpdir, &resolve, &mut warns)
    {
        let _ = fs::remove_file(&stage_path);
        let _ = fs::remove_dir_all(&tmpdir_path);
        // `PdfError` derives `Debug` only — there is no `Display` impl — so the
        // CPython exception text has to come out of the `message` field.
        return Err(e.message);
    }

    // `if not os.path.isfile(output_tmp) or os.path.getsize(output_tmp) <= 0:
    //     raise RuntimeError('导出器未生成有效文件')`
    if fs::metadata(&stage_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = fs::remove_file(&stage_path);
        let _ = fs::remove_dir_all(&tmpdir_path);
        return Err("导出器未生成有效文件".to_string());
    }

    if let Err(e) = fs::rename(&stage_path, out_path) {
        let _ = fs::remove_file(&stage_path);
        let _ = fs::remove_dir_all(&tmpdir_path);
        return Err(format!("Failed to finalize PDF: {}", e));
    }
    let _ = fs::remove_dir_all(&tmpdir_path);

    // The renderer's own warnings (font substitution, unembeddable image) keep
    // their order; the resolver's are appended after them because `render`
    // borrows the warning list mutably and the `Fn` callback cannot reach into
    // the same `Vec`.  Content is identical, interleaving is not.
    warns.extend(resolver.take_warns());
    // The renderer and the resolver both report a missing image; one notice
    // per distinct message is enough (first occurrence keeps its place).
    let mut seen = std::collections::HashSet::new();
    warns.retain(|w| seen.insert(w.clone()));

    let size = fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    Ok(ExportResult {
        ok: true,
        path: Some(out_path.to_string()),
        size: Some(size),
        warns: Some(warns),
        error: None,
        canceled: Some(false),
    })
}

// ============================================================================
// Master Unified Export Dispatcher
// ============================================================================

pub fn export_document(
    format: &str,
    content: &str,
    base_dir: &str,
    out_path: &str,
    options: &Value,
    source_name: &str,
    assets_dir: &Path,
) -> Result<ExportResult, String> {
    let fmt = format.to_lowercase();
    match fmt.as_str() {
        "html" => export_html(content, base_dir, out_path, options, source_name, assets_dir),
        "tex" | "latex" => export_tex(content, base_dir, out_path, options, source_name),
        "epub" => export_epub(content, base_dir, out_path, options, source_name),
        "docx" => export_docx(content, base_dir, out_path, options, source_name),
        "pdf" => export_pdf(content, base_dir, out_path, options, source_name, assets_dir),
        _ => Err(format!("Unsupported export format: {}", fmt)),
    }
}

// ============================================================================
// presentation_render.py — Reveal.js slide compiler / whitelist sanitizer
//
// Faithful port of `src/readmd_modules/mdexport/presentation_render.py`
// (675 lines) including the CPython `html.parser` tokenizer behaviour the
// sanitizer relies on, and the `html.unescape` character-reference handling
// that `convert_charrefs=True` applies to text runs.
// ============================================================================

const PRES_THEMES: &[&str] = &[
    "black", "white", "league", "beige", "night", "serif", "simple", "solarized", "blood", "moon", "sky",
];
const PRES_TRANSITIONS: &[&str] = &["slide", "fade", "zoom", "convex", "concave", "none"];
const PRES_REVEAL_BASE: &str = "assets/vendor/reveal/dist";
const PRES_SCRIPTS: &[&str] = &[
    "reveal.js",
    "plugin/markdown/markdown.js",
    "plugin/highlight/highlight.js",
    "plugin/notes/notes.js",
    "plugin/math/math.js",
];
/// `_PRESENTATION_ALLOWED_TAGS`
const PRES_ALLOWED_TAGS: &[&str] = &[
    "a", "abbr", "article", "b", "blockquote", "br", "caption", "cite", "code", "dd", "del", "details", "div",
    "dl", "dt", "em", "figcaption", "figure", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "i", "img", "ins",
    "kbd", "li", "mark", "ol", "p", "pre", "q", "s", "section", "span", "strike", "strong", "sub", "summary",
    "sup", "table", "tbody", "td", "tfoot", "th", "thead", "time", "tr", "u", "ul",
];
/// `_PRESENTATION_DROP_CONTENT`
const PRES_DROP_CONTENT: &[&str] = &[
    "base", "embed", "form", "frame", "frameset", "iframe", "link", "meta", "noscript", "object", "script",
    "style", "template", "title",
];
/// `_PRESENTATION_VOID_TAGS`
const PRES_VOID_TAGS: &[&str] = &["br", "hr", "img"];
/// `_PRESENTATION_GLOBAL_ATTRS`
/// `dir` is part of the upstream set — dropping it silently stripped the text
/// direction off every RTL slide, which is a content loss, not a hardening.
const PRES_GLOBAL_ATTRS: &[&str] = &["class", "dir", "lang", "role", "style", "title"];
/// `HTMLParser.CDATA_CONTENT_ELEMENTS` (CPython 3.11)
const PRES_CDATA_ELEMENTS: &[&str] = &["script", "style", "xmp", "iframe", "noembed", "noframes"];
/// `HTMLParser.RCDATA_CONTENT_ELEMENTS` (CPython 3.11)
const PRES_RCDATA_ELEMENTS: &[&str] = &["textarea", "title"];

fn pres_tag_attrs(tag: &str) -> &'static [&'static str] {
    match tag {
        "a" => &["href", "target"],
        "details" => &["open"],
        "img" => &["src", "srcset", "alt", "width", "height", "loading"],
        "source" => &["src", "srcset"],
        "ol" => &["start", "type"],
        "td" => &["colspan", "rowspan"],
        "th" => &["colspan", "rowspan", "scope"],
        "time" => &["datetime"],
        "blockquote" => &["cite"],
        _ => &[],
    }
}

fn pres_in(list: &[&str], item: &str) -> bool {
    list.iter().any(|x| *x == item)
}

/// `_PRESENTATION_TAG_ATTRS['*'] + _PRESENTATION_TAG_ATTRS[tag]`
fn pres_attr_allowed(tag: &str, name: &str) -> bool {
    pres_in(PRES_GLOBAL_ATTRS, name) || name == "aria-label" || pres_in(pres_tag_attrs(tag), name)
}

fn pres_is_void(tag: &str) -> bool {
    pres_in(PRES_VOID_TAGS, tag)
}

// --------------------------------------------------------------- whitespace

/// Python `str.isspace()` (the set `re.\s` and `str.strip()` use for `str`).
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()` — delegated to the single authority in `native_system.rs`
/// (`// WIRING: native_system::py_strip`) so the CPython whitespace set cannot
/// drift between the two files.  `py_isspace` stays for `py_split`, which needs
/// the per-character predicate rather than the trimmed slice.
fn py_strip(s: &str) -> &str {
    crate::native_system::py_strip(s)
}

/// `[ \t]*` (the literal class the slide directives use)
fn is_space_or_tab(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// `str.split()` on whitespace runs, dropping empty fields.
fn py_split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if py_isspace(ch) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn chars_at(c: &[char], i: usize) -> Option<char> {
    c.get(i).copied()
}

fn starts_with_chars(c: &[char], i: usize, needle: &[char]) -> bool {
    if i + needle.len() > c.len() {
        return false;
    }
    needle.iter().enumerate().all(|(k, ch)| c[i + k] == *ch)
}

fn starts_with_ci(c: &[char], i: usize, needle: &str) -> bool {
    let n: Vec<char> = needle.chars().collect();
    if i + n.len() > c.len() {
        return false;
    }
    n.iter()
        .enumerate()
        .all(|(k, ch)| c[i + k].eq_ignore_ascii_case(ch))
}

/// `str.find(needle, start)` on a char slice; returns the char index.
fn find_chars(c: &[char], needle: &[char], start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(c.len()));
    }
    let mut i = start;
    while i + needle.len() <= c.len() {
        if starts_with_chars(c, i, needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_char(c: &[char], needle: char, start: usize) -> Option<usize> {
    let mut i = start;
    while i < c.len() {
        if c[i] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn collect(c: &[char], from: usize, to: usize) -> String {
    c[from.min(c.len())..to.min(c.len())].iter().collect()
}

// ------------------------------------------------------------- html.unescape

/// Case-insensitive byte search for `a (\s* b)+` chains, e.g. `url \s* \(`.
fn byte_kw_chain(hay: &[u8], parts: &[&str]) -> bool {
    let first = parts[0].as_bytes();
    if first.is_empty() {
        return false;
    }
    for i in 0..hay.len() {
        if !hay[i..].starts_with(first) {
            continue;
        }
        let mut pos = i + first.len();
        let mut ok = true;
        for p in &parts[1..] {
            while pos < hay.len() && matches!(hay[pos], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
                pos += 1;
            }
            let pb = p.as_bytes();
            if !hay[pos.min(hay.len())..].starts_with(pb) {
                ok = false;
                break;
            }
            pos += pb.len();
        }
        if ok {
            return true;
        }
    }
    false
}

/// `html.unescape` — `&(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^\\t\\n\\f <&#;]{1,32};?)`.
pub(crate) fn html_unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    // Character-reference names CPython resolves (html.entities).  The
    // `;`-terminated set is `name2codepoint`; the bare set is the ISO legacy
    // list `html5` also carries without a trailing semicolon.
    // Character reference names CPython html.unescape can resolve via html.entities.
    // Generated from name2codepoint (as ";"-terminated) plus the ISO legacy set that
    // html5 also lists without a trailing semicolon.
    static NAMED_CHARREFS: &[(&str, &str)] = &[
    ("AElig;", "Æ"),
    ("Aacute;", "Á"),
    ("Acirc;", "Â"),
    ("Agrave;", "À"),
    ("Alpha;", "Α"),
    ("Aring;", "Å"),
    ("Atilde;", "Ã"),
    ("Auml;", "Ä"),
    ("Beta;", "Β"),
    ("Ccedil;", "Ç"),
    ("Chi;", "Χ"),
    ("Dagger;", "‡"),
    ("Delta;", "Δ"),
    ("ETH;", "Ð"),
    ("Eacute;", "É"),
    ("Ecirc;", "Ê"),
    ("Egrave;", "È"),
    ("Epsilon;", "Ε"),
    ("Eta;", "Η"),
    ("Euml;", "Ë"),
    ("Gamma;", "Γ"),
    ("Iacute;", "Í"),
    ("Icirc;", "Î"),
    ("Igrave;", "Ì"),
    ("Iota;", "Ι"),
    ("Iuml;", "Ï"),
    ("Kappa;", "Κ"),
    ("Lambda;", "Λ"),
    ("Mu;", "Μ"),
    ("Ntilde;", "Ñ"),
    ("Nu;", "Ν"),
    ("OElig;", "Œ"),
    ("Oacute;", "Ó"),
    ("Ocirc;", "Ô"),
    ("Ograve;", "Ò"),
    ("Omega;", "Ω"),
    ("Omicron;", "Ο"),
    ("Oslash;", "Ø"),
    ("Otilde;", "Õ"),
    ("Ouml;", "Ö"),
    ("Phi;", "Φ"),
    ("Pi;", "Π"),
    ("Prime;", "″"),
    ("Psi;", "Ψ"),
    ("Rho;", "Ρ"),
    ("Scaron;", "Š"),
    ("Sigma;", "Σ"),
    ("THORN;", "Þ"),
    ("Tau;", "Τ"),
    ("Theta;", "Θ"),
    ("Uacute;", "Ú"),
    ("Ucirc;", "Û"),
    ("Ugrave;", "Ù"),
    ("Upsilon;", "Υ"),
    ("Uuml;", "Ü"),
    ("Xi;", "Ξ"),
    ("Yacute;", "Ý"),
    ("Yuml;", "Ÿ"),
    ("Zeta;", "Ζ"),
    ("aacute;", "á"),
    ("acirc;", "â"),
    ("acute;", "´"),
    ("aelig;", "æ"),
    ("agrave;", "à"),
    ("alefsym;", "ℵ"),
    ("alpha;", "α"),
    ("amp;", "&"),
    ("and;", "∧"),
    ("ang;", "∠"),
    ("aring;", "å"),
    ("asymp;", "≈"),
    ("atilde;", "ã"),
    ("auml;", "ä"),
    ("bdquo;", "„"),
    ("beta;", "β"),
    ("brvbar;", "¦"),
    ("bull;", "•"),
    ("cap;", "∩"),
    ("ccedil;", "ç"),
    ("cedil;", "¸"),
    ("cent;", "¢"),
    ("chi;", "χ"),
    ("circ;", "ˆ"),
    ("clubs;", "♣"),
    ("cong;", "≅"),
    ("copy;", "©"),
    ("crarr;", "↵"),
    ("cup;", "∪"),
    ("curren;", "¤"),
    ("dArr;", "⇓"),
    ("dagger;", "†"),
    ("darr;", "↓"),
    ("deg;", "°"),
    ("delta;", "δ"),
    ("diams;", "♦"),
    ("divide;", "÷"),
    ("eacute;", "é"),
    ("ecirc;", "ê"),
    ("egrave;", "è"),
    ("empty;", "∅"),
    ("emsp;", " "),
    ("ensp;", " "),
    ("epsilon;", "ε"),
    ("equiv;", "≡"),
    ("eta;", "η"),
    ("eth;", "ð"),
    ("euml;", "ë"),
    ("euro;", "€"),
    ("exist;", "∃"),
    ("fnof;", "ƒ"),
    ("forall;", "∀"),
    ("frac12;", "½"),
    ("frac14;", "¼"),
    ("frac34;", "¾"),
    ("frasl;", "⁄"),
    ("gamma;", "γ"),
    ("ge;", "≥"),
    ("gt;", ">"),
    ("hArr;", "⇔"),
    ("harr;", "↔"),
    ("hearts;", "♥"),
    ("hellip;", "…"),
    ("iacute;", "í"),
    ("icirc;", "î"),
    ("iexcl;", "¡"),
    ("igrave;", "ì"),
    ("image;", "ℑ"),
    ("infin;", "∞"),
    ("int;", "∫"),
    ("iota;", "ι"),
    ("iquest;", "¿"),
    ("isin;", "∈"),
    ("iuml;", "ï"),
    ("kappa;", "κ"),
    ("lArr;", "⇐"),
    ("lambda;", "λ"),
    ("lang;", "〈"),
    ("laquo;", "«"),
    ("larr;", "←"),
    ("lceil;", "⌈"),
    ("ldquo;", "“"),
    ("le;", "≤"),
    ("lfloor;", "⌊"),
    ("lowast;", "∗"),
    ("loz;", "◊"),
    ("lrm;", "‎"),
    ("lsaquo;", "‹"),
    ("lsquo;", "‘"),
    ("lt;", "<"),
    ("macr;", "¯"),
    ("mdash;", "—"),
    ("micro;", "µ"),
    ("middot;", "·"),
    ("minus;", "−"),
    ("mu;", "μ"),
    ("nabla;", "∇"),
    ("nbsp;", " "),
    ("ndash;", "–"),
    ("ne;", "≠"),
    ("ni;", "∋"),
    ("not;", "¬"),
    ("notin;", "∉"),
    ("nsub;", "⊄"),
    ("ntilde;", "ñ"),
    ("nu;", "ν"),
    ("oacute;", "ó"),
    ("ocirc;", "ô"),
    ("oelig;", "œ"),
    ("ograve;", "ò"),
    ("oline;", "‾"),
    ("omega;", "ω"),
    ("omicron;", "ο"),
    ("oplus;", "⊕"),
    ("or;", "∨"),
    ("ordf;", "ª"),
    ("ordm;", "º"),
    ("oslash;", "ø"),
    ("otilde;", "õ"),
    ("otimes;", "⊗"),
    ("ouml;", "ö"),
    ("para;", "¶"),
    ("part;", "∂"),
    ("permil;", "‰"),
    ("perp;", "⊥"),
    ("phi;", "φ"),
    ("pi;", "π"),
    ("piv;", "ϖ"),
    ("plusmn;", "±"),
    ("pound;", "£"),
    ("prime;", "′"),
    ("prod;", "∏"),
    ("prop;", "∝"),
    ("psi;", "ψ"),
    ("quot;", "\""),
    ("rArr;", "⇒"),
    ("radic;", "√"),
    ("rang;", "〉"),
    ("raquo;", "»"),
    ("rarr;", "→"),
    ("rceil;", "⌉"),
    ("rdquo;", "”"),
    ("real;", "ℜ"),
    ("reg;", "®"),
    ("rfloor;", "⌋"),
    ("rho;", "ρ"),
    ("rlm;", "‏"),
    ("rsaquo;", "›"),
    ("rsquo;", "’"),
    ("sbquo;", "‚"),
    ("scaron;", "š"),
    ("sdot;", "⋅"),
    ("sect;", "§"),
    ("shy;", "­"),
    ("sigma;", "σ"),
    ("sigmaf;", "ς"),
    ("sim;", "∼"),
    ("spades;", "♠"),
    ("sub;", "⊂"),
    ("sube;", "⊆"),
    ("sum;", "∑"),
    ("sup;", "⊃"),
    ("sup1;", "¹"),
    ("sup2;", "²"),
    ("sup3;", "³"),
    ("supe;", "⊇"),
    ("szlig;", "ß"),
    ("tau;", "τ"),
    ("there4;", "∴"),
    ("theta;", "θ"),
    ("thetasym;", "ϑ"),
    ("thinsp;", " "),
    ("thorn;", "þ"),
    ("tilde;", "˜"),
    ("times;", "×"),
    ("trade;", "™"),
    ("uArr;", "⇑"),
    ("uacute;", "ú"),
    ("uarr;", "↑"),
    ("ucirc;", "û"),
    ("ugrave;", "ù"),
    ("uml;", "¨"),
    ("upsih;", "ϒ"),
    ("upsilon;", "υ"),
    ("uuml;", "ü"),
    ("weierp;", "℘"),
    ("xi;", "ξ"),
    ("yacute;", "ý"),
    ("yen;", "¥"),
    ("yuml;", "ÿ"),
    ("zeta;", "ζ"),
    ("zwj;", "‍"),
    ("zwnj;", "‌"),
    ("AElig", "Æ"),
    ("AMP", "&"),
    ("Aacute", "Á"),
    ("Acirc", "Â"),
    ("Agrave", "À"),
    ("Aring", "Å"),
    ("Atilde", "Ã"),
    ("Auml", "Ä"),
    ("COPY", "©"),
    ("Ccedil", "Ç"),
    ("ETH", "Ð"),
    ("Eacute", "É"),
    ("Ecirc", "Ê"),
    ("Egrave", "È"),
    ("Euml", "Ë"),
    ("GT", ">"),
    ("Iacute", "Í"),
    ("Icirc", "Î"),
    ("Igrave", "Ì"),
    ("Iuml", "Ï"),
    ("LT", "<"),
    ("Ntilde", "Ñ"),
    ("Oacute", "Ó"),
    ("Ocirc", "Ô"),
    ("Ograve", "Ò"),
    ("Oslash", "Ø"),
    ("Otilde", "Õ"),
    ("Ouml", "Ö"),
    ("QUOT", "\""),
    ("REG", "®"),
    ("THORN", "Þ"),
    ("Uacute", "Ú"),
    ("Ucirc", "Û"),
    ("Ugrave", "Ù"),
    ("Uuml", "Ü"),
    ("Yacute", "Ý"),
    ("aacute", "á"),
    ("acirc", "â"),
    ("acute", "´"),
    ("aelig", "æ"),
    ("agrave", "à"),
    ("amp", "&"),
    ("aring", "å"),
    ("atilde", "ã"),
    ("auml", "ä"),
    ("brvbar", "¦"),
    ("ccedil", "ç"),
    ("cedil", "¸"),
    ("cent", "¢"),
    ("copy", "©"),
    ("curren", "¤"),
    ("deg", "°"),
    ("divide", "÷"),
    ("eacute", "é"),
    ("ecirc", "ê"),
    ("egrave", "è"),
    ("eth", "ð"),
    ("euml", "ë"),
    ("frac12", "½"),
    ("frac14", "¼"),
    ("frac34", "¾"),
    ("gt", ">"),
    ("iacute", "í"),
    ("icirc", "î"),
    ("iexcl", "¡"),
    ("igrave", "ì"),
    ("iquest", "¿"),
    ("iuml", "ï"),
    ("laquo", "«"),
    ("lt", "<"),
    ("macr", "¯"),
    ("micro", "µ"),
    ("middot", "·"),
    ("nbsp", " "),
    ("not", "¬"),
    ("ntilde", "ñ"),
    ("oacute", "ó"),
    ("ocirc", "ô"),
    ("ograve", "ò"),
    ("ordf", "ª"),
    ("ordm", "º"),
    ("oslash", "ø"),
    ("otilde", "õ"),
    ("ouml", "ö"),
    ("para", "¶"),
    ("plusmn", "±"),
    ("pound", "£"),
    ("quot", "\""),
    ("raquo", "»"),
    ("reg", "®"),
    ("sect", "§"),
    ("shy", "­"),
    ("sup1", "¹"),
    ("sup2", "²"),
    ("sup3", "³"),
    ("szlig", "ß"),
    ("thorn", "þ"),
    ("times", "×"),
    ("uacute", "ú"),
    ("ucirc", "û"),
    ("ugrave", "ù"),
    ("uml", "¨"),
    ("uuml", "ü"),
    ("yacute", "ý"),
    ("yen", "¥"),
    ("yuml", "ÿ"),
    ];
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    'outer: while i < n {
        if c[i] != '&' {
            out.push(c[i]);
            i += 1;
            continue;
        }
        // `&(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^ \t\n\f <&#;]{1,32};?)`
        let j = i + 1;
        let mut cand: Option<(usize, usize)> = None; // (body_start, exclusive end)
        if j < n && c[j] == '#' {
            let k = j + 1;
            if k < n && (c[k] == 'x' || c[k] == 'X') {
                let ds = k + 1;
                let mut de = ds;
                while de < n && c[de].is_ascii_hexdigit() {
                    de += 1;
                }
                if de > ds {
                    let mut end = de;
                    if end < n && c[end] == ';' {
                        end += 1;
                    }
                    cand = Some((j, end));
                }
            } else {
                let ds = k;
                let mut de = ds;
                while de < n && c[de].is_ascii_digit() {
                    de += 1;
                }
                if de > ds {
                    let mut end = de;
                    if end < n && c[end] == ';' {
                        end += 1;
                    }
                    cand = Some((j, end));
                }
            }
        } else {
            let ds = j;
            let mut de = ds;
            while de < n && de - ds < 32 && !matches!(c[de], '\t' | '\n' | '\u{c}' | ' ' | '<' | '&' | '#' | ';') {
                de += 1;
            }
            if de > ds {
                let mut end = de;
                if end < n && c[end] == ';' {
                    end += 1;
                }
                cand = Some((ds, end));
            }
        }
        let Some((body_start, body_end)) = cand else {
            out.push('&');
            i += 1;
            continue;
        };
        let text: String = c[body_start..body_end].iter().collect();
        if text.starts_with('#') {
            let (radix16, digits) = if text[1..].starts_with('x') || text[1..].starts_with('X') {
                (true, text[2..].trim_end_matches(';').to_string())
            } else {
                (false, text[1..].trim_end_matches(';').to_string())
            };
            let parsed = if radix16 {
                u32::from_str_radix(&digits, 16).ok()
            } else {
                digits.parse::<u32>().ok()
            };
            match parsed {
                Some(num) => {
                    // `_invalid_charrefs` + surrogate/overflow + `_invalid_codepoints`
                    let mapped = if num == 0 || num == 0xd {
                        Some('\u{fffd}')
                    } else if (0xd800..=0xdfff).contains(&num) || num > 0x10ffff {
                        Some('\u{fffd}')
                    } else if matches!(num, 0x1..=0x8 | 0xb | 0xe..=0x1f | 0x7f | 0xf9 | 0xfa | 0xfd | 0xfe | 0xff
                            | 0x100..=0x102 | 0x103 | 0x104..=0x106 | 0x107 | 0x108..=0x10a | 0x10b
                            | 0x10c | 0x110..=0x113 | 0x11b | 0x126 | 0x129 | 0x12e | 0x130..=0x133
                            | 0x138..=0x15b) {
                        None
                    } else {
                        char::from_u32(num)
                    };
                    match mapped {
                        Some(ch) => out.push(ch),
                        None => {}
                    }
                }
                None => {
                    // bad charref -> the match is left as-is
                    out.push('&');
                    out.push_str(&text);
                }
            }
            i = body_end;
            continue;
        }
        if let Some(v) = NAMED_CHARREFS.iter().find(|(k, _)| *k == text).map(|(_, v)| *v) {
            out.push_str(v);
            i = body_end;
            continue;
        }
        // longest matching name, as defined by the standard
        let tc: Vec<char> = text.chars().collect();
        for x in (2..tc.len()).rev() {
            let name: String = tc[..x].iter().collect();
            if let Some((_, v)) = NAMED_CHARREFS.iter().find(|(k, _)| *k == name) {
                out.push_str(v);
                out.extend(tc[x..].iter());
                i = body_end;
                continue 'outer;
            }
        }
        out.push('&');
        out.push_str(&text);
        i = body_end;
    }
    out
}

// --------------------------------------------------- html.parser tokenizer port

#[derive(Clone, Debug, PartialEq)]
enum HtmlTok {
    Start(String, Vec<(String, Option<String>)>),
    StartEnd(String, Vec<(String, Option<String>)>),
    End(String),
    Data(String),
}

/// `findend`/`check_for_whole_start_tag`: the first `>` outside a quoted
/// attribute value terminates the start tag.
fn pres_find_tag_end(c: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    let mut quote: Option<char> = None;
    while i < c.len() {
        match quote {
            Some(q) => {
                if c[i] == q {
                    quote = None;
                }
            }
            None => match c[i] {
                '"' | '\'' => quote = Some(c[i]),
                '>' => return Some(i),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

fn pres_is_name_stop(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{c}' | ' ' | '/' | '>')
}

fn pres_is_attr_stop(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{c}' | ' ')
}

/// `tagfind_tolerant` group (1) at `i`: `[a-zA-Z][^\t\n\r\f />]*`.
fn pres_tag_name(c: &[char], i: usize) -> Option<(String, usize)> {
    match chars_at(c, i) {
        Some(ch) if ch.is_ascii_alphabetic() => {}
        _ => return None,
    }
    let mut k = i;
    while k < c.len() && !pres_is_name_stop(c[k]) {
        k += 1;
    }
    Some((collect(c, i, k).to_lowercase(), k))
}

/// `attrfind_tolerant` at `k`; returns (name, value, next index).
fn pres_attr_at(c: &[char], k: usize, endpos: usize) -> Option<(String, Option<String>, usize)> {
    if k >= endpos {
        return None;
    }
    if matches!(c[k], '\t' | '\n' | '\r' | '\u{c}' | ' ' | '/' | '>') {
        return None;
    }
    let ns = k;
    let mut a = k + 1;
    while a < endpos && !matches!(c[a], '\t' | '\n' | '\r' | '\u{c}' | ' ' | '/' | '=' | '>') {
        a += 1;
    }
    let name = collect(c, ns, a).to_lowercase();
    let mut p = a;
    while p < endpos && pres_is_attr_stop(c[p]) {
        p += 1;
    }
    let mut value: Option<String> = None;
    let mut had_eq = false;
    if p < endpos && c[p] == '=' {
        had_eq = true;
        p += 1;
        while p < endpos && pres_is_attr_stop(c[p]) {
            p += 1;
        }
        if p < endpos && (c[p] == '"' || c[p] == '\'') {
            let q = c[p];
            let vs = p + 1;
            p += 1;
            while p < endpos && c[p] != q {
                p += 1;
            }
            value = Some(collect(c, vs, p));
            if p < endpos {
                p += 1;
            }
        } else {
            let vs = p;
            while p < endpos && !matches!(c[p], '>' | '\t' | '\n' | '\r' | '\u{c}' | ' ') {
                p += 1;
            }
            value = Some(collect(c, vs, p));
        }
    }
    while p < endpos && (pres_is_attr_stop(c[p]) || (c[p] == '/' && !(p + 1 < endpos && c[p + 1] == '>'))) {
        p += 1;
    }
    let value = if had_eq { value.map(|v| html_unescape(&v)) } else { None };
    Some((name, value, p))
}

enum StartOutcome {
    Tag {
        tag: String,
        attrs: Vec<(String, Option<String>)>,
        self_closing: bool,
        next: usize,
    },
    /// `end not in (">", "/>")` -> the raw text becomes data.
    Raw { next: usize },
    /// unterminated tag: CPython consumes the rest of the buffer silently.
    Drop,
}

fn pres_parse_start_tag(c: &[char], i: usize) -> StartOutcome {
    let Some(gt) = pres_find_tag_end(c, i + 1) else {
        return StartOutcome::Drop;
    };
    let endpos = gt + 1;
    let Some((tag, mut k)) = pres_tag_name(c, i + 1) else {
        return StartOutcome::Drop;
    };
    // trailing `(?:[\t\n\r\f ]|/(?!>))*` of tagfind_tolerant
    loop {
        match chars_at(c, k) {
            Some(ch) if pres_is_attr_stop(ch) => k += 1,
            Some('/') if chars_at(c, k + 1) != Some('>') => k += 1,
            _ => break,
        }
    }
    let mut attrs: Vec<(String, Option<String>)> = Vec::new();
    while k < endpos {
        match pres_attr_at(c, k, endpos) {
            Some((name, value, next)) => {
                attrs.push((name, value));
                k = next;
            }
            None => break,
        }
    }
    let end = py_strip(&collect(c, k, endpos)).to_string();
    if end != ">" && end != "/>" {
        return StartOutcome::Raw { next: endpos };
    }
    StartOutcome::Tag {
        self_closing: end == "/>",
        tag,
        attrs,
        next: endpos,
    }
}

/// `parse_endtag`; returns (tag, next index). `None` means "no event".
fn pres_parse_end_tag(c: &[char], i: usize) -> Option<(String, usize)> {
    let gt = find_char(c, '>', i + 2)?;
    if chars_at(c, i + 2) == Some('>') {
        return None; // `</>` is ignored
    }
    let Some((tag, _)) = pres_tag_name(c, i + 2) else {
        return None; // bogus comment -> dropped
    };
    Some((tag, gt + 1))
}

/// Port of `HTMLParser` with `convert_charrefs=True` fed once and closed.
fn html_tokenize(src: &str) -> Vec<HtmlTok> {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out: Vec<HtmlTok> = Vec::new();
    let mut i = 0usize;
    let mut cdata: Option<String> = None;
    let mut escapable = true;
    loop {
        // Locate the next interesting position.
        let j: usize = match &cdata {
            None => find_char(&c, '<', i).unwrap_or(n),
            Some(el) => {
                let mut k = i;
                loop {
                    match find_chars(&c, &['<', '/'], k) {
                        Some(p) => {
                            if starts_with_ci(&c, p + 2, el)
                                && matches!(chars_at(&c, p + 2 + el.chars().count()), None | Some('>') | Some('\t') | Some('\n') | Some('\r') | Some('\u{c}') | Some(' '))
                            {
                                break Some(p);
                            }
                            k = p + 1;
                        }
                        None => break None,
                    }
                }
                .unwrap_or(n)
            }
        };
        if i < j {
            let seg = collect(&c, i, j);
            let text = if escapable { html_unescape(&seg) } else { seg };
            out.push(HtmlTok::Data(text));
            i = j;
        }
        if i >= n {
            break;
        }
        if cdata.is_some() {
            // `</elem` — parse_endtag owns it.
            match pres_parse_end_tag(&c, i) {
                Some((tag, _next)) => out.push(HtmlTok::End(tag)),
                None => {}
            }
            cdata = None;
            escapable = true;
            i = find_char(&c, '>', i).map(|g| g + 1).unwrap_or(n);
            continue;
        }
        match chars_at(&c, i + 1) {
            Some(ch) if ch.is_ascii_alphabetic() => {
                match pres_parse_start_tag(&c, i) {
                    StartOutcome::Drop => break,
                    StartOutcome::Raw { next } => {
                        out.push(HtmlTok::Data(collect(&c, i, next)));
                        i = next;
                    }
                    StartOutcome::Tag { tag, attrs, self_closing, next } => {
                        if self_closing {
                            out.push(HtmlTok::StartEnd(tag, attrs));
                        } else {
                            out.push(HtmlTok::Start(tag.clone(), attrs));
                            if pres_in(PRES_CDATA_ELEMENTS, &tag) {
                                cdata = Some(tag);
                                escapable = false;
                            } else if pres_in(PRES_RCDATA_ELEMENTS, &tag) {
                                cdata = Some(tag);
                                escapable = true;
                            }
                        }
                        i = next;
                    }
                }
                continue;
            }
            _ => {}
        }
        if starts_with_chars(&c, i, &['<', '/']) {
            if i + 2 >= n {
                out.push(HtmlTok::Data("</".to_string()));
                break;
            }
            match pres_parse_end_tag(&c, i) {
                Some((tag, _next)) => out.push(HtmlTok::End(tag)),
                None => {}
            }
            // An unterminated `</x` swallows the remainder (CPython: k = n).
            i = match find_char(&c, '>', i + 2) {
                Some(gt) => gt + 1,
                None => break,
            };
            cdata = None;
            escapable = true;
            continue;
        }
        if starts_with_chars(&c, i, &['<', '!', '-', '-']) {
            // comment: always event-free, ends at the first `-->`
            i = match find_chars(&c, &['-', '-', '>'], i + 4) {
                Some(p) => p + 3,
                None => break,
            };
            continue;
        }
        if starts_with_chars(&c, i, &['<', '?']) {
            i = match find_char(&c, '>', i + 2) {
                Some(p) => p + 1,
                None => break,
            };
            continue;
        }
        if starts_with_chars(&c, i, &['<', '!']) {
            // declaration / CDATA / bogus comment: all produce no events.
            i = match find_char(&c, '>', i + 2) {
                Some(p) => p + 1,
                None => break,
            };
            continue;
        }
        // bogus `<`
        out.push(HtmlTok::Data("<".to_string()));
        i += 1;
    }
    out
}

// --------------------------------------------------------- the sanitizer

/// `_presentation_safe_url`
fn presentation_safe_url(value: &str) -> bool {
    let v = py_strip(value);
    if v.is_empty() || v.starts_with('#') {
        return true;
    }
    let lowered = v.to_lowercase();
    if lowered.starts_with("data:image/") || lowered.starts_with("blob:") {
        return true;
    }
    if v.contains("://") || v.starts_with("//") || v.starts_with('/') {
        return true;
    }
    // `not re.match(r'^[a-z][a-z0-9+.-]*:', lowered)`
    let lc: Vec<char> = lowered.chars().collect();
    let scheme_like = match lc.first() {
        Some(ch) if ch.is_ascii_lowercase() => {
            let mut k = 1;
            while k < lc.len() && (lc[k].is_ascii_lowercase() || lc[k].is_ascii_digit() || "+.-".contains(lc[k])) {
                k += 1;
            }
            k < lc.len() && lc[k] == ':'
        }
        _ => false,
    };
    !scheme_like
}

/// `_safe_inline_css`
fn safe_inline_css(value: &str) -> String {
    let mut declarations: Vec<String> = Vec::new();
    for declaration in value.split(';') {
        let Some((prop, css_value)) = declaration.split_once(':') else {
            continue;
        };
        let prop = py_strip(prop);
        let css_value = py_strip(css_value);
        if prop.is_empty() || css_value.is_empty() {
            continue;
        }
        // `^[A-Za-z][A-Za-z0-9-]*` (prefix match)
        let pc: Vec<char> = prop.chars().collect();
        if pc.is_empty() || !pc[0].is_ascii_alphabetic() {
            continue;
        }
        if !pc[1..]
            .iter()
            .all(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        {
            continue;
        }
        let joined = format!("{prop}:{css_value}").to_lowercase();
        let hb = joined.as_bytes();
        let forbidden = byte_kw_chain(hb, &["javascript"])
            || byte_kw_chain(hb, &["vbscript"])
            || byte_kw_chain(hb, &["data", ":"])
            || byte_kw_chain(hb, &["url", "("])
            || byte_kw_chain(hb, &["expression", "("])
            || byte_kw_chain(hb, &["@import"])
            || byte_kw_chain(hb, &["behavior", ":"])
            || byte_kw_chain(hb, &["position", ":", "fixed"]);
        if forbidden {
            continue;
        }
        declarations.push(format!("{prop}: {css_value}"));
    }
    declarations.join("; ")
}

/// `_presentation_attr`.
///
/// NOTE the upstream quirk kept on purpose: a `style` attribute is only
/// *gated* by `_safe_inline_css` — the raw value is what gets rendered.
fn presentation_attr(tag: &str, name: &str, value: &str) -> Option<(String, String)> {
    if !pres_attr_allowed(tag, name) {
        return None;
    }
    if (name == "href" || name == "src") && !presentation_safe_url(value) {
        return None;
    }
    if name == "srcset" {
        let mut candidates: Vec<String> = Vec::new();
        for candidate in value.split(',') {
            let pieces = py_split(py_strip(candidate));
            if let Some(first) = pieces.first() {
                if !presentation_safe_url(first) {
                    continue;
                }
            }
            candidates.push(pieces.join(" "));
        }
        return Some(("srcset".to_string(), candidates.join(", ")));
    }
    if name == "id" {
        let ic: Vec<char> = value.chars().collect();
        let ok = match ic.first() {
            Some(ch) if ch.is_ascii_alphabetic() => ic[1..].iter().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-')
            }),
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    if name == "style" && safe_inline_css(value).is_empty() {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

/// `_PresentationHTMLSanitizer._safe_start`
fn safe_start(tag: &str, attrs: &[(String, Option<String>)]) -> String {
    let mut rendered = String::new();
    for (raw_name, raw_value) in attrs {
        let name = raw_name.clone();
        if name.starts_with("on") || !pres_attr_allowed(tag, &name) {
            continue;
        }
        let value = raw_value.clone().unwrap_or_default();
        let Some((checked_name, checked_value)) = presentation_attr(tag, &name, &value) else {
            continue;
        };
        rendered.push_str(&format!(
            " {name}=\"{value}\"",
            name = checked_name,
            value = html_escape(&checked_value, true)
        ));
    }
    let suffix = if pres_is_void(tag) { " />" } else { ">" };
    format!("<{tag}{rendered}{suffix}")
}

/// `_PresentationHTMLSanitizer` + `_sanitize_slide_html`
fn sanitize_slide_html(markdown: &str) -> String {
    let (protected, code_blocks) = protect_blocks(markdown);
    let mut inline_counter = 0usize;
    let mut inline_blocks: Vec<(String, String)> = Vec::new();
    let protected = protect_inline_code(&protected, &mut inline_counter, &mut inline_blocks);
    let _ = inline_counter;

    let mut out = String::new();
    let mut stack: Vec<String> = Vec::new();
    let mut skip_tag: Option<String> = None;
    let mut skip_depth = 0usize;
    for tok in html_tokenize(&protected) {
        match tok {
            HtmlTok::Data(d) => {
                if skip_depth == 0 && !d.is_empty() {
                    out.push_str(&html_escape(&d, false));
                }
            }
            HtmlTok::Start(tag, attrs) => {
                if skip_depth > 0 {
                    if !pres_is_void(&tag) {
                        skip_depth += 1;
                    }
                    continue;
                }
                if pres_in(PRES_DROP_CONTENT, &tag) {
                    skip_tag = Some(tag);
                    skip_depth = 1;
                    continue;
                }
                if pres_in(PRES_ALLOWED_TAGS, &tag) {
                    out.push_str(&safe_start(&tag, &attrs));
                    if !pres_is_void(&tag) {
                        stack.push(tag);
                    }
                }
            }
            HtmlTok::StartEnd(tag, attrs) => {
                if skip_depth > 0 {
                    continue;
                }
                if pres_in(PRES_ALLOWED_TAGS, &tag) {
                    out.push_str(&safe_start(&tag, &attrs));
                }
            }
            HtmlTok::End(tag) => {
                if skip_depth > 0 {
                    if skip_tag.as_deref() == Some(tag.as_str()) {
                        skip_depth -= 1;
                        if skip_depth == 0 {
                            skip_tag = None;
                        }
                    } else if !pres_is_void(&tag) {
                        skip_depth += 1;
                    }
                    continue;
                }
                if pres_in(PRES_ALLOWED_TAGS, &tag) && stack.iter().any(|t| t == &tag) {
                    while let Some(open) = stack.pop() {
                        if open == tag {
                            out.push_str(&format!("</{tag}>"));
                            break;
                        }
                        out.push_str(&format!("</{open}>"));
                    }
                }
            }
        }
    }
    while let Some(open) = stack.pop() {
        out.push_str(&format!("</{open}>"));
    }

    let clean = restore_blocks(&out, &inline_blocks);
    restore_blocks(&clean, &code_blocks)
}

// ---------------------------------------------------- block protection

/// `_protect_blocks` — ` ```[\s\S]*?``` ` then ` $$[\s\S]*?$$ `.
pub(crate) fn protect_blocks(text: &str) -> (String, Vec<(String, String)>) {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut counter = 0usize;
    let step1 = protect_delimited(text, "```", "__READMD_CODE_BLOCK_", &mut counter, &mut blocks);
    let step2 = protect_delimited(&step1, "$$", "__READMD_MATH_BLOCK_", &mut counter, &mut blocks);
    (step2, blocks)
}

fn protect_delimited(
    text: &str,
    delim: &str,
    key_prefix: &str,
    counter: &mut usize,
    blocks: &mut Vec<(String, String)>,
) -> String {
    let c: Vec<char> = text.chars().collect();
    let d: Vec<char> = delim.chars().collect();
    let n = c.len();
    let mut out = String::new();
    let mut i = 0usize;
    while i < n {
        if starts_with_chars(&c, i, &d) {
            if let Some(close) = find_chars(&c, &d, i + d.len()) {
                let matched = collect(&c, i, close + d.len());
                let key = format!("{key_prefix}{counter}__");
                *counter += 1;
                blocks.push((key.clone(), matched));
                out.push_str(&key);
                i = close + d.len();
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// `(?<!`)(`+)(?!`)([\s\S]*?)(?<!`)\1(?!`)` — fenced inline code spans.
fn protect_inline_code(
    text: &str,
    counter: &mut usize,
    blocks: &mut Vec<(String, String)>,
) -> String {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = String::new();
    let mut i = 0usize;
    while i < n {
        if c[i] == '`' && !(i > 0 && c[i - 1] == '`') {
            let mut run = 0usize;
            while i + run < n && c[i + run] == '`' {
                run += 1;
            }
            let mut q = i + run;
            let mut close: Option<usize> = None;
            while q < n {
                if c[q] == '`' && (q == 0 || c[q - 1] != '`') {
                    let mut l = 0usize;
                    while q + l < n && c[q + l] == '`' {
                        l += 1;
                    }
                    if l == run {
                        close = Some(q);
                        break;
                    }
                    q += l;
                    continue;
                }
                q += 1;
            }
            if let Some(cp) = close {
                let matched = collect(&c, i, cp + run);
                let key = format!("__READMD_INLINE_CODE_{}__", blocks.len());
                *counter += 1;
                blocks.push((key.clone(), matched));
                out.push_str(&key);
                i = cp + run;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

fn restore_blocks(text: &str, blocks: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (k, v) in blocks {
        out = out.replace(k.as_str(), v.as_str());
    }
    out
}

// ------------------------------------------------------- front-matter

/// `str.splitlines()`.  Measured on CPython 3.11.15, the break set is
/// `\n \r \r\n \v \f \x1c \x1d \x1e \x85 \u{2028} \u{2029}` — `\x1f` is *not* a
/// break — and a trailing break yields no empty last element.
fn py_splitlines(text: &str) -> Vec<&str> {
    fn is_line_break(c: char) -> bool {
        matches!(
            c,
            '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}'
                | '\u{2028}' | '\u{2029}'
        )
    }
    let mut out: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if !is_line_break(c) {
            continue;
        }
        let mut end = i + c.len_utf8();
        if c == '\r' {
            // `\r\n` counts as a single break.
            if chars.peek().map(|(_, n)| *n) == Some('\n') {
                end += 1;
                chars.next();
            }
        }
        out.push(&text[start..i]);
        start = end;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// `parse_frontmatter`
fn parse_frontmatter(content: &str) -> (Vec<(String, String)>, String) {
    let mut meta: Vec<(String, String)> = Vec::new();
    let mut body = content.to_string();
    if content.starts_with("---") {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            let raw_yaml = parts[1];
            body = parts[2].trim_start_matches('\n').to_string();
            for line in py_splitlines(raw_yaml) {
                if !line.contains(':') {
                    continue;
                }
                let (k, v) = line.split_once(':').unwrap();
                let k = py_strip(k).to_lowercase();
                let v = py_strip(v).trim_matches(|c| c == '"' || c == '\'').to_string();
                if ["theme", "transition", "title", "author", "slidenumber", "width", "height"]
                    .contains(&k.as_str())
                {
                    if let Some(slot) = meta.iter_mut().find(|(ek, _)| *ek == k) {
                        slot.1 = v;
                    } else {
                        meta.push((k, v));
                    }
                }
            }
        }
    }
    (meta, body)
}

fn meta_get<'a>(meta: &'a [(String, String)], key: &str) -> Option<&'a str> {
    meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

// ---------------------------------------------------- split helpers

/// `(?=^#{1,max}\s)` MULTILINE split points.
fn heading_split_points(c: &[char], max_hashes: usize, exact: bool) -> Vec<usize> {
    let mut pts = Vec::new();
    let mut i = 0usize;
    while i < c.len() {
        let line_start = i == 0 || c[i - 1] == '\n';
        if !line_start {
            i += 1;
            continue;
        }
        let mut k = 0usize;
        while i + k < c.len() && c[i + k] == '#' {
            k += 1;
        }
        let count_ok = if exact { k == max_hashes } else { k >= 1 && k <= max_hashes };
        if count_ok && matches!(chars_at(c, i + k), Some(ch) if py_isspace(ch)) {
            pts.push(i);
        }
        i += 1;
    }
    pts
}

fn split_at_points(s: &str, pts: &[usize]) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    for p in pts {
        out.push(collect(&c, start, *p));
        start = *p;
    }
    out.push(collect(&c, start, c.len()));
    out
}

/// `<!--\s*<name>(\s+...)?-->` style directives.
///
/// Returns the char index just past `-->` when the directive at line start `i`
/// matches.  `allow_attrs` mirrors `(?:\s+[\s\S]*?)?`, `must_end_line` the
/// trailing `[ \t]*$`.
fn directive_end(c: &[char], i: usize, names: &[&str], allow_attrs: bool, must_end_line: bool) -> Option<usize> {
    let mut k = i;
    while matches!(chars_at(c, k), Some(ch) if is_space_or_tab(ch)) {
        k += 1;
    }
    if !starts_with_chars(c, k, &['<', '!', '-', '-']) {
        return None;
    }
    k += 4;
    while matches!(chars_at(c, k), Some(ch) if py_isspace(ch)) {
        k += 1;
    }
    let mut name_len = None;
    for name in names {
        if starts_with_ci(c, k, name) {
            name_len = Some(name.chars().count());
            break;
        }
    }
    let name_len = name_len?;
    k += name_len;
    let dashes: Vec<char> = vec!['-', '-', '>'];
    if allow_attrs {
        if starts_with_chars(c, k, &dashes) {
            let end = k + 3;
            if !must_end_line || line_tail_blank(c, end) {
                return Some(end);
            }
        }
        if !matches!(chars_at(c, k), Some(ch) if py_isspace(ch)) {
            return None;
        }
        let mut p = k;
        loop {
            p = find_chars(c, &dashes, p)?;
            let end = p + 3;
            if !must_end_line || line_tail_blank(c, end) {
                return Some(end);
            }
            p += 1;
        }
    }
    while matches!(chars_at(c, k), Some(ch) if py_isspace(ch)) {
        k += 1;
    }
    if starts_with_chars(c, k, &dashes) {
        Some(k + 3)
    } else {
        None
    }
}

fn line_tail_blank(c: &[char], from: usize) -> bool {
    let mut k = from;
    while k < c.len() && is_space_or_tab(c[k]) {
        k += 1;
    }
    k >= c.len() || c[k] == '\n'
}

fn find_directive(
    c: &[char],
    from: usize,
    names: &[&str],
    allow_attrs: bool,
    must_end_line: bool,
) -> Option<(usize, usize)> {
    let mut i = from;
    while i < c.len() {
        let line_start = i == 0 || c[i - 1] == '\n';
        if line_start {
            if let Some(e) = directive_end(c, i, names, allow_attrs, must_end_line) {
                return Some((i, e));
            }
        }
        i += 1;
    }
    None
}

fn split_on_directive(s: &str, names: &[&str], allow_attrs: bool) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut segments = Vec::new();
    let mut start = 0usize;
    while let Some((ms, me)) = find_directive(&c, start, names, allow_attrs, true) {
        segments.push(collect(&c, start, ms));
        start = me;
    }
    segments.push(collect(&c, start, c.len()));
    segments
}

/// `^[ \t]*<marker>[ \t]*$` MULTILINE line split.
fn split_on_marker_line(s: &str, marker: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let m: Vec<char> = marker.chars().collect();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < c.len() {
        let line_start = i == 0 || c[i - 1] == '\n';
        if line_start {
            let mut k = i;
            while matches!(chars_at(&c, k), Some(ch) if is_space_or_tab(ch)) {
                k += 1;
            }
            if starts_with_chars(&c, k, &m) {
                let after = k + m.len();
                if matches!(chars_at(&c, after), None | Some('\n')) || {
                    let mut j = after;
                    while matches!(chars_at(&c, j), Some(ch) if is_space_or_tab(ch)) {
                        j += 1;
                    }
                    matches!(chars_at(&c, j), None | Some('\n'))
                } {
                    segments.push(collect(&c, start, i));
                    let mut e = if matches!(chars_at(&c, after), Some('\n')) { after + 1 } else { after };
                    let mut j = after;
                    while matches!(chars_at(&c, j), Some(ch) if is_space_or_tab(ch)) {
                        j += 1;
                    }
                    if matches!(chars_at(&c, j), Some('\n')) {
                        e = j + 1;
                    }
                    start = e;
                    i = e;
                    continue;
                }
            }
        }
        i += 1;
    }
    segments.push(collect(&c, start, c.len()));
    segments
}

/// `NOTE_SPLIT_REGEX` — collect notes and delete them from the slide.
fn extract_notes(s: &str) -> (String, Vec<String>) {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut notes: Vec<String> = Vec::new();
    let mut out = String::new();
    let mut i = 0usize;
    while i < n {
        let line_start = i == 0 || c[i - 1] == '\n';
        let hit = if line_start {
            directive_end(&c, i, &["note"], false, false).map(|after_marker| {
                // the prefix is `<!--\s*note\s*-->` plus `\s*`
                let mut k = after_marker;
                while k < n && py_isspace(c[k]) {
                    k += 1;
                }
                k
            })
        } else {
            None
        };
        let Some(body_start) = hit else {
            out.push(c[i]);
            i += 1;
            continue;
        };
        // lazy capture until the next slide/subslide/note directive at a line
        // start, or the end of the string.
        let mut q = body_start;
        loop {
            if q >= n {
                break;
            }
            let line_start = q == 0 || c[q - 1] == '\n';
            if line_start && directive_end(&c, q, &["slide", "subslide", "note"], false, false).is_some() {
                break;
            }
            q += 1;
        }
        notes.push(collect(&c, body_start, q).trim_matches(py_isspace).to_string());
        i = q;
    }
    (out, notes)
}

/// `_auto_split_long_chunk`
fn auto_split_long_chunk(chunk: &str, max_chars: usize) -> Vec<String> {
    let chunk = py_strip(chunk).to_string();
    if chunk.chars().count() <= max_chars {
        return if chunk.is_empty() { vec![] } else { vec![chunk] };
    }
    let (protected_chunk, block_map) = protect_blocks(&chunk);
    let c: Vec<char> = protected_chunk.chars().collect();
    let h3 = split_at_points(&protected_chunk, &heading_split_points(&c, 3, true));
    let h3: Vec<String> = h3
        .iter()
        .map(|s| py_strip(s).to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if h3.len() > 1 {
        let mut res = Vec::new();
        for s in h3 {
            let restored = restore_blocks(&s, &block_map);
            res.extend(auto_split_long_chunk(&restored, max_chars));
        }
        return res;
    }
    // `\n{2,}` paragraph split
    let paragraphs = split_on_blank_runs(&protected_chunk);
    if paragraphs.len() <= 1 {
        return vec![restore_blocks(&protected_chunk, &block_map)];
    }
    let mut slides: Vec<String> = Vec::new();
    let mut current_acc: Vec<String> = Vec::new();
    let mut current_len = 0usize;
    for p in paragraphs {
        let plen = p.chars().count();
        if current_len + plen > max_chars && !current_acc.is_empty() {
            slides.push(restore_blocks(&current_acc.join("\n\n"), &block_map));
            current_acc = vec![p.clone()];
            current_len = plen;
        } else {
            current_acc.push(p.clone());
            current_len += plen;
        }
    }
    if !current_acc.is_empty() {
        slides.push(restore_blocks(&current_acc.join("\n\n"), &block_map));
    }
    slides
}

fn split_on_blank_runs(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < c.len() {
        if c[i] == '\n' {
            let mut j = i;
            while j < c.len() && c[j] == '\n' {
                j += 1;
            }
            if j - i >= 2 {
                out.push(collect(&c, start, i));
                start = j;
                i = j;
                continue;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out.push(collect(&c, start, c.len()));
    out.into_iter().map(|p| py_strip(&p).to_string()).filter(|p| !p.is_empty()).collect()
}

/// `split_slides_structure` — [[(content, note)]]
pub(crate) fn split_slides_structure(content: &str) -> Vec<Vec<(String, String)>> {
    let (_meta, body) = parse_frontmatter(content);
    let bc: Vec<char> = body.chars().collect();
    let has_explicit_slide = find_directive(&bc, 0, &["slide"], true, false).is_some();

    let raw_horizontal: Vec<String> = if has_explicit_slide {
        split_on_directive(&body, &["slide"], true)
    } else if !split_on_marker_line(&body, "---").is_empty()
        && marker_line_present(&body, "---")
    {
        split_on_marker_line(&body, "---")
    } else {
        let pts = heading_split_points(&bc, 3, false);
        let splits: Vec<String> = split_at_points(&body, &pts)
            .into_iter()
            .map(|s| py_strip(&s).to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if splits.len() > 1 {
            splits
        } else {
            vec![body.clone()]
        }
    };

    let mut matrix: Vec<Vec<(String, String)>> = Vec::new();
    for h_chunk in raw_horizontal {
        let h_chunk = py_strip(&h_chunk).to_string();
        if h_chunk.is_empty() {
            continue;
        }
        let hc: Vec<char> = h_chunk.chars().collect();
        let raw_vertical: Vec<String> = if find_directive(&hc, 0, &["subslide"], true, false).is_some() {
            split_on_directive(&h_chunk, &["subslide"], true)
        } else if !has_explicit_slide && marker_line_present(&h_chunk, "--") {
            split_on_marker_line(&h_chunk, "--")
        } else {
            let split = auto_split_long_chunk(&h_chunk, 800);
            if split.is_empty() {
                vec![h_chunk.clone()]
            } else {
                split
            }
        };

        let mut vertical: Vec<(String, String)> = Vec::new();
        for v_chunk in raw_vertical {
            let v_chunk = py_strip(&v_chunk).to_string();
            if v_chunk.is_empty() {
                continue;
            }
            let (clean, notes) = extract_notes(&v_chunk);
            let clean = py_strip(&clean).to_string();
            let clean = sanitize_slide_html(&clean);
            let note_text = py_strip(&sanitize_slide_html(&notes.join("\n\n"))).to_string();
            vertical.push((clean, note_text));
        }
        if !vertical.is_empty() {
            matrix.push(vertical);
        }
    }
    matrix
}

fn marker_line_present(s: &str, marker: &str) -> bool {
    split_on_marker_line(s, marker).len() > 1
}

// ------------------------------------------------------------- template

fn escape_template_md(md: &str) -> String {
    md.replace("</textarea>", "&lt;/textarea&gt;")
}

/// `_read_vendor` — strict UTF-8, like Python's `open(..., encoding='utf-8')`.
fn read_vendor(assets_dir: &Path, rel: &str) -> Result<String, String> {
    let bytes = read_vendor_bytes(assets_dir, rel)?;
    String::from_utf8(bytes).map_err(|_| "vendor_not_utf8".to_string())
}

fn read_vendor_bytes(assets_dir: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let mut p = assets_dir.join("vendor");
    for part in rel.split('/') {
        p = p.join(part);
    }
    fs::read(&p).map_err(|_| "vendor_missing".to_string())
}

fn vendor_data_uri(assets_dir: &Path, rel: &str) -> Result<String, String> {
    let suffix = Path::new(rel)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let media = match suffix.as_str() {
        "ttf" => "font/ttf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => return Err("vendor_font_unknown".to_string()),
    };
    use base64::Engine;
    let payload = base64::engine::general_purpose::STANDARD.encode(read_vendor_bytes(assets_dir, rel)?);
    Ok(format!("data:{media};base64,{payload}"))
}

/// `_inline_relative_css_assets`
fn inline_relative_css_assets(css: &str, assets_dir: &Path, base_dir: &str) -> Result<String, String> {
    let c: Vec<char> = css.chars().collect();
    let n = c.len();
    let mut out = String::new();
    let mut i = 0usize;
    let open: Vec<char> = vec!['u', 'r', 'l', '('];
    while i < n {
        if !starts_with_chars(&c, i, &open) {
            out.push(c[i]);
            i += 1;
            continue;
        }
        let mut k = i + 4;
        while k < n && py_isspace(c[k]) {
            k += 1;
        }
        let quote = if matches!(chars_at(&c, k), Some('"') | Some('\'')) { Some(c[k]) } else { None };
        if quote.is_some() {
            k += 1;
        }
        let fs: Vec<char> = vec!['f', 'o', 'n', 't', 's', '/'];
        if !starts_with_chars(&c, k, &fs) {
            out.push(c[i]);
            i += 1;
            continue;
        }
        let rel_start = k;
        let mut e = k + fs.len();
        while e < n && !(matches!(chars_at(&c, e), Some(ch) if ch == ')' || py_isspace(ch)) || Some(c[e]) == quote) {
            e += 1;
        }
        let relative = collect(&c, rel_start, e);
        let mut tail = e;
        if let Some(q) = quote {
            // `\1` in the Python pattern: a captured quote must be repeated here,
            // otherwise there is no match at all (group 2's class excludes quotes,
            // so back-tracking cannot rescue it either).
            if chars_at(&c, e) != Some(q) {
                out.push(c[i]);
                i += 1;
                continue;
            }
            tail = e + 1;
        }
        while tail < n && py_isspace(chars_at(&c, tail).unwrap_or(' ')) {
            tail += 1;
        }
        if chars_at(&c, tail) != Some(')') {
            out.push(c[i]);
            i += 1;
            continue;
        }
        let asset = format!("{base_dir}/{relative}");
        out.push_str(&format!("url(\"{}\")", vendor_data_uri(assets_dir, &asset)?));
        i = tail + 1;
    }
    Ok(out)
}

/// The `<!DOCTYPE html>` head up to and including `<title>`.
const PRES_HEAD_START: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
  <title>"#;

/// Everything from `</title>` up to `<body data-transition="`.  The `{head}`
/// placeholder keeps the `{head_css}` insert explicit.
const PRES_STYLE_BLOCK: &str = r#"</title>
{head}
  <style>
    :root {
      --reveal-base-font-size: 24px;
    }
    .reveal {
      font-size: var(--reveal-base-font-size);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans", sans-serif;
    }
    .reveal .slides section {
      height: 100%;
      max-height: 100%;
      overflow-y: auto !important;
      display: flex !important;
      flex-direction: column !important;
      justify-content: center !important;
      padding: 24px 36px;
      box-sizing: border-box;
      text-align: left;
    }
    .reveal .slides section::-webkit-scrollbar {
      width: 6px;
    }
    .reveal .slides section::-webkit-scrollbar-thumb {
      background: rgba(128, 128, 128, 0.4);
      border-radius: 3px;
    }
    .reveal h1 {
      font-size: 1.8em;
      margin-bottom: 0.45em;
      font-weight: 700;
      line-height: 1.25;
      text-transform: none;
    }
    .reveal h2 {
      font-size: 1.4em;
      margin-bottom: 0.35em;
      font-weight: 600;
      line-height: 1.3;
      text-transform: none;
    }
    .reveal h3 {
      font-size: 1.15em;
      margin-bottom: 0.3em;
      font-weight: 600;
      text-transform: none;
    }
    .reveal h4, .reveal h5, .reveal h6 {
      font-size: 1.0em;
      margin-bottom: 0.25em;
      text-transform: none;
    }
    .reveal p, .reveal li {
      font-size: 0.95em;
      line-height: 1.65;
      margin-bottom: 0.6em;
      text-align: left;
    }
    .reveal ul, .reveal ol {
      text-align: left;
      display: block;
      margin: 0 0 1em 1.4em;
    }
    .reveal pre {
      box-shadow: 0 4px 16px rgba(0,0,0,0.25);
      width: 100%;
      border-radius: 8px;
      margin: 14px 0;
      background: #1e1e1e;
    }
    .reveal pre code {
      max-height: 480px;
      font-size: 0.78em;
      line-height: 1.45;
      padding: 12px 16px;
      border-radius: 8px;
      font-family: "Cascadia Code", "Fira Code", Consolas, "SF Mono", monospace;
    }
    .reveal table {
      font-size: 0.78em;
      margin: 16px auto;
      border-collapse: collapse;
      width: 100%;
      max-width: 900px;
    }
    .reveal table th {
      background: rgba(128, 128, 128, 0.2);
      font-weight: 600;
    }
    .reveal table th, .reveal table td {
      padding: 8px 14px;
      border: 1px solid rgba(128, 128, 128, 0.3);
      text-align: left;
    }
    .reveal blockquote {
      border-left: 4px solid #3b82f6;
      padding: 8px 18px;
      background: rgba(59, 130, 246, 0.08);
      font-style: normal;
      border-radius: 0 6px 6px 0;
      text-align: left;
      width: 95%;
      margin: 12px 0;
    }
    /* 禅模式：右下角翻页按钮渐隐，但页码指示器 (.slide-number) 始终常驻显示 */
    body.reveal-zen-controls-hidden .reveal .controls {
      opacity: 0 !important;
      pointer-events: none !important;
      transform: scale(0.92);
      transition: opacity 0.35s ease, transform 0.35s ease !important;
    }
    .reveal .controls {
      transition: opacity 0.35s ease, transform 0.35s ease !important;
    }
    .reveal .slide-number {
      opacity: 0.85 !important;
      pointer-events: auto !important;
      display: block !important;
      transition: opacity 0.25s ease !important;
    }
  </style>
</head>
<body data-transition=""#;

/// `_css_tag`
fn pres_css_tag(rel_path: &str, standalone: bool, link_id: &str, assets_dir: &Path) -> Result<String, String> {
    if standalone {
        let css = read_vendor(assets_dir, &format!("reveal/dist/{rel_path}"))?;
        return Ok(format!("<style>\n{css}\n</style>"));
    }
    let id_attr = if link_id.is_empty() {
        String::new()
    } else {
        format!(" id=\"{link_id}\"")
    };
    Ok(format!(
        "<link rel=\"stylesheet\" href=\"{base}/{rel}\"{id_attr}>",
        base = PRES_REVEAL_BASE,
        rel = rel_path,
        id_attr = id_attr
    ))
}

/// `_script_tag`
fn pres_script_tag(rel_path: &str, standalone: bool, assets_dir: &Path) -> Result<String, String> {
    if standalone {
        let js = read_vendor(assets_dir, &format!("reveal/dist/{rel_path}"))?;
        return Ok(format!("<script>\n{js}\n</script>"));
    }
    Ok(format!("<script src=\"{}/{}\"></script>", PRES_REVEAL_BASE, rel_path))
}

/// `_katex_tags`
fn pres_katex_tags(standalone: bool, assets_dir: &Path) -> Result<String, String> {
    if !standalone {
        return Ok(String::new());
    }
    let katex_css = inline_relative_css_assets(
        &read_vendor(assets_dir, "katex/dist/katex.min.css")?,
        assets_dir,
        "katex/dist",
    )?;
    let katex_js = read_vendor(assets_dir, "katex/dist/katex.min.js")?;
    let auto_render = read_vendor(assets_dir, "katex/dist/contrib/auto-render.min.js")?;
    Ok(format!(
        "<style>\n{katex_css}\n</style>\n<script>\n{katex_js}\n</script>\n<script>\n{auto_render}\n</script>"
    ))
}

/// `render_presentation_html` / `generate_presentation_html`.
///
/// `Err` carries the same signal as a Python exception escaping
/// `_api_export_presentation` (the handler maps it to 500
/// `presentation_export_failed`); `_read_vendor` is what usually fails.
pub fn render_presentation_html(
    content: &str,
    title: &str,
    theme: &str,
    transition: &str,
    standalone: bool,
    assets_dir: &Path,
) -> Result<String, String> {
    let (meta, _) = parse_frontmatter(content);
    let mut theme = meta_get(&meta, "theme").unwrap_or(theme).to_string();
    if theme.ends_with(".css") {
        let cut = theme.chars().count() - 4;
        theme = theme.chars().take(cut).collect();
    }
    let transition_meta = meta_get(&meta, "transition").unwrap_or(transition).to_string();
    let title_raw = meta_get(&meta, "title").unwrap_or(title).to_string();
    let title_esc = html_escape(&title_raw, true);
    let theme_tag = if pres_in(PRES_THEMES, &theme) { theme.clone() } else { "black".to_string() };

    let matrix = split_slides_structure(content);
    let mut sections: Vec<String> = Vec::new();
    for v_slides in &matrix {
        if v_slides.len() == 1 {
            let (slide_content, note) = &v_slides[0];
            let note_tag = if note.is_empty() {
                String::new()
            } else {
                format!("\n<aside class=\"notes\">{note}</aside>")
            };
            let escaped_md = escape_template_md(slide_content);
            sections.push(format!(
                "<section data-markdown><textarea data-template>\n{escaped_md}{note_tag}\n</textarea></section>"
            ));
        } else {
            let mut sub_sections: Vec<String> = Vec::new();
            for (slide_content, note) in v_slides {
                let note_tag = if note.is_empty() {
                    String::new()
                } else {
                    format!("\n<aside class=\"notes\">{note}</aside>")
                };
                let escaped_md = escape_template_md(slide_content);
                sub_sections.push(format!(
                    "<section data-markdown><textarea data-template>\n{escaped_md}{note_tag}\n</textarea></section>"
                ));
            }
            sections.push(format!("<section>\n{}\n</section>", sub_sections.join("\n")));
        }
    }
    let slides_body = sections.join("\n");

    let mut head_parts: Vec<String> = Vec::new();
    head_parts.push(pres_css_tag("reveal.css", standalone, "", assets_dir)?);
    head_parts.push(pres_css_tag(&format!("theme/{theme_tag}.css"), standalone, "theme", assets_dir)?);
    head_parts.push(pres_css_tag("plugin/highlight/monokai.css", standalone, "", assets_dir)?);
    head_parts.push(pres_katex_tags(standalone, assets_dir)?);
    let head_css = head_parts.join("\n");

    let mut body_scripts: Vec<String> = Vec::new();
    for p in PRES_SCRIPTS {
        body_scripts.push(pres_script_tag(p, standalone, assets_dir)?);
    }
    let body_scripts = body_scripts.join("\n");

    // Python reads the boot script even for the non-standalone preview.
    let boot_js = read_vendor(assets_dir, "reveal/dist/readmd-boot.js")?;
    let reveal_transition = if pres_in(PRES_TRANSITIONS, &transition_meta) {
        transition_meta.clone()
    } else {
        "slide".to_string()
    };
    let standalone_txt = if standalone { "true" } else { "false" };
    let config_json = format!(
        "{{\"transition\": \"{tr}\", \"themeBase\": \"{base}/theme/\", \"katexLocal\": \"assets/vendor/katex\", \"standalone\": {st}}}",
        tr = json_escape_python(&reveal_transition),
        base = PRES_REVEAL_BASE,
        st = standalone_txt,
    )
    .replace("</", "<\\/");
    let boot_tag = if standalone {
        format!("<script>\n{boot_js}\n</script>")
    } else {
        format!("<script src=\"{}/readmd-boot.js\"></script>", PRES_REVEAL_BASE)
    };

    Ok(format!(
        "{start}{title}{style_block}{transition}\">\n  <div class=\"reveal\">\n    <div class=\"slides\">\n{slides}\n    </div>\n  </div>\n{scripts}\n  <script type=\"application/json\" id=\"readmd-reveal-config\">{config}</script>\n{boot}\n</body>\n</html>",
        start = PRES_HEAD_START,
        title = title_esc,
        style_block = PRES_STYLE_BLOCK.replace("{head}", &head_css),
        transition = transition_meta,
        slides = slides_body,
        scripts = body_scripts,
        config = config_json,
        boot = boot_tag,
    ))
}

/// `json.dumps` string escaping (ensure_ascii=False).
fn json_escape_python(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presentation_sanitizer_matches_python_goldens() {
        // Every expectation is live `presentation_render._sanitize_slide_html()`
        // output from CPython 3.11 (probe:
        // scratch/rust_parity/small_gaps_s4_probe_css.py).  They cover the
        // whitelist (including `dir`), the `data:`/`blob:`/`#`/scheme URL rules,
        // the `expression(`/`@import`/`javascript:`/`url(`/`position: fixed`
        // CSS forbids, `convert_charrefs` entity decoding, and comment dropping.
        let cases: &[(&str, &str)] = &[
            // `dir` survives alongside the other global attributes.
            (
                "<div dir=\"rtl\" lang=\"he\">שלום</div>",
                "<div dir=\"rtl\" lang=\"he\">שלום</div>",
            ),
            (
                "<p class=\"a\" style=\"color:red\" title=\"t\" role=\"note\" aria-label=\"x\" dir=\"ltr\">k</p>",
                "<p class=\"a\" style=\"color:red\" title=\"t\" role=\"note\" aria-label=\"x\" dir=\"ltr\">k</p>",
            ),
            // HTML comments never reach the output, not even their payload.
            ("<!-- <script>alert(1)</script> -->", ""),
            ("<iframe src=\"http://a\"></iframe>", ""),
            ("<style>body{color:red}</style>", ""),
            // Numeric and hex character references are decoded and then escaped.
            (
                "<div>&#60;script&#62;alert(1)&#60;/script&#62;</div>",
                "<div>&lt;script&gt;alert(1)&lt;/script&gt;</div>",
            ),
            (
                "<div>&#x3c;img src=x onerror=alert(1)&#x3e;</div>",
                "<div>&lt;img src=x onerror=alert(1)&gt;</div>",
            ),
            ("<div>Safepipe &amp; amp <b>bold</b></div>", "<div>Safepipe &amp; amp <b>bold</b></div>"),
            // URL scheme gate, case-insensitive, and the `srcset` per-candidate gate.
            ("<a href=\"javascript:alert(1)\">x</a>", "<a>x</a>"),
            ("<a href=\"JaVaScRiPt:alert(1)\">x</a>", "<a>x</a>"),
            ("<a href=\"#ok\" target=\"_blank\">x</a>", "<a href=\"#ok\" target=\"_blank\">x</a>"),
            (
                "<img src=\"data:image/png;base64,AA\" srcset=\"  javascript:x 2x, ok.png 1x\">",
                "<img src=\"data:image/png;base64,AA\" srcset=\"ok.png 1x\" />",
            ),
            // The `on*` sweep.
            ("<span onclick=\"alert(1)\">y</span>", "<span>y</span>"),
            // `style` is only *gated* by `_safe_inline_css`; upstream then renders
            // the raw attribute, so `url(` survives in the output.  Kept because
            // parity is the requirement, not the stronger behaviour.
            (
                "<div style=\"color:red; background:url(x)\">y</div>",
                "<div style=\"color:red; background:url(x)\">y</div>",
            ),
            ("<span style=\"position : fixed\">f</span>", "<span>f</span>"),
            // Tag/attribute names are lowercased, void tags get ` />`, and the
            // open-element stack is flushed in LIFO order.
            ("<DIV CLASS=\"X\">upper</DIV>", "<div class=\"X\">upper</div>"),
            ("<br/>", "<br />"),
            ("<p>unclosed <em>text", "<p>unclosed <em>text</em></p>"),
            // U+2028 is ordinary data here — `_sanitize_slide_html` does not split.
            ("<div>a\u{2028}b</div>", "<div>a\u{2028}b</div>"),
        ];
        for (input, want) in cases {
            let got = sanitize_slide_html(input);
            assert_eq!(got.as_str(), *want, "sanitize_slide_html({input:?})");
        }
        // The CSS forbid-list itself, straight from `_safe_inline_css`.
        assert_eq!(safe_inline_css("color:red"), "color: red");
        assert_eq!(safe_inline_css("COLOR:RED"), "COLOR: RED");
        assert_eq!(safe_inline_css("background:URL (x)"), "");
        assert_eq!(safe_inline_css("width:expression (1)"), "");
        assert_eq!(safe_inline_css("x:@import y"), "");
        assert_eq!(safe_inline_css("behavior:url(#x)"), "");
        assert_eq!(safe_inline_css("background:data :x"), "");
        assert_eq!(safe_inline_css("-x-foo:bar"), "");
        assert_eq!(safe_inline_css("col or:red"), "");
        assert_eq!(safe_inline_css("color:red;;;"), "color: red");
        assert_eq!(safe_inline_css("font-family:'x'; src:url(y)"), "font-family: 'x'");
        // `_presentation_safe_url`.
        assert!(presentation_safe_url("  "));
        assert!(presentation_safe_url("blob:https://x/y"));
        assert!(presentation_safe_url("rel/x.png"));
        assert!(presentation_safe_url("custom+ scheme:x"));
        assert!(!presentation_safe_url("mailto:a@b"));
        assert!(!presentation_safe_url("data:text/html,<script>"));
        assert!(!presentation_safe_url("C:\\windows\\x.png"));
    }


    #[test]
    fn test_zip_creation() {
        let entries = vec![
            ZipEntry { path: "mimetype".to_string(), data: b"application/epub+zip".to_vec(), compress: false },
            ZipEntry { path: "content.txt".to_string(), data: b"Hello ReadMD Pure Rust ZIP!".to_vec(), compress: true },
        ];
        let zip = write_zip(&entries).expect("write_zip should succeed");
        assert!(zip.len() > 30);
        assert_eq!(&zip[0..4], b"PK\x03\x04");
    }

    #[test]
    fn test_docx_export() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out_docx = temp_dir.path().join("test.docx");
        let md = "# Title\n\nThis is **bold** and `code` with $E=mc^2$.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n";
        let res = export_docx(md, "", out_docx.to_str().unwrap(), &serde_json::json!({}), "test")
            .expect("export_docx should succeed");
        assert!(res.ok);
        assert!(out_docx.is_file());
        assert!(out_docx.metadata().unwrap().len() > 500);
    }

    #[test]
    fn test_epub_export() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out_epub = temp_dir.path().join("test.epub");
        let md = "# Chapter 1\n\nHello *EPUB* world from pure Rust!";
        let res = export_epub(md, "", out_epub.to_str().unwrap(), &serde_json::json!({}), "test")
            .expect("export_epub should succeed");
        assert!(res.ok);
        assert!(out_epub.is_file());
    }

    #[test]
    fn test_tex_export() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out_tex = temp_dir.path().join("test.tex");
        let md = "# Academic Paper\n\nHere is $$E = mc^2$$.\n\n```python\nprint('hello')\n```";
        let res = export_tex(md, "", out_tex.to_str().unwrap(), &serde_json::json!({}), "test")
            .expect("export_tex should succeed");
        assert!(res.ok);
        let content = fs::read_to_string(out_tex).unwrap();
        assert!(content.contains(r"\section{Academic Paper}"));
        assert!(content.contains(r"\begin{lstlisting}"));
    }

    // ------------------------------------------------------------ PDF (native)
    //
    // `export_pdf` used to answer `{"ok": false, "error": "No Chromium-based
    // browser (Edge/Chrome) found for PDF printing"}` on a machine without
    // Edge/Chrome, and `{"ok": true, ...}` with a Skia-printed PDF on a machine
    // with one, so *which* of the two you got depended on installed software.
    // It now composes the document in-process through `pdf_render`.  The
    // assertions below therefore key on discriminators that are visible in the
    // bytes: `pdf_render::write_pdf` stamps
    // `/Producer (readmd rust pdf_render) /Creator (readmd)`
    // (`pdf_render.rs:3803`), which a Chromium/Edge print never contains (those
    // say `Skia/PDF`).  Proving the producer proves the renderer that ran, and
    // so proves no browser process was involved.

    /// The needles the removed browser lane used to require.  Each is spliced
    /// with `concat!` on purpose: this table is itself a *code* line of the file
    /// the test below scans, so a plain literal would match its own declaration.
    const PDF_FORBIDDEN_NEEDLES: &[&str] = &[
        concat!("print-to-", "pdf"),
        concat!("--head", "less"),
        concat!("ms", "edge"),
        concat!("chrome.", "exe"),
        concat!("chrom", "ium"),
        concat!("google-", "chrome"),
        concat!("silent_", "command"),
        concat!("Command::", "new"),
        concat!(".", "spawn("),
        concat!("find_", "headless_browser"),
        concat!("Stdio::", "null"),
        concat!("PDF_PRINT_", "TIMEOUT"),
    ];

    fn first_media_box(bytes: &[u8]) -> (f64, f64) {
        let text = String::from_utf8_lossy(bytes);
        let at = text.find("/MediaBox [ 0 0 ").expect("a /MediaBox entry");
        let rest = &text[at + "/MediaBox [ 0 0 ".len()..];
        let end = rest.find(']').expect("MediaBox close bracket");
        let mut it = rest[..end].split_whitespace();
        (
            it.next().unwrap().parse().expect("MediaBox width"),
            it.next().unwrap().parse().expect("MediaBox height"),
        )
    }

    #[test]
    fn test_pdf_export_is_native_and_spawns_no_browser() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("native.pdf");
        let md = "# Title\n\nA **bold** paragraph with `code` and a [link](https://example.com).\n\n\
                  - item one\n- item two\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```\n";
        let res = export_pdf(md, "", out.to_str().unwrap(), &serde_json::json!({}), "native", dir.path())
            .expect("native PDF export must not depend on any installed browser");
        assert!(res.ok);
        assert_eq!(res.path.as_deref(), Some(out.to_str().unwrap()));
        assert_eq!(res.error, None);
        assert_eq!(res.canceled, Some(false));

        let bytes = fs::read(&out).unwrap();
        assert_eq!(&bytes[0..5], b"%PDF-", "output must be a PDF document");
        assert!(res.size.map(|s| s as usize == bytes.len()).unwrap_or(false), "`size` must be the finalized file size");
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(
            text.contains("/Producer (readmd rust pdf_render)"),
            "the PDF must be composed by the in-process renderer, not printed by a browser: {text}"
        );
        assert!(
            !text.contains("Skia"),
            "a Skia producer means an external browser print engine produced this file"
        );
        assert!(text.trim_end().ends_with("%%EOF"), "trailer must be complete");

        // Output/temp-file semantics: the `.readmd-XXXXXXXX.pdf` staging file is
        // renamed into place, so nothing is left beside the deliverable, and no
        // intermediate `.html` is written at all (the browser lane needed one).
        let leftovers: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok().and_then(|e| e.file_name().to_str().map(|s| s.to_string())))
            .filter(|n| n != "native.pdf")
            .collect();
        assert!(leftovers.is_empty(), "staging/temp artifacts leaked into the output dir: {leftovers:?}");
    }

    #[test]
    fn test_pdf_source_has_no_browser_spawn_sites() {
        // Compile-time self-inclusion: the purity gate is a *code* property, so
        // comment lines (which discuss the deleted lane on purpose) are skipped
        // and only real statements are held to it.
        let src = include_str!("mdexport.rs");
        for (idx, line) in src.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            for needle in PDF_FORBIDDEN_NEEDLES {
                assert!(
                    !line.contains(needle),
                    "browser-lane needle `{needle}` survived on code line {}: {line}",
                    idx + 1
                );
            }
        }
        // The positive half: the PDF endpoint really does hand off to `pdf_render`.
        assert!(
            src.contains("crate::pdf_render::render("),
            "export_pdf must route through the native renderer"
        );
    }

    #[test]
    fn test_pdf_export_document_route_selects_native() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("routed.pdf");
        let res = export_document("pdf", "# Hi\n\nbody\n", "", out.to_str().unwrap(), &serde_json::json!({}), "routed", dir.path())
            .expect("the `pdf` arm of export_document must work with no browser installed");
        assert!(res.ok);
        let text = String::from_utf8_lossy(&fs::read(&out).unwrap()).into_owned();
        assert!(text.contains("/Producer (readmd rust pdf_render)"));
    }

    #[test]
    fn test_pdf_export_overwrites_destination() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("over.pdf");
        fs::write(&out, b"pre-existing junk that is not a pdf").unwrap();
        let res = export_pdf("# Again\n\ntext\n", "", out.to_str().unwrap(), &serde_json::json!({}), "over", dir.path())
            .expect("Python's export() defaults to overwrite=True; so must this");
        assert!(res.ok);
        let bytes = fs::read(&out).unwrap();
        assert_eq!(&bytes[0..5], b"%PDF-");
        assert!(!bytes.starts_with(b"pre-existing"));
    }

    #[test]
    fn test_pdf_math_fallback_warns_like_python() {
        // There is no LaTeX rasteriser in this kernel, so math lands on
        // `formula.prepare`'s own `fallback: true` rung (`formula.py:156-158`).
        // The authority does *not* swallow that silently: it appends
        // `公式无法渲染，已按文本保留：<latex[:60]>` per node (`formula.py:161`).
        // A green `ok` with no warning here would be faked success, so this
        // guards the warning, not just the file.
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("math.pdf");
        let md = "Inline $a+b$ then display:\n\n$$c \\times d$$\n\nA second one $e^2$.\n";
        let res = export_pdf(md, "", out.to_str().unwrap(), &serde_json::json!({}), "math", dir.path()).unwrap();
        let warns = res.warns.unwrap_or_default();
        if crate::math_layout::available() {
            // A math font exists: every formula is drawn as vector paths with
            // its LaTeX as /ActualText, and nothing is reported as degraded.
            assert!(!warns.iter().any(|w| w.starts_with("公式无法渲染")), "{warns:?}");
            let bytes = fs::read(&out).unwrap();
            let text = crate::pdf_render::tests_inflate_all(&bytes);
            assert!(text.contains("/ActualText"), "formulas carry their source");
            return;
        }
        assert!(
            warns.contains(&"公式无法渲染，已按文本保留：a+b".to_string()),
            "inline math must warn: {warns:?}"
        );
        assert!(
            warns.contains(&"公式无法渲染，已按文本保留：c \\times d".to_string()),
            "display math must warn: {warns:?}"
        );
        assert_eq!(
            warns
                .iter()
                .filter(|w| w.starts_with("公式无法渲染，已按文本保留："))
                .count(),
            3,
            "one warning per math node, like CPython: {warns:?}"
        );
    }

    #[test]
    fn test_pdf_remote_image_warns_and_still_exports() {
        // The browser used to *fetch* remote images; the native renderer cannot,
        // and neither can the Python authority, whose `ImageResolver` emits
        // `远程/内联图片不支持嵌入，已跳过：<src[:80]>` and returns `None`
        // (`mdexport/__init__.py:60-63`).  Reusing that string is the explicit,
        // non-silent answer for this input class — no browser is implied.
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("remote.pdf");
        let md = "# Doc\n\n![badge](https://example.invalid/a.png)\n\n![gone](missing-local.png)\n";
        let res = export_pdf(md, "", out.to_str().unwrap(), &serde_json::json!({}), "remote", dir.path()).unwrap();
        let warns = res.warns.unwrap_or_default();
        assert!(
            warns.iter().any(|w| w.starts_with("远程/内联图片不支持嵌入，已跳过：https://example.invalid/a.png")),
            "remote images must be reported, not quietly dropped: {warns:?}"
        );
        assert!(
            warns.iter().any(|w| w.starts_with("图片不存在，已跳过：missing-local.png")),
            "missing local images must be reported: {warns:?}"
        );
    }

    #[test]
    fn test_pdf_page_size_and_orientation_follow_style() {
        // `page.size` / `page.orientation` / `page.margin*` drove the browser
        // through `html_render.py:81`'s `@page` rule; they drive `pdf_render`
        // through `page_dimensions`.  Same keys, so the observable page geometry
        // contract has to survive the switch.
        //
        // The size contrast has to be measured *unrotated*: `styles.py:239`'s
        // landscape branch is `(max(width, height), min(width, height))`, so A5
        // landscape is 210x148mm and therefore exactly as wide as A4 portrait.
        // CPython — `pdf_render.py:230-231`, `size = tuple(v * mm for v in
        // page_dimensions(style))` — measures 595.2756x841.8898 for A4 portrait,
        // 595.2756x419.5276 for A5 landscape and 419.5276x595.2756 for A5
        // portrait, so "A4 is wider than A5 landscape" is not a property of the
        // authority.  A5 *portrait* is the export that proves `page.size`
        // survives, A5 landscape's 210x148 swap the one that proves
        // `page.orientation` does.
        let dir = tempfile::tempdir().unwrap();
        let a4 = dir.path().join("a4.pdf");
        export_pdf(
            "# P\n\ntext\n",
            "",
            a4.to_str().unwrap(),
            &serde_json::json!({"page": {"size": "A4", "orientation": "portrait"}}),
            "a4",
            dir.path(),
        )
        .unwrap();
        let l5 = dir.path().join("a5land.pdf");
        export_pdf(
            "# P\n\ntext\n",
            "",
            l5.to_str().unwrap(),
            &serde_json::json!({"page": {"size": "A5", "orientation": "landscape"}}),
            "a5land",
            dir.path(),
        )
        .unwrap();
        let p5 = dir.path().join("a5port.pdf");
        export_pdf(
            "# P\n\ntext\n",
            "",
            p5.to_str().unwrap(),
            &serde_json::json!({"page": {"size": "A5", "orientation": "portrait"}}),
            "a5port",
            dir.path(),
        )
        .unwrap();

        let (w4, h4) = first_media_box(&fs::read(&a4).unwrap());
        let (w5, h5) = first_media_box(&fs::read(&l5).unwrap());
        let (w5p, h5p) = first_media_box(&fs::read(&p5).unwrap());
        assert!(h4 > w4, "A4 portrait must be taller than wide: {w4}x{h4}");
        assert!(w5 > h5, "A5 landscape must be wider than tall: {w5}x{h5}");
        assert!(h5p > w5p, "A5 portrait must be taller than wide: {w5p}x{h5p}");
        assert!(w4 > w5p, "A4 must be wider than A5 portrait: {w4} vs {w5p}");
        assert!(h4 > h5p, "A4 must be taller than A5 portrait: {h4} vs {h5p}");
        // A4 = 210x297mm, A5 = 148x210mm, at 72/25.4 pt per mm; the landscape
        // box is the same pair swapped, never a third size.
        assert!((w4 - 595.276).abs() < 1.0, "A4 width in pt: {w4}");
        assert!((h4 - 841.890).abs() < 1.0, "A4 height in pt: {h4}");
        assert!((w5 - 595.276).abs() < 1.0, "A5 landscape width = 210mm: {w5}");
        assert!((h5 - 419.528).abs() < 1.0, "A5 landscape height = 148mm: {h5}");
        assert!((w5p - 419.528).abs() < 1.0, "A5 portrait width = 148mm: {w5p}");
        assert!((h5p - 595.276).abs() < 1.0, "A5 portrait height = 210mm: {h5p}");
    }

    #[test]
    fn test_pdf_bridge_keeps_nested_markup_inside_emphasis() {
        // `MdInline::Emph` carries the *raw* inner markdown, and Python re-parses
        // it at draw time (`pdf_render.py:141-143`).  `pdf_render::normalize_inline`
        // cannot, so the bridge must hand over nodes rather than the literal
        // ``a `code` b`` string.
        let ast = crate::md_ast::to_render_json(&crate::md_ast::parse("**a `code` b**\n").blocks);
        let text = ast[0].get("text").unwrap();
        assert_eq!(text[0].get("t").unwrap().as_str().unwrap(), "bold");
        let inner = text[0].get("v").unwrap();
        assert!(inner.is_array(), "`v` must be a node list, not a raw string: {inner}");
        assert!(
            inner.as_array().unwrap().iter().any(|n| n.get("t").and_then(|t| t.as_str()) == Some("code")
                && n.get("v").and_then(|v| v.as_str()) == Some("code")),
            "the inline code inside bold must survive as a code node: {inner}"
        );
    }

    /// Decompress a raw-deflate (method 8) member payload.
    fn inflate(data: &[u8]) -> String {
        use std::io::Read;
        let mut buf = Vec::new();
        flate2::read::DeflateDecoder::new(data)
            .read_to_end(&mut buf)
            .expect("member payload must be a valid raw-deflate stream");
        String::from_utf8_lossy(&buf).into_owned()
    }

    /// Walk the local file headers of a ZIP image: `(name, method, flags,
    /// comp_size, uncomp_size, payload)`.
    fn local_entries(zip: &[u8]) -> Vec<(String, u16, u16, u32, u32, Vec<u8>)> {
        let mut off = 0usize;
        let mut out = Vec::new();
        while off + 30 <= zip.len() && zip[off..off + 4] == *b"PK\x03\x04" {
            let rd16 = |o: usize| u16::from_le_bytes([zip[o], zip[o + 1]]) as u16;
            let rd32 = |o: usize| {
                u32::from_le_bytes([zip[o], zip[o + 1], zip[o + 2], zip[o + 3]])
            };
            let flags = rd16(off + 6);
            let method = rd16(off + 8);
            let csize = rd32(off + 18);
            let usize_ = rd32(off + 22);
            let nl = rd16(off + 26) as usize;
            let el = rd16(off + 28) as usize;
            let name = String::from_utf8_lossy(&zip[off + 30..off + 30 + nl]).to_string();
            let payload = zip[off + 30 + nl + el..off + 30 + nl + el + csize as usize].to_vec();
            out.push((name, method, flags, csize, usize_, payload));
            off += 30 + nl + el + csize as usize;
        }
        out
    }

    #[test]
    fn test_epub_packages_images_and_declares_mathml() {
        let dir = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\n0000";
        fs::create_dir_all(dir.path().join("img")).unwrap();
        fs::write(dir.path().join("img").join("a.png"), png).unwrap();
        let out = dir.path().join("book.epub");
        let md = "# One\n\n![pic](img/a.png) and again ![pic](img/a.png)\n\n![gone](missing.png)\n\n# Two\n\n$$\\frac{1}{2}$$\n";
        let res = export_epub(md, dir.path().to_str().unwrap(), out.to_str().unwrap(), &serde_json::json!({}), "b").unwrap();
        let warns = res.warns.unwrap();
        assert!(warns.iter().any(|w| w.contains("missing.png")), "{warns:?}");
        let bytes = fs::read(&out).unwrap();
        let got = local_entries(&bytes);
        let names: Vec<&str> = got.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(names.iter().filter(|n| n.starts_with("OEBPS/images/")).count(), 1, "{names:?}");
        let img = got.iter().find(|e| e.0 == "OEBPS/images/img_1.png").unwrap();
        assert_eq!(img.1, 0, "images are stored");
        assert_eq!(img.5, png);
        let text = |name: &str| {
            let e = got.iter().find(|e| e.0 == name).unwrap();
            inflate(&e.5)
        };
        let opf = text("OEBPS/content.opf");
        assert!(opf.contains("href=\"images/img_1.png\" media-type=\"image/png\""), "{opf}");
        assert!(opf.contains("href=\"chapter_2.xhtml\" media-type=\"application/xhtml+xml\" properties=\"mathml\""), "{opf}");
        assert!(!opf.contains("href=\"chapter_1.xhtml\" media-type=\"application/xhtml+xml\" properties"), "{opf}");
        assert!(text("OEBPS/chapter_1.xhtml").contains("src=\"images/img_1.png\""));
        assert!(text("OEBPS/chapter_2.xhtml").contains("<math"));
    }

    // ---- ZIP container invariants measured against CPython 3.11.15 ----------

    #[test]
    fn test_zip_container_headers_match_cpython() {
        let entries = vec![
            ZipEntry { path: "mimetype".to_string(), data: b"application/epub+zip".to_vec(), compress: false },
            ZipEntry { path: "a.xhtml".to_string(), data: b"hello world hello world hello world".to_vec(), compress: true },
            ZipEntry { path: "\u{4e2d}\u{6587}.xhtml".to_string(), data: b"x".to_vec(), compress: true },
            ZipEntry { path: "empty.xhtml".to_string(), data: Vec::new(), compress: true },
        ];
        let zip = write_zip_at(&entries, 0, 0).expect("write_zip_at");
        let got = local_entries(&zip);

        assert_eq!(got.len(), 4);
        // mimetype: STORED, exact uncompressed payload.
        assert_eq!((got[0].1, got[0].3, got[0].4), (0, 20, 20));
        assert_eq!(got[0].5, b"application/epub+zip");
        // ASCII names carry no 0x0800 UTF-8 flag, non-ASCII names do.
        assert_eq!(got[1].2, 0);
        assert_eq!(got[2].2, 0x0800);
        // Deflated members are method 8.
        assert_eq!((got[1].1, got[2].1), (8, 8));
        // CPython never falls back to STORED for an empty member: method 8,
        // 2-byte raw-deflate stream `03 00`.
        assert_eq!((got[3].1, got[3].3, got[3].4), (8, 2, 0));
        assert_eq!(got[3].5, vec![0x03u8, 0x00u8]);

        // Central directory: version-made-by 20 (DOS), external_attr 0o600<<16,
        // which is exactly what `writestr` sets for a non-directory name.
        let tail = zip.len() - 22;
        assert_eq!(&zip[tail..tail + 4], b"PK\x05\x06");
        let cd_off = u32::from_le_bytes([zip[tail + 16], zip[tail + 17], zip[tail + 18], zip[tail + 19]]) as usize;
        assert_eq!(&zip[cd_off..cd_off + 4], b"PK\x01\x02");
        assert_eq!(u16::from_le_bytes([zip[cd_off + 4], zip[cd_off + 5]]), 20);
        assert_eq!(u16::from_le_bytes([zip[cd_off + 6], zip[cd_off + 7]]), 20);
        assert_eq!(
            u32::from_le_bytes([
                zip[cd_off + 38],
                zip[cd_off + 39],
                zip[cd_off + 40],
                zip[cd_off + 41]
            ]),
            (0o600u32 << 16)
        );
        // Central directory file record field meanings (CDD layout): 28 name len,
        // 30 extra len, 32 comment len, 34 disk-number-start, 36 internal attrs.
        assert_eq!(u16::from_le_bytes([zip[cd_off + 28], zip[cd_off + 29]]), 8, "mimetype name len");
        assert_eq!(u16::from_le_bytes([zip[cd_off + 30], zip[cd_off + 31]]), 0, "extra len");
        assert_eq!(u16::from_le_bytes([zip[cd_off + 32], zip[cd_off + 33]]), 0, "comment len");
        assert_eq!(u16::from_le_bytes([zip[cd_off + 34], zip[cd_off + 35]]), 0, "disk number start");
        assert_eq!(u16::from_le_bytes([zip[cd_off + 36], zip[cd_off + 37]]), 0, "internal attrs");
    }

    /// Byte-for-byte deflate-stream parity with CPython's zlib at the default
    /// level.  Kept separate so a flate2 backend difference (rust_backend uses
    /// miniz_oxide, not zlib) fails only this probe.
    #[test]
    fn test_deflate_stream_bytes_match_cpython_zlib() {
        let entries = vec![ZipEntry {
            path: "a.xhtml".to_string(),
            data: b"hello world hello world hello world".to_vec(),
            compress: true,
        }];
        let zip = write_zip_at(&entries, 0, 0).unwrap();
        let got = local_entries(&zip);
        assert_eq!(got[0].4, 35);
        assert_eq!(
            got[0].5,
            vec![
                0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x51, 0xc8,
                0xc0, 0xce, 0x06, 0x00
            ],
            "deflate payload differs from CPython zlib level 6 (csize={})",
            got[0].3
        );
    }

    #[test]
    fn test_epub_container_member_names_and_order() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out_epub = temp_dir.path().join("chapters.epub");
        let md = "# First\n\nHello *EPUB* world.\n\n# Second\n\nMore text.\n";
        let res = export_epub(md, "", out_epub.to_str().unwrap(), &serde_json::json!({}), "chapters")
            .expect("export_epub should succeed");
        assert!(res.ok);
        let bytes = fs::read(&out_epub).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        assert_eq!(bytes.len() as u64, res.size.unwrap());

        let got = local_entries(&bytes);
        // Member names and ordering are exact in Python (zipfile writes them in
        // this sequence); only the DOS timestamp fields are wall-clock, so they
        // are deliberately not asserted here.
        let names: Vec<&str> = got.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "mimetype",
                "META-INF/container.xml",
                "OEBPS/content.opf",
                "OEBPS/nav.xhtml",
                "OEBPS/toc.ncx",
                "OEBPS/style.css",
                "OEBPS/chapter_1.xhtml",
                "OEBPS/chapter_2.xhtml",
            ]
        );
        // OCF requires `mimetype` first and STORED; everything else deflated.
        assert_eq!(got[0].1, 0);
        assert_eq!(got[0].5, b"application/epub+zip");
        for e in &got[1..] {
            assert_eq!(e.1, 8, "{} should be deflated", e.0);
            assert_eq!(e.2, 0, "{} is ASCII so no UTF-8 flag", e.0);
        }
        // Every body is deflated, so content assertions must run on inflated
        // bytes, not the raw stream (`mimetype` is method 0 and stays verbatim).
        let bodies: Vec<(String, String)> = got
            .iter()
            .map(|e| {
                (
                    e.0.clone(),
                    if e.1 == 0 {
                        String::from_utf8_lossy(&e.5).into_owned()
                    } else {
                        inflate(&e.5)
                    },
                )
            })
            .collect();
        let body = |name: &str| -> String {
            bodies
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, b)| b.clone())
                .unwrap_or_else(|| panic!("{} missing from container", name))
        };
        let nav = body("OEBPS/nav.xhtml");
        assert!(
            nav.contains("chapter_1.xhtml") && nav.contains("chapter_2.xhtml"),
            "nav.xhtml must list both chapters: {}",
            nav
        );
        assert!(
            nav.contains("First") && nav.contains("Second"),
            "nav.xhtml must carry both chapter titles: {}",
            nav
        );
        assert!(
            body("OEBPS/chapter_1.xhtml").contains("<em>EPUB</em>"),
            "chapter 1 body lost the inline emphasis"
        );
        assert!(
            body("OEBPS/chapter_2.xhtml").contains("More text."),
            "chapter 2 body lost its paragraph"
        );
        assert!(
            body("META-INF/container.xml")
                .contains(r#"<rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>"#),
            "container.xml must point at the OPF exactly like epub_render.py:403: {}",
            body("META-INF/container.xml")
        );
    }

    // ---- D7: presentation_render.py:56 iterates `raw_yaml.splitlines()` ----

    #[test]
    fn test_frontmatter_splits_on_every_cpython_line_break() {
        // CPython breaks lines on \v \f \x1c \x1d \x1e \x85 U+2028 U+2029 as
        // well; `split('\n')` kept the tail inside the value, which reaches the
        // emitted `<title>`/`data-transition` bytes.
        for sep in ['\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}'] {
            let (meta, body) = parse_frontmatter(&format!("---\ntitle: A{sep}B\n---\nbody\n"));
            assert_eq!(meta_get(&meta, "title"), Some("A"), "sep {:?} => {:?}", sep, meta);
            assert_eq!(body, "body\n");
        }
        // \x1f is whitespace for `strip()` but *not* a `splitlines()` break.
        let (meta, _) = parse_frontmatter("---\ntitle: A\u{1f}\n---\n");
        assert_eq!(meta_get(&meta, "title"), Some("A"), "got {:?}", meta);
        // CRLF front-matter keeps parsing (a `\r\n` pair is one break).
        let (meta, body) = parse_frontmatter("---\r\ntitle: X\r\ntheme: dark\r\n---\r\nbody\r\n");
        assert_eq!(meta_get(&meta, "title"), Some("X"), "got {:?}", meta);
        assert_eq!(meta_get(&meta, "theme"), Some("dark"), "got {:?}", meta);
        assert_eq!(body, "\r\nbody\r\n");
    }

    // ---- G7: ZIP empty-entry shape, byte-identical to CPython 3.11.15 -------

    /// Reference archive, captured from CPython 3.11.15 `zipfile` (default level,
    /// every member written through `writestr(ZipInfo(name,
    /// date_time=(1980,1,1,0,0,0)), data, compress_type=…)`): `mimetype` STORED
    /// 20 bytes, `empty.xhtml` DEFLATED empty, `stored-empty.txt` STORED empty,
    /// `中文.xhtml` DEFLATED empty under a non-ASCII name.
    const CPYTHON_EMPTY_ZIP: [u8; 444] = [
            0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x00, 0x6f, 0x61,
            0xab, 0x2c, 0x14, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x6d, 0x69,
            0x6d, 0x65, 0x74, 0x79, 0x70, 0x65, 0x61, 0x70, 0x70, 0x6c, 0x69, 0x63, 0x61, 0x74, 0x69, 0x6f,
            0x6e, 0x2f, 0x65, 0x70, 0x75, 0x62, 0x2b, 0x7a, 0x69, 0x70, 0x50, 0x4b, 0x03, 0x04, 0x14, 0x00,
            0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x65, 0x6d, 0x70, 0x74, 0x79, 0x2e, 0x78, 0x68,
            0x74, 0x6d, 0x6c, 0x03, 0x00, 0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
            0x00, 0x00, 0x00, 0x73, 0x74, 0x6f, 0x72, 0x65, 0x64, 0x2d, 0x65, 0x6d, 0x70, 0x74, 0x79, 0x2e,
            0x74, 0x78, 0x74, 0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x00, 0x08, 0x08, 0x00, 0x00, 0x00, 0x21,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00,
            0x00, 0xe4, 0xb8, 0xad, 0xe6, 0x96, 0x87, 0x2e, 0x78, 0x68, 0x74, 0x6d, 0x6c, 0x03, 0x00, 0x50,
            0x4b, 0x01, 0x02, 0x14, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x00, 0x6f,
            0x61, 0xab, 0x2c, 0x14, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00, 0x6d, 0x69, 0x6d,
            0x65, 0x74, 0x79, 0x70, 0x65, 0x50, 0x4b, 0x01, 0x02, 0x14, 0x00, 0x14, 0x00, 0x00, 0x00, 0x08,
            0x00, 0x00, 0x00, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x3a,
            0x00, 0x00, 0x00, 0x65, 0x6d, 0x70, 0x74, 0x79, 0x2e, 0x78, 0x68, 0x74, 0x6d, 0x6c, 0x50, 0x4b,
            0x01, 0x02, 0x14, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x65, 0x00, 0x00, 0x00, 0x73, 0x74, 0x6f, 0x72,
            0x65, 0x64, 0x2d, 0x65, 0x6d, 0x70, 0x74, 0x79, 0x2e, 0x74, 0x78, 0x74, 0x50, 0x4b, 0x01, 0x02,
            0x14, 0x00, 0x14, 0x00, 0x00, 0x08, 0x08, 0x00, 0x00, 0x00, 0x21, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x93, 0x00, 0x00, 0x00, 0xe4, 0xb8, 0xad, 0xe6, 0x96, 0x87,
            0x2e, 0x78, 0x68, 0x74, 0x6d, 0x6c, 0x50, 0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x04, 0x00, 0xe7, 0x00, 0x00, 0x00, 0xbf, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn test_zip_empty_entries_are_byte_identical_to_cpython() {
        let (dos_time, dos_date) = dos_datetime(1980, 1, 1, 0, 0, 0);
        assert_eq!((dos_time, dos_date), (0, 0x21));
        let entries = vec![
            ZipEntry { path: "mimetype".to_string(), data: b"application/epub+zip".to_vec(), compress: false },
            ZipEntry { path: "empty.xhtml".to_string(), data: Vec::new(), compress: true },
            ZipEntry { path: "stored-empty.txt".to_string(), data: Vec::new(), compress: false },
            ZipEntry { path: "\u{4e2d}\u{6587}.xhtml".to_string(), data: Vec::new(), compress: true },
        ];
        let zip = write_zip_at(&entries, dos_time, dos_date).expect("write_zip_at");

        // Whole-archive equality: an empty member must not lose its slot, its
        // method or a single header byte on the way to disk.
        assert_eq!(zip.len(), 444);
        assert_eq!(zip.as_slice(), CPYTHON_EMPTY_ZIP.as_slice(), "archive differs from CPython zipfile");

        // Read it back the way `zipfile` walks a local file header.
        let got = local_entries(&zip);
        assert_eq!(got.len(), 4, "one member per entry, none skipped");
        assert_eq!(got[1].0, "empty.xhtml");
        assert_eq!((got[1].1, got[1].3, got[1].4), (8, 2, 0), "DEFLATED empty: method/csize/usize");
        assert_eq!(got[1].5, vec![0x03u8, 0x00u8], "CPython's empty raw-deflate stream");
        assert_eq!((got[2].1, got[2].3, got[2].4), (0, 0, 0), "STORED empty is still listed, 0 bytes");
        assert_eq!(got[3].2, 0x0800, "non-ASCII name sets the UTF-8 flag");
        assert_eq!(inflate(&got[1].5), "", "empty member must inflate to nothing");
        assert_eq!(inflate(&got[3].5), "");

        // ... and through the central directory, which is what readers trust.
        let tail = zip.len() - 22;
        assert_eq!(&zip[tail..tail + 4], b"PK\x05\x06");
        let rd16 = |o: usize| u16::from_le_bytes([zip[o], zip[o + 1]]);
        let rd32 = |o: usize| u32::from_le_bytes([zip[o], zip[o + 1], zip[o + 2], zip[o + 3]]);
        let cd_len = rd32(tail + 12) as usize;
        let cd_off = rd32(tail + 16) as usize;
        assert_eq!((cd_off, cd_len, rd16(tail + 8), rd16(tail + 10)), (191, 231, 4, 4));
        let mut names = Vec::new();
        let mut recs = Vec::new();
        let mut off = cd_off;
        while off < cd_off + cd_len {
            assert_eq!(&zip[off..off + 4], b"PK\x01\x02", "central record at {}", off);
            let nl = rd16(off + 28) as usize;
            names.push(String::from_utf8_lossy(&zip[off + 46..off + 46 + nl]).to_string());
            // (method, comp size, uncomp size, local header offset)
            recs.push((rd16(off + 10), rd32(off + 20), rd32(off + 24), rd32(off + 42)));
            assert_eq!(rd32(off + 38), 0o600u32 << 16, "external attrs, CPython writestr");
            off += 46 + nl;
        }
        assert_eq!(
            names,
            vec!["mimetype", "empty.xhtml", "stored-empty.txt", "\u{4e2d}\u{6587}.xhtml"]
        );
        assert_eq!(
            recs,
            vec![
                (0u16, 20u32, 20u32, 0u32),
                (8, 2, 0, 58),
                (0, 0, 0, 101),
                (8, 2, 0, 147),
            ]
        );
    }

    #[test]
    fn test_docx_export_cjk_document_with_display_and_inline_math() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out = temp_dir.path().join("math.docx");
        let md = "# 第一章\n\n中文段落含行内公式 $a+b$ 与 $c$。\n\n$$\nx = y\n$$\n\n结束。\n";
        let res = export_docx(md, "", out.to_str().unwrap(), &serde_json::json!({}), "test")
            .expect("export_docx must not panic on multibyte text before a formula");
        assert!(res.ok);

        let zip = fs::read(&out).unwrap();
        let got = local_entries(&zip);
        let names: Vec<&str> = got.iter().map(|e| e.0.as_str()).collect();
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
            "word/document.xml",
        ] {
            assert!(names.contains(&part), "missing {part}: {names:?}");
        }
        let doc = got.iter().find(|e| e.0 == "word/document.xml").expect("document.xml");
        let xml = inflate(&doc.5);
        assert_eq!(xml.matches("<m:oMathPara").count(), 1, "one display formula: {xml}");
        assert!(
            xml.contains(&crate::latex2omml::latex_to_omml("x = y", true)),
            "display latex must be strip()ed like parser.py:232: {xml}"
        );
        assert_eq!(
            xml.matches(&crate::latex2omml::latex_to_omml("a+b", false)).count(),
            1,
            "inline #1 lost: {xml}"
        );
        assert_eq!(
            xml.matches(&crate::latex2omml::latex_to_omml("c", false)).count(),
            1,
            "inline #2 lost: {xml}"
        );
        assert!(xml.contains("中文段落含行内公式") && xml.contains("结束。"), "text lost: {xml}");
    }

    /// `html_render.py:103` writes
    /// `json.dumps(md, ensure_ascii=False).replace('<', '\\u003c')` into the
    /// `<script type="application/json" id="md-source">` payload. The Rust port
    /// once emitted the *Rust* brace form `\u{003c}` — eight literal characters
    /// in which `{` follows `\u`, a hard `JSON.parse` SyntaxError, so the
    /// exported page never loaded its own markdown.
    #[test]
    fn test_html_export_embeds_json_safe_angle_bracket_escape() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out_html = temp_dir.path().join("export.html");
        let md = "# 标题\n\n</script><b>raw</b>\n\n段落里的 <tag> 与 $\n";
        let res = export_html(md, "", out_html.to_str().unwrap(), &serde_json::json!({}), "test", temp_dir.path())
            .expect("export_html should succeed");
        assert!(res.ok);

        let html = fs::read_to_string(&out_html).unwrap();
        let payload = html.split("id=\"md-source\">")
            .nth(1)
            .expect("md-source script present")
            .split("</script>")
            .next()
            .unwrap()
            .to_string();

        assert!(payload.contains("\\u003c"), "every '<' must become the JSON escape \\u003c: {payload}");
        assert!(!payload.contains("\\u{"), "the Rust brace form is not valid JSON: {payload}");
        // The payload must actually parse, and parse back to the source text.
        let parsed: String = serde_json::from_str(&payload)
            .unwrap_or_else(|e| panic!("embedded markdown is not valid JSON ({e}): {payload}"));
        assert!(parsed.contains("</script><b>raw</b>"), "round-trip lost the markup: {parsed}");
        assert!(parsed.contains("段落里的 <tag>"), "round-trip lost the CJK body: {parsed}");
    }

    /// The crate's own `shutil.which` mirror, `crate::plugin_manager::which`, is
    /// what every external-program lookup in `mdexport` used to route through.
    /// (It replaced a `which::which` call that was in neither `Cargo.toml` nor
    /// `Cargo.lock`; Windows stripped the `#[cfg]` block before name resolution
    /// so five lanes of builds proved nothing, and Linux/macOS failed with
    /// E0433.)  Calling it from inside `mdexport` under the *host* compiler
    /// type-checks the exact expression a non-Windows build compiles (`&&str`
    /// from a `&[&str]` iteration coerced to the `&str` parameter,
    /// `Option<PathBuf>` return).
    ///
    /// The PDF lane no longer launches anything, so this is a plain path
    /// resolver guard now — but it is still the only one over that resolver.
    #[test]
    fn test_in_crate_which_resolver_only_yields_real_files() {
        for prog in &["git", "python", "tar", "no-such-program-on-path-1"] {
            let found: Option<PathBuf> = crate::plugin_manager::which(prog);
            assert!(found.as_ref().map_or(true, |p| p.is_file()), "resolver must only yield real files: {prog}");
        }
        // A missing binary is `None`, never a panic — the rung the old
        // `Result`-returning call relied on.
        assert!(crate::plugin_manager::which("readmd-no-such-binary-9f3a").is_none());
    }

        // ------------------------------------------------------------------------
        // HTML renderer goldens.
        //
        // Every expectation below is a machine-generated capture of the Python
        // authority (`src/readmd_modules/mdexport/html_render.py`, `styles.py`,
        // `__init__.py`) produced by `scratch/rust_parity/md_s2_gen2.py`, embedded
        // verbatim by `scratch/rust_parity/md_s2_splice.py`.  Nothing here is typed
        // by hand, so a change in the Python side shows up as a test failure instead
        // of a silently re-transcribed constant.
        // ------------------------------------------------------------------------
    /// `md_s2_gen2.py` payloads, inserted verbatim.  `r##"..."##` because the CSS
    /// contains `"` and `#` (`--bg:#ffffff`) next to each other.
    const MD_HTML_CSS_CASES: &str = r##"[
     {
      "name": "arial_center_h1break",
      "options": {
       "typography": {
        "font": "Arial",
        "size": 12,
        "lineHeight": 1.8,
        "spacing": 8,
        "align": "center",
        "firstLineIndent": 2
       },
       "headings": {
        "h1": {
         "pageBreakBefore": true
        }
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:Arial, sans-serif; font-size:12px; line-height:1.8; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:12pt; } p { text-align:center; text-indent:2mm; margin-bottom:8pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\nh1 { break-before:page; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "bogus_theme",
      "options": {
       "htmlTheme": "neon"
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "business_preset",
      "options": {
       "typography": {
        "size": 11,
        "lineHeight": 1.65,
        "spacing": 6,
        "color": "#2c3e50",
        "align": "left"
       },
       "headings": {
        "h1": {
         "size": 20,
         "color": "#1f3864",
         "bold": true
        },
        "h2": {
         "size": 16,
         "color": "#2e5395"
        },
        "h3": {
         "size": 13.5,
         "color": "#3a6db5"
        },
        "h4": {
         "size": 12,
         "color": "#4a7fd4"
        }
       },
       "table": {
        "headerBg": "#1f3864",
        "headerColor": "#ffffff",
        "headerBold": true,
        "borderColor": "#9fb3d1",
        "borderWidth": 0.75,
        "banded": true,
        "bandColor": "#eef3fa",
        "cellSize": 10,
        "cellPadding": 6,
        "align": "left",
        "widthPct": 100
       },
       "code": {
        "bg": "#f0f4fa",
        "color": "#1f3864",
        "borderColor": "#c3d0e4",
        "borderWidth": 0.5
       },
       "quote": {
        "barColor": "#1f3864",
        "bg": "#eef3fa",
        "color": "#34507c"
       },
       "link": {
        "color": "#1f3864"
       },
       "hr": {
        "color": "#b9c6da"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.65; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1f3864; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#2e5395; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:13.5pt; color:#3a6db5; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#4a7fd4; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #9fb3d1; padding:6px 6px; text-align:left; }\nth { background:#1f3864; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#eef3fa; }\npre { background:#f0f4fa; color:#1f3864; border:0.50px solid #c3d0e4; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f0f4fa; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#eef3fa; color:#34507c; border-left:4px solid #1f3864; }\na { color:#1f3864; }\nhr { border:none; border-top:1px solid #b9c6da; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "classic_preset",
      "options": {
       "typography": {
        "size": 12,
        "lineHeight": 1.8,
        "spacing": 8,
        "color": "#1a1a1a",
        "align": "left"
       },
       "headings": {
        "h1": {
         "size": 22,
         "color": "#000000",
         "align": "center"
        },
        "h2": {
         "size": 17,
         "color": "#111111"
        },
        "h3": {
         "size": 14.5,
         "color": "#222222"
        },
        "h4": {
         "size": 13,
         "color": "#333333"
        }
       },
       "table": {
        "headerBg": "#d9e2ec",
        "headerColor": "#1f2d3d",
        "headerBold": true,
        "borderColor": "#8a94a6",
        "borderWidth": 1.0,
        "banded": true,
        "bandColor": "#f2f5f8",
        "cellSize": 10.5,
        "cellPadding": 7,
        "align": "left",
        "widthPct": 100
       },
       "code": {
        "bg": "#f4f4f0",
        "color": "#333333",
        "borderColor": "#c9c9c4",
        "borderWidth": 0.75
       },
       "quote": {
        "barColor": "#7a8699",
        "bg": "#f5f6f8",
        "color": "#3d4852"
       },
       "link": {
        "color": "#8a2be2"
       },
       "hr": {
        "color": "#b5b5ad"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:12px; line-height:1.8; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:22pt; color:#000000; font-weight:bold; text-align:center; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:17pt; color:#111111; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14.5pt; color:#222222; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:13pt; color:#333333; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10.5pt; }\nth, td { border:1.00px solid #8a94a6; padding:7px 7px; text-align:left; }\nth { background:#d9e2ec; color:#1f2d3d; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f2f5f8; }\npre { background:#f4f4f0; color:#333333; border:0.75px solid #c9c9c4; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f4f4f0; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f5f6f8; color:#3d4852; border-left:4px solid #7a8699; }\na { color:#8a2be2; }\nhr { border:none; border-top:1px solid #b5b5ad; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:12pt; } p { text-align:left; text-indent:0mm; margin-bottom:8pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "custom_tiny",
      "options": {
       "page": {
        "size": "Custom",
        "width": 100,
        "height": 150,
        "marginTop": 40,
        "marginBottom": 40,
        "marginRight": 30,
        "marginLeft": 30
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:100mm 150mm; margin:40mm 30mm 40mm 30mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "dark",
      "options": {
       "htmlTheme": "dark"
      },
      "css": ":root { --bg:#14161a; --fg:#d6d9de; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#d6d9de; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "empty",
      "options": {},
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "half_steps",
      "options": {
       "typography": {
        "size": 10.5,
        "lineHeight": 1.05
       },
       "headings": {
        "h6": {
         "size": 9.25,
         "before": 0.125,
         "after": 33.333333
        }
       },
       "table": {
        "borderWidth": 1.005,
        "cellSize": 12.5,
        "cellPadding": 2.5
       },
       "code": {
        "size": 9.5,
        "borderWidth": 0.005
       },
       "page": {
        "size": "Custom",
        "width": 215.9,
        "height": 279.4,
        "marginTop": 0.5,
        "marginRight": 0,
        "marginBottom": 60,
        "marginLeft": 60
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:10.5px; line-height:1.05; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:9.25pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:0.125pt; margin-bottom:33.3333pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:12.5pt; }\nth, td { border:1.00px solid #c8cdd4; padding:2.5px 2.5px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.01px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:215.9mm 279.4mm; margin:0.5mm 0mm 60mm 60mm; }\nbody { font-size:10.5pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "heading_flags",
      "options": {
       "headings": {
        "h3": {
         "bold": false,
         "align": "justify",
         "pageBreakBefore": true
        },
        "h5": {
         "size": 8,
         "before": 0,
         "after": 40
        }
       },
       "images": {
        "widthPct": 100
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:normal; text-align:justify; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:8pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:0pt; margin-bottom:40pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:100%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\nh3 { break-before:page; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "heading_non_dict",
      "options": {
       "headings": {
        "h1": 7,
        "h2": [],
        "h3": "x",
        "h6": null
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "junk_numbers",
      "options": {
       "typography": {
        "size": "abc",
        "lineHeight": null,
        "spacing": [
         1
        ],
        "color": "notahex",
        "font": "Comic Sans"
       },
       "headings": {
        "h2": {
         "before": "x",
         "after": null,
         "size": "@@not-a-number",
         "align": "top",
         "color": "#GGG"
        }
       },
       "page": {
        "width": 99999,
        "height": "1e3",
        "size": "A0",
        "orientation": "sideways"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "landscape_letter",
      "options": {
       "page": {
        "size": "Letter",
        "orientation": "landscape"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:279.4mm 215.9mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "legal_portrait",
      "options": {
       "page": {
        "size": "Legal"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:215.9mm 355.6mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "margin_overflow",
      "options": {
       "page": {
        "size": "A5",
        "marginTop": 60,
        "marginBottom": 60,
        "marginRight": 61,
        "marginLeft": 60
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:148mm 210mm; margin:60mm 59mm 60mm 59mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "margin_overflow_custom",
      "options": {
       "page": {
        "size": "Custom",
        "width": 80,
        "height": 80,
        "marginTop": 60,
        "marginBottom": 60,
        "marginRight": 60,
        "marginLeft": 60
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:80mm 80mm; margin:25mm 25mm 25mm 25mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "margin_overflow_landscape",
      "options": {
       "page": {
        "size": "A5",
        "orientation": "landscape",
        "marginTop": 60,
        "marginBottom": 60,
        "marginRight": 60,
        "marginLeft": 60
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 148mm; margin:59mm 60mm 59mm 60mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "margin_overflow_letter",
      "options": {
       "page": {
        "size": "Letter",
        "marginTop": 60,
        "marginBottom": 60,
        "marginRight": 60,
        "marginLeft": 60
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:215.9mm 279.4mm; margin:60mm 60mm 60mm 60mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "minimal_preset",
      "options": {
       "typography": {
        "size": 10.5,
        "lineHeight": 1.55,
        "color": "#333333",
        "align": "left"
       },
       "headings": {
        "h1": {
         "size": 18,
         "color": "#111111"
        },
        "h2": {
         "size": 14.5,
         "color": "#222222"
        },
        "h3": {
         "size": 12.5,
         "color": "#333333"
        }
       },
       "table": {
        "headerBg": "#eef1f5",
        "headerColor": "#333333",
        "headerBold": true,
        "borderColor": "#ccd2da",
        "borderWidth": 0.5,
        "banded": false,
        "cellSize": 9.5,
        "cellPadding": 5,
        "align": "left",
        "widthPct": 100
       },
       "code": {
        "bg": "#f7f8fa",
        "color": "#444444",
        "borderColor": "#e3e6ea",
        "borderWidth": 0.5
       },
       "quote": {
        "barColor": "#9aa3af",
        "bg": "#f6f7f9",
        "color": "#555555"
       },
       "link": {
        "color": "#1a73e8"
       },
       "hr": {
        "color": "#e0e3e8"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:10.5px; line-height:1.55; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:18pt; color:#111111; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:14.5pt; color:#222222; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:12.5pt; color:#333333; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:9.5pt; }\nth, td { border:0.50px solid #ccd2da; padding:5px 5px; text-align:left; }\nth { background:#eef1f5; color:#333333; font-weight:bold; }\npre { background:#f7f8fa; color:#444444; border:0.50px solid #e3e6ea; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f7f8fa; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f6f7f9; color:#555555; border-left:4px solid #9aa3af; }\na { color:#1a73e8; }\nhr { border:none; border-top:1px solid #e0e3e8; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:10.5pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "mono_fonts",
      "options": {
       "code": {
        "font": "SimHei"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:SimHei, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:SimHei, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "no_borders",
      "options": {
       "table": {
        "borderWidth": 0,
        "banded": false,
        "headerBold": false,
        "cellSize": 7,
        "cellPadding": 0,
        "widthPct": 50,
        "align": "right"
       },
       "code": {
        "rounded": false,
        "borderWidth": 0,
        "size": 6,
        "font": "Courier New"
       },
       "images": {
        "widthPct": 10
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:50%; margin:8px auto; font-size:7pt; }\nth, td { border:0.00px solid #c8cdd4; padding:0px 0px; text-align:right; }\nth { background:#3b6ef5; color:#ffffff; font-weight:normal; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.00px solid #dfe3e8; border-radius:0; padding:12px 14px; overflow:auto; font-family:Courier New, Consolas, monospace; font-size:6pt; line-height:1.5; }\ncode { font-family:Courier New, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:10%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "not_a_dict_sections",
      "options": {
       "headings": 5,
       "table": null,
       "code": [],
       "quote": "x",
       "typography": "y",
       "page": 0,
       "images": {},
       "link": {},
       "hr": {}
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "sepia",
      "options": {
       "htmlTheme": "sepia"
      },
      "css": ":root { --bg:#faf4e7; --fg:#3b2f1d; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#3b2f1d; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "simsun_justify",
      "options": {
       "typography": {
        "font": "SimSun",
        "align": "justify"
       },
       "headings": {
        "h4": {
         "align": "right",
         "bold": false
        }
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"SimSun\", \"宋体\", serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:normal; text-align:right; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:justify; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "string_theme_dict",
      "options": {
       "htmlTheme": "light",
       "quote": {
        "bg": "#010203",
        "color": "#040506",
        "barColor": "#070809"
       },
       "link": {
        "color": "#0a0b0c"
       },
       "hr": {
        "color": "#0d0e0f"
       },
       "meta": {
        "title": "A&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>Dé"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#010203; color:#040506; border-left:4px solid #070809; }\na { color:#0a0b0c; }\nhr { border:none; border-top:1px solid #0d0e0f; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "trailing_newline_hex",
      "options": {
       "table": {
        "borderColor": "#ff0000\n",
        "headerBg": "#00ff00\n\n",
        "headerColor": "#0000ff \n"
       },
       "link": {
        "color": "#1234567"
       },
       "hr": {
        "color": "#abc"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #ff0000\n; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     },
     {
      "name": "underscore_numbers",
      "options": {
       "typography": {
        "size": "1_1",
        "spacing": " 12 ",
        "lineHeight": "1_0.5"
       },
       "table": {
        "cellPadding": "1.",
        "borderWidth": "+.25"
       },
       "code": {
        "size": "1_0"
       }
      },
      "css": ":root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:2.5; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.25px solid #c8cdd4; padding:1px 1px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:10pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:12pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }"
     }
    ]"##;
    const MD_HTML_DOC_CASES: &str = r##"[
     {
      "name": "no_assets",
      "md": "# 标题\n\n段落 <tag> 与 $x^2$ 以及 `code`\n\n<!-- page-break -->\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n\tindented\n\n</script>\n\\newpage\n",
      "options": {
       "htmlTheme": "sepia",
       "typography": {
        "size": 11.5
       }
      },
      "source_name": "文档.txt",
      "assets": {},
      "html": "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"ReadMD\">\n<meta name=\"keywords\" content=\"\">\n<title>文档.txt</title>\n<style>\n:root { --bg:#faf4e7; --fg:#3b2f1d; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#3b2f1d; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11.5px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11.5pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }\n</style>\n</head>\n<body>\n<article id=\"content\" class=\"readmd-export\"></article>\n<script type=\"application/json\" id=\"md-source\">\"# 标题\\n\\n段落 \\u003ctag> 与 $x^2$ 以及 `code`\\n\\n\\u003c!-- page-break -->\\n\\n| a | b |\\n|---|---|\\n| 1 | 2 |\\n\\n- [x] done\\n\\n\\tindented\\n\\n\\u003c/script>\\n\\\\newpage\\n\"</script>\n<script>\nwindow.MathJax = {\n  tex: { inlineMath: [['$', '$'], ['\\\\(', '\\\\)']], displayMath: [['$$', '$$'], ['\\\\[', '\\\\]']] },\n  svg: { fontCache: 'global' },\n  options: { skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'] }\n};\n</script>\n<script>\n\n</script>\n<script>\n\n</script>\n<script>\n(function () {\n  var md = JSON.parse(document.getElementById('md-source').textContent);\n  var html;\n  try { html = marked.parse(md, { gfm: true, breaks: true }); }\n  catch (e) { html = '<p>渲染失败：' + e.message + '</p>'; }\n  document.getElementById('content').innerHTML = html;\n  var root = document.getElementById('content');\n  var highlights = {};\n  root.querySelectorAll('pre > code').forEach(function (code) {\n    var key = code.textContent.replace(/\n+$/, '');\n    if (Object.prototype.hasOwnProperty.call(highlights, key)) code.innerHTML = highlights[key];\n  });\n  var walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);\n  var breaks = [], node;\n  while ((node = walker.nextNode())) {\n    if (/^page-?break$/.test(node.textContent.trim())) breaks.push(node);\n  }\n  root.querySelectorAll('p').forEach(function (p) {\n    if (p.textContent.trim() === '\\\\newpage') breaks.push(p);\n  });\n  breaks.forEach(function (node) {\n    var div = document.createElement('div');\n    div.className = 'readmd-pagebreak'; node.replaceWith(div);\n  });\n  if (window.MathJax && MathJax.typesetPromise) {\n    try { MathJax.typesetPromise().catch(function () {}); } catch (e) {}\n  }\n})();\n</script>\n</body>\n</html>\n",
      "warns": [
       "marked.min.js 未找到，HTML 导出可能无法渲染",
       "MathJax 未找到，公式可能无法渲染"
      ]
     },
     {
      "name": "vendor_assets",
      "md": "# 标题\n\n段落 <tag> 与 $x^2$ 以及 `code`\n\n<!-- page-break -->\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n\tindented\n\n</script>\n\\newpage\n",
      "options": {
       "htmlTheme": "dark"
      },
      "source_name": "notes.md",
      "assets": {
       "marked.min.js": "/*__TITLE__*/window.marked={parse:function(m){return m}};var s='__MD__'+__CSS__;",
       "vendor/mathjax/tex-svg.js": "/*__KEYWORDS__*/MathJax;/*__HIGHLIGHTS__*/"
      },
      "html": "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"ReadMD\">\n<meta name=\"keywords\" content=\"\">\n<title>notes.md</title>\n<style>\n:root { --bg:#14161a; --fg:#d6d9de; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#d6d9de; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }\n</style>\n</head>\n<body>\n<article id=\"content\" class=\"readmd-export\"></article>\n<script type=\"application/json\" id=\"md-source\">\"# 标题\\n\\n段落 \\u003ctag> 与 $x^2$ 以及 `code`\\n\\n\\u003c!-- page-break -->\\n\\n| a | b |\\n|---|---|\\n| 1 | 2 |\\n\\n- [x] done\\n\\n\\tindented\\n\\n\\u003c/script>\\n\\\\newpage\\n\"</script>\n<script>\nwindow.MathJax = {\n  tex: { inlineMath: [['$', '$'], ['\\\\(', '\\\\)']], displayMath: [['$$', '$$'], ['\\\\[', '\\\\]']] },\n  svg: { fontCache: 'global' },\n  options: { skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'] }\n};\n</script>\n<script>\n/*__TITLE__*/window.marked={parse:function(m){return m}};var s='\"# 标题\\n\\n段落 \\u003ctag> 与 $x^2$ 以及 `code`\\n\\n\\u003c!-- page-break -->\\n\\n| a | b |\\n|---|---|\\n| 1 | 2 |\\n\\n- [x] done\\n\\n\\tindented\\n\\n\\u003c/script>\\n\\\\newpage\\n\"'+__CSS__;\n</script>\n<script>\n/*__KEYWORDS__*/MathJax;/*__HIGHLIGHTS__*/\n</script>\n<script>\n(function () {\n  var md = JSON.parse(document.getElementById('md-source').textContent);\n  var html;\n  try { html = marked.parse(md, { gfm: true, breaks: true }); }\n  catch (e) { html = '<p>渲染失败：' + e.message + '</p>'; }\n  document.getElementById('content').innerHTML = html;\n  var root = document.getElementById('content');\n  var highlights = {};\n  root.querySelectorAll('pre > code').forEach(function (code) {\n    var key = code.textContent.replace(/\n+$/, '');\n    if (Object.prototype.hasOwnProperty.call(highlights, key)) code.innerHTML = highlights[key];\n  });\n  var walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);\n  var breaks = [], node;\n  while ((node = walker.nextNode())) {\n    if (/^page-?break$/.test(node.textContent.trim())) breaks.push(node);\n  }\n  root.querySelectorAll('p').forEach(function (p) {\n    if (p.textContent.trim() === '\\\\newpage') breaks.push(p);\n  });\n  breaks.forEach(function (node) {\n    var div = document.createElement('div');\n    div.className = 'readmd-pagebreak'; node.replaceWith(div);\n  });\n  if (window.MathJax && MathJax.typesetPromise) {\n    try { MathJax.typesetPromise().catch(function () {}); } catch (e) {}\n  }\n})();\n</script>\n</body>\n</html>\n",
      "warns": []
     },
     {
      "name": "keywords_in_title",
      "md": "x",
      "options": {
       "meta": {
        "title": "<a href=\"q\">&</a>"
       }
      },
      "source_name": "",
      "assets": {},
      "html": "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"ReadMD\">\n<meta name=\"keywords\" content=\"\">\n<title>&lt;a href=&quot;q&quot;&gt;&amp;&lt;/a&gt;</title>\n<style>\n:root { --bg:#ffffff; --fg:#262626; }\n* { box-sizing: border-box; }\nbody { margin:0; background:var(--bg); color:#262626; font-family:\"Microsoft YaHei\", \"Microsoft YaHei UI\", \"PingFang SC\", \"微软雅黑\", sans-serif; font-size:11px; line-height:1.6; padding:24px 16px 64px; }\n#content { max-width:820px; margin:0 auto; word-wrap:break-word; }\nh1 { font-size:20pt; color:#1a1a1a; font-weight:bold; text-align:left; margin-top:18pt; margin-bottom:10pt; line-height:1.35; }\nh2 { font-size:16pt; color:#1f2937; font-weight:bold; text-align:left; margin-top:14pt; margin-bottom:8pt; line-height:1.35; }\nh3 { font-size:14pt; color:#2d3748; font-weight:bold; text-align:left; margin-top:12pt; margin-bottom:6pt; line-height:1.35; }\nh4 { font-size:12pt; color:#374151; font-weight:bold; text-align:left; margin-top:10pt; margin-bottom:6pt; line-height:1.35; }\nh5 { font-size:11pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\nh6 { font-size:10.5pt; color:#4a5568; font-weight:bold; text-align:left; margin-top:8pt; margin-bottom:4pt; line-height:1.35; }\ntable { border-collapse:collapse; width:100%; margin:8px auto; font-size:10pt; }\nth, td { border:0.75px solid #c8cdd4; padding:6px 6px; text-align:left; }\nth { background:#3b6ef5; color:#ffffff; font-weight:bold; }\ntbody tr:nth-child(even) { background:#f3f5f9; }\npre { background:#f5f6f8; color:#2f3b4a; border:0.50px solid #dfe3e8; border-radius:8px; padding:12px 14px; overflow:auto; font-family:Consolas, Consolas, monospace; font-size:9.5pt; line-height:1.5; }\ncode { font-family:Consolas, Consolas, monospace; }\n:not(pre) > code { background:#f5f6f8; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }\nblockquote { margin:8px 0; padding:8px 14px; background:#f3f6ff; color:#4a5568; border-left:4px solid #3b6ef5; }\na { color:#2b6cb0; }\nhr { border:none; border-top:1px solid #d8dce2; margin:16px 0; }\nimg { max-width:100%; height:auto; }\np > img:only-child { display:block; max-width:92%; margin:0 auto; object-fit:contain; }\nli.task-list-item { list-style:none; margin-left:-20px; }\nblockquote p, blockquote li { margin:4px 0; }\n@page { size:210mm 297mm; margin:20mm 18mm 20mm 18mm; }\nbody { font-size:11pt; } p { text-align:left; text-indent:0mm; margin-bottom:6pt; }\nh1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }\n@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }\n.readmd-pagebreak { break-after:page; page-break-after:always; }\n</style>\n</head>\n<body>\n<article id=\"content\" class=\"readmd-export\"></article>\n<script type=\"application/json\" id=\"md-source\">\"x\"</script>\n<script>\nwindow.MathJax = {\n  tex: { inlineMath: [['$', '$'], ['\\\\(', '\\\\)']], displayMath: [['$$', '$$'], ['\\\\[', '\\\\]']] },\n  svg: { fontCache: 'global' },\n  options: { skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'] }\n};\n</script>\n<script>\n\n</script>\n<script>\n\n</script>\n<script>\n(function () {\n  var md = JSON.parse(document.getElementById('md-source').textContent);\n  var html;\n  try { html = marked.parse(md, { gfm: true, breaks: true }); }\n  catch (e) { html = '<p>渲染失败：' + e.message + '</p>'; }\n  document.getElementById('content').innerHTML = html;\n  var root = document.getElementById('content');\n  var highlights = {};\n  root.querySelectorAll('pre > code').forEach(function (code) {\n    var key = code.textContent.replace(/\n+$/, '');\n    if (Object.prototype.hasOwnProperty.call(highlights, key)) code.innerHTML = highlights[key];\n  });\n  var walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);\n  var breaks = [], node;\n  while ((node = walker.nextNode())) {\n    if (/^page-?break$/.test(node.textContent.trim())) breaks.push(node);\n  }\n  root.querySelectorAll('p').forEach(function (p) {\n    if (p.textContent.trim() === '\\\\newpage') breaks.push(p);\n  });\n  breaks.forEach(function (node) {\n    var div = document.createElement('div');\n    div.className = 'readmd-pagebreak'; node.replaceWith(div);\n  });\n  if (window.MathJax && MathJax.typesetPromise) {\n    try { MathJax.typesetPromise().catch(function () {}); } catch (e) {}\n  }\n})();\n</script>\n</body>\n</html>\n",
      "warns": [
       "marked.min.js 未找到，HTML 导出可能无法渲染",
       "MathJax 未找到，公式可能无法渲染"
      ]
     }
    ]"##;
    const MD_HTML_TITLE_CASES: &str = r#"[
     {
      "name": "angles",
      "options": {
       "meta": {
        "title": "<b>x</b>"
       }
      },
      "source_name": "s.md",
      "title": "&lt;b&gt;x&lt;/b&gt;"
     },
     {
      "name": "bool",
      "options": {
       "meta": {
        "title": true
       }
      },
      "source_name": "s.md",
      "title": "True"
     },
     {
      "name": "both_empty",
      "options": {},
      "source_name": "",
      "title": "ReadMD 导出"
     },
     {
      "name": "dict",
      "options": {
       "meta": {
        "title": {
         "k": "v"
        }
       },
       "source": 1
      },
      "source_name": "s.md",
      "title": "{'k': 'v'}"
     },
     {
      "name": "empty_meta",
      "options": {
       "meta": {
        "title": ""
       }
      },
      "source_name": "文档.txt",
      "title": "文档.txt"
     },
     {
      "name": "float",
      "options": {
       "meta": {
        "title": 1.5
       }
      },
      "source_name": "s.md",
      "title": "1.5"
     },
     {
      "name": "list",
      "options": {
       "meta": {
        "title": [
         "a",
         "b"
        ]
       }
      },
      "source_name": "s.md",
      "title": "['a', 'b']"
     },
     {
      "name": "long_cjk",
      "options": {
       "meta": {
        "title": "A&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>DéA&B<C>Dé"
       }
      },
      "source_name": "文档.txt",
      "title": "A&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;DéA&amp;B&lt;C&gt;Dé"
     },
     {
      "name": "no_source",
      "options": {
       "meta": {
        "title": ""
       }
      },
      "source_name": "",
      "title": "ReadMD 导出"
     },
     {
      "name": "non_dict_meta",
      "options": {
       "meta": "nope"
      },
      "source_name": "s.md",
      "title": "s.md"
     },
     {
      "name": "null",
      "options": {
       "meta": {
        "title": null
       }
      },
      "source_name": "s.md",
      "title": "s.md"
     },
     {
      "name": "number",
      "options": {
       "meta": {
        "title": 5
       }
      },
      "source_name": "s.md",
      "title": "5"
     },
     {
      "name": "quotes",
      "options": {
       "meta": {
        "title": "he said \"hi\" & bye"
       }
      },
      "source_name": "s.md",
      "title": "he said &quot;hi&quot; &amp; bye"
     },
     {
      "name": "surrogate_text",
      "options": {
       "meta": {
        "title": "emoji 😀 ok"
       }
      },
      "source_name": "s.md",
      "title": "emoji 😀 ok"
     },
     {
      "name": "zero",
      "options": {
       "meta": {
        "title": 0
       }
      },
      "source_name": "s.md",
      "title": "s.md"
     }
    ]"#;
    const MD_HTML_G_CASES: &str = r#"{
     "g": {
      "0": "0",
      "1": "1",
      "2": "2",
      "6": "6",
      "8": "8",
      "10": "10",
      "11": "11",
      "12": "12",
      "20": "20",
      "30": "30",
      "40": "40",
      "60": "60",
      "92": "92",
      "100": "100",
      "210": "210",
      "297": "297",
      "420": "420",
      "148": "148",
      "176": "176",
      "250": "250",
      "215.9": "215.9",
      "279.4": "279.4",
      "355.6": "355.6",
      "0.5": "0.5",
      "1.5": "1.5",
      "1.55": "1.55",
      "1.65": "1.65",
      "1.8": "1.8",
      "0.75": "0.75",
      "0.05": "0.05",
      "0.005": "0.005",
      "9.5": "9.5",
      "10.5": "10.5",
      "12.5": "12.5",
      "1.005": "1.005",
      "33.333333": "33.3333",
      "6.25": "6.25",
      "0.0001": "0.0001",
      "1e-05": "1e-05",
      "123456.0": "123456",
      "1234567.0": "1.23457e+06",
      "1000000.0": "1e+06",
      "999999.0": "999999",
      "9999999.0": "1e+07",
      "0.000123456": "0.000123456",
      "1e+20": "1e+20",
      "1e-20": "1e-20",
      "2.5e-07": "2.5e-07",
      "0.3333333333333333": "0.333333",
      "0.6666666666666666": "0.666667",
      "1000000000000000.0": "1e+15",
      "1e+16": "1e+16",
      "3.0000004": "3",
      "24.0": "24",
      "64.0": "64",
      "0.0": "0",
      "-0.0": "-0",
      "59.0": "59",
      "59.00000000000001": "59",
      "185.9": "185.9",
      "2.5e-08": "2.5e-08",
      "9.99999e-05": "9.99999e-05",
      "1.000005": "1.00001",
      "0.125": "0.125",
      "3.14159": "3.14159",
      "1000.5": "1000.5",
      "777777.0": "777777"
     },
     "f2": {
      "0": "0.00",
      "1": "1.00",
      "2": "2.00",
      "6": "6.00",
      "8": "8.00",
      "10": "10.00",
      "11": "11.00",
      "12": "12.00",
      "20": "20.00",
      "30": "30.00",
      "40": "40.00",
      "60": "60.00",
      "92": "92.00",
      "100": "100.00",
      "210": "210.00",
      "297": "297.00",
      "420": "420.00",
      "148": "148.00",
      "176": "176.00",
      "250": "250.00",
      "215.9": "215.90",
      "279.4": "279.40",
      "355.6": "355.60",
      "0.5": "0.50",
      "1.5": "1.50",
      "1.55": "1.55",
      "1.65": "1.65",
      "1.8": "1.80",
      "0.75": "0.75",
      "0.05": "0.05",
      "0.005": "0.01",
      "9.5": "9.50",
      "10.5": "10.50",
      "12.5": "12.50",
      "1.005": "1.00",
      "33.333333": "33.33",
      "6.25": "6.25",
      "0.0001": "0.00",
      "1e-05": "0.00",
      "123456.0": "123456.00",
      "1234567.0": "1234567.00",
      "1000000.0": "1000000.00",
      "999999.0": "999999.00",
      "9999999.0": "9999999.00",
      "0.000123456": "0.00",
      "1e+20": "100000000000000000000.00",
      "1e-20": "0.00",
      "2.5e-07": "0.00",
      "0.3333333333333333": "0.33",
      "0.6666666666666666": "0.67",
      "1000000000000000.0": "1000000000000000.00",
      "1e+16": "10000000000000000.00",
      "3.0000004": "3.00",
      "24.0": "24.00",
      "64.0": "64.00",
      "0.0": "0.00",
      "-0.0": "-0.00",
      "59.0": "59.00",
      "59.00000000000001": "59.00",
      "185.9": "185.90",
      "2.5e-08": "0.00",
      "9.99999e-05": "0.00",
      "1.000005": "1.00",
      "0.125": "0.12",
      "3.14159": "3.14",
      "1000.5": "1000.50",
      "777777.0": "777777.00"
     }
    }"#;
    const MD_HTML_JSON_CASES: &str = r#"[
     {
      "src": "<a>",
      "json": "\"\\u003ca>\"",
      "attr": "&lt;a&gt;"
     },
     {
      "src": "中文",
      "json": "\"中文\"",
      "attr": "中文"
     },
     {
      "src": "\t\n\r\b\f\u0000\u0001\u001f",
      "json": "\"\\t\\n\\r\\b\\f\\u0000\\u0001\\u001f\"",
      "attr": "\t\n\r\b\f\u0000\u0001\u001f"
     },
     {
      "src": "\"\\/'",
      "json": "\"\\\"\\\\/'\"",
      "attr": "&quot;\\/'"
     },
     {
      "src": "  ",
      "json": "\"  \"",
      "attr": "  "
     },
     {
      "src": "😀",
      "json": "\"😀\"",
      "attr": "😀"
     },
     {
      "src": "a<b>c&d",
      "json": "\"a\\u003cb>c&d\"",
      "attr": "a&lt;b&gt;c&amp;d"
     },
     {
      "src": "</script>",
      "json": "\"\\u003c/script>\"",
      "attr": "&lt;/script&gt;"
     },
     {
      "src": "﻿",
      "json": "\"﻿\"",
      "attr": "﻿"
     },
     {
      "src": "",
      "json": "\"\"",
      "attr": ""
     },
     {
      "src": " ",
      "json": "\" \"",
      "attr": " "
     },
     {
      "src": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
      "json": "\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"",
      "attr": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
     },
     {
      "src": "line1\nline2\r\n",
      "json": "\"line1\\nline2\\r\\n\"",
      "attr": "line1\nline2\r\n"
     },
     {
      "src": "\\",
      "json": "\"\\\\\"",
      "attr": "\\"
     },
     {
      "src": "  ",
      "json": "\"  \"",
      "attr": "  "
     },
     {
      "src": "ab",
      "json": "\"ab\"",
      "attr": "ab"
     }
    ]"#;
    const MD_HTML_IMG_CASES: &str = r#"{
     "files": {
      "a.png": "iVBORw0KGgogdGlueQ==",
      "sub/b.gif": "R0lGODlhLi4uLg==",
      "UP.PNG": "iVBORw0KGgogdXBwZXI=",
      "图.png": "iVBORw0KGgogY2pr",
      "notes.txt": "bm90IGFuIGltYWdl",
      "a.png.gz": "H4sgZ3ppcHBlZA==",
      "empty.png": ""
     },
     "cases": [
      {
       "md": "![x](a.png)",
       "embedded": "![x](data:image/png;base64,iVBORw0KGgogdGlueQ==)",
       "warns": []
      },
      {
       "md": "![alt text](sub/b.gif)",
       "embedded": "![alt text](data:image/gif;base64,R0lGODlhLi4uLg==)",
       "warns": []
      },
      {
       "md": "![missing](nope.png)",
       "embedded": "![missing](nope.png)",
       "warns": [
        "图片不存在，已跳过：nope.png"
       ]
      },
      {
       "md": "![t](notes.txt)",
       "embedded": "![t](notes.txt)",
       "warns": []
      },
      {
       "md": "![r](https://cdn.example/y.png)",
       "embedded": "![r](https://cdn.example/y.png)",
       "warns": [
        "远程/内联图片不支持嵌入，已跳过：https://cdn.example/y.png"
       ]
      },
      {
       "md": "![r2](http://cdn.example/y.png)",
       "embedded": "![r2](http://cdn.example/y.png)",
       "warns": [
        "远程/内联图片不支持嵌入，已跳过：http://cdn.example/y.png"
       ]
      },
      {
       "md": "![d](data:image/png;base64,QUJD)",
       "embedded": "![d](data:image/png;base64,QUJD)",
       "warns": []
      },
      {
       "md": "![d2](data:image/png;base64,QQ==)",
       "embedded": "![d2](data:image/png;base64,QQ==)",
       "warns": []
      },
      {
       "md": "![d3](data:image/jpeg;base64,8A==)",
       "embedded": "![d3](data:image/jpeg;base64,8A==)",
       "warns": []
      },
      {
       "md": "![dbad](data:image/png;base64,QUJDR)",
       "embedded": "![dbad](data:image/png;base64,QUJDR)",
       "warns": [
        "远程/内联图片不支持嵌入，已跳过：data:image/png;base64,QUJDR"
       ]
      },
      {
       "md": "![dws](data:image/png;base64,QU JD)",
       "embedded": "![dws](data:image/png;base64,QU JD)",
       "warns": []
      },
      {
       "md": "![dsvg](data:image/svg+xml;base64,QUJD)",
       "embedded": "![dsvg](data:image/svg+xml;base64,QUJD)",
       "warns": [
        "远程/内联图片不支持嵌入，已跳过：data:image/svg+xml;base64,QUJD"
       ]
      },
      {
       "md": "![dpdf](data:application/pdf;base64,QUJD)",
       "embedded": "![dpdf](data:application/pdf;base64,QUJD)",
       "warns": [
        "远程/内联图片不支持嵌入，已跳过：data:application/pdf;base64,QUJD"
       ]
      },
      {
       "md": "![de](data:image/png;base64,)",
       "embedded": "![de](data:image/png;base64,)",
       "warns": [
        "远程/内联图片不支持嵌入，已跳过：data:image/png;base64,"
       ]
      },
      {
       "md": "![empty](empty.png)",
       "embedded": "![empty](data:image/png;base64,)",
       "warns": []
      },
      {
       "md": "![pct](%E5%9B%BE.png)",
       "embedded": "![pct](data:image/png;base64,iVBORw0KGgogY2pr)",
       "warns": []
      },
      {
       "md": "![up](UP.PNG)",
       "embedded": "![up](data:image/png;base64,iVBORw0KGgogdXBwZXI=)",
       "warns": []
      },
      {
       "md": "![gz](a.png.gz)",
       "embedded": "![gz](data:image/png;base64,H4sgZ3ppcHBlZA==)",
       "warns": []
      },
      {
       "md": "![title](a.png \"t\")",
       "embedded": "![title](a.png \"t\")",
       "warns": []
      },
      {
       "md": "![](a.png)",
       "embedded": "![](data:image/png;base64,iVBORw0KGgogdGlueQ==)",
       "warns": []
      },
      {
       "md": "![x]()",
       "embedded": "![x]()",
       "warns": []
      },
      {
       "md": "![x](a\u001cb.png)",
       "embedded": "![x](a\u001cb.png)",
       "warns": []
      },
      {
       "md": "![x](a.png)\n\nagain ![x](a.png)\n\nand ![missing](nope.png) ![missing again](nope.png)",
       "embedded": "![x](data:image/png;base64,iVBORw0KGgogdGlueQ==)\n\nagain ![x](data:image/png;base64,iVBORw0KGgogdGlueQ==)\n\nand ![missing](nope.png) ![missing again](nope.png)",
       "warns": [
        "图片不存在，已跳过：nope.png"
       ]
      },
      {
       "md": "text ![inline](a.png) text ![other](sub/b.gif)",
       "embedded": "text ![inline](data:image/png;base64,iVBORw0KGgogdGlueQ==) text ![other](data:image/gif;base64,R0lGODlhLi4uLg==)",
       "warns": []
      },
      {
       "md": "![abs](a.png) ![rel](./a.png) ![dotdot](sub/../a.png)",
       "embedded": "![abs](data:image/png;base64,iVBORw0KGgogdGlueQ==) ![rel](data:image/png;base64,iVBORw0KGgogdGlueQ==) ![dotdot](data:image/png;base64,iVBORw0KGgogdGlueQ==)",
       "warns": []
      },
      {
       "md": "not an image [a](b.png) or !broken[a](a.png)",
       "embedded": "not an image [a](b.png) or !broken[a](a.png)",
       "warns": []
      },
      {
       "md": "![multi](a.png) trailing ) paren",
       "embedded": "![multi](data:image/png;base64,iVBORw0KGgogdGlueQ==) trailing ) paren",
       "warns": []
      }
     ]
    }"#;
    const MD_HTML_ASSET_CASES: &str = r#"[
     {
      "name": "empty_tree",
      "files": {},
      "dirs": [],
      "lookup": "marked.min.js",
      "text": ""
     },
     {
      "name": "vendor_is_a_file",
      "files": {
       "vendor": "not a directory"
      },
      "dirs": [],
      "lookup": "marked.min.js",
      "text": ""
     },
     {
      "name": "first_base_is_a_dir",
      "files": {
       "vendor/marked.min.js": "SHOULD NOT BE READ"
      },
      "dirs": [
       "marked.min.js"
      ],
      "lookup": "marked.min.js",
      "text": ""
     },
     {
      "name": "second_base",
      "files": {
       "vendor/mathjax/tex-svg.js": "MathJax();\n"
      },
      "dirs": [],
      "lookup": "mathjax/tex-svg.js",
      "text": "MathJax();\n"
     },
     {
      "name": "first_base_wins",
      "files": {
       "a.js": "first;",
       "vendor/a.js": "second;"
      },
      "dirs": [],
      "lookup": "a.js",
      "text": "first;"
     }
    ]"#;
    fn md_html_cases(payload: &str) -> Value {
        serde_json::from_str(payload).expect("python golden payload is valid JSON")
    }
    /// `styles.sanitize(options)` + `html_render._build_css` over 28 option
    /// fixtures: themes, presets, clamped junk, `_clamp` defaults that differ from
    /// the schema defaults (`before`/`after`/`color`), the `page_dimensions`
    /// margin-pair rescale, `Custom`/landscape sizes, `%.2f` borders, `%g` sizes and
    /// the `pageBreakBefore` / `banded` switches.
    #[test]
    fn test_build_export_css_matches_python_goldens() {
        let cases = md_html_cases(MD_HTML_CSS_CASES);
        let cases = cases.as_array().unwrap();
        assert!(cases.len() >= 20, "expected a broad css fixture set, got {}", cases.len());
        let mut checked = 0usize;
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let css = build_export_css(&case["options"]);
            assert_eq!(css, case["css"].as_str().unwrap(), "css differs for fixture {}", name);
            checked += 1;
        }
        assert_eq!(checked, cases.len());
    }
    /// `html_render.render()` end to end: template, substitution order, asset
    /// warnings, `newline='\n'` write.
    #[test]
    fn test_export_html_matches_python_render() {
        for case in md_html_cases(MD_HTML_DOC_CASES).as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let assets = dir.path().join("assets");
            fs::create_dir_all(&assets).unwrap();
            if let Some(obj) = case["assets"].as_object() {
                for (rel, text) in obj {
                    let full = assets.join(rel);
                    if let Some(parent) = full.parent() {
                        fs::create_dir_all(parent).unwrap();
                    }
                    fs::write(&full, text.as_str().unwrap().as_bytes()).unwrap();
                }
            }
            let out = dir.path().join("out.html");
            let res = export_html(
                case["md"].as_str().unwrap(),
                "",
                out.to_str().unwrap(),
                &case["options"],
                case["source_name"].as_str().unwrap(),
                &assets,
            )
            .unwrap_or_else(|e| panic!("export_html failed for {}: {}", name, e));
            assert!(res.ok, "{name}");
            let produced = fs::read(&out).unwrap();
            let expected = case["html"].as_str().unwrap().as_bytes();
            assert_eq!(produced.len(), expected.len(), "html length differs for {}", name);
            assert_eq!(
                String::from_utf8_lossy(&produced).as_ref(),
                case["html"].as_str().unwrap(),
                "html differs for {}", name
            );
            let warns = res.warns.unwrap_or_default();
            assert_eq!(warns, case["warns"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect::<Vec<_>>().as_slice(), "warns differ for {}", name);
        }
    }
    /// `render`'s `title = style['meta'].get('title') or source_name or 'ReadMD 导出'`
    /// after `str(meta['title'] or '')[:120]`, then `_esc_attr` (which escapes `"` but
    /// not `'`).
    #[test]
    fn test_export_html_title_cases() {
        for case in md_html_cases(MD_HTML_TITLE_CASES).as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let out = dir.path().join("t.html");
            export_html(
                "x",
                "",
                out.to_str().unwrap(),
                &case["options"],
                case["source_name"].as_str().unwrap(),
                dir.path(),
            )
            .unwrap_or_else(|e| panic!("title case {}: {}", name, e));
            let html = fs::read_to_string(&out).unwrap();
            let title = html
                .split("<title>")
                .nth(1)
                .unwrap()
                .split("</title>")
                .next()
                .unwrap();
            assert_eq!(title, case["title"].as_str().unwrap(), "title differs for {}", name);
        }
    }
    /// CPython `'%g' % v` (precision 6) and `'%.2f' % v` over the numeric domain the
    /// style clamps can produce.
    #[test]
    fn test_printf_goldens_match_cpython() {
        let payload = md_html_cases(MD_HTML_G_CASES);
        for (kind, f) in [("g", css_g as fn(f64) -> String), ("f2", css_f2)] {
            let obj = payload[kind].as_object().unwrap();
            for (literal, expected) in obj {
                let value: f64 = literal.parse().unwrap_or_else(|e| panic!("{} key {}: {}", kind, literal, e));
                assert_eq!(f(value), expected.as_str().unwrap(), "{} of {}", kind, literal);
            }
            assert!(!obj.is_empty());
        }
    }
    /// `json.dumps(s, ensure_ascii=False).replace('<', '\\u003c')` and `_esc_attr`.
    #[test]
    fn test_json_and_attribute_escaping_match_python() {
        for (i, case) in md_html_cases(MD_HTML_JSON_CASES).as_array().unwrap().iter().enumerate() {
            let src = case["src"].as_str().unwrap();
            assert_eq!(json_dump_escaped(src), case["json"].as_str().unwrap(), "json escape #{}", i);
            assert_eq!(esc_attr(src), case["attr"].as_str().unwrap(), "esc_attr #{}", i);
        }
    }
    /// `ImageResolver.resolve` + the `html` branch's `embed_image`: cached misses
    /// (one warning per missing source), remote/inline skips, `percent-decoded`
    /// names, non-image extensions, the 24 MiB gate, `data:` decode failures and the
    /// `[^)\s]` class that excludes `U+001C`-`U+001F`.
    #[test]
    fn test_image_embedding_matches_python_resolver() {
        let payload = md_html_cases(MD_HTML_IMG_CASES);
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("doc");
        for (rel, b64) in payload["files"].as_object().unwrap() {
            let full = base.join(rel);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, base64::engine::general_purpose::STANDARD.decode(b64.as_str().unwrap()).unwrap()).unwrap();
        }
        for case in payload["cases"].as_array().unwrap() {
            let mut warns: Vec<String> = Vec::new();
            let out = embed_images_for_html(case["md"].as_str().unwrap(), &base.to_string_lossy(), &mut warns);
            assert_eq!(out, case["embedded"].as_str().unwrap(), "embedded md differs");
            let expected: Vec<String> = case["warns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w.as_str().unwrap().to_string())
                .collect();
            assert_eq!(warns, expected, "warns differ for {:?}", case["md"].as_str().unwrap());
        }
    }
    /// `_read_asset`: two bases only, first *existing* path wins, and a directory
    /// (or undecodable file) yields `''` without falling through to `vendor/`.
    #[test]
    fn test_read_asset_matches_python() {
        for case in md_html_cases(MD_HTML_ASSET_CASES).as_array().unwrap() {
            let dir = tempfile::tempdir().unwrap();
            for d in case["dirs"].as_array().unwrap() {
                fs::create_dir_all(dir.path().join(d.as_str().unwrap())).unwrap();
            }
            for (rel, text) in case["files"].as_object().unwrap() {
                let full = dir.path().join(rel);
                fs::create_dir_all(full.parent().unwrap()).unwrap();
                fs::write(&full, text.as_str().unwrap().as_bytes()).unwrap();
            }
            assert_eq!(
                read_asset_file(dir.path(), case["lookup"].as_str().unwrap()),
                case["text"].as_str().unwrap(),
                "asset read differs for {:?}",
                case["name"].as_str().unwrap()
            );
        }
    }
    /// `binascii.a2b_base64(..., strict_mode=True)`: the caller's regex lets padding
    /// and whitespace through, so every strict-mode rejection has to fail exactly
    /// where Python fails (`data:image/png;base64,QUJDR` is the shipped example).
    #[test]
    fn test_strict_base64_matches_binascii() {
        let ok: [(&str, &[u8]); 8] = [
            ("", b""),
            ("QUJD", b"ABC"),
            ("QQ==", b"A"),
            ("QUI=", b"AB"),
            ("/+8=", &[0xff, 0xef]),
            ("QUJDRA==", b"ABCD"),
            ("QUJD==", b"ABC"), // trailing padding after a whole quad is tolerated
            ("QUJD=", b"ABC"),
        ];
        for (src, want) in ok {
            assert_eq!(a2b_base64_strict(src).as_deref(), Some(want), "a2b({:?})", src);
        }
        for src in ["QQ", "QQ=", "QUJDR", "A", "Q===", "QQQ==", "=QQ", "QQ==QQ==", "QUJD\x20", "QU.JD", "AAAAAA=", "QQ== QQ=="] {
            assert!(a2b_base64_strict(src).is_none(), "a2b({:?}) must be rejected", src);
        }
    }
}
