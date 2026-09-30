//! Vector formula layout for the PDF writer.
//!
//! LaTeX → OMML ([`crate::latex2omml`]) → node tree ([`crate::omml`]) → boxes
//! laid out with the OpenType `MATH` constants of a system math font
//! (Cambria Math, STIX Two Math, Latin Modern Math, …) → glyph outlines as PDF
//! path operators.  Formulas are therefore drawn as vector art: sharp at any
//! zoom, independent of fonts installed on the reader's machine, and wrapped
//! in an `/ActualText` span so copying the formula yields its LaTeX source.
//!
//! Characters the math font lacks (CJK inside `\text{}`) are taken from a
//! secondary text font.  When no math font exists at all, or a formula fails
//! to parse, the caller falls back to text and reports it.

use crate::omml::{self, Node};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use ttf_parser::{GlyphId, OutlineBuilder};

/// Positions and sizes below are in em units of the formula's base size
/// (1.0 = font size) until [`Formula::draw`] scales them to points.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Glyph { font: u8, gid: u16, x: f64, y: f64, scale: f64 },
    Rule { x: f64, y: f64, w: f64, h: f64 },
}

#[derive(Debug, Clone, Default)]
struct MBox {
    w: f64,
    /// Height above the baseline.
    h: f64,
    /// Depth below the baseline (positive).
    d: f64,
    items: Vec<Item>,
    /// Italic correction of the last glyph (for superscripts).
    italic: f64,
    class: Option<omml::Class>,
    /// A single large operator (for limits placement).
    large_op: bool,
}

impl MBox {
    fn shifted(mut self, dx: f64, dy: f64) -> MBox {
        for it in &mut self.items {
            match it {
                Item::Glyph { x, y, .. } | Item::Rule { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
            }
        }
        self
    }

    fn append(&mut self, other: MBox, dx: f64, dy: f64) {
        self.h = self.h.max(other.h + dy);
        self.d = self.d.max(other.d - dy);
        let o = other.shifted(dx, dy);
        self.items.extend(o.items);
    }

    fn empty() -> MBox {
        MBox::default()
    }
}

// ------------------------------------------------------------------ fonts

struct MathFont {
    data: Arc<Vec<u8>>,
    index: u32,
    upem: f64,
}

impl MathFont {
    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, self.index).ok()
    }
}

/// Font 0 is the math font, font 1 the text fallback for anything it lacks.
struct Fonts {
    fonts: Vec<MathFont>,
    c: Constants,
}

#[derive(Debug, Clone, Copy)]
struct Constants {
    axis: f64,
    rule: f64,
    num_up: f64,
    num_up_d: f64,
    den_down: f64,
    den_down_d: f64,
    num_gap: f64,
    num_gap_d: f64,
    den_gap: f64,
    den_gap_d: f64,
    sup_up: f64,
    sup_up_cramped: f64,
    sup_bottom_min: f64,
    sub_down: f64,
    sub_top_max: f64,
    sub_sup_gap: f64,
    sup_drop_max: f64,
    sub_drop_min: f64,
    space_after_script: f64,
    rad_gap: f64,
    rad_gap_d: f64,
    rad_rule: f64,
    rad_extra: f64,
    upper_gap: f64,
    upper_rise: f64,
    lower_gap: f64,
    lower_drop: f64,
    script_scale: f64,
    script_script_scale: f64,
    display_op_min: f64,
    overbar_gap: f64,
    delim_min: f64,
}

impl Constants {
    /// TeX-like defaults, used for any value a font leaves out.
    fn defaults() -> Constants {
        Constants {
            axis: 0.25,
            rule: 0.05,
            num_up: 0.39,
            num_up_d: 0.68,
            den_down: 0.34,
            den_down_d: 0.69,
            num_gap: 0.05,
            num_gap_d: 0.15,
            den_gap: 0.05,
            den_gap_d: 0.15,
            sup_up: 0.36,
            sup_up_cramped: 0.29,
            sup_bottom_min: 0.11,
            sub_down: 0.2,
            sub_top_max: 0.35,
            sub_sup_gap: 0.2,
            sup_drop_max: 0.39,
            sub_drop_min: 0.05,
            space_after_script: 0.05,
            rad_gap: 0.06,
            rad_gap_d: 0.15,
            rad_rule: 0.05,
            rad_extra: 0.05,
            upper_gap: 0.11,
            upper_rise: 0.11,
            lower_gap: 0.17,
            lower_drop: 0.6,
            script_scale: 0.7,
            script_script_scale: 0.5,
            display_op_min: 1.3,
            overbar_gap: 0.15,
            delim_min: 1.3,
        }
    }
}

fn load_constants(f: &MathFont) -> Constants {
    let mut c = Constants::defaults();
    let Some(face) = f.face() else { return c };
    let Some(k) = face.tables().math.and_then(|m| m.constants) else { return c };
    let e = |v: ttf_parser::math::MathValue| v.value as f64 / f.upem;
    c.axis = e(k.axis_height());
    c.rule = e(k.fraction_rule_thickness());
    c.num_up = e(k.fraction_numerator_shift_up());
    c.num_up_d = e(k.fraction_numerator_display_style_shift_up());
    c.den_down = e(k.fraction_denominator_shift_down());
    c.den_down_d = e(k.fraction_denominator_display_style_shift_down());
    c.num_gap = e(k.fraction_numerator_gap_min());
    c.num_gap_d = e(k.fraction_num_display_style_gap_min());
    c.den_gap = e(k.fraction_denominator_gap_min());
    c.den_gap_d = e(k.fraction_denom_display_style_gap_min());
    c.sup_up = e(k.superscript_shift_up());
    c.sup_up_cramped = e(k.superscript_shift_up_cramped());
    c.sup_bottom_min = e(k.superscript_bottom_min());
    c.sub_down = e(k.subscript_shift_down());
    c.sub_top_max = e(k.subscript_top_max());
    c.sub_sup_gap = e(k.sub_superscript_gap_min());
    c.sup_drop_max = e(k.superscript_baseline_drop_max());
    c.sub_drop_min = e(k.subscript_baseline_drop_min());
    c.space_after_script = e(k.space_after_script());
    c.rad_gap = e(k.radical_vertical_gap());
    c.rad_gap_d = e(k.radical_display_style_vertical_gap());
    c.rad_rule = e(k.radical_rule_thickness());
    c.rad_extra = e(k.radical_extra_ascender());
    c.upper_gap = e(k.upper_limit_gap_min());
    c.upper_rise = e(k.upper_limit_baseline_rise_min());
    c.lower_gap = e(k.lower_limit_gap_min());
    c.lower_drop = e(k.lower_limit_baseline_drop_min());
    c.overbar_gap = e(k.overbar_vertical_gap());
    let pct = |p: i16, d: f64| if p > 0 { p as f64 / 100.0 } else { d };
    c.script_scale = pct(k.script_percent_scale_down(), 0.7);
    c.script_script_scale = pct(k.script_script_percent_scale_down(), 0.5);
    let m = k.display_operator_min_height() as f64 / f.upem;
    if m > 0.0 {
        c.display_op_min = m;
    }
    let dm = k.delimited_sub_formula_min_height() as f64 / f.upem;
    if dm > 0.0 {
        c.delim_min = dm;
    }
    c
}

fn font_file(names: &[(&str, u32)]) -> Vec<(PathBuf, u32)> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        dirs.push(PathBuf::from(windir).join("Fonts"));
        if let Ok(l) = std::env::var("LOCALAPPDATA") {
            dirs.push(PathBuf::from(l).join("Microsoft/Windows/Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/System/Library/Fonts/Supplemental", "/Library/Fonts"].map(PathBuf::from));
    } else {
        dirs.extend(
            [
                "/usr/share/fonts/opentype/stix",
                "/usr/share/fonts/opentype/stix-two",
                "/usr/share/fonts/stix",
                "/usr/share/fonts/opentype/lmodern",
                "/usr/share/texmf/fonts/opentype/public/lm-math",
                "/usr/share/fonts/texlive-lm-math",
                "/usr/share/fonts/opentype/freefont",
                "/usr/share/fonts/truetype/dejavu",
            ]
            .map(PathBuf::from),
        );
    }
    let mut out = Vec::new();
    for (n, i) in names {
        for d in &dirs {
            let p = d.join(n);
            if p.is_file() {
                out.push((p, *i));
                break;
            }
        }
    }
    out
}

fn open(path: &PathBuf, index: u32, need_math: bool) -> Option<MathFont> {
    let data = Arc::new(std::fs::read(path).ok()?);
    let face = ttf_parser::Face::parse(&data, index).ok()?;
    if need_math && face.tables().math.is_none() {
        return None;
    }
    if matches!(face.permissions(), Some(ttf_parser::Permissions::Restricted)) {
        return None;
    }
    let upem = face.units_per_em() as f64;
    drop(face);
    Some(MathFont { data, index, upem })
}

fn fonts() -> Option<&'static Fonts> {
    static FONTS: OnceLock<Option<Fonts>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let math = font_file(&[
                ("cambria.ttc", 1),
                ("STIXTwoMath-Regular.otf", 0),
                ("STIXTwoMath.otf", 0),
                ("STIXMath-Regular.otf", 0),
                ("latinmodern-math.otf", 0),
                ("Latin Modern Math.otf", 0),
                ("DejaVuMathTeXGyre.ttf", 0),
                ("FreeSerif.otf", 0),
            ])
            .into_iter()
            .find_map(|(p, i)| open(&p, i, true))?;
            let c = load_constants(&math);
            let mut fonts = vec![math];
            let text = font_file(&[
                ("msyh.ttc", 0),
                ("simhei.ttf", 0),
                ("simsun.ttc", 0),
                ("PingFang.ttc", 0),
                ("Songti.ttc", 0),
                ("wqy-microhei.ttc", 0),
                ("NotoSansSC-Regular.ttf", 0),
                ("DroidSansFallbackFull.ttf", 0),
            ]);
            if let Some(t) = text.into_iter().find_map(|(p, i)| open(&p, i, false)) {
                fonts.push(t);
            }
            Some(Fonts { fonts, c })
        })
        .as_ref()
}

/// Whether vector math is available on this machine.
pub fn available() -> bool {
    fonts().is_some()
}

// ------------------------------------------------------------------ layout

#[derive(Clone, Copy)]
struct Style {
    display: bool,
    /// 0 = text, 1 = script, 2 = scriptscript.
    level: u8,
    cramped: bool,
}

impl Style {
    fn scale(&self, c: &Constants) -> f64 {
        match self.level {
            0 => 1.0,
            1 => c.script_scale,
            _ => c.script_script_scale,
        }
    }
    fn sup(self) -> Style {
        Style { display: false, level: (self.level + 1).min(2), cramped: self.cramped }
    }
    fn sub(self) -> Style {
        Style { display: false, level: (self.level + 1).min(2), cramped: true }
    }
    fn frac_part(self, den: bool) -> Style {
        if self.display {
            Style { display: false, level: self.level, cramped: den || self.cramped }
        } else {
            Style { display: false, level: (self.level + 1).min(2), cramped: den || self.cramped }
        }
    }
}

struct Ctx<'f> {
    f: &'f Fonts,
    faces: Vec<ttf_parser::Face<'f>>,
    /// (font, char) → (gid, advance em, h, d, italic)
    cache: HashMap<(u8, char), Option<(u16, f64, f64, f64, f64)>>,
    missing: bool,
}

/// Math alphanumerics for `\mathbb` / `\mathcal` / bold / italic letters.
fn math_alnum(c: char, st: &omml::RunStyle, italic: bool) -> char {
    let off = |base: u32, c: char, from: char| char::from_u32(base + (c as u32 - from as u32));
    let exceptions = |c: char| -> Option<char> {
        Some(match c {
            'h' if italic => 'ℎ',
            _ => return None,
        })
    };
    if st.double_struck {
        let special = match c {
            'C' => Some('ℂ'),
            'H' => Some('ℍ'),
            'N' => Some('ℕ'),
            'P' => Some('ℙ'),
            'Q' => Some('ℚ'),
            'R' => Some('ℝ'),
            'Z' => Some('ℤ'),
            _ => None,
        };
        if let Some(s) = special {
            return s;
        }
        return match c {
            'A'..='Z' => off(0x1D538, c, 'A'),
            'a'..='z' => off(0x1D552, c, 'a'),
            '0'..='9' => off(0x1D7D8, c, '0'),
            _ => None,
        }
        .unwrap_or(c);
    }
    if st.script {
        let special = match c {
            'B' => Some('ℬ'),
            'E' => Some('ℰ'),
            'F' => Some('ℱ'),
            'H' => Some('ℋ'),
            'I' => Some('ℐ'),
            'L' => Some('ℒ'),
            'M' => Some('ℳ'),
            'R' => Some('ℛ'),
            _ => None,
        };
        if let Some(s) = special {
            return s;
        }
        return match c {
            'A'..='Z' => off(0x1D49C, c, 'A'),
            _ => None,
        }
        .unwrap_or(c);
    }
    match (st.bold, italic) {
        (true, true) => match c {
            'A'..='Z' => off(0x1D468, c, 'A'),
            'a'..='z' => off(0x1D482, c, 'a'),
            _ => None,
        },
        (true, false) => match c {
            'A'..='Z' => off(0x1D400, c, 'A'),
            'a'..='z' => off(0x1D41A, c, 'a'),
            '0'..='9' => off(0x1D7CE, c, '0'),
            _ => None,
        },
        (false, true) => exceptions(c).or(match c {
            'A'..='Z' => off(0x1D434, c, 'A'),
            'a'..='z' => off(0x1D44E, c, 'a'),
            'α'..='ω' => off(0x1D6FC, c, 'α'),
            _ => None,
        }),
        (false, false) => None,
    }
    .unwrap_or(c)
}

impl<'f> Ctx<'f> {
    fn new(f: &'f Fonts) -> Ctx<'f> {
        let faces = f.fonts.iter().filter_map(|m| m.face()).collect();
        Ctx { f, faces, cache: HashMap::new(), missing: false }
    }

    fn c(&self) -> &Constants {
        &self.f.c
    }

    fn metrics(&mut self, font: u8, c: char) -> Option<(u16, f64, f64, f64, f64)> {
        if let Some(hit) = self.cache.get(&(font, c)) {
            return *hit;
        }
        let face = self.faces.get(font as usize)?;
        let upem = self.f.fonts[font as usize].upem;
        let got = face.glyph_index(c).filter(|g| g.0 != 0).map(|g| {
            let adv = face.glyph_hor_advance(g).unwrap_or(0) as f64 / upem;
            let (h, d) = face
                .glyph_bounding_box(g)
                .map(|b| (b.y_max as f64 / upem, -(b.y_min as f64) / upem))
                .unwrap_or((0.0, 0.0));
            let italic = if font == 0 {
                face.tables()
                    .math
                    .and_then(|m| m.glyph_info)
                    .and_then(|gi| gi.italic_corrections)
                    .and_then(|ic| ic.get(g))
                    .map(|v| v.value as f64 / upem)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            (g.0, adv, h, d, italic)
        });
        self.cache.insert((font, c), got);
        got
    }

    fn glyph_box(&mut self, c: char, scale: f64) -> MBox {
        for font in 0..self.faces.len() as u8 {
            if let Some((gid, adv, h, d, it)) = self.metrics(font, c) {
                return MBox {
                    w: adv * scale,
                    h: h.max(0.0) * scale,
                    d: d.max(0.0) * scale,
                    items: vec![Item::Glyph { font, gid, x: 0.0, y: 0.0, scale }],
                    italic: it * scale,
                    class: Some(omml::classify(c)),
                    large_op: false,
                };
            }
        }
        self.missing = true;
        // Unknown glyph: an em-quad placeholder keeps spacing sane.
        MBox { w: 0.5 * scale, h: 0.7 * scale, d: 0.0, ..MBox::default() }
    }

    /// Glyph id variant that reaches `target` (em, at scale 1) vertically.
    fn vertical_variant(&mut self, c: char, target: f64) -> Option<(u16, f64, f64, f64)> {
        let face = self.faces.first()?;
        let upem = self.f.fonts[0].upem;
        let g = face.glyph_index(c)?;
        let vars = face.tables().math?.variants?;
        let cons = vars.vertical_constructions.get(g)?;
        let mut best = None;
        for v in cons.variants {
            let gid = v.variant_glyph;
            let bb = face.glyph_bounding_box(gid)?;
            let total = (bb.y_max - bb.y_min) as f64 / upem;
            let adv = face.glyph_hor_advance(gid).unwrap_or(0) as f64 / upem;
            best = Some((gid.0, adv, bb.y_max as f64 / upem, -(bb.y_min as f64) / upem));
            if total >= target {
                break;
            }
        }
        best
    }

    /// Build a vertical glyph assembly (top/extender/middle/bottom parts) at
    /// least `target` em tall, when even the largest size variant is short.
    fn vertical_assembly(&mut self, c: char, target: f64, scale: f64) -> Option<MBox> {
        let face = self.faces.first()?;
        let upem = self.f.fonts[0].upem;
        let g = face.glyph_index(c)?;
        let vars = face.tables().math?.variants?;
        let asm = vars.vertical_constructions.get(g)?.assembly?;
        let parts: Vec<_> = asm.parts.into_iter().collect();
        if parts.is_empty() || !parts.iter().any(|p| p.part_flags.extender()) {
            return None;
        }
        let overlap = vars.min_connector_overlap as f64 / upem;
        let adv = |p: &ttf_parser::math::GlyphPart| p.full_advance as f64 / upem;
        let fixed: f64 = parts.iter().filter(|p| !p.part_flags.extender()).map(|p| adv(p) - overlap).sum();
        let ext: f64 = parts.iter().filter(|p| p.part_flags.extender()).map(|p| adv(p) - overlap).sum();
        let reps = if ext <= 0.0 { 0 } else { (((target / scale) - fixed - overlap) / ext).ceil().clamp(0.0, 200.0) as usize };
        let mut out = MBox::empty();
        let mut pos = 0.0f64;
        let mut width = 0.0f64;
        for p in &parts {
            let n = if p.part_flags.extender() { reps } else { 1 };
            for _ in 0..n {
                let bb_min = face.glyph_bounding_box(p.glyph_id).map(|b| b.y_min as f64 / upem).unwrap_or(0.0);
                let w = face.glyph_hor_advance(p.glyph_id).unwrap_or(0) as f64 / upem;
                width = width.max(w);
                out.items.push(Item::Glyph { font: 0, gid: p.glyph_id.0, x: 0.0, y: (pos - bb_min) * scale, scale });
                pos += adv(p) - overlap;
            }
        }
        let total = (pos + overlap) * scale;
        out.w = width * scale;
        out.h = total;
        out.d = 0.0;
        out.class = Some(omml::Class::Ord);
        Some(out)
    }

    fn sized_glyph(&mut self, c: char, target: f64, scale: f64) -> MBox {
        if let Some((_, _, h, d)) = self.vertical_variant(c, target / scale) {
            if (h + d) * scale < target * 0.98 {
                if let Some(b) = self.vertical_assembly(c, target, scale) {
                    return b;
                }
            }
        }
        match self.vertical_variant(c, target / scale) {
            Some((gid, adv, h, d)) => MBox {
                w: adv * scale,
                h: h * scale,
                d: d * scale,
                items: vec![Item::Glyph { font: 0, gid, x: 0.0, y: 0.0, scale }],
                italic: 0.0,
                class: Some(omml::classify(c)),
                large_op: false,
            },
            None => self.glyph_box(c, scale),
        }
    }

    /// Stretchy delimiter centred on the axis, covering `h`/`d`.
    fn delimiter(&mut self, c: char, h: f64, d: f64, st: Style) -> MBox {
        let s = st.scale(self.c());
        let axis = self.c().axis * s;
        let half = (h - axis).max(d + axis);
        // TeX's \delimiterfactor 901 / \delimitershortfall: cover ~90 %.
        let target = 2.0 * half * 0.9;
        let mut b = if 2.0 * half > 1.05 * s { self.sized_glyph(c, target, s) } else { self.glyph_box(c, s) };
        // Centre on the axis.
        let mid = (b.h - b.d) / 2.0;
        let dy = axis - mid;
        b = b.shifted(0.0, dy);
        let (h2, d2) = (b.h + dy, b.d - dy);
        b.h = h2;
        b.d = d2;
        b
    }

    fn row(&mut self, nodes: &[Node], st: Style) -> MBox {
        let mut parts: Vec<MBox> = Vec::new();
        let mut i = 0usize;
        while i < nodes.len() {
            let n = &nodes[i];
            if n.is_props() {
                i += 1;
                continue;
            }
            // Large operator followed by its operand as siblings is handled in `nary`.
            parts.push(self.node(n, st));
            i += 1;
        }
        self.hlist(parts, st)
    }

    /// Concatenate with TeX-like inter-atom spacing.
    fn hlist(&mut self, parts: Vec<MBox>, st: Style) -> MBox {
        use omml::Class::*;
        let s = st.scale(self.c());
        let mut out = MBox::empty();
        let mut prev: Option<omml::Class> = None;
        let n = parts.len();
        for (idx, p) in parts.into_iter().enumerate() {
            let mut cls = p.class;
            // A binary operator at the start, or after an operator/open, is unary.
            if cls == Some(Bin) && (prev.is_none() || matches!(prev, Some(Bin | Rel | Open | Punct)) || idx + 1 == n) {
                cls = Some(Ord);
            }
            let gap = match (prev, cls) {
                (Some(_), Some(Rel)) | (Some(Rel), Some(_)) if st.level == 0 => 5.0 / 18.0,
                (Some(_), Some(Bin)) | (Some(Bin), Some(_)) if st.level == 0 => 4.0 / 18.0,
                (Some(Punct), Some(_)) if st.level == 0 => 3.0 / 18.0,
                _ => 0.0,
            };
            let x = out.w + gap * s;
            out.w = x + p.w;
            out.italic = p.italic;
            out.append(p, x, 0.0);
            if cls.is_some() {
                prev = cls;
            }
        }
        out.class = None;
        out
    }

    fn child_row(&mut self, n: &Node, name: &str, st: Style) -> MBox {
        match n.child(name) {
            Some(c) => self.row(&c.children, st),
            None => MBox::empty(),
        }
    }

    fn run(&mut self, r: &Node, st: Style) -> MBox {
        let text = r.run_text();
        let rs = omml::run_style(r);
        let s = st.scale(self.c());
        let word = omml::is_word(&text);
        let mut parts = Vec::new();
        for c in text.chars() {
            if c == ' ' {
                // Operator names carry a trailing space: thin space.
                parts.push(MBox { w: if rs.normal_text { 0.25 * s } else { 1.0 / 6.0 * s }, ..MBox::empty() });
                continue;
            }
            if c == '\u{2009}' || c == '\u{2005}' {
                parts.push(MBox { w: 0.17 * s, ..MBox::empty() });
                continue;
            }
            if c == '\u{2003}' {
                parts.push(MBox { w: 1.0 * s, ..MBox::empty() });
                continue;
            }
            let italic = !rs.normal_text && !rs.upright && !word && (c.is_ascii_alphabetic() || ('α'..='ω').contains(&c));
            let mapped = if rs.normal_text { c } else { math_alnum(c, &rs, italic) };
            let mut b = self.glyph_box(mapped, s);
            if b.items.is_empty() && mapped != c {
                b = self.glyph_box(c, s);
            }
            if c == '-' && !rs.normal_text {
                b = self.glyph_box('−', s);
            }
            parts.push(b);
        }
        let mut out = MBox::empty();
        let cls = if parts.len() == 1 { parts[0].class } else { Some(omml::Class::Ord) };
        for p in parts {
            let x = out.w;
            out.w += p.w;
            out.italic = p.italic;
            out.append(p, x, 0.0);
        }
        out.class = if rs.normal_text { Some(omml::Class::Ord) } else { cls };
        out
    }

    fn scripts(&mut self, base: MBox, sub: Option<MBox>, sup: Option<MBox>, st: Style) -> MBox {
        let c = *self.c();
        let s = st.scale(&c);
        let single = base.items.len() <= 1;
        let mut out = base.clone();
        let italic = base.italic;
        out.class = base.class;
        let (mut u, mut v) = (0.0f64, 0.0f64);
        if let Some(p) = &sup {
            u = if st.cramped { c.sup_up_cramped } else { c.sup_up } * s;
            if !single {
                u = u.max(base.h - c.sup_drop_max * s);
            }
            u = u.max(p.d + c.sup_bottom_min * s);
        }
        if let Some(b) = &sub {
            v = c.sub_down * s;
            if !single {
                v = v.max(base.d + c.sub_drop_min * s);
            }
            v = v.max(b.h - c.sub_top_max * s);
        }
        if let (Some(p), Some(b)) = (&sup, &sub) {
            let gap = (u - p.d) - (b.h - v);
            if gap < c.sub_sup_gap * s {
                v += c.sub_sup_gap * s - gap;
            }
        }
        let x0 = base.w;
        let mut w = x0;
        if let Some(p) = sup {
            let pw = p.w;
            out.append(p, x0 + italic, u);
            w = w.max(x0 + italic + pw);
        }
        if let Some(b) = sub {
            let bw = b.w;
            out.append(b, x0, -v);
            w = w.max(x0 + bw);
        }
        out.w = w + c.space_after_script * s;
        out.italic = 0.0;
        out
    }

    fn fraction(&mut self, n: &Node, st: Style) -> MBox {
        let c = *self.c();
        let s = st.scale(&c);
        let nobar = n.prop("fPr", "type") == Some("noBar");
        let num = self.child_row(n, "num", st.frac_part(false));
        let den = self.child_row(n, "den", st.frac_part(true));
        let t = if nobar { 0.0 } else { c.rule * s };
        let axis = c.axis * s;
        let (mut up, mut down) = if st.display { (c.num_up_d * s, c.den_down_d * s) } else { (c.num_up * s, c.den_down * s) };
        let (ng, dg) = if st.display { (c.num_gap_d * s, c.den_gap_d * s) } else { (c.num_gap * s, c.den_gap * s) };
        up = up.max(axis + t / 2.0 + ng + num.d);
        down = down.max(dg + t / 2.0 - axis + den.h);
        let pad = 0.12 * s;
        let w = num.w.max(den.w) + 2.0 * pad;
        let mut out = MBox::empty();
        let (nw, dw) = (num.w, den.w);
        out.append(num, (w - nw) / 2.0, up);
        out.append(den, (w - dw) / 2.0, -down);
        if !nobar {
            out.items.push(Item::Rule { x: pad * 0.5, y: axis - t / 2.0, w: w - pad, h: t });
        }
        out.w = w;
        out.h = out.h.max(axis + t);
        out.class = Some(omml::Class::Ord);
        out
    }

    fn radical(&mut self, n: &Node, st: Style) -> MBox {
        let c = *self.c();
        let s = st.scale(&c);
        let body = self.child_row(n, "e", Style { cramped: true, ..st });
        let gap = if st.display { c.rad_gap_d } else { c.rad_gap } * s;
        let t = c.rad_rule * s;
        let need = body.h + body.d + gap + t;
        let mut sign = self.sized_glyph('√', need, s);
        // Put the top of the sign at body.h + gap + t.
        let top = body.h + gap + t;
        let dy = top - sign.h;
        sign = sign.shifted(0.0, dy);
        let (sh, sd) = (sign.h + dy, sign.d - dy);
        let mut out = MBox::empty();
        let hide = n.prop("radPr", "degHide").is_some();
        let deg = n.child("deg").filter(|d| !d.children.is_empty() && !hide);
        let mut x = 0.0;
        if let Some(dn) = deg {
            let db = self.row(&dn.children, Style { display: false, level: 2, cramped: false });
            let raise = sd.max(0.0) * -1.0 + (sh + sd) * 0.6;
            let dw = db.w;
            out.append(db, 0.05 * s, raise);
            x = (dw - 0.25 * s).max(0.0);
        }
        let sw = sign.w;
        out.items.extend(sign.shifted(x, 0.0).items);
        out.h = out.h.max(sh + c.rad_extra * s);
        out.d = out.d.max(sd);
        let bx = x + sw;
        let bw = body.w;
        out.append(body, bx, 0.0);
        out.items.push(Item::Rule { x: bx, y: top - t, w: bw + 0.05 * s, h: t });
        out.w = bx + bw + 0.1 * s;
        out.class = Some(omml::Class::Ord);
        out
    }

    fn nary(&mut self, n: &Node, st: Style) -> MBox {
        let c = *self.c();
        let s = st.scale(&c);
        let chr = n.prop("naryPr", "chr").unwrap_or("∫").chars().next().unwrap_or('∫');
        let und_ovr = n.prop("naryPr", "limLoc") == Some("undOvr");
        let mut op = if st.display { self.sized_glyph(chr, c.display_op_min, s) } else { self.glyph_box(chr, s) };
        // Centre on the math axis.
        let mid = (op.h - op.d) / 2.0;
        let dy = c.axis * s - mid;
        op = op.shifted(0.0, dy);
        op.h += dy;
        op.d -= dy;
        op.large_op = true;
        let sub = n.child("sub").filter(|x| !x.children.is_empty()).map(|x| self.row(&x.children, st.sub()));
        let sup = n.child("sup").filter(|x| !x.children.is_empty()).map(|x| self.row(&x.children, st.sup()));
        let limits = und_ovr && st.display;
        let mut head = if limits {
            let w = op.w.max(sub.as_ref().map(|b| b.w).unwrap_or(0.0)).max(sup.as_ref().map(|b| b.w).unwrap_or(0.0));
            let mut out = MBox::empty();
            let (oh, od, ow) = (op.h, op.d, op.w);
            out.append(op, (w - ow) / 2.0, 0.0);
            if let Some(p) = sup {
                let y = (oh + c.upper_gap * s + p.d).max(oh + c.upper_rise * s);
                let pw = p.w;
                out.append(p, (w - pw) / 2.0, y);
            }
            if let Some(b) = sub {
                let y = (od + c.lower_gap * s + b.h).max(od + c.lower_drop * s * 0.5);
                let bw = b.w;
                out.append(b, (w - bw) / 2.0, -y);
            }
            out.w = w;
            out
        } else {
            let italic = op.italic;
            let mut b = self.scripts(op, sub, sup, st);
            b.w += italic * 0.5;
            b
        };
        head.w += 0.1 * s;
        let body = self.child_row(n, "e", st);
        let hw = head.w;
        head.w = hw + body.w;
        head.append(body, hw, 0.0);
        head.class = Some(omml::Class::Ord);
        head
    }

    fn delimited(&mut self, n: &Node, st: Style) -> MBox {
        let s = st.scale(self.c());
        let beg = omml::delimiter_char(n.prop("dPr", "begChr").unwrap_or("("));
        let end = omml::delimiter_char(n.prop("dPr", "endChr").unwrap_or(")"));
        let mut inner_parts = Vec::new();
        let es: Vec<&Node> = n.children.iter().filter(|c| c.name == "e").collect();
        for (i, e) in es.iter().enumerate() {
            if i > 0 {
                let sep = n.prop("dPr", "sepChr").unwrap_or("|").chars().next().unwrap_or('|');
                inner_parts.push(self.glyph_box(sep, s));
            }
            inner_parts.push(self.row(&e.children, st));
        }
        let inner = self.hlist(inner_parts, st);
        let (h, d) = (inner.h.max(0.7 * s), inner.d.max(0.2 * s));
        let mut out = MBox::empty();
        let mut x = 0.0;
        if let Some(ch) = beg.chars().next() {
            let b = self.delimiter(ch, h, d, st);
            let bw = b.w;
            out.append(b, 0.0, 0.0);
            x = bw;
        }
        let iw = inner.w;
        out.append(inner, x, 0.0);
        x += iw;
        if let Some(ch) = end.chars().next() {
            let b = self.delimiter(ch, h, d, st);
            let bw = b.w;
            out.append(b, x, 0.0);
            x += bw;
        }
        out.w = x;
        out.class = Some(omml::Class::Ord);
        out
    }

    fn table(&mut self, rows: Vec<Vec<&Node>>, st: Style, left: bool) -> MBox {
        let s = st.scale(self.c());
        let cells: Vec<Vec<MBox>> = rows.iter().map(|r| r.iter().map(|c| self.row(&c.children, st)).collect()).collect();
        let ncol = cells.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut colw = vec![0.0f64; ncol];
        for r in &cells {
            for (i, c) in r.iter().enumerate() {
                colw[i] = colw[i].max(c.w);
            }
        }
        let col_gap = if left { 0.0 } else { 0.8 * s };
        let row_gap = 0.25 * s;
        let heights: Vec<(f64, f64)> = cells
            .iter()
            .map(|r| {
                (
                    r.iter().map(|c| c.h).fold(0.7 * s, f64::max),
                    r.iter().map(|c| c.d).fold(0.25 * s, f64::max),
                )
            })
            .collect();
        let total: f64 = heights.iter().map(|(h, d)| h + d).sum::<f64>() + row_gap * (heights.len().saturating_sub(1)) as f64;
        // Centre the whole table on the axis.
        let mut y = total / 2.0 + self.c().axis * s;
        let mut out = MBox::empty();
        for (ri, r) in cells.into_iter().enumerate() {
            let (h, d) = heights[ri];
            y -= h;
            let mut x = 0.0;
            for (ci, c) in r.into_iter().enumerate() {
                let cw = c.w;
                let dx = if left { 0.0 } else { (colw[ci] - cw) / 2.0 };
                out.append(c, x + dx, y);
                x += colw[ci] + col_gap;
            }
            y -= d + row_gap;
        }
        out.w = colw.iter().sum::<f64>() + col_gap * (ncol.saturating_sub(1)) as f64;
        out.class = Some(omml::Class::Ord);
        out
    }

    fn accent(&mut self, n: &Node, st: Style) -> MBox {
        let s = st.scale(self.c());
        let chr = n.prop("accPr", "chr").unwrap_or("^");
        let base = self.child_row(n, "e", Style { cramped: true, ..st });
        let ch = match chr {
            "^" | "\u{302}" => '\u{302}',
            "~" | "\u{303}" => '\u{303}',
            "\u{2192}" | "\u{20D7}" => '\u{20D7}',
            "¯" | "\u{305}" | "\u{304}" => '\u{304}',
            "\u{307}" | "˙" => '\u{307}',
            "\u{308}" | "¨" => '\u{308}',
            other => other.chars().next().unwrap_or('^'),
        };
        let mut acc = self.glyph_box(ch, s);
        if acc.items.is_empty() {
            acc = self.glyph_box(if ch == '\u{20D7}' { '→' } else { '^' }, s * 0.8);
        }
        let mut out = MBox::empty();
        let (bw, bh) = (base.w, base.h);
        let bi = base.italic;
        out.append(base, 0.0, 0.0);
        // Combining marks sit above x-height; move them over the base.
        let acc_bottom = -acc.d;
        let y = (bh - 0.45 * s).max(0.0) + if acc_bottom > 0.3 * s { 0.0 } else { 0.05 * s };
        let aw = acc.w;
        let dx = if aw > 0.0 { (bw - aw) / 2.0 + bi * 0.5 } else { bw / 2.0 };
        out.append(acc, dx, y);
        out.w = bw;
        out.class = Some(omml::Class::Ord);
        out
    }

    fn bar(&mut self, n: &Node, st: Style) -> MBox {
        let c = *self.c();
        let s = st.scale(&c);
        let bottom = n.prop("barPr", "pos") == Some("bot");
        let base = self.child_row(n, "e", st);
        let t = c.rule * s;
        let mut out = MBox::empty();
        let (bw, bh, bd) = (base.w, base.h, base.d);
        out.append(base, 0.0, 0.0);
        if bottom {
            out.items.push(Item::Rule { x: 0.0, y: -bd - c.overbar_gap * s - t, w: bw, h: t });
            out.d = out.d.max(bd + c.overbar_gap * s + t);
        } else {
            out.items.push(Item::Rule { x: 0.0, y: bh + c.overbar_gap * s, w: bw, h: t });
            out.h = out.h.max(bh + c.overbar_gap * s + t);
        }
        out.w = bw;
        out.class = Some(omml::Class::Ord);
        out
    }

    fn limit(&mut self, n: &Node, st: Style, lower: bool) -> MBox {
        let c = *self.c();
        let s = st.scale(&c);
        let base = self.child_row(n, "e", st);
        let lim = self.child_row(n, "lim", st.sub());
        if !st.display {
            return if lower { self.scripts(base, Some(lim), None, st) } else { self.scripts(base, None, Some(lim), st) };
        }
        let w = base.w.max(lim.w);
        let mut out = MBox::empty();
        let (bw, bh, bd) = (base.w, base.h, base.d);
        out.append(base, (w - bw) / 2.0, 0.0);
        let lw = lim.w;
        if lower {
            let y = bd + c.lower_gap * s + lim.h;
            out.append(lim, (w - lw) / 2.0, -y);
        } else {
            let y = bh + c.upper_gap * s + lim.d;
            out.append(lim, (w - lw) / 2.0, y);
        }
        out.w = w;
        out.class = Some(omml::Class::Ord);
        out
    }

    fn node(&mut self, n: &Node, st: Style) -> MBox {
        match n.name.as_str() {
            "r" => self.run(n, st),
            "f" => self.fraction(n, st),
            "sSup" => {
                let base = self.child_row(n, "e", st);
                let sup = self.child_row(n, "sup", st.sup());
                self.scripts(base, None, Some(sup), st)
            }
            "sSub" => {
                let base = self.child_row(n, "e", st);
                let sub = self.child_row(n, "sub", st.sub());
                self.scripts(base, Some(sub), None, st)
            }
            "sSubSup" => {
                let base = self.child_row(n, "e", st);
                let sub = self.child_row(n, "sub", st.sub());
                let sup = self.child_row(n, "sup", st.sup());
                self.scripts(base, Some(sub), Some(sup), st)
            }
            "rad" => self.radical(n, st),
            "nary" => self.nary(n, st),
            "d" => self.delimited(n, st),
            "m" => {
                let rows = n.children.iter().filter(|c| c.name == "mr").map(|r| r.children.iter().filter(|c| c.name == "e").collect()).collect();
                self.table(rows, st, false)
            }
            "eqArr" => {
                let rows = n.children.iter().filter(|c| c.name == "e").map(|e| vec![e]).collect();
                self.table(rows, st, true)
            }
            "acc" => self.accent(n, st),
            "bar" => self.bar(n, st),
            "limLow" => self.limit(n, st, true),
            "limUpp" => self.limit(n, st, false),
            "borderBox" => {
                let s = st.scale(self.c());
                let t = self.c().rule * s;
                let pad = 0.15 * s;
                let inner = self.child_row(n, "e", st);
                let (w, h, d) = (inner.w + 2.0 * pad, inner.h + pad, inner.d + pad);
                let mut out = MBox::empty();
                out.append(inner, pad, 0.0);
                out.items.push(Item::Rule { x: 0.0, y: -d, w, h: t });
                out.items.push(Item::Rule { x: 0.0, y: h - t, w, h: t });
                out.items.push(Item::Rule { x: 0.0, y: -d, w: t, h: h + d });
                out.items.push(Item::Rule { x: w - t, y: -d, w: t, h: h + d });
                out.w = w;
                out.h = out.h.max(h);
                out.d = out.d.max(d);
                out.class = Some(omml::Class::Ord);
                out
            }
            _ => self.row(&n.children, st),
        }
    }
}

// ------------------------------------------------------------------ output

/// A laid-out formula, in points once [`Formula::at_size`] is applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    /// Width / height above baseline / depth below baseline, in points.
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    items: Vec<Item>,
    size: f64,
    pub latex: String,
}

/// Lay out `latex` at `size` points.  `None` when there is no math font, the
/// formula does not parse, or glyphs are missing (the caller shows text).
pub fn layout(latex: &str, display: bool, size: f64) -> Option<Formula> {
    let f = fonts()?;
    let root = omml::from_latex(latex, display).ok()?;
    let mut ctx = Ctx::new(f);
    let st = Style { display, level: 0, cramped: false };
    let b = ctx.row(&root.children, st);
    if ctx.missing || b.items.is_empty() {
        return None;
    }
    Some(Formula { width: b.w * size, height: b.h * size, depth: b.d * size, items: b.items, size, latex: latex.to_string() })
}

struct PathOut<'a> {
    s: &'a mut String,
    k: f64,
    x: f64,
    y: f64,
}

impl PathOut<'_> {
    fn p(&self, x: f32, y: f32) -> (String, String) {
        (num(self.x + x as f64 * self.k), num(self.y + y as f64 * self.k))
    }
}

fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let s = format!("{r:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s
    }
}

impl OutlineBuilder for PathOut<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.s.push_str(&format!("{a} {b} m\n"));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.s.push_str(&format!("{a} {b} l\n"));
    }
    fn quad_to(&mut self, _x1: f32, _y1: f32, x: f32, y: f32) {
        // Only reached through `QuadFix`, which converts quadratics exactly.
        self.line_to(x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x2, y2);
        let (e, f) = self.p(x, y);
        self.s.push_str(&format!("{a} {b} {c} {d} {e} {f} c\n"));
    }
    fn close(&mut self) {
        self.s.push_str("h\n");
    }
}

/// Exact quadratic→cubic conversion needs the start point; this builder keeps it.
struct QuadFix<'a> {
    inner: PathOut<'a>,
    cur: (f32, f32),
    start: (f32, f32),
}

impl OutlineBuilder for QuadFix<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        self.start = (x, y);
        self.inner.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        self.inner.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.cur;
        let c1 = (x0 + 2.0 / 3.0 * (x1 - x0), y0 + 2.0 / 3.0 * (y1 - y0));
        let c2 = (x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
        self.inner.curve_to(c1.0, c1.1, c2.0, c2.1, x, y);
        self.cur = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cur = (x, y);
        self.inner.curve_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.cur = self.start;
        self.inner.close();
    }
}

fn utf16_hex(s: &str) -> String {
    let mut h = String::from("FEFF");
    for u in s.encode_utf16() {
        h.push_str(&format!("{u:04X}"));
    }
    h
}

impl Formula {
    /// PDF content-stream operators drawing the formula with its baseline at
    /// (`x`, `y`) in `rgb` (already formatted `r g b`).
    pub fn draw(&self, x: f64, y: f64, rgb: &str) -> String {
        let Some(f) = fonts() else { return String::new() };
        let faces: Vec<_> = f.fonts.iter().map(|m| m.face()).collect();
        let mut s = String::new();
        s.push_str(&format!("/Span << /ActualText <{}> >> BDC\nq\n{rgb} rg\n", utf16_hex(&self.latex)));
        for it in &self.items {
            match *it {
                Item::Glyph { font, gid, x: gx, y: gy, scale } => {
                    let Some(Some(face)) = faces.get(font as usize) else { continue };
                    let k = self.size * scale / f.fonts[font as usize].upem;
                    let mut b = QuadFix {
                        inner: PathOut { s: &mut s, k, x: x + gx * self.size, y: y + gy * self.size },
                        cur: (0.0, 0.0),
                        start: (0.0, 0.0),
                    };
                    if face.outline_glyph(GlyphId(gid), &mut b).is_some() {
                        s.push_str("f\n");
                    }
                }
                Item::Rule { x: rx, y: ry, w, h } => {
                    s.push_str(&format!(
                        "{} {} {} {} re f\n",
                        num(x + rx * self.size),
                        num(y + ry * self.size),
                        num(w * self.size),
                        num(h.max(0.02) * self.size)
                    ));
                }
            }
        }
        s.push_str("Q\nEMC\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format_is_compact() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(12.34567), "12.346");
    }

    #[test]
    fn alphanumerics_map() {
        let plain = omml::RunStyle::default();
        assert_eq!(math_alnum('x', &plain, true), '𝑥');
        assert_eq!(math_alnum('h', &plain, true), 'ℎ');
        let bb = omml::RunStyle { double_struck: true, ..Default::default() };
        assert_eq!(math_alnum('R', &bb, false), 'ℝ');
        assert_eq!(math_alnum('A', &bb, false), '𝔸');
    }

    #[test]
    fn formulas_lay_out_when_a_math_font_exists() {
        if !available() {
            return;
        }
        let frac = layout(r"\frac{a}{b}", true, 10.0).expect("fraction");
        assert!(frac.height > 5.0 && frac.depth > 3.0, "{frac:?}");
        let inline = layout(r"\frac{a}{b}", false, 10.0).unwrap();
        assert!(inline.height < frac.height, "display fractions are taller");
        let sup = layout("x^2", false, 10.0).unwrap();
        assert!(sup.height > layout("x", false, 10.0).unwrap().height);
        let big = layout(r"\sum_{i=1}^{n} i + \sqrt{x^2+y^2} = \int_0^1 f", true, 12.0).unwrap();
        assert!(big.width > 60.0);
        let ops = big.draw(10.0, 20.0, "0 0 0");
        assert!(ops.contains("/ActualText") && ops.contains(" c\n") && ops.ends_with("EMC\n"));
        let m = layout(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}", true, 10.0).unwrap();
        assert!(m.height + m.depth > 20.0, "{m:?}");
        assert!(layout(r"\text{中文} + x", false, 10.0).is_some());
    }

    #[test]
    fn never_panics_on_odd_input() {
        for s in ["", "{", "}", r"\frac", r"\sqrt", "^^", "_", r"\left(", r"\begin{matrix}", "x^{y^{z^{w}}}", r"\\\\"] {
            let _ = layout(s, true, 10.0);
            let _ = layout(s, false, 10.0);
        }
    }
}
