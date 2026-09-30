//! Native OCR through `Windows.Media.Ocr` (Windows 10+), no Python and no
//! external executable.
//!
//! Images go InMemoryRandomAccessStream → BitmapDecoder → SoftwareBitmap
//! (scaled to the engine's `MaxImageDimension`) → `RecognizeAsync`; word
//! rectangles are handed to [`crate::ocr::xy_cut_lines`] so multi-column
//! pages keep their reading order.  Scanned PDF pages are rasterised by
//! `Windows.Data.Pdf` at ~200 DPI and recognised page by page.
//!
//! Every WinRT call runs on a dedicated MTA thread so request threads never
//! inherit an apartment, and the whole recognition is wrapped in
//! `catch_unwind`.

use crate::ocr::LayoutItem;

/// Result of recognising one image: layout-ordered lines.
pub type Lines = Vec<String>;

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::HSTRING;
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform,
        ColorManagementMode, ExifOrientationMode, SoftwareBitmap,
    };
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    fn init() {
        // S_FALSE / RPC_E_CHANGED_MODE are fine: the thread already has an apartment.
        let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED) };
    }

    fn engine() -> Option<OcrEngine> {
        if let Ok(e) = OcrEngine::TryCreateFromUserProfileLanguages() {
            return Some(e);
        }
        for tag in ["zh-Hans-CN", "zh-Hans", "en-US"] {
            if let Ok(lang) = Language::CreateLanguage(&HSTRING::from(tag)) {
                if let Ok(e) = OcrEngine::TryCreateFromLanguage(&lang) {
                    return Some(e);
                }
            }
        }
        None
    }

    fn stream_of(bytes: &[u8]) -> windows::core::Result<InMemoryRandomAccessStream> {
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(bytes)?;
        writer.StoreAsync()?.join()?;
        writer.FlushAsync()?.join()?;
        writer.DetachStream()?;
        stream.Seek(0)?;
        Ok(stream)
    }

    fn bitmap(stream: &InMemoryRandomAccessStream, max_dim: u32) -> windows::core::Result<SoftwareBitmap> {
        let dec = BitmapDecoder::CreateAsync(stream)?.join()?;
        let (w, h) = (dec.PixelWidth()?, dec.PixelHeight()?);
        let t = BitmapTransform::new()?;
        let longest = w.max(h).max(1);
        // Tiny scans recognise better upscaled; huge ones must fit the engine.
        let scale = if longest > max_dim {
            max_dim as f64 / longest as f64
        } else if longest < 1000 {
            (1600.0 / longest as f64).min(max_dim as f64 / longest as f64)
        } else {
            1.0
        };
        if (scale - 1.0).abs() > 1e-3 {
            t.SetScaledWidth(((w as f64 * scale).round() as u32).max(1))?;
            t.SetScaledHeight(((h as f64 * scale).round() as u32).max(1))?;
            t.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
        }
        dec.GetSoftwareBitmapTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &t,
            ExifOrientationMode::RespectExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()
    }

    fn recognize(engine: &OcrEngine, bmp: &SoftwareBitmap) -> windows::core::Result<Lines> {
        let res = engine.RecognizeAsync(bmp)?.join()?;
        // CJK ideographs/kana/hangul plus CJK and fullwidth punctuation.
        let cjk = |s: &str| {
            s.chars().any(|c| {
                ('\u{3000}'..='\u{9FFF}').contains(&c) || ('\u{AC00}'..='\u{D7AF}').contains(&c) || ('\u{FF00}'..='\u{FFEF}').contains(&c)
            })
        };
        let mut items = Vec::new();
        for line in res.Lines()? {
            let words = line.Words()?;
            let mut text = String::new();
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            let mut prev_cjk = false;
            for (k, w) in words.into_iter().enumerate() {
                let t = w.Text()?.to_string_lossy();
                let r = w.BoundingRect()?;
                let is_cjk = cjk(&t);
                // WinRT splits CJK into single-character "words"; only Latin gets spaces.
                if k > 0 && !(is_cjk && prev_cjk) {
                    text.push(' ');
                }
                prev_cjk = is_cjk;
                text.push_str(&t);
                x0 = x0.min(r.X as f64);
                y0 = y0.min(r.Y as f64);
                x1 = x1.max((r.X + r.Width) as f64);
                y1 = y1.max((r.Y + r.Height) as f64);
            }
            if !text.trim().is_empty() && x1 > x0 {
                items.push(LayoutItem { text, x_left: x0, y_top: y0, width: x1 - x0, height: (y1 - y0).max(1.0) });
            }
        }
        Ok(crate::ocr::xy_cut_lines(&items))
    }

    pub fn available() -> bool {
        on_mta(|| engine().is_some()).unwrap_or(false)
    }

    pub fn ocr_image_bytes(bytes: &[u8]) -> Result<Lines, String> {
        let data = bytes.to_vec();
        on_mta(move || {
            let e = engine().ok_or("ocr_no_engine")?;
            let max = OcrEngine::MaxImageDimension().unwrap_or(10_000);
            let s = stream_of(&data).map_err(|e| format!("图片解码失败：{e}"))?;
            let b = bitmap(&s, max).map_err(|e| format!("图片解码失败：{e}"))?;
            recognize(&e, &b).map_err(|e| format!("识别失败：{e}"))
        })?
    }

    /// OCR each page of a PDF (optionally only `pages`, 0-based), stopping when
    /// `cancel` returns true.  Returns per-page lines.
    pub fn ocr_pdf_bytes(
        bytes: &[u8],
        max_pages: usize,
        only: Option<Vec<usize>>,
        cancel: &'static (dyn Fn() -> bool + Sync),
    ) -> Result<Vec<(usize, Lines)>, String> {
        let data = bytes.to_vec();
        on_mta(move || {
            let e = engine().ok_or("ocr_no_engine")?;
            let max = OcrEngine::MaxImageDimension().unwrap_or(10_000);
            let s = stream_of(&data).map_err(|e| format!("PDF 读取失败：{e}"))?;
            let doc = PdfDocument::LoadFromStreamAsync(&s).and_then(|op| op.join()).map_err(|e| format!("PDF 读取失败：{e}"))?;
            let n = doc.PageCount().unwrap_or(0) as usize;
            let mut out = Vec::new();
            let pages: Vec<usize> = match only {
                Some(v) => v.into_iter().filter(|&p| p < n).take(max_pages).collect(),
                None => (0..n.min(max_pages)).collect(),
            };
            for p in pages {
                if cancel() {
                    break;
                }
                let lines = (|| -> windows::core::Result<Lines> {
                    let page = doc.GetPage(p as u32)?;
                    let size = page.Size()?;
                    // ~200 DPI (points are 1/72 in), capped by the engine.
                    let mut w = (size.Width as f64 * 200.0 / 72.0).round().max(1.0);
                    let mut h = (size.Height as f64 * 200.0 / 72.0).round().max(1.0);
                    let longest = w.max(h);
                    if longest > max as f64 {
                        w *= max as f64 / longest;
                        h *= max as f64 / longest;
                    }
                    let opts = PdfPageRenderOptions::new()?;
                    opts.SetDestinationWidth(w as u32)?;
                    opts.SetDestinationHeight(h as u32)?;
                    let png = InMemoryRandomAccessStream::new()?;
                    page.RenderWithOptionsToStreamAsync(&png, &opts)?.join()?;
                    png.Seek(0)?;
                    let bmp = bitmap(&png, max)?;
                    recognize(&e, &bmp)
                })()
                .unwrap_or_default();
                out.push((p, lines));
            }
            Ok(out)
        })?
    }

    type Job = Box<dyn FnOnce() + Send>;

    /// One long-lived MTA worker.  WinRT activation factories are cached
    /// process-wide by the `windows` crate; if the thread that created them
    /// left the apartment (thread exit tears the MTA down) the cached
    /// pointers would dangle, so every WinRT call runs on this thread.
    fn worker() -> Option<&'static std::sync::Mutex<std::sync::mpsc::Sender<Job>>> {
        static W: std::sync::OnceLock<Option<std::sync::Mutex<std::sync::mpsc::Sender<Job>>>> = std::sync::OnceLock::new();
        W.get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("readmd-ocr".into())
                .spawn(move || {
                    init();
                    for job in rx {
                        job();
                    }
                })
                .ok()?;
            Some(std::sync::Mutex::new(tx))
        })
        .as_ref()
    }

    /// Run `f` on the OCR worker, containing panics.
    fn on_mta<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let job: Job = Box::new(move || {
            let _ = tx.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
        });
        worker()
            .ok_or("ocr worker unavailable")?
            .lock()
            .map_err(|_| "ocr worker poisoned")?
            .send(job)
            .map_err(|_| "ocr worker stopped")?;
        rx.recv().map_err(|_| "ocr worker stopped".to_string())?.map_err(|_| "ocr panicked".to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn available() -> bool {
        false
    }
    pub fn ocr_image_bytes(_bytes: &[u8]) -> Result<Lines, String> {
        Err("ocr_no_engine".into())
    }
    pub fn ocr_pdf_bytes(
        _bytes: &[u8],
        _max_pages: usize,
        _only: Option<Vec<usize>>,
        _cancel: &'static (dyn Fn() -> bool + Sync),
    ) -> Result<Vec<(usize, Lines)>, String> {
        Err("ocr_no_engine".into())
    }
}

pub use imp::{available, ocr_image_bytes, ocr_pdf_bytes};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_image_is_an_error_not_a_panic() {
        if !available() {
            assert!(ocr_image_bytes(b"x").is_err());
            return;
        }
        assert!(ocr_image_bytes(b"definitely not an image").is_err());
        static NO: fn() -> bool = || false;
        assert!(ocr_pdf_bytes(b"%PDF-broken", 3, None, &NO).is_err());
    }
}

