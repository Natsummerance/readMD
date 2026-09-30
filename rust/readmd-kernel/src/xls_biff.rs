//! Native Excel 97-2003 (`.xls`, BIFF8) and Excel 5/95 (BIFF5) reader.
//!
//! Reads the `Workbook` / `Book` stream of the OLE2 container and emits one
//! `## <sheet>` + GFM table per worksheet, in the same shape as the `.xlsx`
//! converter.  Numbers keep Excel's 15 significant digits, cells whose XF
//! format is a date/time are written as ISO dates, shared strings follow
//! `CONTINUE` boundaries (including the per-segment option byte), and BIFF5
//! byte strings are decoded with the workbook `CODEPAGE`.
//!
//! Every read is bounds-checked: malformed input yields `Err`, never a panic,
//! and the grid is capped at 65 536 × 256 cells (the BIFF8 sheet limits).

use crate::convert::{md_cell, CfbReader};
use std::collections::{BTreeMap, HashMap};

const MAX_ROWS: u32 = 65_536;
const MAX_COLS: u32 = 256;
const MAX_OUT: usize = 64 << 20;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn f64_at(d: &[u8], o: usize) -> Option<f64> {
    d.get(o..o + 8).map(|b| f64::from_le_bytes(b.try_into().unwrap()))
}

struct Rec<'a> {
    typ: u16,
    pos: usize,
    data: &'a [u8],
}

fn records(s: &[u8]) -> Vec<Rec<'_>> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p + 4 <= s.len() {
        let typ = u16::from_le_bytes([s[p], s[p + 1]]);
        let len = u16::from_le_bytes([s[p + 2], s[p + 3]]) as usize;
        let end = (p + 4 + len).min(s.len());
        out.push(Rec { typ, pos: p, data: &s[p + 4..end] });
        p += 4 + len;
    }
    out
}

/// Windows code page → encoding for BIFF5 byte strings.
pub(crate) fn codepage(cp: u16) -> &'static encoding_rs::Encoding {
    use encoding_rs::*;
    match cp {
        874 => WINDOWS_874,
        932 => SHIFT_JIS,
        936 => GBK,
        949 => EUC_KR,
        950 => BIG5,
        1250 => WINDOWS_1250,
        1251 => WINDOWS_1251,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        1257 => WINDOWS_1257,
        1258 => WINDOWS_1258,
        10000 => MACINTOSH,
        65001 => UTF_8,
        _ => WINDOWS_1252,
    }
}

/// A byte cursor over a record and its `CONTINUE` records.
struct Segs<'a> {
    segs: Vec<&'a [u8]>,
    i: usize,
    p: usize,
}

impl<'a> Segs<'a> {
    fn byte(&mut self) -> Option<u8> {
        loop {
            let s = self.segs.get(self.i)?;
            if self.p < s.len() {
                let b = s[self.p];
                self.p += 1;
                return Some(b);
            }
            self.i += 1;
            self.p = 0;
        }
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes([self.byte()?, self.byte()?]))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes([self.byte()?, self.byte()?, self.byte()?, self.byte()?]))
    }
    fn skip(&mut self, mut n: usize) -> Option<()> {
        while n > 0 {
            let s = self.segs.get(self.i)?;
            let take = (s.len() - self.p).min(n);
            self.p += take;
            n -= take;
            if n > 0 {
                self.i += 1;
                self.p = 0;
            }
        }
        Some(())
    }
    /// `cch` characters; a segment boundary inside the character data starts
    /// with a fresh option byte that may switch between 8- and 16-bit.
    fn chars(&mut self, cch: usize, mut high: bool) -> Option<String> {
        let mut units: Vec<u16> = Vec::with_capacity(cch.min(32_768));
        let mut left = cch;
        while left > 0 {
            let seg_len = self.segs.get(self.i)?.len();
            if self.p >= seg_len {
                self.i += 1;
                self.p = 0;
                self.segs.get(self.i)?;
                high = self.byte()? & 1 != 0;
                continue;
            }
            let s = self.segs[self.i];
            let avail = if high { (s.len() - self.p) / 2 } else { s.len() - self.p };
            if avail == 0 {
                self.p = s.len();
                continue;
            }
            let n = avail.min(left);
            for k in 0..n {
                if high {
                    let o = self.p + 2 * k;
                    units.push(u16::from_le_bytes([s[o], s[o + 1]]));
                } else {
                    units.push(s[self.p + k] as u16);
                }
            }
            self.p += if high { 2 * n } else { n };
            left -= n;
        }
        Some(String::from_utf16_lossy(&units))
    }
}

/// `XLUnicodeRichExtendedString` (SST entries).
fn rich_string(c: &mut Segs) -> Option<String> {
    let cch = c.u16()? as usize;
    let flags = c.byte()?;
    let runs = if flags & 0x08 != 0 { c.u16()? as usize } else { 0 };
    let ext = if flags & 0x04 != 0 { c.u32()? as usize } else { 0 };
    let s = c.chars(cch, flags & 1 != 0)?;
    c.skip(runs * 4)?;
    c.skip(ext)?;
    Some(s)
}

/// `XLUnicodeString` inside one record (LABEL / STRING / FORMAT, BIFF8).
fn unicode_string(d: &[u8], o: usize, len16: bool) -> Option<String> {
    let (cch, o) = if len16 { (u16_at(d, o)? as usize, o + 2) } else { (*d.get(o)? as usize, o + 1) };
    let flags = *d.get(o)?;
    let body = &d[o + 1..];
    if flags & 1 != 0 {
        let n = cch.min(body.len() / 2);
        let units: Vec<u16> = (0..n).map(|k| u16::from_le_bytes([body[2 * k], body[2 * k + 1]])).collect();
        Some(String::from_utf16_lossy(&units))
    } else {
        Some(body[..cch.min(body.len())].iter().map(|&b| b as char).collect())
    }
}

fn byte_string(d: &[u8], o: usize, len16: bool, enc: &'static encoding_rs::Encoding) -> Option<String> {
    let (cch, o) = if len16 { (u16_at(d, o)? as usize, o + 2) } else { (*d.get(o)? as usize, o + 1) };
    let body = d.get(o..)?;
    Some(enc.decode_without_bom_handling(&body[..cch.min(body.len())]).0.into_owned())
}

fn rk_value(rk: u32) -> f64 {
    let v = if rk & 2 != 0 { ((rk as i32) >> 2) as f64 } else { f64::from_bits(((rk & 0xFFFF_FFFC) as u64) << 32) };
    if rk & 1 != 0 {
        v / 100.0
    } else {
        v
    }
}

/// Excel shows numbers with 15 significant digits.
pub(crate) fn fmt_number(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let r: f64 = format!("{v:.14e}").parse().unwrap_or(v);
    format!("{r}")
}

fn is_date_format(code: &str) -> bool {
    let mut s = String::new();
    let mut chars = code.chars().peekable();
    let mut in_q = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => in_q = !in_q,
            _ if in_q => {}
            '\\' | '_' | '*' => {
                chars.next();
            }
            '[' => {
                // `[h]` / `[mm]` / `[ss]` are elapsed time; colours and locales are not.
                let mut inner = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    inner.push(d);
                }
                let l = inner.to_ascii_lowercase();
                if !l.is_empty() && l.chars().all(|c| matches!(c, 'h' | 'm' | 's')) {
                    s.push('h');
                }
            }
            _ => s.push(c.to_ascii_lowercase()),
        }
    }
    let s = s.split(';').next().unwrap_or("").to_string();
    s.contains(['y', 'd', 'h', 's']) || (s.contains('m') && !s.contains('0') && !s.contains('#'))
}

fn builtin_date(ifmt: u16) -> bool {
    matches!(ifmt, 14..=22 | 27..=36 | 45..=47 | 50..=58)
}

/// Days-from-civil inverse (Howard Hinnant).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub(crate) fn fmt_date(serial: f64, date1904: bool) -> String {
    if !serial.is_finite() || !(0.0..3_000_000.0).contains(&serial) {
        return fmt_number(serial);
    }
    let mut days = serial.floor() as i64;
    let mut secs = ((serial - serial.floor()) * 86_400.0).round() as i64;
    if secs >= 86_400 {
        days += 1;
        secs -= 86_400;
    }
    let time = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    if days == 0 && !date1904 {
        return time;
    }
    let date = if !date1904 && days == 60 {
        "1900-02-29".to_string()
    } else {
        // Unix-epoch day numbers of the two Excel epochs.
        let epoch = if date1904 { -24_107 } else if days < 60 { -25_568 } else { -25_569 };
        let (y, m, d) = civil(epoch + days);
        format!("{y:04}-{m:02}-{d:02}")
    };
    if secs == 0 {
        date
    } else {
        format!("{date} {time}")
    }
}

fn error_text(code: u8) -> &'static str {
    match code {
        0x00 => "#NULL!",
        0x07 => "#DIV/0!",
        0x0F => "#VALUE!",
        0x17 => "#REF!",
        0x1D => "#NAME?",
        0x24 => "#NUM!",
        0x2A => "#N/A",
        _ => "#ERR",
    }
}

struct Sheet {
    name: String,
    pos: u32,
    cells: BTreeMap<(u32, u32), String>,
}

/// Convert an `.xls` file's bytes; `title` becomes the `# ` heading.
pub fn xls_to_md(data: &[u8], title: &str) -> Result<String, String> {
    let cfb = CfbReader::parse(data).ok_or("不是有效的 OLE2 复合文档")?;
    let stream = cfb
        .get_stream("Workbook")
        .or_else(|| cfb.get_stream("Book"))
        .ok_or("缺少 Workbook 数据流")?;
    workbook_to_md(&stream, title)
}

fn workbook_to_md(stream: &[u8], title: &str) -> Result<String, String> {
    let recs = records(stream);
    let first = recs.first().filter(|r| r.typ == 0x0809).ok_or("不支持的 Excel 版本（缺少 BIFF5/8 BOF）")?;
    if let Some(sub) = u16_at(first.data, 2) {
        if sub != 0x0005 {
            return Err("不是工作簿数据流".into());
        }
    }
    let biff8 = u16_at(first.data, 0) == Some(0x0600);
    if recs.iter().any(|r| r.typ == 0x002F) {
        return Err("工作簿已加密，无法读取".into());
    }

    let mut enc = codepage(1252);
    let mut date1904 = false;
    let mut formats: HashMap<u16, String> = HashMap::new();
    let mut xf_fmt: Vec<u16> = Vec::new();
    let mut sst: Vec<String> = Vec::new();
    let mut sheets: Vec<Sheet> = Vec::new();

    // --- workbook globals -------------------------------------------------
    let mut i = 0usize;
    while i < recs.len() {
        let r = &recs[i];
        match r.typ {
            0x000A => break,
            0x0042 => {
                if let Some(cp) = u16_at(r.data, 0) {
                    enc = codepage(cp);
                }
            }
            0x0022 => date1904 = u16_at(r.data, 0) == Some(1),
            0x041E => {
                if let Some(id) = u16_at(r.data, 0) {
                    let s = if biff8 { unicode_string(r.data, 2, true) } else { byte_string(r.data, 2, false, enc) };
                    formats.insert(id, s.unwrap_or_default());
                }
            }
            0x00E0 => xf_fmt.push(u16_at(r.data, 2).unwrap_or(0)),
            0x0085 => {
                let pos = u32_at(r.data, 0).unwrap_or(u32::MAX);
                let kind = r.data.get(5).copied().unwrap_or(0);
                let name = if biff8 { unicode_string(r.data, 6, false) } else { byte_string(r.data, 6, false, enc) };
                if kind == 0 {
                    sheets.push(Sheet { name: name.unwrap_or_default(), pos, cells: BTreeMap::new() });
                }
            }
            0x00FC if biff8 => {
                let mut segs = vec![r.data];
                let mut j = i + 1;
                while j < recs.len() && recs[j].typ == 0x003C {
                    segs.push(recs[j].data);
                    j += 1;
                }
                let mut c = Segs { segs, i: 0, p: 0 };
                let _total = c.u32();
                let unique = c.u32().unwrap_or(0).min(4_000_000);
                for _ in 0..unique {
                    match rich_string(&mut c) {
                        Some(s) => sst.push(s),
                        None => break,
                    }
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if sheets.is_empty() {
        return Err("工作簿中没有工作表".into());
    }

    let is_date = |xf: u16| -> bool {
        let ifmt = match xf_fmt.get(xf as usize) {
            Some(f) => *f,
            None => return false,
        };
        builtin_date(ifmt) || formats.get(&ifmt).map(|c| is_date_format(c)).unwrap_or(false)
    };
    let num = |v: f64, xf: u16| if is_date(xf) { fmt_date(v, date1904) } else { fmt_number(v) };

    // --- worksheets -------------------------------------------------------
    let by_pos: HashMap<u32, usize> = sheets.iter().enumerate().map(|(k, s)| (s.pos, k)).collect();
    let mut cur: Option<usize> = None;
    let mut pending_string: Option<(u32, u32)> = None;
    for r in &recs[i.min(recs.len())..] {
        let rc = |d: &[u8]| -> Option<(u32, u32, u16)> { Some((u16_at(d, 0)? as u32, u16_at(d, 2)? as u32, u16_at(d, 4)?)) };
        match r.typ {
            0x0809 => {
                cur = by_pos.get(&(r.pos as u32)).copied();
                continue;
            }
            0x000A => {
                cur = None;
                continue;
            }
            _ => {}
        }
        let Some(k) = cur else { continue };
        let mut put = |row: u32, col: u32, v: String| {
            if row < MAX_ROWS && col < MAX_COLS && !v.is_empty() {
                sheets[k].cells.insert((row, col), v);
            }
        };
        match r.typ {
            0x00FD => {
                if let (Some((row, col, _)), Some(idx)) = (rc(r.data), u32_at(r.data, 6)) {
                    put(row, col, sst.get(idx as usize).cloned().unwrap_or_default());
                }
            }
            0x0204 | 0x00D6 => {
                if let Some((row, col, _)) = rc(r.data) {
                    let s = if biff8 { unicode_string(r.data, 6, true) } else { byte_string(r.data, 6, true, enc) };
                    put(row, col, s.unwrap_or_default());
                }
            }
            0x0203 => {
                if let (Some((row, col, xf)), Some(v)) = (rc(r.data), f64_at(r.data, 6)) {
                    put(row, col, num(v, xf));
                }
            }
            0x027E => {
                if let (Some((row, col, xf)), Some(rk)) = (rc(r.data), u32_at(r.data, 6)) {
                    put(row, col, num(rk_value(rk), xf));
                }
            }
            0x00BD => {
                if let (Some(row), Some(first)) = (u16_at(r.data, 0), u16_at(r.data, 2)) {
                    let n = r.data.len().saturating_sub(6) / 6;
                    for k2 in 0..n {
                        let o = 4 + k2 * 6;
                        if let (Some(xf), Some(rk)) = (u16_at(r.data, o), u32_at(r.data, o + 2)) {
                            put(row as u32, first as u32 + k2 as u32, num(rk_value(rk), xf));
                        }
                    }
                }
            }
            0x0205 => {
                if let Some((row, col, _)) = rc(r.data) {
                    let v = r.data.get(6).copied().unwrap_or(0);
                    let is_err = r.data.get(7).copied().unwrap_or(0) != 0;
                    let s = if is_err { error_text(v).to_string() } else if v != 0 { "TRUE".into() } else { "FALSE".into() };
                    put(row, col, s);
                }
            }
            0x0006 => {
                if let Some((row, col, xf)) = rc(r.data) {
                    let res = r.data.get(6..14).unwrap_or(&[]);
                    if res.len() == 8 && res[6] == 0xFF && res[7] == 0xFF {
                        match res[0] {
                            0 => pending_string = Some((row, col)),
                            1 => put(row, col, if res[2] != 0 { "TRUE".into() } else { "FALSE".into() }),
                            2 => put(row, col, error_text(res[2]).to_string()),
                            _ => {}
                        }
                    } else if let Some(v) = f64_at(r.data, 6) {
                        put(row, col, num(v, xf));
                    }
                }
            }
            0x0207 => {
                if let Some((row, col)) = pending_string.take() {
                    let s = if biff8 { unicode_string(r.data, 0, true) } else { byte_string(r.data, 0, true, enc) };
                    put(row, col, s.unwrap_or_default());
                }
            }
            _ => {}
        }
    }

    let mut out: Vec<String> = vec![format!("# {title}"), String::new()];
    let mut produced = false;
    let mut size = 0usize;
    for s in &sheets {
        let Some(table) = grid_table(&s.cells) else { continue };
        let name = if s.name.trim().is_empty() { "Sheet".to_string() } else { s.name.clone() };
        out.push(format!("## {name}"));
        out.push(String::new());
        for l in table {
            size += l.len() + 1;
            if size > MAX_OUT {
                return Err("表格内容超过 64 MiB 上限".into());
            }
            out.push(l);
        }
        out.push(String::new());
        produced = true;
    }
    if !produced {
        return Err("工作表中没有数据".into());
    }
    Ok(out.join("\n").trim().to_string() + "\n")
}

/// Dense GFM table from sparse cells, trimming blank edge rows/columns.
pub(crate) fn grid_table(cells: &BTreeMap<(u32, u32), String>) -> Option<Vec<String>> {
    if cells.is_empty() {
        return None;
    }
    let r0 = cells.keys().map(|k| k.0).min()?;
    let r1 = cells.keys().map(|k| k.0).max()?;
    let c0 = cells.keys().map(|k| k.1).min()?;
    let c1 = cells.keys().map(|k| k.1).max()?;
    let width = (c1 - c0 + 1) as usize;
    let mut lines = Vec::new();
    let mut first = true;
    for r in r0..=r1 {
        let row: Vec<String> = (c0..=c1).map(|c| cells.get(&(r, c)).map(|v| md_cell(v)).unwrap_or_default()).collect();
        lines.push(format!("| {} |", row.join(" | ")));
        if first {
            lines.push(format!("| {} |", vec!["---"; width].join(" | ")));
            first = false;
        }
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(typ: u16, body: &[u8]) -> Vec<u8> {
        let mut v = typ.to_le_bytes().to_vec();
        v.extend((body.len() as u16).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn cell(row: u16, col: u16, xf: u16) -> Vec<u8> {
        let mut v = row.to_le_bytes().to_vec();
        v.extend(col.to_le_bytes());
        v.extend(xf.to_le_bytes());
        v
    }

    /// A minimal BIFF8 workbook: globals with an SST split across CONTINUE,
    /// one sheet with strings, numbers, an RK, a date and a boolean.
    fn workbook() -> Vec<u8> {
        let mut globals = Vec::new();
        globals.extend(rec(0x0809, &[0x00, 0x06, 0x05, 0x00, 0, 0, 0, 0]));
        globals.extend(rec(0x0042, &1252u16.to_le_bytes()));
        // XF 0 general, XF 1 date (builtin 14)
        let mut xf0 = vec![0u8; 20];
        xf0[2..4].copy_from_slice(&0u16.to_le_bytes());
        let mut xf1 = vec![0u8; 20];
        xf1[2..4].copy_from_slice(&14u16.to_le_bytes());
        globals.extend(rec(0x00E0, &xf0));
        globals.extend(rec(0x00E0, &xf1));
        // SST: "名称" (16-bit) and "Hello" split across CONTINUE with a new flag byte.
        let mut sst = Vec::new();
        sst.extend(2u32.to_le_bytes());
        sst.extend(2u32.to_le_bytes());
        sst.extend(2u16.to_le_bytes());
        sst.push(1);
        for u in "名称".encode_utf16() {
            sst.extend(u.to_le_bytes());
        }
        sst.extend(5u16.to_le_bytes());
        sst.push(0);
        sst.extend(b"He");
        globals.extend(rec(0x00FC, &sst));
        let mut cont = vec![0u8];
        cont.extend(b"llo");
        globals.extend(rec(0x003C, &cont));
        // BOUNDSHEET placeholder, patched below.
        let bs_at = globals.len();
        let mut bs = vec![0u8; 6];
        bs.extend([6u8, 0]);
        bs.extend(b"Sheet1");
        globals.extend(rec(0x0085, &bs));
        globals.extend(rec(0x000A, &[]));
        let sheet_pos = globals.len() as u32;
        globals[bs_at + 4..bs_at + 8].copy_from_slice(&sheet_pos.to_le_bytes());

        let mut s = Vec::new();
        s.extend(rec(0x0809, &[0x00, 0x06, 0x10, 0x00, 0, 0, 0, 0]));
        let mut l = cell(0, 0, 0);
        l.extend(0u32.to_le_bytes());
        s.extend(rec(0x00FD, &l));
        let mut l = cell(0, 1, 0);
        l.extend(1u32.to_le_bytes());
        s.extend(rec(0x00FD, &l));
        let mut n = cell(1, 0, 0);
        n.extend(3.25f64.to_le_bytes());
        s.extend(rec(0x0203, &n));
        let mut rk = cell(1, 1, 0);
        rk.extend(((42u32 << 2) | 2).to_le_bytes());
        s.extend(rec(0x027E, &rk));
        let mut d = cell(2, 0, 1);
        d.extend(45292f64.to_le_bytes()); // 2024-01-01
        s.extend(rec(0x0203, &d));
        let mut b = cell(2, 1, 0);
        b.extend([1, 0]);
        s.extend(rec(0x0205, &b));
        s.extend(rec(0x000A, &[]));
        globals.extend(s);
        globals
    }

    #[test]
    fn biff8_workbook_to_table() {
        let md = workbook_to_md(&workbook(), "t.xls").unwrap();
        assert_eq!(
            md,
            "# t.xls\n\n## Sheet1\n\n| 名称 | Hello |\n| --- | --- |\n| 3.25 | 42 |\n| 2024-01-01 | TRUE |\n"
        );
    }

    #[test]
    fn numbers_and_dates_format_like_excel() {
        assert_eq!(fmt_number(0.1 + 0.2), "0.3");
        assert_eq!(fmt_number(1e20), "100000000000000000000");
        assert_eq!(fmt_date(1.0, false), "1900-01-01");
        assert_eq!(fmt_date(61.0, false), "1900-03-01");
        assert_eq!(fmt_date(45292.5, false), "2024-01-01 12:00:00");
        assert_eq!(fmt_date(0.0, true), "1904-01-01");
        assert!(is_date_format("yyyy/mm/dd"));
        assert!(is_date_format("[h]:mm"));
        assert!(!is_date_format("#,##0.00"));
        assert!(!is_date_format("\"m\"0"));
        assert_eq!(rk_value(0x3FF0_0000), 1.0);
        assert_eq!(rk_value((1234u32 << 2) | 3), 12.34);
    }

    #[test]
    fn garbage_never_panics() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let good = workbook();
        for n in 0..400 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut v = good.clone();
            v.truncate((seed as usize) % (good.len() + 1));
            if n % 2 == 0 && !v.is_empty() {
                let at = (seed >> 20) as usize % v.len();
                v[at] ^= (seed >> 8) as u8;
            }
            let _ = workbook_to_md(&v, "x");
            let _ = xls_to_md(&v, "x");
        }
    }
}
