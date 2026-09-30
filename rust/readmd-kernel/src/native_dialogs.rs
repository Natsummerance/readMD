//! Cross-platform native file dialogs and "open / reveal in file manager".
//!
//! * Windows: the in-process `IFileDialog` from [`crate::win_dialogs`], with the
//!   WinForms-in-PowerShell picker kept only as a fallback for a machine where
//!   COM refuses to start.
//! * macOS: `osascript` (`choose file` / `choose folder` / `choose file name`).
//! * Linux/BSD: `zenity`, then `kdialog`.
//!
//! Every backend reports the same three outcomes as `win_dialogs`: a pick, a
//! user cancel, or "no dialog could be shown" (`Unavailable`), so callers can
//! tell the user *why* nothing happened instead of treating it as a cancel.

pub use crate::win_dialogs::{DialogOutcome, DialogShape};
use std::path::Path;
#[cfg(not(windows))]
use std::process::Command;

/// A request independent of the backend.
#[derive(Debug, Clone)]
pub struct Request {
    pub shape: DialogShape,
    /// Suggested file name for save shapes.
    pub default_name: Option<String>,
    /// Folder the dialog should open in.
    pub initial_dir: Option<String>,
    /// Save dialogs: `(extension, "Label (*.ext)|*.ext|…")`.
    pub export_format: Option<String>,
}

impl Request {
    pub fn new(shape: DialogShape) -> Self {
        Request { shape, default_name: None, initial_dir: None, export_format: None }
    }
    pub fn name(mut self, n: &str) -> Self {
        if !n.trim().is_empty() {
            self.default_name = Some(n.to_string());
        }
        self
    }
    pub fn dir(mut self, d: &str) -> Self {
        if !d.trim().is_empty() && Path::new(d).is_dir() {
            self.initial_dir = Some(d.to_string());
        }
        self
    }
    pub fn export(mut self, fmt: &str) -> Self {
        self.export_format = Some(fmt.to_string());
        self
    }

    /// The suggested name with the export extension applied.
    pub fn effective_name(&self) -> Option<String> {
        let base = self.default_name.clone();
        match (&self.export_format, base) {
            (Some(fmt), Some(b)) => {
                let ext = DialogShape::export_filter_text(fmt).map(|(e, _)| e).unwrap_or(fmt.as_str());
                Some(crate::win_dialogs::with_extension(&b, ext))
            }
            (_, b) => b,
        }
    }

    fn filter_text(&self) -> Option<&'static str> {
        match &self.export_format {
            Some(fmt) => DialogShape::export_filter_text(fmt).map(|(_, t)| t),
            None => self.shape.filter_text(),
        }
    }

    fn title(&self) -> &'static str {
        self.shape.title()
    }
}

/// Show the dialog and block until the user decides.
pub fn run(req: &Request) -> DialogOutcome {
    #[cfg(windows)]
    {
        let mut wr = match &req.export_format {
            Some(fmt) => crate::win_dialogs::DialogRequest::for_export(fmt, &req.effective_name().unwrap_or_default()),
            None => crate::win_dialogs::DialogRequest::for_shape(req.shape),
        };
        if req.export_format.is_none() {
            if let Some(n) = req.effective_name() {
                wr = wr.with_default_name(&n);
            }
        }
        if let Some(d) = &req.initial_dir {
            wr = wr.with_initial_dir(d);
        }
        match crate::win_dialogs::run(&wr) {
            DialogOutcome::Unavailable => powershell_fallback(req),
            other => other,
        }
    }
    #[cfg(target_os = "macos")]
    {
        mac::run(req)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::run(req)
    }
}

#[cfg(windows)]
fn powershell_fallback(req: &Request) -> DialogOutcome {
    fn q(s: &str) -> String {
        s.replace('\'', "''")
    }
    let (class, extra) = match req.shape {
        DialogShape::ChooseFolder => ("FolderBrowserDialog", "$dialog.ShowNewFolderButton = $true\n$dialog.Description = '选择文件夹'".to_string()),
        DialogShape::ChooseManyFiles => ("OpenFileDialog", "$dialog.Multiselect = $true".to_string()),
        DialogShape::ChooseFile | DialogShape::ChooseAnyFile => ("OpenFileDialog", String::new()),
        _ => ("SaveFileDialog", String::new()),
    };
    let mut lines = vec![
        "$OutputEncoding = [System.Text.Encoding]::UTF8".to_string(),
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8".to_string(),
        "[void][System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms')".to_string(),
        "$form = New-Object System.Windows.Forms.Form".to_string(),
        "$form.TopMost = $true".to_string(),
        format!("$dialog = New-Object System.Windows.Forms.{class}"),
        extra,
    ];
    if class != "FolderBrowserDialog" {
        lines.push(format!("$dialog.Title = '{}'", q(req.title())));
        if let Some(f) = req.filter_text() {
            lines.push(format!("$dialog.Filter = '{}'", q(f)));
        }
        if let Some(n) = req.effective_name() {
            lines.push(format!("$dialog.FileName = '{}'", q(&n)));
        }
        if let Some(d) = &req.initial_dir {
            lines.push(format!("$dialog.InitialDirectory = '{}'", q(d)));
        }
    }
    let out = if class == "FolderBrowserDialog" { "$dialog.SelectedPath" } else if req.shape == DialogShape::ChooseManyFiles { "($dialog.FileNames -join \"`n\")" } else { "$dialog.FileName" };
    lines.push(format!(
        "if ($dialog.ShowDialog($form) -eq [System.Windows.Forms.DialogResult]::OK) {{ [Console]::Out.Write({out}) }} else {{ [Console]::Out.Write('::cancel::') }}"
    ));
    match crate::server::run_powershell_encoded(&lines.join("\n")) {
        None => DialogOutcome::Unavailable,
        Some(s) if s.trim() == "::cancel::" => DialogOutcome::Canceled,
        Some(s) => picked(&s),
    }
}

fn picked(text: &str) -> DialogOutcome {
    let v: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    if v.is_empty() {
        DialogOutcome::Canceled
    } else {
        DialogOutcome::Picked(v)
    }
}

/// Extensions of a `Label|*.a;*.b|…` filter (first group only), for the Unix
/// backends that take a plain glob list.
#[cfg(not(windows))]
fn filter_globs(req: &Request) -> Option<(String, Vec<String>)> {
    let text = req.filter_text()?;
    let mut parts = text.split('|');
    let label = parts.next()?.to_string();
    let spec = parts.next()?;
    let globs: Vec<String> = spec.split(';').map(|s| s.trim().to_string()).filter(|s| s != "*.*" && !s.is_empty()).collect();
    if globs.is_empty() {
        None
    } else {
        Some((label, globs))
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;

    fn as_str(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }

    pub fn run(req: &Request) -> DialogOutcome {
        let prompt = as_str(req.title());
        let loc = req
            .initial_dir
            .as_ref()
            .map(|d| format!(" default location (POSIX file {})", as_str(d)))
            .unwrap_or_default();
        let script = match req.shape {
            DialogShape::ChooseFolder => format!("POSIX path of (choose folder with prompt {prompt}{loc})"),
            DialogShape::ChooseManyFiles => format!(
                "set fs to (choose file with prompt {prompt}{loc} with multiple selections allowed)\nset out to \"\"\nrepeat with f in fs\nset out to out & POSIX path of f & linefeed\nend repeat\nreturn out"
            ),
            DialogShape::ChooseFile => {
                let types = filter_globs(req)
                    .map(|(_, g)| {
                        let list: Vec<String> = g.iter().map(|x| as_str(x.trim_start_matches("*."))).collect();
                        format!(" of type {{{}}}", list.join(", "))
                    })
                    .unwrap_or_default();
                format!("POSIX path of (choose file with prompt {prompt}{types}{loc})")
            }
            DialogShape::ChooseAnyFile => format!("POSIX path of (choose file with prompt {prompt}{loc})"),
            _ => {
                let name = req.effective_name().map(|n| format!(" default name {}", as_str(&n))).unwrap_or_default();
                format!("POSIX path of (choose file name with prompt {prompt}{name}{loc})")
            }
        };
        match Command::new("osascript").arg("-e").arg(&script).output() {
            Err(_) => DialogOutcome::Unavailable,
            Ok(o) if o.status.success() => picked(&String::from_utf8_lossy(&o.stdout)),
            // -128 is "User canceled."
            Ok(o) if String::from_utf8_lossy(&o.stderr).contains("-128") => DialogOutcome::Canceled,
            Ok(_) => DialogOutcome::Unavailable,
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod linux {
    use super::*;

    fn start_path(req: &Request) -> Option<String> {
        let dir = req.initial_dir.clone();
        match (dir, req.effective_name()) {
            (Some(d), Some(n)) => Some(format!("{}/{}", d.trim_end_matches('/'), n)),
            (Some(d), None) => Some(format!("{}/", d.trim_end_matches('/'))),
            (None, Some(n)) => Some(n),
            (None, None) => None,
        }
    }

    fn zenity(req: &Request) -> Option<DialogOutcome> {
        let mut c = Command::new("zenity");
        c.arg("--file-selection").arg(format!("--title={}", req.title()));
        match req.shape {
            DialogShape::ChooseFolder => {
                c.arg("--directory");
            }
            DialogShape::ChooseManyFiles => {
                c.arg("--multiple").arg("--separator=\n");
            }
            DialogShape::ChooseFile | DialogShape::ChooseAnyFile => {}
            _ => {
                c.arg("--save").arg("--confirm-overwrite");
            }
        }
        if let Some((label, globs)) = filter_globs(req) {
            c.arg(format!("--file-filter={} | {}", label, globs.join(" ")));
            c.arg("--file-filter=* | *");
        }
        if let Some(p) = start_path(req) {
            c.arg(format!("--filename={p}"));
        }
        let o = c.output().ok()?;
        Some(match o.status.code() {
            Some(0) => picked(&String::from_utf8_lossy(&o.stdout)),
            Some(1) => DialogOutcome::Canceled,
            _ => DialogOutcome::Unavailable,
        })
    }

    fn kdialog(req: &Request) -> Option<DialogOutcome> {
        let mut c = Command::new("kdialog");
        c.arg("--title").arg(req.title());
        let start = start_path(req).unwrap_or_else(|| ".".to_string());
        let filter = filter_globs(req).map(|(label, g)| format!("{} ({})", label, g.join(" ")));
        match req.shape {
            DialogShape::ChooseFolder => {
                c.arg("--getexistingdirectory").arg(&start);
            }
            DialogShape::ChooseManyFiles => {
                c.arg("--getopenfilename").arg(&start).arg("--multiple").arg("--separate-output");
            }
            DialogShape::ChooseFile | DialogShape::ChooseAnyFile => {
                c.arg("--getopenfilename").arg(&start);
                if let Some(f) = &filter {
                    c.arg(f);
                }
            }
            _ => {
                c.arg("--getsavefilename").arg(&start);
                if let Some(f) = &filter {
                    c.arg(f);
                }
            }
        }
        let o = c.output().ok()?;
        Some(match o.status.code() {
            Some(0) => picked(&String::from_utf8_lossy(&o.stdout)),
            Some(1) => DialogOutcome::Canceled,
            _ => DialogOutcome::Unavailable,
        })
    }

    pub fn run(req: &Request) -> DialogOutcome {
        zenity(req).or_else(|| kdialog(req)).unwrap_or(DialogOutcome::Unavailable)
    }
}

/// Open a file or folder with the desktop's default handler.
pub fn open_path(path: &str) -> Result<(), &'static str> {
    if path.trim().is_empty() || !Path::new(path).exists() {
        return Err("path_not_found");
    }
    #[cfg(windows)]
    {
        if crate::native_system::windows_open_path(path) {
            Ok(())
        } else {
            Err("open_failed")
        }
    }
    #[cfg(target_os = "macos")]
    {
        spawn_ok(Command::new("open").arg(path))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        spawn_ok(Command::new("xdg-open").arg(path))
    }
}

/// Show a file selected in the file manager (its folder, for platforms that
/// cannot select).
pub fn reveal_path(path: &str) -> Result<(), &'static str> {
    if path.trim().is_empty() || !Path::new(path).exists() {
        return Err("path_not_found");
    }
    #[cfg(windows)]
    {
        if crate::native_system::windows_reveal_path(path) {
            Ok(())
        } else {
            Err("open_failed")
        }
    }
    #[cfg(target_os = "macos")]
    {
        spawn_ok(Command::new("open").arg("-R").arg(path))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // The FreeDesktop FileManager1 interface selects the item; fall back to
        // opening the parent folder.
        let uri = format!("file://{}", path);
        let dbus = Command::new("dbus-send")
            .args([
                "--session",
                "--dest=org.freedesktop.FileManager1",
                "--type=method_call",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
            ])
            .arg(format!("array:string:{uri}"))
            .arg("string:")
            .status();
        if matches!(dbus, Ok(s) if s.success()) {
            return Ok(());
        }
        let p = Path::new(path);
        let dir = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
        spawn_ok(Command::new("xdg-open").arg(dir))
    }
}

#[cfg(not(windows))]
fn spawn_ok(cmd: &mut Command) -> Result<(), &'static str> {
    use std::process::Stdio;
    match cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(_) => Ok(()),
        Err(_) => Err("open_failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_name_gets_extension_once() {
        let r = Request::new(DialogShape::Export).name("report").export("docx");
        assert_eq!(r.effective_name().as_deref(), Some("report.docx"));
        let r = Request::new(DialogShape::Export).name("report.DOCX").export("docx");
        assert_eq!(r.effective_name().as_deref(), Some("report.DOCX"));
    }

    #[test]
    fn missing_dir_is_ignored() {
        let r = Request::new(DialogShape::SaveAs).dir("Z:/definitely/not/here");
        assert!(r.initial_dir.is_none());
    }

    #[test]
    fn open_and_reveal_report_missing_paths() {
        assert_eq!(open_path(""), Err("path_not_found"));
        assert_eq!(reveal_path("Z:/definitely/not/here.md"), Err("path_not_found"));
    }

    #[test]
    fn picked_splits_lines_and_empty_is_cancel() {
        assert_eq!(picked("a\r\nb\n"), DialogOutcome::Picked(vec!["a".into(), "b".into()]));
        assert_eq!(picked("  \n"), DialogOutcome::Canceled);
    }
}
