//! Native MOBI / AZW (PalmDB + MOBI header) reader.
//!
//! Handles uncompressed and PalmDOC (LZ77) text records, multibyte/TBS
//! trailing entries (`extra_data_flags`), UTF-8 and CP1252 text, and
//! `recindex` images (exported next to the source).  DRM-protected books and
//! HUFF/CDIC compression are reported as `unsupported_format` with a reason.
//! Every offset is bounds-checked and the decoded text is capped at 64 MiB.

const MAX_TEXT: usize = 64 << 20;

#[derive(Debug, PartialEq)]
pub enum MobiError {
    Invalid(&'static str),
    Drm,
    HuffCdic,
}

impl MobiError {
    pub fn message(&self) -> String {
        match self {
            MobiError::Invalid(why) => format!("MOBI 解析失败：{why}"),
            MobiError::Drm => "unsupported_format: mobi_drm：该电子书受 DRM 保护，无法转换".into(),
            MobiError::HuffCdic => "unsupported_format: mobi_huffcdic：暂不支持 HUFF/CDIC 压缩的 MOBI".into(),
        }
    }
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}
fn be32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

pub struct Book {
    pub title: String,
    pub html: String,
    /// `recindex` (1-based) → image bytes and extension.
    pub images: Vec<(usize, Vec<u8>, &'static str)>,
}

/// Split a PalmDB into its records.
fn palm_records(d: &[u8]) -> Result<Vec<&[u8]>, MobiError> {
    let n = be16(d, 76).ok_or(MobiError::Invalid("文件过短"))? as usize;
    if n == 0 || 78 + n * 8 > d.len() {
        return Err(MobiError::Invalid("记录表越界"));
    }
    let offs: Vec<usize> = (0..n).map(|i| be32(d, 78 + i * 8).unwrap_or(0) as usize).collect();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let s = offs[i];
        let e = if i + 1 < n { offs[i + 1] } else { d.len() };
        if s > e || e > d.len() {
            return Err(MobiError::Invalid("记录偏移无效"));
        }
        out.push(&d[s..e]);
    }
    Ok(out)
}

/// PalmDOC LZ77 decompression (compression type 2).
pub fn palmdoc_decompress(src: &[u8], out: &mut Vec<u8>) {
    let start = out.len();
    let mut i = 0usize;
    while i < src.len() {
        let c = src[i];
        i += 1;
        match c {
            0x01..=0x08 => {
                let n = (c as usize).min(src.len() - i);
                out.extend_from_slice(&src[i..i + n]);
                i += n;
            }
            0x00 | 0x09..=0x7F => out.push(c),
            0x80..=0xBF => {
                let Some(&c2) = src.get(i) else { break };
                i += 1;
                let pair = ((c as usize) << 8 | c2 as usize) & 0x3FFF;
                let dist = pair >> 3;
                let len = (pair & 7) + 3;
                if dist == 0 || dist > out.len() - start {
                    continue;
                }
                for _ in 0..len {
                    let b = out[out.len() - dist];
                    out.push(b);
                }
            }
            _ => {
                out.push(b' ');
                out.push(c ^ 0x80);
            }
        }
        if out.len() - start > 8 * 4096 {
            break;
        }
    }
}

/// Size of the trailing entries described by `extra_data_flags`.
fn trailing_size(rec: &[u8], flags: u16) -> usize {
    let mut size = 0usize;
    let mut end = rec.len();
    for bit in 1..16 {
        if flags & (1 << bit) == 0 {
            continue;
        }
        // Backward-encoded varint at the end of the remaining data.
        let mut v = 0usize;
        let mut shift = 0;
        let mut p = end;
        while p > 0 && shift < 28 {
            p -= 1;
            let b = rec[p];
            v |= ((b & 0x7F) as usize) << shift;
            shift += 7;
            if b & 0x80 != 0 {
                break;
            }
        }
        let v = v.min(end);
        end -= v;
        size += v;
    }
    if flags & 1 != 0 && end > 0 {
        let n = (rec[end - 1] & 3) as usize + 1;
        size += n.min(end);
    }
    size.min(rec.len())
}

fn image_ext(d: &[u8]) -> Option<&'static str> {
    if d.starts_with(b"\xFF\xD8\xFF") {
        Some("jpg")
    } else if d.starts_with(b"\x89PNG") {
        Some("png")
    } else if d.starts_with(b"GIF8") {
        Some("gif")
    } else if d.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

pub fn parse(d: &[u8]) -> Result<Book, MobiError> {
    if d.len() < 78 {
        return Err(MobiError::Invalid("文件过短"));
    }
    let kind = &d[60..68];
    if kind != b"BOOKMOBI" && kind != b"TEXtREAd" {
        return Err(MobiError::Invalid("不是 MOBI/PalmDOC 文件"));
    }
    let recs = palm_records(d)?;
    let r0 = recs[0];
    let compression = be16(r0, 0).ok_or(MobiError::Invalid("缺少 PalmDOC 头"))?;
    let text_len = be32(r0, 4).unwrap_or(0) as usize;
    let text_count = be16(r0, 8).unwrap_or(0) as usize;
    let encryption = be16(r0, 12).unwrap_or(0);
    if encryption != 0 {
        return Err(MobiError::Drm);
    }
    match compression {
        1 | 2 => {}
        17480 => return Err(MobiError::HuffCdic),
        _ => return Err(MobiError::Invalid("未知的压缩方式")),
    }
    let mut utf8 = false;
    let mut extra_flags = 0u16;
    let mut first_image: Option<usize> = None;
    let mut title = String::new();
    if r0.get(16..20) == Some(b"MOBI") {
        let hlen = be32(r0, 20).unwrap_or(0) as usize;
        utf8 = be32(r0, 28) == Some(65001);
        if let Some(fi) = be32(r0, 16 + 0x5C).filter(|&v| v != 0xFFFF_FFFF && (v as usize) < recs.len()) {
            first_image = Some(fi as usize);
        }
        if hlen >= 0xE4 {
            // MOBI-header offset 0xE2 (record 0 offset 0xF2).
            extra_flags = be16(r0, 16 + 0xE2).unwrap_or(0);
        }
        let (toff, tlen) = (be32(r0, 16 + 0x44).unwrap_or(0) as usize, be32(r0, 16 + 0x48).unwrap_or(0) as usize);
        if let Some(t) = r0.get(toff..toff.saturating_add(tlen)) {
            title = if utf8 { String::from_utf8_lossy(t).into_owned() } else { t.iter().map(|&b| cp1252(b)).collect() };
        }
        // EXTH 0x191 (401) is DRM-related; a non-zero DRM offset means locked.
        if be32(r0, 16 + 0x98).map(|v| v != 0xFFFF_FFFF && v != 0).unwrap_or(false)
            && be32(r0, 16 + 0x9C).map(|v| v > 0).unwrap_or(false)
        {
            return Err(MobiError::Drm);
        }
    }
    if title.is_empty() {
        title = String::from_utf8_lossy(&d[..32]).trim_end_matches('\0').replace('_', " ");
    }

    let mut raw: Vec<u8> = Vec::with_capacity(text_len.min(MAX_TEXT));
    for rec in recs.iter().skip(1).take(text_count) {
        let body = &rec[..rec.len() - trailing_size(rec, extra_flags)];
        if compression == 2 {
            palmdoc_decompress(body, &mut raw);
        } else {
            raw.extend_from_slice(body);
        }
        if raw.len() > MAX_TEXT {
            return Err(MobiError::Invalid("正文超过 64 MiB 上限"));
        }
    }
    if text_len > 0 && raw.len() > text_len {
        raw.truncate(text_len);
    }
    let mut html = if utf8 { String::from_utf8_lossy(&raw).into_owned() } else { raw.iter().map(|&b| cp1252(b)).collect() };
    if kind == b"TEXtREAd" {
        html = format!("<pre>{}</pre>", html.replace('&', "&amp;").replace('<', "&lt;"));
    }

    let mut images = Vec::new();
    if let Some(fi) = first_image {
        for (k, rec) in recs.iter().enumerate().skip(fi) {
            match image_ext(rec) {
                Some(ext) => images.push((k - fi + 1, rec.to_vec(), ext)),
                None => {
                    if rec.starts_with(b"FLIS") || rec.starts_with(b"FCIS") || rec.starts_with(b"EOF") {
                        break;
                    }
                }
            }
        }
    }
    Ok(Book { title, html, images })
}

fn cp1252(b: u8) -> char {
    const HI: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D,
        0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A,
        0x0153, 0x9D, 0x017E, 0x0178,
    ];
    match b {
        0x80..=0x9F => char::from_u32(HI[(b - 0x80) as usize] as u32).unwrap_or('\u{fffd}'),
        _ => b as char,
    }
}

/// Rewrite MOBI markup into ordinary HTML: `<mbp:pagebreak/>` → `<hr>`,
/// `recindex="N"` images → saved asset paths.
pub fn to_html(book: &Book, image_url: &mut dyn FnMut(usize) -> Option<String>) -> String {
    let re_pb = regex::Regex::new(r"(?i)<mbp:pagebreak\s*/?>").unwrap();
    let re_mbp = regex::Regex::new(r"(?i)</?mbp:[^>]*>").unwrap();
    let re_img = regex::Regex::new(r#"(?i)<img\b([^>]*?)\brecindex\s*=\s*["']?0*(\d+)["']?([^>]*)>"#).unwrap();
    let h = re_pb.replace_all(&book.html, "<hr/>");
    let h = re_img.replace_all(&h, |c: &regex::Captures| {
        let n: usize = c[2].parse().unwrap_or(0);
        match image_url(n) {
            Some(u) => format!("<img{} src=\"{}\"{}>", &c[1], u, &c[3]),
            None => String::new(),
        }
    });
    re_mbp.replace_all(&h, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palmdb(kind: &[u8; 8], records: &[Vec<u8>]) -> Vec<u8> {
        let mut d = vec![0u8; 78];
        d[..8].copy_from_slice(b"TestBook");
        d[60..68].copy_from_slice(kind);
        d[76..78].copy_from_slice(&(records.len() as u16).to_be_bytes());
        let mut off = 78 + records.len() * 8 + 2;
        for r in records {
            d.extend((off as u32).to_be_bytes());
            d.extend([0u8; 4]);
            off += r.len();
        }
        d.extend([0, 0]);
        for r in records {
            d.extend_from_slice(r);
        }
        d
    }

    fn rec0(compression: u16, text_len: usize, count: u16, encryption: u16) -> Vec<u8> {
        let mut r = vec![0u8; 16];
        r[0..2].copy_from_slice(&compression.to_be_bytes());
        r[4..8].copy_from_slice(&(text_len as u32).to_be_bytes());
        r[8..10].copy_from_slice(&count.to_be_bytes());
        r[12..14].copy_from_slice(&encryption.to_be_bytes());
        r
    }

    #[test]
    fn lz77_round_trip_examples() {
        let mut out = Vec::new();
        // "abcabcabc": literal "abc" then a back-reference dist 3 len 6.
        let pair: u16 = 0x8000 | (3 << 3) | (6 - 3);
        let mut src = b"abc".to_vec();
        src.extend(pair.to_be_bytes());
        src.push(0xC1); // " A"
        palmdoc_decompress(&src, &mut out);
        assert_eq!(out, b"abcabcabc A");
        // Invalid distance and truncated pair are ignored, not panics.
        let mut out = Vec::new();
        palmdoc_decompress(&[0x80, 0x09, 0x85], &mut out);
    }

    #[test]
    fn uncompressed_book_and_pagebreaks() {
        let html = b"<html><body><h1>Chapter</h1><p>Hi</p><mbp:pagebreak/><p>Two</p></body></html>".to_vec();
        let d = palmdb(b"BOOKMOBI", &[rec0(1, html.len(), 1, 0), html]);
        let b = parse(&d).unwrap();
        assert_eq!(b.title, "TestBook");
        let h = to_html(&b, &mut |_| None);
        assert!(h.contains("<hr/>") && !h.contains("mbp:"), "{h}");
    }

    #[test]
    fn drm_and_huff_are_reported() {
        let d = palmdb(b"BOOKMOBI", &[rec0(2, 1, 1, 2), vec![b'a']]);
        assert_eq!(parse(&d).err(), Some(MobiError::Drm));
        let d = palmdb(b"BOOKMOBI", &[rec0(17480, 1, 1, 0), vec![b'a']]);
        assert_eq!(parse(&d).err(), Some(MobiError::HuffCdic));
        assert!(MobiError::Drm.message().contains("mobi_drm"));
    }

    #[test]
    fn trailing_entries_are_stripped() {
        // flags bit1: one backward varint of size 3 (0x83) at the end.
        let rec = b"text\x00\x00\x83".to_vec();
        assert_eq!(trailing_size(&rec, 0b10), 3);
        // multibyte flag: last byte & 3 + 1.
        assert_eq!(trailing_size(b"ab\x01", 1), 2);
    }

    #[test]
    fn garbage_never_panics() {
        let html = b"<p>hello world hello world</p>".to_vec();
        let good = palmdb(b"BOOKMOBI", &[rec0(1, html.len(), 1, 0), html]);
        let mut seed = 0xDEAD_BEEF_CAFE_F00Du64;
        for n in 0..600 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut v = good.clone();
            v.truncate((seed as usize) % (good.len() + 1));
            if n % 2 == 0 && !v.is_empty() {
                let at = (seed >> 16) as usize % v.len();
                v[at] = (seed >> 40) as u8;
            }
            let _ = parse(&v);
            let mut out = Vec::new();
            palmdoc_decompress(&v, &mut out);
        }
    }
}
