//! `ole2.rs` — minimal pure-Rust OLE2 / CFBF (compound-file binary format) reader.
//!
//! Python authority: `src/readmd_modules/convert.py`
//!   * `OLE2_MAGIC`                                  → [`OLE2_MAGIC`]
//!   * `_extract_ole2_streams(data, wanted_names)`   → [`extract_ole2_streams`]
//!   * `_extract_worddocument_stream(data)`          → [`extract_worddocument_stream`]
//!
//! # Deliberate fidelity, not a "better" CFB reader
//!
//! This module reproduces *exactly* what the Python helper does — including the
//! places where the Python helper is knowingly incomplete.  The upstream code is
//! a heuristic stream extractor for Word 97-2003 documents, not a conformant
//! CFB implementation, and downstream behaviour depends on the difference:
//!
//! 1. **No mini-stream / short-sector support.**  Python only ever walks the
//!    FAT.  A stream below the 4096-byte mini-sector cutoff that actually lives
//!    in the mini stream is *not* found here; it comes back as zero bytes (the
//!    key is still present, so `.get(name)` yields `b''`, which is falsy).  A
//!    conformant reader would find it, and would therefore produce a different
//!    Markdown.  See [`extract_ole2_streams`]’s `get_stream`.
//! 2. **The DIFAT chain is never followed.**  Only the 109 FAT sector ids in the
//!    header at byte offset 76 are read, so a container with more than 109 FAT
//!    sectors has a truncated FAT and its later streams resolve to nothing.
//!    (`difat_sectors_count`/`first_difat_sector` are not read at all.)
//! 3. **`sector_shift` must be 9 or 12.**  A conformant reader accepts 7..=16;
//!    Python returns `{}` for anything else, so a 4096-byte-sector file works
//!    and a 1024-byte-sector file yields no streams.
//! 4. **Stream size is read as a 32-bit little-endian word** at directory
//!    offset 120 — the upper half of the 64-bit `ulSize` field is ignored.
//!    (Offset 124 is never touched.)
//! 5. **Any internal failure returns the partial result**, because the whole
//!    body sits in one `try/except Exception: pass` after `streams = {}`.
//!
//! # Capacity / panics
//!
//! Python is safe here by laziness: `data[offset:offset + n]` clamps silently
//! for out-of-range offsets, and `max_stream_size = min(size, len(data))` is
//! computed *before* anything is copied.  Rust must not `Vec::with_capacity()`
//! from an attacker-controlled header, so every offset here is bounds-checked
//! and copies grow one sector at a time.  Nothing in this module can panic on
//! malformed input; where Python would have produced a short or empty stream,
//! so does this.
//!
//! No `crate::` path is referenced, so the file compiles standalone:
//! `rustc --edition 2021 --crate-type lib ole2.rs`.

/// `convert.py:667` — `OLE2_MAGIC = b'\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1'`
pub const OLE2_MAGIC: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];

/// `FATSECT`/`DIFSECT`/`ENDOFCHAIN`/`FREESECT` boundary used by the Python
/// guard `if sec_id >= 0xFFFFFFFA: continue`.
const SPECIAL_SECTOR_FLOOR: u32 = 0xFFFF_FFFA;

/// The header reserves 109 DIFAT slots at byte offset 76 (`convert.py:888`).
const HEADER_DIFAT_SLOTS: usize = 109;
/// `get_stream(first_dir_sector, 65536)` — the directory chain read cap.
const DIRECTORY_READ_CAP: u32 = 65536;

fn u16_le(data: &[u8], at: usize) -> Option<u16> {
    let a = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([a[0], a[1]]))
}

fn u32_le(data: &[u8], at: usize) -> Option<u32> {
    let a = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([a[0], a[1], a[2], a[3]]))
}

/// Result of [`extract_ole2_streams`], mirroring Python’s `dict`.
///
/// Python’s `dict` keeps **insertion order**, and `_extract_ole2_streams`
/// inserts in *directory entry order*, which is not the order of
/// `wanted_names`.  Real fixtures show both orders: one document yields
/// `{'1Table', 'WordDocument'}` and another `{'WordDocument', '0Table'}`.
/// `entries` therefore preserves that order verbatim; `get` is the
/// `dict.get(name)` accessor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ole2Streams {
    /// `(name, bytes)` in directory-entry insertion order.
    pub entries: Vec<(String, Vec<u8>)>,
}

impl Ole2Streams {
    /// `streams.get(name)`
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.entries.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_slice())
    }

    /// `name in streams`
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == name)
    }

    /// `len(streams)` — number of *distinct* names found.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `if not streams:`
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insertion-order name list.
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    fn insert(&mut self, name: String, bytes: Vec<u8>) {
        // `streams[name] = ...` replaces the value but keeps the *original*
        // insertion position for an already-present key (Python dict rule).
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == name) {
            slot.1 = bytes;
        } else {
            self.entries.push((name, bytes));
        }
    }
}

/// `convert.py:897-908` — the `get_stream` closure.
///
/// Walks `start_sec` through the FAT, appending one sector at a time, capped by
/// `max_stream_size = min(size, len(data))`.  Termination conditions are exactly
/// Python's four-part `while` guard, evaluated left to right:
/// `cur < len(fat)`, `cur < 0xFFFFFFFA`, `len(stream_bytes) < max_stream_size`,
/// `cur not in visited`.
///
/// Out-of-range sector offsets are *not* an error: Python’s slice yields `b''`,
/// the buffer does not grow, and the walk continues to the next FAT link.  That
/// is why a chain pointing past EOF returns `b''` rather than a partial stream.
fn get_stream(data: &[u8], fat: &[u32], sector_size: usize, start_sec: u32, size: u32) -> Vec<u8> {
    // `min(size, len(data))` — a stream can never exceed its container.
    let max_stream_size = std::cmp::min(size as usize, data.len());
    let mut out: Vec<u8> = Vec::new();
    let mut cur: u32 = start_sec;
    let mut visited: Vec<u32> = Vec::new();

    while (cur as usize) < fat.len()
        && cur < SPECIAL_SECTOR_FLOOR
        && out.len() < max_stream_size
        && !visited.contains(&cur)
    {
        visited.push(cur);
        let offset = 512usize + (cur as usize).saturating_mul(sector_size);
        // Python: data[offset : offset + min(sector_size, max_stream_size - len)]
        // Rust slicing would panic on a bad range, so clamp explicitly; an
        // offset at or past EOF must yield an empty chunk, exactly like the
        // out-of-range Python slice.
        let want = std::cmp::min(sector_size, max_stream_size - out.len());
        if let Some(chunk) = slice_clamped(data, offset, want) {
            out.extend_from_slice(chunk);
        }
        cur = fat[cur as usize];
    }
    out
}

/// `data[from .. from + len]` clamped to the buffer, `None` when `from >= len`.
fn slice_clamped(data: &[u8], from: usize, len: usize) -> Option<&[u8]> {
    if from >= data.len() {
        return None;
    }
    let end = std::cmp::min(from.saturating_add(len), data.len());
    Some(&data[from..end])
}

/// `convert.py:873-935` — `_extract_ole2_streams(data, wanted_names)`.
///
/// Returns `{}` (`Ole2Streams::default()`) for every input Python rejects:
/// shorter than 512 bytes, wrong magic, `sector_shift` other than 9 or 12, an
/// unreadable directory chain, or any raised exception (see the module docs
/// for the failure-path tests that pin each of these).
pub fn extract_ole2_streams(data: &[u8], wanted_names: &[&str]) -> Ole2Streams {
    let mut streams = Ole2Streams::default();
    if data.len() < 512 || data[..8] != OLE2_MAGIC {
        return streams;
    }

    // --- header -------------------------------------------------------------
    let sector_shift = match u16_le(data, 30) {
        Some(v) => v,
        None => return streams,
    };
    if sector_shift != 9 && sector_shift != 12 {
        return streams;
    }
    let sector_size = 1usize << sector_shift;
    let fat_sectors_count = match u32_le(data, 44) {
        Some(v) => v,
        None => return streams,
    };
    let first_dir_sector = match u32_le(data, 48) {
        Some(v) => v,
        None => return streams,
    };

    // --- FAT from the header's 109 DIFAT slots only -------------------------
    let fat_entries_per_sector = sector_size / 4;
    let mut fat: Vec<u32> = Vec::new();
    let slots = std::cmp::min(HEADER_DIFAT_SLOTS, fat_sectors_count as usize);
    for i in 0..slots {
        let sec_id = match u32_le(data, 76 + i * 4) {
            Some(v) => v,
            // i < 109 keeps the offset inside the 512-byte header, so this is
            // unreachable for a >=512-byte file; kept for total safety.
            None => return streams,
        };
        if sec_id >= SPECIAL_SECTOR_FLOOR {
            continue;
        }
        let offset = 512usize + (sec_id as usize).saturating_mul(sector_size);
        if offset + sector_size <= data.len() {
            for j in 0..fat_entries_per_sector {
                match u32_le(data, offset + j * 4) {
                    Some(v) => fat.push(v),
                    None => break,
                }
            }
        }
    }

    // --- directory stream ---------------------------------------------------
    let dir_data = get_stream(data, &fat, sector_size, first_dir_sector, DIRECTORY_READ_CAP);
    if dir_data.is_empty() {
        return streams;
    }

    // `wanted_set = set(wanted_names)`; the early `break` compares
    // `len(streams) == len(wanted_set)`, i.e. the count of *distinct* names.
    let mut wanted: Vec<&str> = Vec::new();
    for n in wanted_names {
        if !wanted.contains(n) {
            wanted.push(n);
        }
    }

    let mut i = 0usize;
    while i < dir_data.len() {
        let end = i + 128;
        let entry = match dir_data.get(i..std::cmp::min(end, dir_data.len())) {
            Some(e) => e,
            None => break,
        };
        if entry.len() < 128 {
            break; // `if len(entry) < 128: break`
        }
        let name_len = match u16_le(entry, 64) {
            Some(v) => v as usize,
            None => break,
        };
        if name_len <= 2 || name_len > 64 {
            i = end;
            continue;
        }
        // `name_raw = entry[:name_len - 2]` — a short slice is not an error in
        // Python, and an odd length makes `decode('utf-16le')` raise, which the
        // inner `except Exception: continue` swallows.
        let name_raw = &entry[..std::cmp::min(name_len - 2, entry.len())];
        let name = match decode_utf16le_strict(name_raw) {
            Some(n) => n,
            None => {
                i = end;
                continue;
            }
        };
        if wanted.contains(&name.as_str()) {
            let start_sec = match u32_le(entry, 116) {
                Some(v) => v,
                None => break,
            };
            // NOTE: '<I' at offset 120 — 32-bit, upper half of ulSize ignored.
            let size = match u32_le(entry, 120) {
                Some(v) => v,
                None => break,
            };
            streams.insert(name, get_stream(data, &fat, sector_size, start_sec, size));
            if streams.len() == wanted.len() {
                break;
            }
        }
        i = end;
    }
    streams
}

/// `convert.py:938-941` — `_extract_worddocument_stream(data)`.
///
/// Python returns `res.get('WordDocument')`, so a *present but empty* stream
/// yields `b''` and an *absent* one yields `None`; the two are distinct states
/// and callers test them with `if not word_doc`.  This returns `Option<Vec<u8>>`
/// where `None` is the absent key.
pub fn extract_worddocument_stream(data: &[u8]) -> Option<Vec<u8>> {
    extract_ole2_streams(data, &["WordDocument"])
        .get("WordDocument")
        .map(|v| v.to_vec())
}

/// Python's `bytes.decode('utf-16le')` — **strict**: an odd byte count, a lone
/// surrogate, or an unpaired low surrogate all raise `UnicodeDecodeError`, which
/// `_extract_ole2_streams` turns into "skip this directory entry".
pub fn decode_utf16le_strict(raw: &[u8]) -> Option<String> {
    if raw.len() % 2 != 0 {
        return None;
    }
    let mut out = String::with_capacity(raw.len() / 2);
    let mut i = 0usize;
    while i < raw.len() {
        let unit = u16::from_le_bytes([raw[i], raw[i + 1]]);
        i += 2;
        if (0xD800..=0xDBFF).contains(&unit) {
            if i + 1 >= raw.len() {
                return None;
            }
            let low = u16::from_le_bytes([raw[i], raw[i + 1]]);
            if !(0xDC00..=0xDFFF).contains(&low) {
                return None;
            }
            i += 2;
            let cp = 0x10000 + (((unit as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
            out.push(char::from_u32(cp)?);
        } else if (0xDC00..=0xDFFF).contains(&unit) {
            return None;
        } else {
            out.push(char::from_u32(unit as u32)?);
        }
    }
    Some(out)
}

/// Python's `bytes.decode('utf-16le', errors='ignore')` — silently drops a
/// trailing odd byte, a lone high surrogate, a lone low surrogate, and a high
/// surrogate not followed by a low one.  (`_parse_doc_stream_to_md` uses this.)
pub fn decode_utf16le_ignore(raw: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    while i + 1 < raw.len() {
        let unit = u16::from_le_bytes([raw[i], raw[i + 1]]);
        i += 2;
        if (0xD800..=0xDBFF).contains(&unit) {
            if i + 1 < raw.len() {
                let low = u16::from_le_bytes([raw[i], raw[i + 1]]);
                if (0xDC00..=0xDFFF).contains(&low) {
                    i += 2;
                    let cp = 0x10000 + (((unit as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                    if let Some(c) = char::from_u32(cp) {
                        out.push(c);
                    }
                    continue;
                }
            }
            // unpaired high surrogate: dropped, and the next unit is decoded
            // normally (measured: b'\x00\xd8\x00A' -> "伀").
            continue;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            continue;
        }
        if let Some(c) = char::from_u32(unit as u32) {
            out.push(c);
        }
    }
    out
}

/// Python's `bytes.decode('utf-16le', errors='replace')` — same walk, but each
/// rejected unit (and a trailing odd byte) becomes exactly one U+FFFD.
/// Measured: `b'\x00\xd8\x00A'` -> `['\ufffd', '伀']`; `b'A'` -> `['\ufffd']`.
pub fn decode_utf16le_replace(raw: &[u8]) -> String {
    const REPL: char = '\u{fffd}';
    let mut out = String::new();
    let mut i = 0usize;
    while i < raw.len() {
        if i + 1 >= raw.len() {
            out.push(REPL); // trailing odd byte
            break;
        }
        let unit = u16::from_le_bytes([raw[i], raw[i + 1]]);
        i += 2;
        if (0xD800..=0xDBFF).contains(&unit) {
            if i + 1 < raw.len() {
                let low = u16::from_le_bytes([raw[i], raw[i + 1]]);
                if (0xDC00..=0xDFFF).contains(&low) {
                    i += 2;
                    let cp = 0x10000 + (((unit as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                    match char::from_u32(cp) {
                        Some(c) => out.push(c),
                        None => out.push(REPL),
                    }
                    continue;
                }
            }
            out.push(REPL);
            continue;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            out.push(REPL);
            continue;
        }
        match char::from_u32(unit as u32) {
            Some(c) => out.push(c),
            None => out.push(REPL),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal one-shot compound-file builder, so the failure-path tests do not
    /// depend on fixture availability.  Header + FAT sector + dir sector.
    fn craft(sector_shift: u16, difat: &[u32], dir_sec: u32, entries: &[(&str, u32, u32)]) -> Vec<u8> {
        // Only ever used for buffer padding; the real code rejects every shift
        // but 9 and 12 before it computes a sector size.
        let sector_size = 1usize << sector_shift.min(12);
        let mut d = vec![0u8; 512];
        d[..8].copy_from_slice(&OLE2_MAGIC);
        d[30..32].copy_from_slice(&sector_shift.to_le_bytes());
        d[32..34].copy_from_slice(&6u16.to_le_bytes());
        d[44..48].copy_from_slice(&(difat.len() as u32).to_le_bytes());
        d[48..52].copy_from_slice(&dir_sec.to_le_bytes());
        for (i, s) in difat.iter().enumerate().take(109) {
            d[76 + i * 4..80 + i * 4].copy_from_slice(&s.to_le_bytes());
        }
        let mut out = d;
        out.resize(512 + sector_size * 8, 0);
        // FAT into each listed sector
        let mut fat = vec![0xFFFF_FFFFu32; sector_size / 4];
        for &s in difat.iter() {
            if (s as usize) < fat.len() {
                fat[s as usize] = 0xFFFF_FFFE;
            }
        }
        for &s in difat.iter() {
            let off = 512 + (s as usize) * sector_size;
            if off + sector_size <= out.len() {
                for (j, v) in fat.iter().enumerate() {
                    out[off + j * 4..off + j * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        // directory at dir_sec
        let doff = 512 + (dir_sec as usize) * sector_size;
        for (i, (name, start, size)) in entries.iter().enumerate() {
            let e = doff + i * 128;
            if e + 128 > out.len() {
                break;
            }
            let nb = name.encode_utf16().collect::<Vec<u16>>();
            for (k, u) in nb.iter().take(32).enumerate() {
                out[e + k * 2..e + k * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            let nl = (nb.len() * 2 + 2) as u16;
            out[e + 64..e + 66].copy_from_slice(&nl.to_le_bytes());
            out[e + 66] = 2;
            out[e + 116..e + 120].copy_from_slice(&start.to_le_bytes());
            out[e + 120..e + 124].copy_from_slice(&size.to_le_bytes());
        }
        out.truncate(doff + entries.len().max(1) * 128);
        out
    }

    fn fatsec(vals: &[u32]) -> Vec<u8> {
        let mut s = vec![0u8; 512];
        for (i, v) in vals.iter().enumerate() {
            s[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        s
    }

    /// A well-formed one-sector file: FAT in sector 0, dir in sector 1, payload
    /// ("HELLO") in sector 2.  Reproduces the golden `fat_sec_id_0` probe.
    fn happy_path() -> Vec<u8> {
        let mut out = vec![0u8; 512];
        out[..8].copy_from_slice(&OLE2_MAGIC);
        out[30..32].copy_from_slice(&9u16.to_le_bytes());
        out[44..48].copy_from_slice(&1u32.to_le_bytes()); // fat_sectors_count
        out[48..52].copy_from_slice(&1u32.to_le_bytes()); // first_dir_sector
        out[76..80].copy_from_slice(&0u32.to_le_bytes()); // DIFAT[0] = sector 0
        out.extend_from_slice(&fatsec(&[0xFFFF_FFFD, 0xFFFF_FFFE, 0xFFFF_FFFE]));
        let mut dir = vec![0u8; 512];
        let nb = "WordDocument".encode_utf16().collect::<Vec<u16>>();
        for (k, u) in nb.iter().enumerate() {
            dir[k * 2..k * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        dir[64..66].copy_from_slice(&((nb.len() * 2 + 2) as u16).to_le_bytes());
        dir[66] = 2;
        dir[116..120].copy_from_slice(&2u32.to_le_bytes());
        dir[120..124].copy_from_slice(&5u32.to_le_bytes());
        out.extend_from_slice(&dir);
        out.extend_from_slice(b"HELLO");
        out
    }

    #[test]
    fn extracts_stream_from_zero_id_fat_sector() {
        let d = happy_path();
        let s = extract_ole2_streams(&d, &["WordDocument"]);
        assert_eq!(s.names(), vec!["WordDocument"]);
        assert_eq!(s.get("WordDocument"), Some(&b"HELLO"[..]));
        assert_eq!(extract_worddocument_stream(&d), Some(b"HELLO".to_vec()));
    }

    #[test]
    fn rejects_short_and_bad_magic() {
        assert_eq!(extract_ole2_streams(b"", &["WordDocument"]).len(), 0);
        assert_eq!(extract_ole2_streams(&[0xd0, 0xcf, 0x11, 0xe0], &["WordDocument"]).len(), 0);
        assert!(extract_worddocument_stream(&[0u8; 512]).is_none());
    }

    /// Python: `if sector_shift not in (9, 12): return {}`.
    #[test]
    fn only_sector_shift_9_and_12_are_accepted() {
        for shift in [0u16, 7, 8, 10, 11, 16, 511] {
            let d = craft(shift, &[0], 1, &[("WordDocument", 2, 5)]);
            assert_eq!(
                extract_ole2_streams(&d, &["WordDocument"]).len(),
                0,
                "shift {shift} must yield no streams (golden shift_0/7/8/10/11/16/511)"
            );
        }
        assert_eq!(extract_ole2_streams(&happy_path(), &["WordDocument"]).len(), 1);
    }

    /// Python: `for i in range(min(109, fat_sectors_count))` — a zero count
    /// never builds a FAT, so `get_stream` returns `b''` and `{}` is returned.
    #[test]
    fn zero_sector_fat_yields_no_streams() {
        let mut d = happy_path();
        d[44..48].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(extract_ole2_streams(&d, &["WordDocument"]).len(), 0);
    }

    /// DIFAT slots at or above `0xFFFFFFFA` are skipped (`continue`).
    #[test]
    fn fat_sector_ids_at_the_special_floor_are_skipped() {
        let mut d = happy_path();
        d[76..80].copy_from_slice(&0xFFFF_FFFCu32.to_le_bytes());
        assert_eq!(extract_ole2_streams(&d, &["WordDocument"]).len(), 0);
    }

    /// The DIFAT *chain* is never followed: only 109 header slots are read.
    /// Golden `difat_110` -> `{}`.
    #[test]
    fn difat_chain_is_never_followed() {
        let mut d = vec![0u8; 512];
        d[..8].copy_from_slice(&OLE2_MAGIC);
        d[30..32].copy_from_slice(&9u16.to_le_bytes());
        d[44..48].copy_from_slice(&110u32.to_le_bytes()); // claims 110 FAT sectors
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[76..80].copy_from_slice(&1u32.to_le_bytes()); // DIFAT[0] = sector 1
        for i in 1..109 {
            d[76 + i * 4..80 + i * 4].copy_from_slice(&0xFFFF_FFFCu32.to_le_bytes());
        }
        d.extend_from_slice(&fatsec(&[0xFFFF_FFFE; 128])); // sector 0
        d.extend_from_slice(&fatsec(&[0xFFFF_FFFE; 128])); // sector 1 = "FAT"
        // Every 128-byte directory record here is made of 0xFE bytes, so
        // name_len == 0xFFFE > 64 -> all skipped -> {} (golden difat_110).
        assert_eq!(extract_ole2_streams(&d, &["WordDocument"]).len(), 0);
    }

    /// A FAT cycle must terminate via Python's `visited` set.  Golden
    /// `fat_cycle` -> `{'WordDocument': 0}` — the key exists, the value empty.
    #[test]
    fn fat_cycle_terminates_and_yields_an_empty_but_present_stream() {
        let mut d = vec![0u8; 512];
        d[..8].copy_from_slice(&OLE2_MAGIC);
        d[30..32].copy_from_slice(&9u16.to_le_bytes());
        d[44..48].copy_from_slice(&1u32.to_le_bytes());
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[76..80].copy_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&fatsec(&[1, 0, 0xFFFF_FFFE])); // 0 <-> 1 cycle
        let mut dir = vec![0u8; 512];
        let nb = "WordDocument".encode_utf16().collect::<Vec<u16>>();
        for (k, u) in nb.iter().enumerate() {
            dir[k * 2..k * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        dir[64..66].copy_from_slice(&((nb.len() * 2 + 2) as u16).to_le_bytes());
        dir[116..120].copy_from_slice(&2u32.to_le_bytes());
        dir[120..124].copy_from_slice(&5u32.to_le_bytes());
        d.extend_from_slice(&dir);
        let s = extract_ole2_streams(&d, &["WordDocument"]);
        assert_eq!(s.names(), vec!["WordDocument"], "key must still be present");
        assert_eq!(s.get("WordDocument").unwrap().len(), 0);
        // `if not word_doc` in Python treats it as missing.
        assert!(extract_worddocument_stream(&d).is_some());
        assert!(extract_worddocument_stream(&d).unwrap().is_empty());
    }

    /// A stream size beyond EOF must not over-read or abort.  Golden `oversize`.
    #[test]
    fn oversized_stream_claim_is_clamped_to_the_container() {
        let d = craft(9, &[0], 1, &[("WordDocument", 2, u32::MAX)]);
        let s = extract_ole2_streams(&d, &["WordDocument"]);
        // The chain leaves the file, so Python's slices yield b''.
        assert_eq!(s.get("WordDocument").map(|v| v.len()), Some(0));
    }

    /// `name_len <= 2 or name_len > 64` -> skip.  Golden `name_len_2`,
    /// `name_len_66`; `name_len_64` keeps exactly 31 characters.
    #[test]
    fn directory_name_length_window_is_3_to_64_bytes() {
        let build = |nl: u16, name: &str| {
            let mut d = vec![0u8; 512];
            d[..8].copy_from_slice(&OLE2_MAGIC);
            d[30..32].copy_from_slice(&9u16.to_le_bytes());
            d[44..48].copy_from_slice(&1u32.to_le_bytes());
            d[48..52].copy_from_slice(&1u32.to_le_bytes());
            d[76..80].copy_from_slice(&0u32.to_le_bytes());
            d.extend_from_slice(&fatsec(&[0xFFFF_FFFD, 0xFFFF_FFFE, 0xFFFF_FFFE]));
            let mut dir = vec![0u8; 512];
            for (k, u) in name.encode_utf16().enumerate().take(32) {
                dir[k * 2..k * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            dir[64..66].copy_from_slice(&nl.to_le_bytes());
            dir[116..120].copy_from_slice(&2u32.to_le_bytes());
            dir[120..124].copy_from_slice(&5u32.to_le_bytes());
            d.extend_from_slice(&dir);
            d.extend_from_slice(b"HELLO");
            d
        };
        let long = "A".repeat(31);
        assert_eq!(
            extract_ole2_streams(&build(64, &long), &[long.as_str()]).names(),
            vec![long.as_str()],
            "name_len 64 -> entry[:62] -> 31 chars (golden name_len_64)"
        );
        assert_eq!(extract_ole2_streams(&build(66, &long), &[long.as_str()]).len(), 0,
            "name_len 66 > 64 is rejected (golden name_len_66)");
        assert_eq!(extract_ole2_streams(&build(2, &long), &["A"]).len(), 0,
            "name_len <= 2 is rejected (golden name_len_2)");
        assert_eq!(extract_ole2_streams(&build(65, &long), &[long.as_str()]).len(), 0,
            "odd name_len leaves an odd slice -> utf-16le raises -> skip (golden name_len_65)");
        assert_eq!(extract_ole2_streams(&build(5, "WordDocument"), &["WordDocument"]).len(), 0,
            "odd name_len 5 -> skip (golden odd_name_len)");
    }

    /// Truncation of a real file: the FAT sector falls outside the buffer, the
    /// FAT stays empty, `get_stream` yields `b''`, `{}` is returned.  Matches
    /// golden `truncated_streams == {}` measured on a real fixture cut to 1/3.
    #[test]
    fn truncated_file_yields_no_streams() {
        let full = happy_path();
        let trunc = &full[..full.len() / 3];
        assert_eq!(extract_ole2_streams(trunc, &["WordDocument"]).len(), 0);
        assert!(extract_worddocument_stream(trunc).is_none());
    }

    #[test]
    fn stream_names_are_returned_in_directory_order_not_wanted_order() {
        // Two directory records: 1Table first, WordDocument second.
        let mut d = vec![0u8; 512];
        d[..8].copy_from_slice(&OLE2_MAGIC);
        d[30..32].copy_from_slice(&9u16.to_le_bytes());
        d[44..48].copy_from_slice(&1u32.to_le_bytes());
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[76..80].copy_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&fatsec(&[0xFFFF_FFFD, 0xFFFF_FFFE, 1, 0xFFFF_FFFE]));
        let mut dir = vec![0u8; 512];
        for (slot, nm) in [(0usize, "1Table"), (1usize, "WordDocument")] {
            let base = slot * 128;
            for (k, u) in nm.encode_utf16().enumerate() {
                dir[base + k * 2..base + k * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            dir[base + 64..base + 66]
                .copy_from_slice(&((nm.len() * 2 + 2) as u16).to_le_bytes());
            dir[base + 116..base + 120].copy_from_slice(&2u32.to_le_bytes());
            dir[base + 120..base + 124].copy_from_slice(&5u32.to_le_bytes());
        }
        d.extend_from_slice(&dir);
        d.extend_from_slice(b"HELLO");
        // Wanted order is reversed against directory order.
        let s = extract_ole2_streams(&d, &["WordDocument", "1Table", "0Table"]);
        assert_eq!(s.names(), vec!["1Table", "WordDocument"]);
    }

    /// The `len(streams) == len(wanted_set)` early break: asking for only
    /// `WordDocument` stops at the first match, so a later duplicate cannot
    /// overwrite it.
    #[test]
    fn single_wanted_name_breaks_early() {
        let mut d = vec![0u8; 512];
        d[..8].copy_from_slice(&OLE2_MAGIC);
        d[30..32].copy_from_slice(&9u16.to_le_bytes());
        d[44..48].copy_from_slice(&1u32.to_le_bytes());
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[76..80].copy_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&fatsec(&[0xFFFF_FFFD, 0xFFFF_FFFE, 0xFFFF_FFFE]));
        let mut dir = vec![0u8; 512];
        for (slot, nm) in [(0usize, "WordDocument"), (1usize, "WordDocument")] {
            let base = slot * 128;
            for (k, u) in nm.encode_utf16().enumerate() {
                dir[base + k * 2..base + k * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            dir[base + 64..base + 66]
                .copy_from_slice(&((nm.len() * 2 + 2) as u16).to_le_bytes());
            dir[base + 116..base + 120].copy_from_slice(&2u32.to_le_bytes());
            dir[base + 120..base + 124].copy_from_slice(&5u32.to_le_bytes());
        }
        d.extend_from_slice(&dir);
        d.extend_from_slice(b"HELLO");
        assert_eq!(extract_ole2_streams(&d, &["WordDocument"]).len(), 1);
    }

    #[test]
    fn utf16le_strict_matches_python_error_semantics() {
        assert_eq!(decode_utf16le_strict(b"W\x00o\x00"), Some("Wo".into()));
        assert_eq!(decode_utf16le_strict(b"W\x00o"), None, "odd length raises");
        assert_eq!(decode_utf16le_strict(&[0x00, 0xd8]), None, "lone surrogate raises");
        assert_eq!(decode_utf16le_strict(&[0x3d, 0xd8, 0xb7, 0xde]), Some("\u{1f6b7}".into()));
    }

    /// Goldens from `golden/30_utf16_probes.json`, measured with CPython.
    #[test]
    fn utf16le_ignore_and_replace_match_cpython() {
        let cases: &[(&[u8], &[u32], &[u32])] = &[
            (&[0x00, 0xd8], &[], &[0xfffd]),
            (&[0x00, 0xdc, 0x00, 0xde], &[], &[0xfffd, 0xfffd]),
            (&[0x00, 0xd8, 0x00, 0x41], &[0x4100], &[0xfffd, 0x4100]),
            (&[0x00, 0x41, 0x00, 0xd8], &[0x4100], &[0x4100, 0xfffd]),
            (&[0x41], &[], &[0xfffd]),
            (&[0xd8, 0x00], &[0xd8], &[0xd8]),
            (&[0x00, 0xd8, 0x00, 0xdc], &[0x10000], &[0x10000]),
            (&[0x41, 0x42, 0x43], &[0x4241], &[0x4241, 0xfffd]),
            (&[0x07, 0x00], &[0x07], &[0x07]),
        ];
        for (raw, ign, rep) in cases {
            let s = decode_utf16le_ignore(raw);
            let got: Vec<u32> = s.chars().map(|c| c as u32).collect();
            assert_eq!(&got, ign, "ignore for {}", hex(raw));
            let s = decode_utf16le_replace(raw);
            let got: Vec<u32> = s.chars().map(|c| c as u32).collect();
            assert_eq!(&got, rep, "replace for {}", hex(raw));
        }
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{:02x}", x)).collect::<Vec<_>>().join("")
    }

    // ------------------------------------------------------------------
    // Real-file evidence: the nine .doc samples that ship with the repo.
    // ------------------------------------------------------------------

    /// FNV-1a/64, computed identically in `gen_fixture_fnv.py`, so a stream can
    /// be pinned byte-for-byte without pulling in a hash dependency.
    ///
    /// The digit grouping on the prime is significant: FNV-1a/64 uses
    /// 2^40 + 435 = `0x100_0000_01B3`, *not* `0x1000_0000_01B3`.  The two agree
    /// on the low 32 bits of every hash, so a mis-grouped literal silently looks
    /// like "the file bytes moved".
    pub fn fnv1a64(b: &[u8]) -> u64 {
        let mut h: u64 = 0xCBF2_9CE4_8422_2325;
        for &x in b {
            h ^= x as u64;
            h = h.wrapping_mul(0x100_0000_01B3);
        }
        h
    }

    /// Independent check of the helper itself.  These four values come from the
    /// published FNV-1a test vectors, *not* from `gen_fixture_fnv.py`, so they
    /// catch a mistyped prime/offset basis that a self-referential golden could
    /// not.  (Mis-grouping the prime as `0x1000_0000_01B3` keeps the low 32 bits
    /// of every hash intact and was therefore invisible until now.)
    #[test]
    fn fnv1a64_matches_published_test_vectors() {
        let cases: &[(&[u8], u64)] = &[
            (b"", 0xCBF2_9CE4_8422_2325),
            (b"a", 0xAF63_DC4C_8601_EC8C),
            (b"abc", 0xE71F_A219_0541_574B),
            (b"foobar", 0x8594_4171_F739_67E8),
        ];
        for (input, want) in cases {
            assert_eq!(fnv1a64(input), *want, "FNV-1a/64({})", String::from_utf8_lossy(input));
        }
    }

    struct FixtureGolden {
        rel: &'static str,
        size: usize,
        file_fnv: u64,
        /// (stream name, byte length, FNV-1a/64) in *directory* order.
        streams: &'static [(&'static str, usize, u64)],
        /// (len, FNV-1a/64 of the UTF-8 bytes, newline count) of
        /// `_doc_extract_text_pure_python(path)` -- carried so `doc_legacy.rs`
        /// and this reader are pinned off the same measurement.  Read there,
        /// not here, hence the allow.
        #[allow(dead_code)]
        pure: (usize, u64, usize),
    }

    /// Generated one-shot by `scratch/rust_parity/legacy/gen_fixture_fnv.py`
    /// against CPython 3.11.15; never hand-typed.
    const GOLDENS: &[FixtureGolden] = &[
        FixtureGolden {
            rel: "showcase/v239_full_coverage/samples/sample_old.doc",
            size: 6144,
            file_fnv: 0x443C12DF8FA20FC2,
            streams: &[("WordDocument", 4424, 0x5B6230BB0EAB0ADB)],
            pure: (206, 0xC8B75F7C1B83E528, 13),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/_\u{539f}\u{59cb}\u{5907}\u{4efd}/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{672c}\u{79d1}\u{751f}\u{5b9e}\u{4e60}\u{7533}\u{8bf7}\u{8868}.doc",
            size: 36352,
            file_fnv: 0x9BCC47A0ECEC624C,
            streams: &[("1Table", 8698, 0xDA81F07B0A49DE93), ("WordDocument", 11826, 0x6438B07131727B1E)],
            pure: (1278, 0xC0B16813F36D37B2, 18),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/_\u{539f}\u{59cb}\u{5907}\u{4efd}/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{6bd5}\u{4e1a}\u{5b9e}\u{4e60}\u{6587}\u{6863} \u{ff08}\u{542b}\u{5b9e}\u{4e60}\u{8bb0}\u{5f55}\u{8868}\u{ff09}.doc",
            size: 69120,
            file_fnv: 0x4E7CDFD56B088782,
            streams: &[("1Table", 10861, 0x81B67968CAE14CF2), ("WordDocument", 33842, 0x4FCC7C83E5E96AFF)],
            pure: (3049, 0xC21D4A0EBE01981D, 64),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/_\u{539f}\u{59cb}\u{5907}\u{4efd}/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{79bb}\u{4eac}\u{5b9e}\u{4e60}\u{8bf7}\u{5047}\u{8868}-\u{4ea4}\u{8f85}\u{5bfc}\u{5458}.doc",
            size: 34816,
            file_fnv: 0x346A62416B73EA99,
            streams: &[("1Table", 7893, 0xDA28F937A719C207), ("WordDocument", 10802, 0xF1CAB52FF75D14BC)],
            pure: (792, 0x7C41A39CC8199AE3, 33),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{672c}\u{79d1}\u{751f}\u{5b9e}\u{4e60}\u{7533}\u{8bf7}\u{8868}.doc",
            size: 37376,
            file_fnv: 0xCC824D643AA4F8F8,
            streams: &[("1Table", 8816, 0x0B7E920CEEBEEED5), ("WordDocument", 12338, 0x1339F7D450BF1A57)],
            pure: (1180, 0x70FDEFE749288CB6, 17),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{6bd5}\u{4e1a}\u{5b9e}\u{4e60}\u{6587}\u{6863} \u{ff08}\u{542b}\u{5b9e}\u{4e60}\u{8bb0}\u{5f55}\u{8868}\u{ff09}.doc",
            size: 209920,
            file_fnv: 0x091A5B36E03ECCEB,
            streams: &[("1Table", 23338, 0xBD0CDDE7EED3A6CB), ("WordDocument", 141362, 0xE4331DB5119DC2E3)],
            pure: (21566, 0xA058C6B52AC069FC, 189),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{6bd5}\u{4e1a}\u{5b9e}\u{4e60}\u{6587}\u{6863}-\u{5176}\u{4ed6}\u{90e8}\u{5206}\u{ff08}\u{5df2}\u{586b}\u{5199}\u{ff09}.doc",
            size: 55808,
            file_fnv: 0xDE35AC798531B9EC,
            streams: &[("WordDocument", 39475, 0x034E591128926963), ("0Table", 5890, 0x15915BCC205DA81A)],
            pure: (3919, 0x4A52BB304F969024, 65),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{6bd5}\u{4e1a}\u{5b9e}\u{4e60}\u{6587}\u{6863}-\u{5355}\u{72ec}\u{5468}\u{62a5}.doc",
            size: 165888,
            file_fnv: 0x04F1ABC5005197C7,
            streams: &[("1Table", 19778, 0x36E9BE94196182AE), ("WordDocument", 112178, 0xED149173A545FFB0)],
            pure: (17698, 0x3892CFA79114EE8D, 118),
        },
        FixtureGolden {
            rel: "test_copies/bjtu_internship/\u{5317}\u{4eac}\u{4ea4}\u{901a}\u{5927}\u{5b66}\u{8f6f}\u{4ef6}\u{5b66}\u{9662}\u{79bb}\u{4eac}\u{5b9e}\u{4e60}\u{8bf7}\u{5047}\u{8868}-\u{4ea4}\u{8f85}\u{5bfc}\u{5458}.doc",
            size: 23552,
            file_fnv: 0x59F665A002EEA33C,
            streams: &[("WordDocument", 15923, 0xE06F657468551283), ("0Table", 3200, 0xBE11470778BE9C0E)],
            pure: (836, 0x97CDE30E40CBAA29, 33),
        },
    ];

    /// Walk up from the CWD looking for the repo root; `READMD_REPO` overrides.
    fn repo_root() -> Option<std::path::PathBuf> {
        if let Ok(v) = std::env::var("READMD_REPO") {
            let p = std::path::PathBuf::from(v);
            if p.join("test_copies").is_dir() {
                return Some(p);
            }
        }
        let mut dir = std::env::current_dir().ok()?;
        loop {
            if dir.join("test_copies").join("bjtu_internship").is_dir() {
                return Some(dir);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    /// The whole point of this lane's method requirement: the reader is verified
    /// against real Word 97-2003 files, not just crafted ones.
    #[test]
    fn real_doc_fixtures_extract_the_same_streams_as_cpython() {
        let root = repo_root().expect(
            "real .doc fixtures are this lane's evidence; set READMD_REPO to the repo root",
        );
        assert_eq!(GOLDENS.len(), 9);
        for g in GOLDENS {
            let path = root.join(g.rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {}", g.rel, e));
            assert_eq!(data.len(), g.size, "size moved: {}", g.rel);
            assert_eq!(fnv1a64(&data), g.file_fnv, "file bytes moved: {}", g.rel);
            let s = extract_ole2_streams(&data, &["WordDocument", "0Table", "1Table"]);
            let got: Vec<(&str, usize, u64)> = s
                .entries
                .iter()
                .map(|(k, v)| (k.as_str(), v.len(), fnv1a64(v)))
                .collect();
            let want: Vec<(&str, usize, u64)> =
                g.streams.iter().map(|&(n, l, h)| (n, l, h)).collect();
            assert_eq!(got, want, "stream set/order/content: {}", g.rel);
            // `_extract_worddocument_stream` is the same walk with one wanted
            // name, so it must agree with the three-name call.
            let wd = extract_worddocument_stream(&data).expect("fixtures all have WordDocument");
            assert_eq!(wd, s.get("WordDocument").unwrap());
        }
    }
}
