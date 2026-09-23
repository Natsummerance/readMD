//! 音视频转写：`src/readmd_modules/transcribe.py` 的 Rust 对等实现。
//!
//! Python 侧的真实转写依赖插件沙箱里的 `faster_whisper` / `whisper` 加系统
//! ffmpeg。Rust 内核不加载 Python 插件、也不把音频丢给外部二进制，因此永远落在
//! Python 自己的降级分支上：`transcribe_to_md()` 返回
//! `_make_whisper_notice()` 生成的安装指引 Markdown **加**一个 warning，
//! 于是 `_api_transcribe` 走 200 `{ok, content, path, warning}`——不是错误码。
//! 这条降级路径本身就是要对等的目标（见 `parity_web::h_transcribe`）。

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Python `SUPPORTED_AUDIO_VIDEO_EXTS`，顺序与内容都不许改动。
pub const SUPPORTED_AUDIO_VIDEO_EXTS: &[&str] = &[
    ".mp3", ".wav", ".m4a", ".mp4", ".flac", ".ogg", ".webm", ".aac", ".wma", ".mkv", ".mov",
    ".avi",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscribeErrorCode {
    #[serde(rename = "transcribe_dependency_missing")]
    TranscribeDependencyMissing,
    #[serde(rename = "transcribe_audio_not_found")]
    TranscribeAudioNotFound,
    #[serde(rename = "transcribe_audio_format_unsupported")]
    TranscribeAudioFormatUnsupported,
    #[serde(rename = "transcribe_process_failed")]
    TranscribeProcessFailed,
    #[serde(rename = "transcribe_invalid_audio")]
    TranscribeInvalidAudio,
}

#[derive(Debug, Clone)]
pub struct TranscribeError {
    pub code: String,
    pub error_code: TranscribeErrorCode,
}

impl std::fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}

/// Python `transcribe.load()`：刻意保持轻量，只返回 True。
pub fn load() -> Result<(), String> {
    Ok(())
}

/// Python `is_supported_media(path)`。
pub fn is_supported_media(path: &str) -> bool {
    let ext = extension(path);
    SUPPORTED_AUDIO_VIDEO_EXTS.contains(&ext.as_str())
}

/// `os.path.splitext(path)[1].lower()`。
fn extension(path: &str) -> String {
    let tail = match path.rfind(['/', '\\']) {
        Some(i) => &path[i + 1..],
        None => path,
    };
    match tail.rfind('.') {
        Some(0) | None => String::new(),
        Some(i) => tail[i..].to_ascii_lowercase(),
    }
}

/// Python `os.path.splitext(path)[1].lstrip('.').lower()`。
fn extension_stripped(path: &str) -> String {
    extension(path).trim_start_matches('.').to_string()
}

fn basename(path: &str) -> String {
    match path.rfind(['/', '\\']) {
        Some(i) => path[i + 1..].to_string(),
        None => path.to_string(),
    }
}

/// Python `format_timestamp(seconds, brackets=True)`。
pub fn format_timestamp(seconds: f64, brackets: bool) -> String {
    let total = seconds as i64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    let ts = if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    };
    if brackets {
        format!("[{}]", ts)
    } else {
        ts
    }
}

/// 一条 Whisper 段落，只需要 `start` 与 `text`。
#[derive(Debug, Clone, Default)]
pub struct Segment {
    pub start: f64,
    pub text: String,
}

/// Python `format_segments(...)`。
pub fn format_segments(
    segments: &[Segment],
    title: Option<&str>,
    language: Option<&str>,
    duration: Option<f64>,
    file_format: Option<&str>,
    model_name: Option<&str>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut frontmatter: Vec<String> = Vec::new();
    if let Some(t) = title {
        frontmatter.push(format!("title: \"{}\"", t));
    }
    if let Some(f) = file_format {
        frontmatter.push(format!("format: \"{}\"", f));
    }
    if let Some(d) = duration {
        if d >= 0.0 {
            frontmatter.push(format!("duration: \"{}\"", format_timestamp(d, false)));
        }
    }
    if let Some(m) = model_name {
        frontmatter.push(format!("model: \"{}\"", m));
    }
    if let Some(l) = language {
        frontmatter.push(format!("language: \"{}\"", l));
    }
    if !frontmatter.is_empty() {
        lines.push("---".to_string());
        lines.extend(frontmatter);
        lines.push("---".to_string());
    }
    if let Some(t) = title {
        lines.push(format!("# 音频/视频转写：{}", t));
    }
    if let Some(l) = language {
        lines.push(format!("> 识别语言：`{}`", l));
    }
    for seg in segments {
        let text = seg.text.trim();
        if !text.is_empty() {
            lines.push(format!("**{}** {}", format_timestamp(seg.start, true), text));
        }
    }
    let has_valid = segments.iter().any(|s| !s.text.trim().is_empty());
    if !has_valid {
        if let Some(t) = title {
            if !t.is_empty() {
                lines.push("> （未识别到有效语音内容）".to_string());
            }
        }
    }
    let joined = lines.join("\n\n");
    format!("{}\n", joined.trim())
}

/// Python `_make_whisper_notice(path, details)`。
///
/// 里面那行 `pip install openai-whisper` 是**给用户看的指引文本**，逐字符来自
/// `transcribe.py:126-144`（`pip` 那一行在 `:138`），本内核从不执行它、也从不
/// 启动任何外部程序。字节级一致性由
/// `scratch/rust_parity/_transcribe_notice_compare.py` 对照真实 Python 函数输出
/// 量过（`ALL_IDENTICAL=True`）。
pub fn make_whisper_notice(path: &str, details: &str) -> String {
    let title = basename(path);
    let ext = extension_stripped(path);
    format!(
        "---\ntitle: \"{title}\"\nformat: \"{ext}\"\nstatus: \"unprocessed\"\n---\n\n\
         # 音频/视频转写：{title}\n\n\
         > **{details}**\n>\n\
         > **快速安装指引**：\n\
         > 1. **方式一（推荐）**：在 ReadMD 右上角打开「插件中心」，启用或一键安装 `whisper` 插件。\n\
         > 2. **方式二（手动 CLI 命令）**：\n\
         >    ```bash\n\
         >    pip install openai-whisper\n\
         >    ```\n\
         >    若系统缺少 FFmpeg，请运行对应命令安装并加入环境变量 PATH：\n\
         >    - **Windows**: `winget install Gyan.FFmpeg` 或从官网解压\n\
         >    - **macOS**: `brew install ffmpeg`\n\
         >    - **Linux**: `sudo apt install ffmpeg`\n"
    )
}

/// Python `transcribe_to_md(path, language=None, model_name='base')`
/// → `(text, error)`，两者都可能为 `None`。
pub fn transcribe_to_md(
    path: &str,
    language: Option<&str>,
    model_name: &str,
) -> (Option<String>, Option<String>) {
    if !Path::new(path).is_file() {
        return (None, Some("file_not_found".to_string()));
    }
    if !is_supported_media(path) {
        return (None, Some("unsupported_media_format".to_string()));
    }
    let _ = (language, model_name);
    // 无 faster_whisper，且 whisper/ffmpeg 不成对可用：Python 在这一步直接返回
    // 安装指引 + warning，HTTP 层看到的是 200。
    (
        Some(make_whisper_notice(path, "未检测到语音转写模型或 FFmpeg 工具")),
        Some("Whisper plugin or FFmpeg not available.".to_string()),
    )
}

/// 既有内核调用点（`convert.rs` 的音视频分支、`batch2::h_transcribe`）使用的
/// 兼容入口：Python 的 `convert._convert_media` 拿到 notice 文本时算成功。
pub fn transcribe_audio(
    audio_path: &str,
    language: Option<&str>,
    model: Option<&str>,
) -> Result<String, TranscribeError> {
    let model_name = model.unwrap_or("base");
    let (text, err) = transcribe_to_md(audio_path, language, model_name);
    match text {
        Some(t) if !t.trim().is_empty() => Ok(t),
        _ => Err(TranscribeError {
            code: err.unwrap_or_else(|| "transcribe_empty".to_string()),
            error_code: if !Path::new(audio_path).is_file() {
                TranscribeErrorCode::TranscribeAudioNotFound
            } else {
                TranscribeErrorCode::TranscribeDependencyMissing
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_media(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("readmd-transcribe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        let mut handle = std::fs::File::create(&file).unwrap();
        handle.write_all(b"fake audio bytes").unwrap();
        file
    }

    #[test]
    fn module_load_is_always_ready_like_python() {
        assert!(load().is_ok());
    }

    #[test]
    fn supported_media_matches_the_python_extension_tuple() {
        assert_eq!(SUPPORTED_AUDIO_VIDEO_EXTS.len(), 12);
        for name in [".mp3", ".wav", ".m4a", ".mp4", ".flac", ".ogg", ".webm", ".aac", ".wma", ".mkv", ".mov", ".avi"] {
            assert!(is_supported_media(&format!("C:\\m\\x{}", name)), "{}", name);
        }
        assert!(is_supported_media("C:\\m\\x.MP3"));
        assert!(!is_supported_media("C:\\m\\x.flv"));
        assert!(!is_supported_media("C:\\m\\notes.txt"));
        assert!(!is_supported_media("C:\\m\\noext"));
    }

    #[test]
    fn timestamps_match_the_python_formatter() {
        assert_eq!(format_timestamp(65.9, true), "[01:05]");
        assert_eq!(format_timestamp(65.9, false), "01:05");
        assert_eq!(format_timestamp(3661.0, true), "[01:01:01]");
        assert_eq!(format_timestamp(0.0, false), "00:00");
    }

    #[test]
    fn degraded_run_returns_notice_plus_warning() {
        let file = temp_media("degraded.mp3");
        let path = file.to_str().unwrap();
        let (text, err) = transcribe_to_md(path, None, "base");
        let notice = text.unwrap();
        assert_eq!(
            err.as_deref(),
            Some("Whisper plugin or FFmpeg not available.")
        );
        assert!(notice.starts_with("---\ntitle: \"degraded.mp3\"\nformat: \"mp3\"\nstatus: \"unprocessed\"\n---\n\n"), "{notice}");
        assert!(notice.contains("# 音频/视频转写：degraded.mp3"), "{notice}");
        assert!(notice.ends_with(">    - **Linux**: `sudo apt install ffmpeg`\n"), "{notice}");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn missing_and_unsupported_files_short_circuit() {
        let (text, err) = transcribe_to_md("C:\\nope\\gone.mp3", None, "base");
        assert!(text.is_none());
        assert_eq!(err.as_deref(), Some("file_not_found"));
        let file = temp_media("notes.txt");
        let (text, err) = transcribe_to_md(file.to_str().unwrap(), None, "base");
        assert!(text.is_none());
        assert_eq!(err.as_deref(), Some("unsupported_media_format"));
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn segments_render_frontmatter_and_empty_notice() {
        let md = format_segments(
            &[Segment { start: 0.0, text: "你好".into() }, Segment { start: 75.0, text: "  ".into() }],
            Some("a.mp3"),
            Some("zh"),
            Some(80.0),
            Some("mp3"),
            Some("base"),
        );
        assert!(md.starts_with("---\n\ntitle: \"a.mp3\"\n"), "{md}");
        assert!(md.contains("duration: \"01:20\""), "{md}");
        assert!(md.contains("**[00:00]** 你好"), "{md}");
        assert!(!md.contains("未识别到有效语音内容"), "{md}");

        let empty = format_segments(&[], Some("a.mp3"), None, None, Some("mp3"), Some("base"));
        assert!(empty.contains("> （未识别到有效语音内容）"), "{empty}");
        assert!(empty.ends_with('\n'), "{empty}");
    }
}
