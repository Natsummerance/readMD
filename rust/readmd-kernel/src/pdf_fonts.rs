//! Embedded TrueType fonts for the native PDF writer.
//!
//! Text the base-14 faces cannot encode (CJK, Greek, arrows, check boxes…) is
//! drawn with a system TrueType font that is *subset* into the PDF: only the
//! glyphs a document uses are copied (composite components included), renumbered
//! from 0, and written as `/CIDFontType2` + `/Identity-H` with an exact `/W`
//! array and a `/ToUnicode` CMap, so the PDF looks the same on every machine
//! and copy/search return the original text.
//!
//! Font files are only read once a document actually needs a glyph from them,
//! so an English-only export never touches the 20 MB CJK font.  Only
//! glyf-outlined fonts that permit embedding and subsetting are used; when none
//! is available the caller falls back to the reader-supplied `STSong-Light`
//! and reports it.
//!
//! State lives in a thread-local context bracketed by [`begin`] / [`finish`],
//! because measurement happens deep inside free functions of the layout code.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use ttf_parser::{GlyphId, Tag};

/// Where a candidate font lives (`index` = face inside a `.ttc`).
#[derive(Debug, Clone, PartialEq)]
pub struct FontSource {
    pub path: PathBuf,
    pub index: u32,
}

impl FontSource {
    fn new(p: impl Into<PathBuf>, index: u32) -> Self {
        FontSource { path: p.into(), index }
    }
}

fn font_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if cfg!(windows) {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        v.push(Path::new(&windir).join("Fonts"));
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            v.push(Path::new(&local).join("Microsoft").join("Windows").join("Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        v.push("/System/Library/Fonts".into());
        v.push("/System/Library/Fonts/Supplemental".into());
        v.push("/Library/Fonts".into());
        if let Ok(home) = std::env::var("HOME") {
            v.push(Path::new(&home).join("Library/Fonts"));
        }
    } else {
        for d in [
            "/usr/share/fonts/truetype/wqy",
            "/usr/share/fonts/wenquanyi/wqy-microhei",
            "/usr/share/fonts/wenquanyi/wqy-zenhei",
            "/usr/share/fonts/wqy-microhei",
            "/usr/share/fonts/wqy-zenhei",
            "/usr/share/fonts/truetype/droid",
            "/usr/share/fonts/google-droid",
            "/usr/share/fonts/truetype/noto",
            "/usr/share/fonts/noto",
            "/usr/share/fonts/truetype/arphic",
            "/usr/share/fonts/truetype/dejavu",
            "/usr/share/fonts/dejavu",
            "/usr/share/fonts/TTF",
        ] {
            v.push(d.into());
        }
        if let Ok(home) = std::env::var("HOME") {
            v.push(Path::new(&home).join(".local/share/fonts"));
            v.push(Path::new(&home).join(".fonts"));
        }
    }
    v
}

fn find(files: &[(&str, u32)]) -> Vec<FontSource> {
    let dirs = font_dirs();
    let mut out = Vec::new();
    for (f, idx) in files {
        for d in &dirs {
            let p = d.join(f);
            if p.is_file() {
                out.push(FontSource::new(p, *idx));
                break;
            }
        }
    }
    out
}

/// Regular and bold CJK candidates for the export font family, best first.
pub fn cjk_candidates(family: &str) -> (Vec<FontSource>, Vec<FontSource>) {
    let (mut reg, mut bold): (Vec<(&str, u32)>, Vec<(&str, u32)>) = match family {
        "SimHei" => (vec![("simhei.ttf", 0)], vec![]),
        "SimSun" => (vec![("simsun.ttc", 0)], vec![]),
        "KaiTi" => (vec![("simkai.ttf", 0)], vec![]),
        "DengXian" => (vec![("Deng.ttf", 0)], vec![("Dengb.ttf", 0)]),
        _ => (vec![("msyh.ttc", 0)], vec![("msyhbd.ttc", 0)]),
    };
    // Shared fallback ladder: Windows, macOS, Linux.
    reg.extend_from_slice(&[
        ("msyh.ttc", 0),
        ("simhei.ttf", 0),
        ("Deng.ttf", 0),
        ("simsun.ttc", 0),
        ("PingFang.ttc", 0),
        ("STHeiti Light.ttc", 0),
        ("Songti.ttc", 0),
        ("Arial Unicode.ttf", 0),
        ("wqy-microhei.ttc", 0),
        ("wqy-zenhei.ttc", 0),
        ("NotoSansSC-Regular.ttf", 0),
        ("DroidSansFallbackFull.ttf", 0),
        ("DroidSansFallback.ttf", 0),
        ("uming.ttc", 0),
    ]);
    bold.extend_from_slice(&[("msyhbd.ttc", 0), ("STHeiti Medium.ttc", 0), ("NotoSansSC-Bold.ttf", 0)]);
    (dedupe(find(&reg)), dedupe(find(&bold)))
}

/// Symbol coverage the CJK fonts lack (☐ ☑ ✓ ★ arrows, math operators).
pub fn symbol_candidates() -> Vec<FontSource> {
    dedupe(find(&[("seguisym.ttf", 0), ("segoeui.ttf", 0), ("DejaVuSans.ttf", 0), ("Apple Symbols.ttf", 0)]))
}

fn dedupe(v: Vec<FontSource>) -> Vec<FontSource> {
    let mut out: Vec<FontSource> = Vec::new();
    for s in v {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

// ------------------------------------------------------------------ slots

struct Loaded {
    data: Arc<Vec<u8>>,
    index: u32,
    upem: u16,
    ascent: i16,
    descent: i16,
    cap: i16,
    bbox: (i16, i16, i16, i16),
    ps_name: String,
    cmap: HashMap<char, Option<(u16, u16)>>, // char -> (old gid, advance)
    new_of_old: HashMap<u16, u16>,
    olds: Vec<u16>,
    uni: Vec<String>,
}

enum SlotState {
    Pending(Vec<FontSource>),
    Ready(Box<Loaded>),
    Unavailable,
}

struct Slot {
    state: SlotState,
}

impl Slot {
    fn new(cands: Vec<FontSource>) -> Slot {
        Slot { state: if cands.is_empty() { SlotState::Unavailable } else { SlotState::Pending(cands) } }
    }

    fn loaded(&mut self) -> Option<&mut Loaded> {
        if let SlotState::Pending(c) = &self.state {
            let cands = c.clone();
            self.state = cands.iter().find_map(load).map(|l| SlotState::Ready(Box::new(l))).unwrap_or(SlotState::Unavailable);
        }
        match &mut self.state {
            SlotState::Ready(l) => Some(l),
            _ => None,
        }
    }
}

fn load(src: &FontSource) -> Option<Loaded> {
    let data = std::fs::read(&src.path).ok()?;
    let data = Arc::new(data);
    let face = ttf_parser::Face::parse(&data, src.index).ok()?;
    let raw = face.raw_face();
    if raw.table(Tag::from_bytes(b"glyf")).is_none() || raw.table(Tag::from_bytes(b"loca")).is_none() {
        return None;
    }
    if matches!(face.permissions(), Some(ttf_parser::Permissions::Restricted)) || !face.is_subsetting_allowed() {
        return None;
    }
    let ps_name = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .find_map(|n| n.to_string())
        .unwrap_or_else(|| "ReadMDFont".into());
    let ps_name: String = ps_name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(48).collect();
    let bb = face.global_bounding_box();
    let upem = face.units_per_em();
    let mut l = Loaded {
        index: src.index,
        upem,
        ascent: face.ascender(),
        descent: face.descender(),
        cap: face.capital_height().unwrap_or(face.ascender()),
        bbox: (bb.x_min, bb.y_min, bb.x_max, bb.y_max),
        ps_name: if ps_name.is_empty() { "ReadMDFont".into() } else { ps_name },
        cmap: HashMap::new(),
        new_of_old: HashMap::new(),
        olds: Vec::new(),
        uni: Vec::new(),
        data: data.clone(),
    };
    // .notdef keeps id 0.
    l.new_of_old.insert(0, 0);
    l.olds.push(0);
    l.uni.push(String::new());
    Some(l)
}

impl Loaded {
    fn lookup(&mut self, c: char) -> Option<(u16, u16)> {
        if let Some(hit) = self.cmap.get(&c) {
            return *hit;
        }
        let face = ttf_parser::Face::parse(&self.data, self.index).ok();
        let got = face.and_then(|f| {
            let g = f.glyph_index(c)?;
            if g.0 == 0 {
                return None;
            }
            Some((g.0, f.glyph_hor_advance(g).unwrap_or(self.upem)))
        });
        self.cmap.insert(c, got);
        got
    }

    fn use_gid(&mut self, old: u16, text: &str) -> u16 {
        if let Some(n) = self.new_of_old.get(&old) {
            let n = *n;
            if self.uni[n as usize].is_empty() && !text.is_empty() {
                self.uni[n as usize] = text.to_string();
            }
            return n;
        }
        let n = self.olds.len() as u16;
        self.new_of_old.insert(old, n);
        self.olds.push(old);
        self.uni.push(text.to_string());
        n
    }

    fn scale(&self, v: f64) -> f64 {
        v * 1000.0 / self.upem.max(1) as f64
    }
}

// ------------------------------------------------------------------ context

/// Faces handed to the PDF layer: `Face::Embedded(id)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceInfo {
    pub slot: usize,
    /// Drawn with fill+stroke (no real bold file on this machine).
    pub fake_bold: bool,
}

struct Ctx {
    slots: Vec<Slot>, // 0 = regular, 1 = bold, 2 = symbols
    faces: Vec<FaceInfo>,
    missing: BTreeSet<char>,
    fallback_used: bool,
}

const REGULAR: usize = 0;
const BOLD: usize = 1;
const SYMBOL: usize = 2;

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

/// Start an export using `family` (the style's `typography.font`).
pub fn begin(family: &str) {
    let (reg, bold) = cjk_candidates(family);
    begin_with(reg, bold, symbol_candidates());
}

/// [`begin`] with explicit candidates (tests, custom font settings).
pub fn begin_with(regular: Vec<FontSource>, bold: Vec<FontSource>, symbols: Vec<FontSource>) {
    CTX.with(|c| {
        *c.borrow_mut() = Some(Ctx {
            slots: vec![Slot::new(regular), Slot::new(bold), Slot::new(symbols)],
            faces: Vec::new(),
            missing: BTreeSet::new(),
            fallback_used: false,
        })
    });
}

fn with_ctx<R>(f: impl FnOnce(&mut Ctx) -> R) -> Option<R> {
    CTX.with(|c| c.borrow_mut().as_mut().map(f))
}

impl Ctx {
    /// Slot order to try for one character.
    fn order(&self, bold: bool) -> [(usize, bool); 3] {
        if bold {
            [(BOLD, false), (REGULAR, true), (SYMBOL, false)]
        } else {
            [(REGULAR, false), (SYMBOL, false), (SYMBOL, false)]
        }
    }

    fn resolve(&mut self, c: char, bold: bool) -> Option<(usize, bool, u16, f64)> {
        for (slot, fake) in self.order(bold) {
            if let Some(l) = self.slots[slot].loaded() {
                if let Some((gid, adv)) = l.lookup(c) {
                    let fake = fake || (bold && slot == SYMBOL);
                    return Some((slot, fake, gid, l.scale(adv as f64)));
                }
            }
        }
        None
    }

    fn face_id(&mut self, slot: usize, fake_bold: bool) -> u8 {
        let info = FaceInfo { slot, fake_bold };
        match self.faces.iter().position(|f| *f == info) {
            Some(i) => i as u8,
            None => {
                self.faces.push(info);
                (self.faces.len() - 1) as u8
            }
        }
    }
}

/// Advance of `c` at `size`, or `None` when no embedded font has it (or no
/// export is in progress).
pub fn advance(c: char, bold: bool, size: f64) -> Option<f64> {
    with_ctx(|ctx| ctx.resolve(c, bold).map(|(_, _, _, w)| w * size / 1000.0)).flatten()
}

/// One drawable piece of a non-WinAnsi run.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// `Some(face id)` for an embedded face; `None` = fall back to STSong-Light.
    pub face: Option<u8>,
    pub text: String,
    /// Content-stream bytes: 2-byte glyph ids, or UTF-16BE for the fallback.
    pub bytes: Vec<u8>,
    pub advance: f64,
}

/// Split `text` into embedded-font pieces and register the glyphs as used.
pub fn encode(text: &str, bold: bool, size: f64) -> Vec<Piece> {
    let got = with_ctx(|ctx| {
        let mut out: Vec<Piece> = Vec::new();
        for c in text.chars() {
            let (face, bytes, adv) = match ctx.resolve(c, bold) {
                Some((slot, fake, gid, w)) => {
                    let s = c.to_string();
                    let new = ctx.slots[slot].loaded().map(|l| l.use_gid(gid, &s)).unwrap_or(0);
                    (Some(ctx.face_id(slot, fake)), new.to_be_bytes().to_vec(), w * size / 1000.0)
                }
                None => {
                    if !c.is_whitespace() && !c.is_control() {
                        ctx.missing.insert(c);
                    }
                    ctx.fallback_used = true;
                    let mut b = Vec::new();
                    let mut buf = [0u16; 2];
                    for u in c.encode_utf16(&mut buf) {
                        b.extend_from_slice(&u.to_be_bytes());
                    }
                    (None, b, size)
                }
            };
            match out.last_mut() {
                Some(p) if p.face == face => {
                    p.text.push(c);
                    p.bytes.extend_from_slice(&bytes);
                    p.advance += adv;
                }
                _ => out.push(Piece { face, text: c.to_string(), bytes, advance: adv }),
            }
        }
        out
    });
    got.unwrap_or_else(|| {
        let mut b = Vec::new();
        for u in text.encode_utf16() {
            b.extend_from_slice(&u.to_be_bytes());
        }
        vec![Piece { face: None, text: text.to_string(), bytes: b, advance: text.chars().count() as f64 * size }]
    })
}

// ------------------------------------------------------------------ output

/// A finished subset, ready for the PDF writer.
#[derive(Clone)]
pub struct SubsetFont {
    pub base_name: String,
    /// The subset `.ttf` (uncompressed).
    pub file: Vec<u8>,
    /// Advance per glyph id, in 1/1000 em.
    pub widths: Vec<i64>,
    /// Text per glyph id for `/ToUnicode` (empty = no mapping).
    pub unicode: Vec<String>,
    pub ascent: i64,
    pub descent: i64,
    pub cap_height: i64,
    pub bbox: [i64; 4],
}

impl std::fmt::Debug for SubsetFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SubsetFont({}, {} glyphs, {} bytes)", self.base_name, self.widths.len(), self.file.len())
    }
}

#[derive(Debug, Clone, Default)]
pub struct EmbeddedSet {
    /// Per slot that produced glyphs (`None` for unused slots).
    pub fonts: Vec<Option<SubsetFont>>,
    pub faces: Vec<FaceInfo>,
}

/// End the export: build the subsets and return them with any warnings.
pub fn finish() -> (EmbeddedSet, Vec<String>) {
    let ctx = CTX.with(|c| c.borrow_mut().take());
    let Some(mut ctx) = ctx else { return (EmbeddedSet::default(), Vec::new()) };
    let mut warns = Vec::new();
    let mut fonts = Vec::new();
    for slot in ctx.slots.iter_mut() {
        let sub = match &mut slot.state {
            SlotState::Ready(l) if l.olds.len() > 1 => subset(l),
            _ => None,
        };
        fonts.push(sub);
    }
    let regular_ok = matches!(ctx.slots[REGULAR].state, SlotState::Ready(_));
    if ctx.fallback_used && !regular_ok {
        warns.push("未找到可嵌入的 CJK 字体，非拉丁文字使用阅读器内置字体（STSong-Light）显示，其他电脑上可能不一致".to_string());
    } else if !ctx.missing.is_empty() {
        let list: String = ctx.missing.iter().take(24).collect();
        let more = if ctx.missing.len() > 24 { "…" } else { "" };
        warns.push(format!("嵌入字体缺少以下字符，已使用阅读器内置字体：{list}{more}"));
    }
    (EmbeddedSet { fonts, faces: ctx.faces }, warns)
}

fn rd16(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}

fn rd32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// Component glyph ids of a composite glyph and the byte offsets holding them.
fn components(g: &[u8]) -> Vec<(usize, u16)> {
    let mut out = Vec::new();
    if g.len() < 10 || (rd16(g, 0).unwrap_or(0) as i16) >= 0 {
        return out;
    }
    let mut pos = 10usize;
    loop {
        let (Some(flags), Some(gid)) = (rd16(g, pos), rd16(g, pos + 2)) else { break };
        out.push((pos + 2, gid));
        pos += 4;
        pos += if flags & 0x0001 != 0 { 4 } else { 2 };
        if flags & 0x0008 != 0 {
            pos += 2;
        } else if flags & 0x0040 != 0 {
            pos += 4;
        } else if flags & 0x0080 != 0 {
            pos += 8;
        }
        if flags & 0x0020 == 0 || out.len() > 64 {
            break;
        }
    }
    out
}

fn subset(l: &mut Loaded) -> Option<SubsetFont> {
    let data = l.data.clone();
    let face = ttf_parser::Face::parse(&data, l.index).ok()?;
    let raw = face.raw_face();
    let table = |t: &[u8; 4]| raw.table(Tag::from_bytes(t));
    let head = table(b"head")?;
    let loca = table(b"loca")?;
    let glyf = table(b"glyf")?;
    let hhea = table(b"hhea")?;
    let maxp = table(b"maxp")?;
    let long_loca = rd16(head, 50)? == 1;
    let n_orig = face.number_of_glyphs() as usize;
    let glyph_range = |gid: u16| -> Option<(usize, usize)> {
        let i = gid as usize;
        if i >= n_orig {
            return None;
        }
        let (a, b) = if long_loca {
            (rd32(loca, i * 4)? as usize, rd32(loca, i * 4 + 4)? as usize)
        } else {
            (rd16(loca, i * 2)? as usize * 2, rd16(loca, i * 2 + 2)? as usize * 2)
        };
        (a <= b && b <= glyf.len()).then_some((a, b))
    };

    // Pull in composite components (they get ids after the drawn glyphs).
    let mut i = 0usize;
    while i < l.olds.len() && l.olds.len() < 65_000 {
        if let Some((a, b)) = glyph_range(l.olds[i]) {
            for (_, comp) in components(&glyf[a..b]) {
                l.use_gid(comp, "");
            }
        }
        i += 1;
    }

    let n = l.olds.len();
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_loca: Vec<u8> = Vec::with_capacity((n + 1) * 4);
    let mut hmtx: Vec<u8> = Vec::with_capacity(n * 4);
    let mut widths = Vec::with_capacity(n);
    for &old in &l.olds {
        new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        if let Some((a, b)) = glyph_range(old) {
            let mut g = glyf[a..b].to_vec();
            for (off, comp) in components(&g) {
                let nid = *l.new_of_old.get(&comp).unwrap_or(&0);
                g[off..off + 2].copy_from_slice(&nid.to_be_bytes());
            }
            new_glyf.extend_from_slice(&g);
            while new_glyf.len() % 4 != 0 {
                new_glyf.push(0);
            }
        }
        let adv = face.glyph_hor_advance(GlyphId(old)).unwrap_or(0);
        let lsb = face.glyph_hor_side_bearing(GlyphId(old)).unwrap_or(0);
        hmtx.extend_from_slice(&adv.to_be_bytes());
        hmtx.extend_from_slice(&lsb.to_be_bytes());
        widths.push(l.scale(adv as f64).round() as i64);
    }
    new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());

    let mut head2 = head.to_vec();
    head2.get_mut(8..12)?.copy_from_slice(&[0; 4]);
    head2.get_mut(50..52)?.copy_from_slice(&1u16.to_be_bytes());
    let mut hhea2 = hhea.to_vec();
    hhea2.get_mut(34..36)?.copy_from_slice(&(n as u16).to_be_bytes());
    let mut maxp2 = maxp.to_vec();
    maxp2.get_mut(4..6)?.copy_from_slice(&(n as u16).to_be_bytes());
    // post 3.0: no glyph names.
    let mut post = vec![0u8; 32];
    post[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
    if let Some(p) = table(b"post") {
        if p.len() >= 32 {
            post[4..32].copy_from_slice(&p[4..32]);
        }
    }

    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"glyf", new_glyf),
        (*b"head", head2),
        (*b"hhea", hhea2),
        (*b"hmtx", hmtx),
        (*b"loca", new_loca),
        (*b"maxp", maxp2),
        (*b"post", post),
    ];
    for t in [b"cvt ", b"fpgm", b"prep"] {
        if let Some(d) = table(t) {
            tables.push((*t, d.to_vec()));
        }
    }
    tables.sort_by(|a, b| a.0.cmp(&b.0));
    let file = write_sfnt(&tables);

    // Deterministic 6-letter subset tag from the glyph set.
    let mut h: u32 = 0x811c_9dc5;
    for g in &l.olds {
        for b in g.to_be_bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
    }
    let tag: String = (0..6).map(|k| (b'A' + ((h >> (k * 5)) % 26) as u8) as char).collect();
    let s = |v: i16| l.scale(v as f64).round() as i64;
    Some(SubsetFont {
        base_name: format!("{tag}+{}", l.ps_name),
        file,
        widths,
        unicode: l.uni.clone(),
        ascent: s(l.ascent),
        descent: s(l.descent),
        cap_height: s(l.cap),
        bbox: [s(l.bbox.0), s(l.bbox.1), s(l.bbox.2), s(l.bbox.3)],
    })
}

fn checksum(d: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for ch in d.chunks(4) {
        let mut w = [0u8; 4];
        w[..ch.len()].copy_from_slice(ch);
        sum = sum.wrapping_add(u32::from_be_bytes(w));
    }
    sum
}

fn write_sfnt(tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n * 16 - pow * 16).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    let mut head_at = None;
    for (tag, d) in tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(d).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(d.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_at = Some(offset);
        }
        body.extend_from_slice(d);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = 12 + 16 * tables.len() + body.len();
    }
    out.extend_from_slice(&body);
    if let Some(h) = head_at {
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[h + 8..h + 12].copy_from_slice(&adj.to_be_bytes());
    }
    out
}

/// `/ToUnicode` CMap for glyph ids `0..unicode.len()`.
pub fn to_unicode_cmap(unicode: &[String]) -> String {
    let entries: Vec<(usize, &String)> = unicode.iter().enumerate().filter(|(_, s)| !s.is_empty()).collect();
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in entries.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (gid, text) in chunk {
            let hex: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
            s.push_str(&format!("<{gid:04X}> <{hex}>\n"));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// `/W` array: every id in one run.
pub fn w_array(widths: &[i64]) -> String {
    let body: Vec<String> = widths.iter().map(|w| w.to_string()).collect();
    format!("[ 0 [ {} ] ]", body.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cjk_font() -> Option<FontSource> {
        let (r, _) = cjk_candidates("MicrosoftYaHei");
        r.into_iter().find(|s| load(s).is_some())
    }

    #[test]
    fn sfnt_checksum_adjustment_makes_file_sum_magic() {
        let tables = vec![(*b"head", vec![0u8; 54]), (*b"maxp", vec![0, 1, 0, 0, 0, 3])];
        let f = write_sfnt(&tables);
        assert_eq!(checksum(&f), 0xB1B0_AFBA);
        assert_eq!(rd16(&f, 4), Some(2));
    }

    #[test]
    fn composite_components_are_found() {
        // numberOfContours = -1, bbox, one component (flags ARG_WORDS, gid 7).
        let mut g = vec![0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0];
        g.extend_from_slice(&[0x00, 0x01, 0x00, 0x07, 0, 0, 0, 0]);
        assert_eq!(components(&g), vec![(12, 7)]);
        assert!(components(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0]).is_empty());
    }

    #[test]
    fn to_unicode_lists_mapped_glyphs() {
        let c = to_unicode_cmap(&["".into(), "中".into(), "𝑥".into()]);
        assert!(c.contains("<0001> <4E2D>"), "{c}");
        assert!(c.contains("<0002> <D835DC65>"), "{c}");
        assert!(!c.contains("<0000>  "));
    }

    #[test]
    fn without_context_encode_falls_back() {
        let p = encode("中文", false, 10.0);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].face, None);
        assert_eq!(p[0].bytes, vec![0x4e, 0x2d, 0x65, 0x87]);
        assert_eq!(advance('中', false, 10.0), None);
    }

    #[test]
    fn subset_round_trips_through_ttf_parser() {
        let Some(src) = cjk_font() else { return };
        begin_with(vec![src], vec![], vec![]);
        let pieces = encode("中文 A→", false, 12.0);
        assert!(pieces.iter().all(|p| p.face.is_some()), "{pieces:?}");
        assert!(advance('中', false, 10.0).unwrap() > 5.0);
        let (set, warns) = finish();
        assert!(warns.is_empty(), "{warns:?}");
        let f = set.fonts[0].as_ref().expect("regular subset");
        assert!(f.file.len() < 200_000, "subset should be small: {}", f.file.len());
        let parsed = ttf_parser::Face::parse(&f.file, 0).expect("subset parses");
        assert_eq!(parsed.number_of_glyphs() as usize, f.widths.len());
        assert!(f.unicode.iter().any(|u| u == "中"));
        assert!(f.base_name.len() > 7 && f.base_name.as_bytes()[6] == b'+');
        // The glyph for 中 still has an outline after renumbering.
        let id = f.unicode.iter().position(|u| u == "中").unwrap() as u16;
        struct Count(usize);
        impl ttf_parser::OutlineBuilder for Count {
            fn move_to(&mut self, _: f32, _: f32) { self.0 += 1 }
            fn line_to(&mut self, _: f32, _: f32) { self.0 += 1 }
            fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { self.0 += 1 }
            fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { self.0 += 1 }
            fn close(&mut self) {}
        }
        let mut c = Count(0);
        assert!(parsed.outline_glyph(GlyphId(id), &mut c).is_some());
        assert!(c.0 > 4);
    }

    #[test]
    fn missing_everything_warns_once() {
        begin_with(vec![], vec![], vec![]);
        let p = encode("中", false, 10.0);
        assert_eq!(p[0].face, None);
        let (_, warns) = finish();
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("未找到可嵌入的 CJK 字体"));
    }
}
