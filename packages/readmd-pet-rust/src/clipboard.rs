//! System-clipboard capture for the `clipboard` command.
//!
//! The shipped overlay publishes a bare `toggle-app` control and the reference
//! Electron adapter turns exactly that control into a clipboard read
//! (`electron-main.ts:296` → `clipboardCommand()`); the app consumes only the
//! resulting `clipboard` command (`readmd.py:5637`). This module is that read.
//!
//! The conversion from a Windows `CF_DIB` blob to PNG lives here rather than
//! behind a dependency: the consumer base64-decodes `image_png` and rejects
//! anything without the PNG signature (`readmd.py:5769-5771`), so a raw BMP
//! would fail the gesture anyway.

use crate::protocol::{ClipboardCommand, MAX_CLIPBOARD_IMAGE_PNG_CHARS};

/// Raw clipboard contents before the protocol caps are applied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardCapture {
    pub text: String,
    pub image_png: String,
    pub paths: Vec<String>,
}

impl ClipboardCapture {
    /// Apply the wire caps and produce the command the authority accepts.
    pub fn into_command(self) -> ClipboardCommand {
        ClipboardCommand::new(self.text, self.image_png, self.paths)
    }
}

/// Read the clipboard and serialise it as the `clipboard` command.
pub fn clipboard_command() -> serde_json::Value {
    let command = capture().into_command();
    serde_json::to_value(command).unwrap_or_else(|_| {
        serde_json::json!({"type": ClipboardCommand::TYPE_NAME, "text": "", "image_png": "", "paths": []})
    })
}

#[cfg(windows)]
fn capture() -> ClipboardCapture {
    windows::read()
}

#[cfg(not(windows))]
fn capture() -> ClipboardCapture {
    // No clipboard reader is compiled in outside Windows: the Electron
    // reference reads `clipboard` from Electron's own module, and neither
    // `objc2` nor `gtk` clipboard access is wired here. The command still
    // carries the full key set so the consumer takes its documented empty
    // path (`readmd.py:5758-5761`) instead of discarding the command.
    ClipboardCapture::default()
}

/// Largest DIB pixel count encoded to PNG. The encoder below stores, rather
/// than compresses, pixel data; this bound keeps the base64 payload inside
/// `MAX_CLIPBOARD_IMAGE_PNG_CHARS` so an oversized grab degrades to an empty
/// image instead of a command the consumer would discard.
const MAX_PNG_PIXELS: u64 = 4_000_000;

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity(height as usize * (stride + 1));
    for row in 0..height as usize {
        raw.push(0u8); // filter type 0 (None) per scanline
        let start = row * stride;
        let end = (start + stride).min(rgba.len());
        let line = &rgba[start..end];
        raw.extend_from_slice(line);
        // Pad a short final line so the scanline keeps its declared length.
        raw.extend(std::iter::repeat_n(0u8, stride - line.len()));
    }
    let mut out = Vec::with_capacity(raw.len() + 64);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8u8, 6u8, 0u8, 0u8, 0u8]); // depth, RGBA, deflate, filters, none
    write_chunk(&mut out, b"IHDR", &header);
    write_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    write_chunk(&mut out, b"IEND", &[]);
    out
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// A `zlib` stream of stored (uncompressed) DEFLATE blocks. Valid PNG, and it
/// avoids vendoring a compressor for a best-effort image path.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65536 + 9);
    out.extend_from_slice(&[0x78u8, 0x01u8]); // CMF/FLG: deflate, 32K window, no preset
    let mut offset = 0usize;
    loop {
        let block = data.len().saturating_sub(offset).min(0xFFFF);
        let final_block = offset + block >= data.len();
        out.push(if final_block { 1u8 } else { 0u8 });
        out.extend_from_slice(&(block as u16).to_le_bytes());
        out.extend_from_slice(&(!(block as u16)).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + block]);
        offset += block;
        if offset >= data.len() {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + *byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (index, entry) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 1 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *entry = value;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc = table[((crc ^ *byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

/// Convert a `CF_DIB` global-memory blob (BITMAPINFOHEADER plus pixel bits)
/// into PNG bytes. Returns `None` for headers or formats this path does not
/// claim to render, so the caller can publish an empty image instead of a
/// corrupt one.
pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let header_size = read_u32(dib, 0)?;
    // BITMAPINFOHEADER and the larger V4/V5 variants share the leading fields.
    if !matches!(header_size, 40 | 108 | 124) {
        return None;
    }
    let width = read_i32(dib, 4)?;
    let raw_height = read_i32(dib, 8)?;
    let bit_count = read_u16(dib, 14)?;
    let compression = read_u32(dib, 16)?;
    let colors_used = read_u32(dib, 32).unwrap_or(0);
    if width <= 0 || raw_height == 0 {
        return None;
    }
    let (width, height) = (width as u64, raw_height.unsigned_abs() as u64);
    // BI_RGB is 0 and BI_BITFIELDS is 3; the latter is the only 32-bit case
    // with a meaningful alpha channel.
    let indexed = matches!(bit_count, 1 | 4 | 8);
    let channels = match (bit_count, compression) {
        (32, 0 | 3) => 4usize,
        (24, 0) => 3usize,
        _ if indexed && compression == 0 => 3usize,
        _ => return None,
    };
    let alpha_is_meaningful = bit_count == 32 && compression == 3;
    if width * height > MAX_PNG_PIXELS || width * height * channels as u64 > u32::MAX as u64 {
        return None;
    }
    let (width, height) = (width as u32, height as u32);
    let stride = ((width as u64 * bit_count as u64 + 31) / 32 * 4) as usize;
    let palette_entries = if indexed {
        if colors_used == 0 {
            1usize << bit_count
        } else {
            colors_used as usize
        }
    } else {
        0usize
    };
    let bits_offset = header_size as usize + palette_entries * 4;
    if dib.len() < bits_offset + stride * height as usize {
        return None;
    }
    // A bottom-up DIB (positive height) stores row 0 as the last visual row.
    let bottom_up = raw_height > 0;
    let mut rgba = vec![0u8; (width as usize * height as usize) * 4];
    for y in 0..height as usize {
        let source_row = if bottom_up { height as usize - 1 - y } else { y };
        let row = &dib[bits_offset + source_row * stride..bits_offset + source_row * stride + stride];
        for x in 0..width as usize {
            let (r, g, b, a) = if bit_count == 32 {
                let pixel = &row[x * 4..x * 4 + 4];
                (pixel[2], pixel[1], pixel[0], if alpha_is_meaningful { pixel[3] } else { 255 })
            } else if bit_count == 24 {
                let pixel = &row[x * 3..x * 3 + 3];
                (pixel[2], pixel[1], pixel[0], 255)
            } else {
                let index = indexed_index(row, x, bit_count)? as usize;
                let entry = index.min(palette_entries.saturating_sub(1)) * 4;
                let color = dib.get(header_size as usize + entry..header_size as usize + entry + 4)?;
                (color[2], color[1], color[0], 255)
            };
            let target = (y * width as usize + x) * 4;
            rgba[target] = r;
            rgba[target + 1] = g;
            rgba[target + 2] = b;
            rgba[target + 3] = a;
        }
    }
    Some(encode_png(width, height, &rgba))
}

fn indexed_index(row: &[u8], x: usize, bit_count: u16) -> Option<u32> {
    match bit_count {
        8 => Some(row[x] as u32),
        4 => {
            let byte = row[x / 2] as u32;
            Some(if x % 2 == 0 { byte >> 4 } else { byte & 0x0F })
        }
        1 => Some((row[x / 8] as u32 >> (7 - (x % 8))) & 1),
        _ => None,
    }
}

/// Base64 with the standard alphabet and `=` padding, matching Node's
/// `Buffer.toString('base64')` that the reference adapter and the Python
/// consumer both use.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let value = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(value >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(value >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(value >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Keep an encoded image inside the consumer's cap. A truncated base64 string
/// would decode to garbage and be rejected as `invalid_clipboard_image`, so an
/// oversized image becomes empty instead.
pub fn image_png_or_empty(png: Vec<u8>) -> String {
    if png.is_empty() || png.len() > MAX_CLIPBOARD_IMAGE_PNG_CHARS / 4 * 3 {
        return String::new();
    }
    let encoded = base64(&png);
    if encoded.len() > MAX_CLIPBOARD_IMAGE_PNG_CHARS {
        return String::new();
    }
    encoded
}

/// Decode NUL-terminated UTF-16 units (a `CF_UNICODETEXT` block).
#[allow(dead_code)] // only the Windows reader consumes this
fn utf16_to_string(units: &[u16]) -> String {
    let trimmed = match units.iter().position(|unit| *unit == 0) {
        Some(end) => &units[..end],
        None => units,
    };
    String::from_utf16_lossy(trimmed)
}

/// Parse a `CF_HDROP` block into its file list. `DROPFILES` is packed:
/// `pFiles:u32`, `pt:{x:i32,y:i32}`, `fNC:i32`, `fWide:i32`.
#[allow(dead_code)] // only the Windows reader consumes this
fn hdrop_paths(bytes: &[u8]) -> Vec<String> {
    let list_offset = read_u32(bytes, 0).unwrap_or(0) as usize;
    let wide = read_i32(bytes, 16).unwrap_or(1) != 0;
    let list = match bytes.get(list_offset..) {
        Some(list) if list.len() > 2 => list,
        _ => return Vec::new(),
    };
    if !wide {
        // ANSI HDROP payloads need the active code page to decode; the
        // reference adapter reads the wide `FileNameW` form too, so an ANSI
        // list is skipped rather than published as mojibake.
        return Vec::new();
    }
    let units: Vec<u16> = list
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let mut paths = Vec::new();
    let mut start = 0usize;
    while start < units.len() {
        if units[start] == 0 {
            break;
        }
        let end = units[start..]
            .iter()
            .position(|unit| *unit == 0)
            .map(|offset| start + offset)
            .unwrap_or(units.len());
        let path = utf16_to_string(&units[start..end]);
        if !path.is_empty() {
            paths.push(path);
        }
        start = end + 1;
    }
    paths
}

#[cfg(windows)]
mod windows {
    use super::{ClipboardCapture, dib_to_png, hdrop_paths, image_png_or_empty, utf16_to_string};
    use crate::protocol::MAX_CLIPBOARD_TEXT_BYTES;
    use windows_sys::Win32::Foundation::{HANDLE, HWND};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows_sys::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};

    /// Clipboard memory blocks larger than this are not copied out; the wire
    /// caps reject them anyway and a huge block should not stall the loop.
    const MAX_SOURCE_BYTES: usize = 96 * 1024 * 1024;

    /// `CF_UNICODETEXT` is UTF-16, so twice the UTF-8 byte budget of units is
    /// a safe upper bound; `ClipboardCommand::new` applies the real cap.
    const MAX_TEXT_UNITS: usize = MAX_CLIPBOARD_TEXT_BYTES / 2;

    struct Guard;

    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    /// Copy one clipboard block, bounded and lock-then-unlock scoped.
    unsafe fn with_block<T>(format: u32, fallback: T, read: impl FnOnce(&[u8]) -> T) -> T {
        let handle: HANDLE = GetClipboardData(format);
        if handle.is_null() {
            return fallback;
        }
        let size = GlobalSize(handle);
        let pointer = GlobalLock(handle) as *const u8;
        if pointer.is_null() || size == 0 || size > MAX_SOURCE_BYTES {
            if !pointer.is_null() {
                GlobalUnlock(handle);
            }
            return fallback;
        }
        let value = read(std::slice::from_raw_parts(pointer, size));
        GlobalUnlock(handle);
        value
    }

    pub(super) fn read() -> ClipboardCapture {
        // `NULL` owner: the pet window must not take clipboard ownership away
        // from the app that copied, only observe it.
        if unsafe { OpenClipboard(std::ptr::null_mut() as HWND) } == 0 {
            return ClipboardCapture::default();
        }
        let _guard = Guard;
        let text = unsafe {
            with_block(CF_UNICODETEXT as u32, String::new(), |bytes| {
                // Trim to the cap before decoding: a 4 MiB limit never needs a
                // larger UTF-16 buffer converted.
                let usable = bytes.len().min(MAX_TEXT_UNITS * 2);
                let units: Vec<u16> = bytes[..usable]
                    .chunks_exact(2)
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .collect();
                utf16_to_string(&units)
            })
        };
        let paths = unsafe { with_block(CF_HDROP as u32, Vec::new(), hdrop_paths) };
        let image_png = unsafe {
            with_block(CF_DIB as u32, String::new(), |bytes| {
                match dib_to_png(bytes) {
                    Some(png) => image_png_or_empty(png),
                    None => String::new(),
                }
            })
        };
        ClipboardCapture {
            text,
            image_png,
            paths,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dib_24bpp_top_down(rows: &[[u8; 6]], width: i32, height: i32) -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&width.to_le_bytes());
        dib.extend_from_slice(&height.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[0u8; 20]); // biSizeImage..biClrImportant
        for row in rows {
            dib.extend_from_slice(row);
            dib.extend_from_slice(&[0u8; 2]); // stride padded to 4 bytes
        }
        dib
    }

    /// Independently generated (Python `zlib` level 0) and verified with
    /// Pillow: RGBA(2,2) == [(255,0,0,255),(0,255,0,255),(0,0,255,255),
    /// (255,255,255,255)].
    const GOLDEN_2X2: &str = "89504e470d0a1a0a0000000d4948445200000002000000020806000000\
                              72b60d240000001d494441547801011200edff00ff0000ff00ff00ff00\
                              0000ffffffffffff49c809f7adab561b0000000049454e44ae426082";

    fn hex_bytes(value: &str) -> Vec<u8> {
        let digits: String = value.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        (0..digits.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&digits[index..index + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn dib_converts_to_png_the_consumer_accepts() {
        // BGR triples: row 0 holds red then green, row 1 blue then white.
        let rows = [[0u8, 0, 255, 0, 255, 0], [255, 0, 0, 255, 255, 255]];
        let golden = hex_bytes(GOLDEN_2X2);
        // Tripwire for the line-continuation transcription above: the vector is
        // an 86 byte PNG (8 sig + 25 IHDR + 41 IDAT + 12 IEND).
        assert_eq!(golden.len(), 86, "golden PNG literal is truncated");
        let top_down = dib_to_png(&dib_24bpp_top_down(&rows, 2, -2)).expect("encodable DIB");
        assert_eq!(top_down, golden);
        assert!(top_down.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(&top_down[12..16], b"IHDR");
        // A positive height stores row 0 last, so the visual rows swap.
        let bottom_up = dib_to_png(&dib_24bpp_top_down(&rows, 2, 2)).expect("encodable DIB");
        let reversed = dib_to_png(&dib_24bpp_top_down(&[rows[1], rows[0]], 2, -2))
            .expect("encodable DIB");
        assert_ne!(bottom_up, top_down);
        assert_eq!(bottom_up, reversed);
    }

    #[test]
    fn png_encoder_emits_byte_exact_stored_stream() {
        let png = encode_png(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]);
        // Golden vector produced independently (Python zlib level 0 + Pillow
        // decode: RGBA(2,1) == [(255,0,0,255),(0,255,0,255)]).
        let expected = "89504e470d0a1a0a0000000d4948445200000002000000010806000000f4227f8a\
                        00000014494441547801010900f6ff00ff0000ff00ff00ff10f803fdd964bc53\
                        0000000049454e44ae426082";
        let expected: String = expected.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        let actual: String = png.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(actual, expected);
        assert_eq!(
            base64(&png),
            "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAAFElEQVR4AQEJAPb/AP8AAP8A/wD/EPgD/dlkvFMAAAAASUVORK5CYII="
        );
    }

    #[test]
    fn dib_rejects_unsupported_or_truncated_payloads() {
        assert_eq!(dib_to_png(&[]), None);
        assert_eq!(
            dib_to_png(&dib_24bpp_top_down(&[[0, 0, 0, 0, 0, 0]], 0, -1)),
            None,
            "zero width"
        );
        assert_eq!(
            dib_to_png(&dib_24bpp_top_down(&[[0, 0, 0, 0, 0, 0]], 1, 0)),
            None,
            "zero height"
        );
        let mut bad_header = dib_24bpp_top_down(&[[0, 0, 0, 0, 0, 0]], 1, -1);
        bad_header[0..4].copy_from_slice(&12u32.to_le_bytes());
        assert_eq!(dib_to_png(&bad_header), None, "BITMAPCOREHEADER");
        let mut truncated = dib_24bpp_top_down(&[[0, 0, 0, 0, 0, 0]], 1, -1);
        // 40-byte header + a 1x1 24bpp row padded to a 4-byte stride: dropping
        // the last byte leaves less than one stride, which must be rejected.
        truncated.truncate(43);
        assert_eq!(
            dib_to_png(&truncated),
            None,
            "pixel row shorter than its stride"
        );
        let mut sixteen_bit = dib_24bpp_top_down(&[[0, 0, 0, 0, 0, 0]], 1, -1);
        sixteen_bit[14..16].copy_from_slice(&16u16.to_le_bytes());
        assert_eq!(dib_to_png(&sixteen_bit), None, "16-bit DIB is not decoded");
    }

    #[test]
    fn hdrop_reads_wide_file_list_and_skips_ansi() {
        let names = ["C:\\notes\\a.md\0", "C:\\notes\\b.txt\0", "\0"];
        let mut units: Vec<u16> = Vec::new();
        for name in names {
            units.extend(name.encode_utf16());
        }
        let mut block = Vec::new();
        block.extend_from_slice(&20u32.to_le_bytes()); // pFiles
        block.extend_from_slice(&[0u8; 8]); // pt
        block.extend_from_slice(&0i32.to_le_bytes()); // fNC
        block.extend_from_slice(&1i32.to_le_bytes()); // fWide
        for unit in units {
            block.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(
            hdrop_paths(&block),
            vec!["C:\\notes\\a.md".to_string(), "C:\\notes\\b.txt".to_string()]
        );
        let mut ansi = block.clone();
        ansi[16..20].copy_from_slice(&0i32.to_le_bytes());
        assert_eq!(hdrop_paths(&ansi), Vec::<String>::new());
        assert_eq!(hdrop_paths(&[0u8; 4]), Vec::<String>::new());
    }

    #[test]
    fn base64_matches_the_std_alphabet_and_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn oversized_image_is_emptied_not_truncated() {
        assert_eq!(image_png_or_empty(Vec::new()), "");
        let huge = vec![0u8; MAX_CLIPBOARD_IMAGE_PNG_CHARS / 4 * 3 + 1];
        assert_eq!(image_png_or_empty(huge), "");
    }

    #[test]
    fn utf16_text_stops_at_the_first_nul() {
        let units: Vec<u16> = "ab\0cd".encode_utf16().collect();
        assert_eq!(utf16_to_string(&units), "ab");
    }

    #[test]
    fn command_shape_matches_the_consumer_contract() {
        let command = ClipboardCommand::new(
            "hello".into(),
            "AAAA".into(),
            vec!["C:\\a.md".into(), "C:\\b.md".into()],
        );
        let json = serde_json::to_value(&command).unwrap();
        // These four keys are the complete set the authority reads
        // (`hermes_adapter.py:223-236`, `readmd.py:5754-5756`) and `type` must
        // be `clipboard`, never the renderer's `toggle-app`.
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["image_png", "paths", "text", "type"]);
        assert_eq!(json["type"], "clipboard");
        assert_eq!(json["text"], "hello");
        assert_eq!(json["image_png"], "AAAA");
        assert_eq!(json["paths"], serde_json::json!(["C:\\a.md", "C:\\b.md"]));
        assert!(serde_json::from_value::<ClipboardCommand>(json).is_ok());
    }

    #[test]
    fn published_command_is_a_capture_never_the_raw_control() {
        // Whatever the platform or clipboard state, the FIFO must never carry
        // the renderer's `toggle-app` name (no consumer branch exists for it
        // in `readmd.py:5620-5641`).
        let value = clipboard_command();
        assert_eq!(value["type"], "clipboard");
        assert_ne!(value["type"], "toggle-app");
        for key in ["text", "image_png", "paths"] {
            assert!(value.get(key).is_some(), "consumer key {key} is missing");
        }
        assert!(value["paths"].is_array());
    }

    #[test]
    fn unreadable_clipboard_still_serialises_every_key() {
        // An empty clipboard must not omit keys: the consumer distinguishes
        // "nothing copied" from a malformed command
        // (`readmd.py:5758-5761`).
        let json = serde_json::to_value(ClipboardCommand::empty()).unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["image_png", "paths", "text", "type"]);
        assert_eq!(json["type"], "clipboard");
        assert_eq!(json["text"], "");
        assert_eq!(json["image_png"], "");
        assert_eq!(json["paths"].as_array().unwrap().len(), 0);
    }
}
