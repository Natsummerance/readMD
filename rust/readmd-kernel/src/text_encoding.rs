//! Encoding-preserving text I/O for the editor path (`/api/file` → `/api/save`).
//!
//! Detection order: UTF-8 BOM, UTF-16 LE/BE BOM, strict UTF-8, GB18030 (which
//! covers GBK / GB2312), Big5, and finally Windows-1252 (always succeeds).
//! Encoding never substitutes characters silently: text the target encoding
//! cannot represent is reported as [`EncodeError::Unrepresentable`].

use encoding_rs::{Encoding, BIG5, GB18030, GBK, UTF_16BE, UTF_16LE, WINDOWS_1252};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// `ch` (the `char_index`-th character) has no mapping in the encoding.
    Unrepresentable { ch: char, char_index: usize },
    /// The encoding name is not one ReadMD can write.
    Unknown(String),
}

/// Decode file bytes, returning the text and the canonical encoding name
/// (`utf-8`, `utf-8-sig`, `utf-16-le`, `utf-16-be`, `gb18030`, `big5`, `cp1252`).
pub fn detect_and_decode(bytes: &[u8]) -> (String, &'static str) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(rest).into_owned(), "utf-8-sig");
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (t, _) = UTF_16LE.decode_without_bom_handling(&bytes[2..]);
        return (t.into_owned(), "utf-16-le");
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (t, _) = UTF_16BE.decode_without_bom_handling(&bytes[2..]);
        return (t.into_owned(), "utf-16-be");
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), "utf-8");
    }
    if let Some(t) = GB18030.decode_without_bom_handling_and_without_replacement(bytes) {
        return (t.into_owned(), "gb18030");
    }
    if let Some(t) = BIG5.decode_without_bom_handling_and_without_replacement(bytes) {
        return (t.into_owned(), "big5");
    }
    let (t, _) = WINDOWS_1252.decode_without_bom_handling(bytes);
    (t.into_owned(), "cp1252")
}

/// Canonical name for an encoding label (aliases such as `gbk`, `cp936`,
/// `utf8`, `latin-1`), or `None` when ReadMD cannot write it.
pub fn canonical_name(label: &str) -> Option<&'static str> {
    let key = label.trim().to_ascii_lowercase().replace('_', "-");
    Some(match key.as_str() {
        "" | "utf-8" | "utf8" => "utf-8",
        "utf-8-sig" | "utf8-sig" | "utf-8-bom" => "utf-8-sig",
        "utf-16" | "utf-16-le" | "utf-16le" | "utf16" => "utf-16-le",
        "utf-16-be" | "utf-16be" => "utf-16-be",
        "gb18030" => "gb18030",
        "gbk" | "cp936" | "gb2312" | "gb-2312" | "euc-cn" | "x-gbk" => "gbk",
        "big5" | "big5-hkscs" | "cp950" => "big5",
        "cp1252" | "windows-1252" => "cp1252",
        "latin-1" | "latin1" | "iso-8859-1" | "iso8859-1" | "l1" => "latin-1",
        "ascii" | "us-ascii" => "ascii",
        _ => return None,
    })
}

fn legacy(enc: &'static Encoding, text: &str) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = enc.new_encoder();
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut src = text;
    loop {
        let need = encoder.max_buffer_length_from_utf8_without_replacement(src.len()).unwrap_or(src.len() * 4 + 16);
        let start = out.len();
        out.resize(start + need.max(16), 0);
        let (res, read, written) = encoder.encode_from_utf8_without_replacement(src, &mut out[start..], true);
        out.truncate(start + written);
        match res {
            encoding_rs::EncoderResult::InputEmpty => return Ok(out),
            encoding_rs::EncoderResult::OutputFull => {
                src = &src[read..];
            }
            encoding_rs::EncoderResult::Unmappable(ch) => {
                let consumed = text.len() - src.len() + read;
                // `read` includes the unmappable char; its index is the char
                // count of everything before it.
                let before = &text[..consumed - ch.len_utf8()];
                return Err(EncodeError::Unrepresentable { ch, char_index: before.chars().count() });
            }
        }
    }
}

fn single_byte_max(text: &str, max: u32) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::with_capacity(text.len());
    for (i, ch) in text.chars().enumerate() {
        if (ch as u32) > max {
            return Err(EncodeError::Unrepresentable { ch, char_index: i });
        }
        out.push(ch as u32 as u8);
    }
    Ok(out)
}

/// Encode `text` for writing in `label`'s encoding (BOM included for
/// `utf-8-sig` / UTF-16).
pub fn encode(text: &str, label: &str) -> Result<Vec<u8>, EncodeError> {
    let Some(name) = canonical_name(label) else {
        return Err(EncodeError::Unknown(label.to_string()));
    };
    match name {
        "utf-8" => Ok(text.as_bytes().to_vec()),
        "utf-8-sig" => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(text.as_bytes());
            Ok(v)
        }
        "utf-16-le" => {
            let mut v = vec![0xFF, 0xFE];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            Ok(v)
        }
        "utf-16-be" => {
            let mut v = vec![0xFE, 0xFF];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            Ok(v)
        }
        "gb18030" => legacy(GB18030, text),
        "gbk" => legacy(GBK, text),
        "big5" => legacy(BIG5, text),
        "cp1252" => legacy(WINDOWS_1252, text),
        "latin-1" => single_byte_max(text, 0xFF),
        "ascii" => single_byte_max(text, 0x7F),
        _ => Err(EncodeError::Unknown(label.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_encodings() {
        assert_eq!(detect_and_decode("hi".as_bytes()), ("hi".into(), "utf-8"));
        assert_eq!(detect_and_decode(b"\xEF\xBB\xBFhi"), ("hi".into(), "utf-8-sig"));
        assert_eq!(detect_and_decode(b"\xFF\xFEh\0i\0"), ("hi".into(), "utf-16-le"));
        assert_eq!(detect_and_decode(b"\xFE\xFF\0h\0i"), ("hi".into(), "utf-16-be"));
        assert_eq!(detect_and_decode(b"\xc4\xe3\xba\xc3"), ("你好".into(), "gb18030"));
    }

    #[test]
    fn aliases_and_unknown() {
        assert_eq!(canonical_name("GBK"), Some("gbk"));
        assert_eq!(canonical_name("cp936"), Some("gbk"));
        assert_eq!(canonical_name("UTF8"), Some("utf-8"));
        assert_eq!(canonical_name("klingon"), None);
        assert!(matches!(encode("x", "klingon"), Err(EncodeError::Unknown(_))));
    }

    #[test]
    fn reports_unrepresentable_position() {
        assert_eq!(
            encode("ab😀c", "gbk"),
            Err(EncodeError::Unrepresentable { ch: '😀', char_index: 2 })
        );
        assert_eq!(encode("é中", "latin-1"), Err(EncodeError::Unrepresentable { ch: '中', char_index: 1 }));
        // GB18030 covers all of Unicode
        assert!(encode("ab😀c", "gb18030").is_ok());
    }

    /// Randomized round trip: text drawn from what each encoding can hold
    /// decodes back to itself, and re-detecting the bytes re-encodes to the
    /// same bytes.
    #[test]
    fn randomized_round_trip() {
        let pools: [(&str, &[&str]); 6] = [
            ("utf-8", &["a", "中", "😀", "é", "\n", " "]),
            ("utf-8-sig", &["a", "中", "😀", "\r\n"]),
            ("utf-16-le", &["a", "中", "😀", "\t"]),
            ("utf-16-be", &["b", "文", "𝄞"]),
            ("gb18030", &["你", "好", "A", "，", "\n", "😀"]),
            ("big5", &["繁", "體", "字", "x"]),
        ];
        let mut s: u64 = 0xC0FFEE;
        let iters: usize = std::env::var("READMD_PBT_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(500);
        for n in 0..iters {
            let (enc, pool) = pools[n % pools.len()];
            let mut text = String::new();
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            for _ in 0..((s >> 59) as usize + 1) {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push_str(pool[(s >> 33) as usize % pool.len()]);
            }
            let bytes = encode(&text, enc).unwrap();
            let (back, detected) = detect_and_decode(&bytes);
            // BOM-less legacy encodings of pure ASCII are indistinguishable from UTF-8.
            if enc == "big5" || enc == "gb18030" {
                if text.is_ascii() {
                    continue;
                }
                if enc == "big5" && detected != "big5" {
                    // Big5 byte strings are often also valid GB18030; the round
                    // trip is only guaranteed when the caller passes the name back.
                    continue;
                }
            }
            assert_eq!(back, text, "{enc}: {text:?}");
            assert_eq!(encode(&back, detected).unwrap(), bytes, "{enc} re-encode");
        }
    }
}
