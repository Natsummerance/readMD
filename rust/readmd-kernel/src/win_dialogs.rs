// Native Win32 common-item dialogs — the replacement for the six PowerShell
// `System.Windows.Forms` file/folder pickers in `server.rs`.
//
// # Parity contract
//
// The six `/api/dialog/*` routes are annotated in the route table
// (`server.rs:683-688`) as `KERNEL BRIDGE — no readmd.py route`, so there is no
// Python authority to diff against.  Two different things are authoritative and
// they are not the same thing:
//
//   * the **contract** is the JSON envelope the current handlers return
//     (`{"ok":true,"path":null}` on a cancel, `{"ok":true,"paths":[]}` for the
//     multi-select route, `{"ok":false,"canceled":true}` for `save-as`);
//   * the **behavior** is what a real Windows file dialog does, which today is
//     produced by WinForms inside a spawned `powershell.exe`.
//
// This module keeps both.  Every string it hands the OS is the string the
// PowerShell script builds today (same titles, same `Filter` text, same
// default file name), and `DialogOutcome` is designed so the callers in
// `server.rs` can emit byte-identical JSON — see `scratch/rust_parity/
// win_dialogs_s14/apply_plan.md`.
//
// # Why this replaces a process spawn
//
// `run_powershell_dialog` pays a `powershell.exe` start-up (a few hundred ms,
// plus the .NET/WinForms assembly load) per click, needs the execution policy
// to allow scripts at all, and swallows every interesting failure: a cancel, a
// missing `powershell.exe`, a script that threw and a dialog that produced an
// empty path are all `None`.  Interpolating the caller's file name into a
// single-quoted script literal (`default_name.replace('\'', "''")`) is also a
// quoting context that only has to be right once.  `IFileDialog` is a direct
// in-process COM call: no spawn, no shell dependency, no quoting context, and
// a cancel that arrives as its own HRESULT.
//
// # Measured deviations from the lane brief
//
// All three were found with a live-OS probe, not from documentation; the
// evidence is `scratch/rust_parity/win_dialogs_s14/probe_guids.py` and its
// `probe_guids.log`.
//
//   * **`COINIT_DISABLE_OLE1DDE` is rejected on this build.**  The brief asked
//     for `COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE` (0x22); measured
//     on a brand-new thread that mask returns `E_INVALIDARG`, and so does
//     0x20 alone (log lines 71-72).  `COINIT_APARTMENTTHREADED` (0x02) returns
//     `S_OK` (line 70) and is what `init_com` uses.
//   * **`CLSID_FileBrowserDialog` does not exist here.**  Unregistered under
//     every `CLSCTX`, absent from `HKCR\CLSID`, and not declared anywhere in
//     the 10.0.22621.0 SDK headers (log line 19).  The folder shape is
//     therefore `CLSID_FileOpenDialog` + `FOS_PICKFOLDERS`, which the OS
//     accepts and reports back (log line 62).  There is likewise no
//     `IFolderDialog` to declare.
//   * **`Show` is vtable slot 3, not a slot of `IFileDialog`.**  `IFileDialog`
//     derives from `IModalWindow`, so the common prefix is
//     `QueryInterface/AddRef/Release/Show` and everything below it shifts by
//     one against the IDL's own listing.  Getting this wrong is not a type
//     error: calling slot 3 as if it were `SetOptions` returns `E_INVALIDARG`
//     because 0x204 is consumed as an `HWND`, and calling slot 4 as
//     `GetOptions` returned `E_OUTOFMEMORY` and then took the process with it.
//
// Two more shape bugs the probe caught and that are easy to copy from the web:
// `IShellItemArray::GetItemAt` takes `(DWORD index, IShellItem **out)` with **no
// REFIID** — the 3-argument form answers `S_OK` and writes nothing at all — and
// three of the GUID literals in circulation (including the ones the brief
// quoted) differ from the SDK values by hex digits that only fail at runtime.
//
// # FFI budget
//
// `ole32` (`CoInitializeEx`, `CoUninitialize`, `CoCreateInstance`,
// `CoTaskMemFree`) and `shell32` (`SHCreateItemFromParsingName`), plus
// `user32` for one `GetForegroundWindow` call used as the owner handle.  No new
// crate dependency: `Cargo.toml` gains nothing, and the interfaces are declared
// by hand as `#[repr(C)]` vtables, the precedent being the hand-written
// `extern "system"` blocks in `crypto.rs` and `native_system.rs`.
//
// Linking the SDK's own `EXTERN_C const IID IID_IFileDialog` symbols was
// considered and rejected: they live in `uuid.lib`/the import libraries, which
// is a second thing to get right per toolchain, and the hand-rolled literals
// are provable on any machine by the layout test in the probe instead.
//
// # Threading
//
// Handlers run on worker threads, so COM is initialised per call on the calling
// thread and released when this module's own guard drops.  The dialog objects
// are `*mut c_void` behind a wrapper with no `Send`/`Sync` impl, which means
// the compiler enforces the apartment rule for us: a dialog cannot be moved to
// another thread, and nothing here outlives the call that created it.
// `IFileDialog::Show` runs its own modal message loop, so the calling thread
// does not need a pre-existing pump — but it does block for as long as the user
// takes, exactly like the `Command::output()` it replaces.  That is the current
// contract; see the hazard note on `run`.

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Constants, verbatim from the Windows SDK 10.0.22621.0
// ---------------------------------------------------------------------------

/// Win32 `HRESULT` as Rust sees it: a signed 32-bit value, where the sign bit
/// is the failure flag.  Keeping it signed makes `hr < 0` the one test that
/// works for every code, and it is why the constants below are `as i32` rather
/// than unsigned literals.
pub type HResult = i32;

/// `S_OK`.
pub const S_OK: HResult = 0;
/// `S_FALSE` — succeeded, but the state was already what we asked for.
pub const S_FALSE: HResult = 1;
/// `E_INVALIDARG` (0x80070057).
pub const E_INVALIDARG: HResult = 0x8007_0057_u32 as i32;
/// `E_NOINTERFACE` (0x80004002).
pub const E_NOINTERFACE: HResult = 0x8000_4002_u32 as i32;
/// `E_UNEXPECTED` (0x8000FFFF) — what `GetResult` says before `Show`.
pub const E_UNEXPECTED: HResult = 0x8000_ffff_u32 as i32;
/// `REGDB_E_CLASSNOTREG` (0x80040154) — what a missing coclass says.
pub const REGDB_E_CLASSNOTREG: HResult = 0x8004_0154_u32 as i32;
/// `RPC_E_CHANGED_MODE` (0x80010106): the thread is already in an apartment of
/// a different model.  The dialog is still creatable (probe log line 76) but the
/// apartment is not ours, so we must not `CoUninitialize` it.
pub const RPC_E_CHANGED_MODE: HResult = 0x8001_0106_u32 as i32;
/// `ERROR_CANCELLED` (Win32 1223), the code `Show` returns when the user
/// pressed Cancel or closed the window.  This is the only HRESULT that means
/// "the user decided", and `is_cancel` is the one place that reads it.
pub const ERROR_CANCELLED: HResult = 0x8007_04c7_u32 as i32;

/// `COINIT_APARTMENTTHREADED` from `combaseapi.h`.  An STA is what a dialog
/// needs: `Show` pumps messages on this thread.
pub const COINIT_APARTMENTTHREADED: u32 = 0x2;
/// `CLSCTX_INPROC_SERVER` — the dialog is a DLL server, never out of process.
pub const CLSCTX_INPROC_SERVER: u32 = 0x1;

// `FILEOPENDIALOGOPTIONS`, `ShObjIdl_core.h:19534-19556`.
pub const FOS_OVERWRITEPROMPT: u32 = 0x2;
pub const FOS_STRICTFILETYPES: u32 = 0x4;
pub const FOS_NOCHANGEDIR: u32 = 0x8;
pub const FOS_PICKFOLDERS: u32 = 0x20;
pub const FOS_FORCEFILESYSTEM: u32 = 0x40;
pub const FOS_ALLOWMULTISELECT: u32 = 0x200;
pub const FOS_PATHMUSTEXIST: u32 = 0x800;
pub const FOS_FILEMUSTEXIST: u32 = 0x1000;
pub const FOS_CREATEPROMPT: u32 = 0x2000;
pub const FOS_HIDEPINNEDPLACES: u32 = 0x40000;

/// `SIGDN` values, `ShObjIdl_core.h:8714-8727`.  In C `SIGDN` is a signed enum
/// of the same width; declaring these `u32` and using them unmodified keeps the
/// bit pattern (`0x80058000`) honest, which is all the ABI transfers.
pub const SIGDN_FILESYSPATH: u32 = 0x8005_8000;
/// Fallback for a shell item that is not on the filesystem: a library, a
/// search results folder, a `\\OneDrive` placeholder.  WinForms ends up with the
/// parsing name for those too, so this is parity rather than a bonus.
pub const SIGDN_DESKTOPABSOLUTEPARSING: u32 = 0x8002_8000;

// ---------------------------------------------------------------------------
// Identifiers
//
// These live at module level rather than inside the `#[cfg(windows)]` block on
// purpose: they are plain data, and a wrong nibble in one of them is invisible
// to the compiler and shows up only as `REGDB_E_CLASSNOTREG` or `E_NOINTERFACE`
// at runtime.  Keeping them portable means the layout test runs on any host.
// ---------------------------------------------------------------------------

/// `GUID` / `CLSID` / `REFIID`.  `Data1/2/3` are native-endian and `Data4` is a
/// byte array in written order, which is why the literals below are split the
/// way they are.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    #[must_use]
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Guid {
        Guid { data1, data2, data3, data4 }
    }
}

// `ShObjIdl_core.h:18439` `MIDL_INTERFACE("b4db1657-70d7-485e-8e3e-6fcb5a5c1802")`.
pub const IID_IMODAL_WINDOW: Guid =
    Guid::new(0xb4db_1657, 0x70d7, 0x485e, [0x8e, 0x3e, 0x6f, 0xcb, 0x5a, 0x5c, 0x18, 0x02]);
// `ShObjIdl_core.h:19566` `MIDL_INTERFACE("42f85136-db7e-439c-85f1-e4075d135fc8")`.
pub const IID_IFILE_DIALOG: Guid =
    Guid::new(0x42f8_5136, 0xdb7e, 0x439c, [0x85, 0xf1, 0xe4, 0x07, 0x5d, 0x13, 0x5f, 0xc8]);
// `ShObjIdl_core.h:20239` `MIDL_INTERFACE("d57c7288-d4ad-4768-be02-9d969532d960")`.
pub const IID_IFILE_OPEN_DIALOG: Guid =
    Guid::new(0xd57c_7288, 0xd4ad, 0x4768, [0xbe, 0x02, 0x9d, 0x96, 0x95, 0x32, 0xd9, 0x60]);
// `ShObjIdl_core.h:8747` `MIDL_INTERFACE("43826d1e-e718-42ee-bc55-a1e261c37bfe")`.
pub const IID_ISHELL_ITEM: Guid =
    Guid::new(0x4382_6d1e, 0xe718, 0x42ee, [0xbc, 0x55, 0xa1, 0xe2, 0x61, 0xc3, 0x7b, 0xfe]);
// `ShObjIdl_core.h:10889`
// `MIDL_INTERFACE("b63ea76d-1f85-456f-a19c-48159efa858b")`.  The value this lane
// was handed (`b63ea768-1f1f-4d3f-813d-d5fbf84bdbd8`) is a *different*
// interface's IID: with it `SHCreateShellItemArrayFromShellItem` answers
// `E_NOINTERFACE`, which reads like a shell bug and is a hex digit.
pub const IID_ISHELL_ITEM_ARRAY: Guid =
    Guid::new(0xb63e_a76d, 0x1f85, 0x456f, [0xa1, 0x9c, 0x48, 0x15, 0x9e, 0xfa, 0x85, 0x8b]);
// `ShObjIdl_core.h:28933` `DECLSPEC_UUID("DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7")`.
pub const CLSID_FILE_OPEN_DIALOG: Guid =
    Guid::new(0xdc1c_5a9c, 0xe88a, 0x4dde, [0xa5, 0xa1, 0x60, 0xf8, 0x2a, 0x20, 0xae, 0xf7]);
// `ShObjIdl_core.h:28941` `DECLSPEC_UUID("C0B4E2F3-BA21-4773-8DBA-335EC946EB8B")`.
// Also confirmed by `HKCR\CLSID` naming this one "File Save Dialog"; the value
// this lane was handed (`…335EC9E46BDD`) is unregistered and would only surface
// as `REGDB_E_CLASSNOTREG` at runtime.
pub const CLSID_FILE_SAVE_DIALOG: Guid =
    Guid::new(0xc0b4_e2f3, 0xba21, 0x4773, [0x8d, 0xba, 0x33, 0x5e, 0xc9, 0x46, 0xeb, 0x8b]);

/// The one discriminator that matters: `Show` failing with `ERROR_CANCELLED` is
/// a decision by the user, anything else is a failure of ours.  `GetResult`
/// before `Show` answers `E_UNEXPECTED`, *not* `ERROR_CANCELLED` (probe log
/// lines 49-50), so a false "canceled" cannot be produced by reading a dialog
/// that was never shown.
#[must_use]
pub fn is_cancel(hr: HResult) -> bool {
    hr == ERROR_CANCELLED
}

/// `FAILED(hr)` in the SDK's own terms.
#[must_use]
pub fn is_failure(hr: HResult) -> bool {
    hr < 0
}

// ---------------------------------------------------------------------------
// The pure layer: everything a test can reach without a desktop
// ---------------------------------------------------------------------------

/// One entry of a WinForms `Filter` string: `"Markdown 文件 (*.md;*.markdown)"`
/// with patterns `"*.md;*.markdown"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinFilter {
    pub name: String,
    pub patterns: String,
}

/// Parse the `Filter` syntax the PowerShell scripts spell out literally:
/// `Name|Pattern|Name|Pattern`, 1-based groups, `;`-separated patterns inside a
/// group.  This is the shape `IFileDialog::SetFileTypes` wants as a
/// `COMDLG_FILTERSPEC` array, so the strings in `server.rs` can be kept as the
/// single source of truth and the mapping is one function call.
///
/// A trailing or dangling separator yields nothing for that group instead of a
/// filter with an empty pattern list, because an empty `pszSpec` would make the
/// combo box show a selectable entry that matches no file.
pub fn parse_win_filter(text: &str) -> Vec<WinFilter> {
    let mut out = Vec::new();
    let parts: Vec<&str> = text.split('|').collect();
    let mut index = 0usize;
    while index + 1 < parts.len() {
        let name = parts[index].trim();
        let patterns = parts[index + 1].trim();
        if !patterns.is_empty() {
            out.push(WinFilter {
                name: if name.is_empty() {
                    patterns.to_string()
                } else {
                    name.to_string()
                },
                patterns: patterns.to_string(),
            });
        }
        index += 2;
    }
    out
}

/// The inverse of `parse_win_filter`, used by the tests to prove the literals in
/// this module are the same strings `server.rs` interpolates into PowerShell.
pub fn render_win_filter(filters: &[WinFilter]) -> String {
    let mut out = String::new();
    for (position, filter) in filters.iter().enumerate() {
        if position > 0 {
            out.push('|');
        }
        out.push_str(&filter.name);
        out.push('|');
        out.push_str(&filter.patterns);
    }
    out
}

/// The six dialog routes plus `h_export`'s save dialog, which has the same
/// problem and the same fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogShape {
    /// `/api/dialog/choose-folder` — `FolderBrowserDialog`, `ShowNewFolderButton`.
    ChooseFolder,
    /// `/api/dialog/choose-file` — `OpenFileDialog`, Markdown filter.
    ChooseFile,
    /// `/api/dialog/choose-any-file` — `OpenFileDialog`, `*.*` only.
    ChooseAnyFile,
    /// `/api/dialog/choose-many-files` — `OpenFileDialog`, `Multiselect = $true`.
    ChooseManyFiles,
    /// `/api/dialog/save-file` — `SaveFileDialog` with a suggested name.
    SaveFile,
    /// `/api/dialog/save-as` — `SaveFileDialog`, then the caller writes the file.
    SaveAs,
    /// `/api/export` — `SaveFileDialog` with a per-format filter.  Not one of the
    /// six, but it spawns PowerShell for the same reason.
    Export,
}

/// Which shell item(s) a shape reads back after `Show`.  Internal, but it is a
/// `pub`-visible part of the outcome contract: `single` shapes use
/// `IFileDialog::GetResult`, `Many` shapes use `IFileOpenDialog::GetResults`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultMode {
    Single,
    Many,
}

impl DialogShape {
    /// `$dialog.Title = '…'` / `$dialog.Description = '…'`, character for
    /// character, including the full-width parentheses in the titles.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            // FolderBrowserDialog has no Title; its caption text is Description.
            DialogShape::ChooseFolder => "选择文件夹",
            DialogShape::ChooseFile => "打开文件",
            DialogShape::ChooseAnyFile => "选择文件",
            DialogShape::ChooseManyFiles => "选择文件（可多选）",
            DialogShape::SaveFile | DialogShape::SaveAs => "另存为",
            DialogShape::Export => "导出文档",
        }
    }

    /// The PowerShell `Filter` literal for this shape, or `None` for the folder
    /// picker, which has no filter concept at all.
    #[must_use]
    pub fn filter_text(self) -> Option<&'static str> {
        match self {
            DialogShape::ChooseFolder => None,
            DialogShape::ChooseFile => {
                Some("Markdown 文件 (*.md;*.markdown)|*.md;*.markdown|所有文件 (*.*)|*.*")
            }
            DialogShape::ChooseAnyFile => Some("所有文件 (*.*)|*.*"),
            DialogShape::ChooseManyFiles => Some("所有支持的文件 (*.*)|*.*"),
            DialogShape::SaveFile | DialogShape::SaveAs => {
                Some("Markdown 文件 (*.md)|*.md|所有文件 (*.*)|*.*")
            }
            // `h_export` picks its filter from the format; see `export_filter_text`.
            DialogShape::Export => Some("PDF 文档 (*.pdf)|*.pdf|所有文件 (*.*)|*.*"),
        }
    }

    /// `h_export:7286-7293`'s `ext_filter` table, kept here so the export route
    /// can reuse one dialog implementation without re-spelling the strings.
    #[must_use]
    pub fn export_filter_text(format: &str) -> Option<(&'static str, &'static str)> {
        match format {
            "pdf" => Some(("pdf", "PDF 文档 (*.pdf)|*.pdf|所有文件 (*.*)|*.*")),
            "docx" => Some(("docx", "Word 文档 (*.docx)|*.docx|所有文件 (*.*)|*.*")),
            "epub" => Some(("epub", "EPUB 电子书 (*.epub)|*.epub|所有文件 (*.*)|*.*")),
            "html" => Some(("html", "HTML 网页 (*.html)|*.html|所有文件 (*.*)|*.*")),
            "tex" => Some(("tex", "LaTeX 文档 (*.tex)|*.tex|所有文件 (*.*)|*.*")),
            "presentation" => Some(("html", "演示文稿 HTML (*.html)|*.html|所有文件 (*.*)|*.*")),
            _ => None,
        }
    }

    /// `FILEOPENDIALOGOPTIONS` for the shape.
    ///
    /// `SetOptions` *replaces* the mask — measured, not assumed: setting 0xA40
    /// on a dialog whose default was 0x1808 reads back as exactly 0xA40 (probe
    /// log lines 35-37).  So every bit that should be on has to be listed here;
    /// this is the same set WinForms turns on for the equivalent dialog, which
    /// is why `Save*` reads as 0x842 (`OVERWRITEPROMPT|FORCEFILESYSTEM|
    /// PATHMUSTEXIST`, the value probe section F round-tripped).
    #[must_use]
    pub fn options(self) -> u32 {
        match self {
            // No FOS_FILEMUSTEXIST: a folder picker must be able to accept a
            // folder it is about to create, which is what the
            // `ShowNewFolderButton = $true` the PowerShell script set implies.
            DialogShape::ChooseFolder => {
                FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
            }
            DialogShape::ChooseFile | DialogShape::ChooseAnyFile => {
                FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM
            }
            DialogShape::ChooseManyFiles => {
                FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM
                    | FOS_ALLOWMULTISELECT
            }
            DialogShape::SaveFile | DialogShape::SaveAs | DialogShape::Export => {
                FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM
            }
        }
    }

    /// `Multiselect = $true` is the only difference in *reading* the result.
    #[must_use]
    pub fn result_mode(self) -> ResultMode {
        match self {
            DialogShape::ChooseManyFiles => ResultMode::Many,
            _ => ResultMode::Single,
        }
    }

    /// The name the dialog should pre-fill.  `unwrap_or("document.md")` in
    /// `h_dialog_save_file` / `h_dialog_save_as`; `h_export` additionally
    /// appends the format's extension.
    #[must_use]
    pub fn default_name(self, requested: Option<&str>, format: Option<&str>) -> String {
        let base = match requested.map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => text.to_string(),
            None => "document.md".to_string(),
        };
        match (self, format) {
            (DialogShape::Export, Some(ext)) => with_extension(&base, ext),
            _ => base,
        }
    }
}

/// `h_export:7299-7301`: append `.{format}` unless the suggestion already ends
/// with it, comparing the suffix case-insensitively exactly like
/// `def_name.to_lowercase().ends_with(&format!(".{}", ext))`.
#[must_use]
pub fn with_extension(name: &str, extension: &str) -> String {
    let suffix = format!(".{}", extension.to_lowercase());
    if name.to_lowercase().ends_with(&suffix) {
        return name.to_string();
    }
    format!("{}{}", name, suffix)
}

/// Everything `run` needs, built by the pure constructors so the interactive
/// part has no policy of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogRequest {
    pub shape: DialogShape,
    pub title: String,
    pub filters: Vec<WinFilter>,
    pub options: u32,
    pub default_name: Option<String>,
    /// Folder to open the dialog in.  Today's handlers never set one; the field
    /// exists because `h_export` has a `baseDir` and `save-as` has the current
    /// document's parent, and passing `None` preserves the current behaviour.
    pub initial_dir: Option<String>,
    /// An owning top-level window.  `None` means "use the foreground window",
    /// which is what the PowerShell `$form.TopMost = $true` trick approximated.
    pub owner_hwnd: Option<isize>,
}

impl DialogRequest {
    /// Derive the whole request from the shape, matching the PowerShell script
    /// for that shape byte for byte.  Pure and total — no desktop involved.
    #[must_use]
    pub fn for_shape(shape: DialogShape) -> Self {
        DialogRequest {
            shape,
            title: shape.title().to_string(),
            filters: shape.filter_text().map(parse_win_filter).unwrap_or_default(),
            options: shape.options(),
            default_name: None,
            initial_dir: None,
            owner_hwnd: None,
        }
    }

    #[must_use]
    pub fn with_default_name(mut self, name: &str) -> Self {
        self.default_name = Some(name.to_string());
        self
    }

    #[must_use]
    pub fn with_initial_dir(mut self, dir: &str) -> Self {
        self.initial_dir = Some(dir.to_string());
        self
    }

    #[must_use]
    pub fn with_owner(mut self, hwnd: isize) -> Self {
        self.owner_hwnd = Some(hwnd);
        self
    }

    /// `h_export` variant: swap the derived filter for the format's own text.
    #[must_use]
    pub fn for_export(format: &str, suggested_name: &str) -> Self {
        let filters = DialogShape::export_filter_text(format)
            .map(|(_, text)| parse_win_filter(text))
            .unwrap_or_default();
        DialogRequest {
            shape: DialogShape::Export,
            title: DialogShape::Export.title().to_string(),
            filters,
            options: DialogShape::Export.options(),
            default_name: Some(suggested_name.to_string()),
            initial_dir: None,
            owner_hwnd: None,
        }
    }
}

/// What a dialog produced.  Deliberately three states, because the current
/// handlers can only express two of them: see the per-variant notes for the
/// mapping `apply_plan.md` uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogOutcome {
    /// The user confirmed with a non-empty selection.  One element unless the
    /// shape's `result_mode` is `Many`.
    Picked(Vec<String>),
    /// `Show` returned `ERROR_CANCELLED`, i.e. Cancel or the close box.
    Canceled,
    /// No dialog could be shown at all: not Windows, COM refused to start, the
    /// coclass is missing, or the call failed with an HRESULT that is not a
    /// cancel.  Today's `None`/empty-`Vec` return covers this case identically
    /// to `Canceled`, which is why the handlers do not need to branch on it.
    Unavailable,
}

impl DialogOutcome {
    /// `path` for the five single-result routes: the choice, or JSON `null`.
    /// An empty `Picked` degrades to `None` so a dialog that somehow confirmed
    /// nothing still matches what `run_powershell_dialog`'s
    /// `if !res.is_empty()` guard produced.
    #[must_use]
    pub fn single(&self) -> Option<&str> {
        match self {
            DialogOutcome::Picked(paths) => paths.first().map(String::as_str),
            _ => None,
        }
    }

    /// `paths` for `/api/dialog/choose-many-files`: empty on a cancel, which is
    /// what `run_powershell_dialog_lines` returned.
    #[must_use]
    pub fn many(&self) -> Vec<String> {
        match self {
            DialogOutcome::Picked(paths) => paths.clone(),
            _ => Vec::new(),
        }
    }

    /// Whether `h_dialog_save_as` / `h_export` should continue and write.
    #[must_use]
    pub fn is_pick(&self) -> bool {
        matches!(self, DialogOutcome::Picked(paths) if !paths.is_empty())
    }
}

/// Whether this build of this module can show anything.  `server.rs` does not
/// need it (the `Unavailable` path already matches today's behaviour on other
/// platforms) but callers that want to hide a button do.
#[must_use]
pub fn dialog_supported() -> bool {
    cfg!(windows)
}

// ---------------------------------------------------------------------------
// The interactive layer
// ---------------------------------------------------------------------------

/// Show one dialog, block for the user's decision, return the outcome.
///
/// HAZARD (`batch2.rs:1496-1501`, `batch2.rs:2611-2614`): this call blocks the
/// request thread for as long as the dialog is open, and the kernel's
/// `accept_loop` is single-threaded.  That is already true of the
/// `Command::output()` it replaces — with one difference worth remembering when
/// wiring: a PowerShell that never gets a click can be reaped, a modal dialog
/// cannot.  Never call this from a code path that can be reached without a human
/// behind the window; `h_export_epub` and `h_export_presentation` are the two
/// paths the hazard notes name.
///
/// Nothing in this function is safe to run from `cargo test`: it opens a window.
#[must_use]
pub fn run(request: &DialogRequest) -> DialogOutcome {
    #[cfg(windows)]
    {
        win::run(request)
    }
    #[cfg(not(windows))]
    {
        let _ = request;
        DialogOutcome::Unavailable
    }
}

/// Per-shape wrappers, so a `server.rs` handler reads as one line and the
/// shape/table choice lives in exactly one place.
pub fn choose_folder() -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::ChooseFolder))
}

pub fn choose_file() -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::ChooseFile))
}

pub fn choose_any_file() -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::ChooseAnyFile))
}

pub fn choose_many_files() -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::ChooseManyFiles))
}

pub fn save_file(default_name: &str) -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::SaveFile).with_default_name(default_name))
}

pub fn save_as(default_name: &str) -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::SaveAs).with_default_name(default_name))
}

#[cfg(windows)]
mod win {
    use super::{
        DialogOutcome, DialogRequest, DialogShape, ResultMode, WinFilter, CLSID_FILE_OPEN_DIALOG,
        CLSID_FILE_SAVE_DIALOG, COINIT_APARTMENTTHREADED, ERROR_CANCELLED, Guid,
        IID_IFILE_DIALOG, IID_IFILE_OPEN_DIALOG, IID_ISHELL_ITEM, CLSCTX_INPROC_SERVER,
        RPC_E_CHANGED_MODE, S_FALSE, S_OK, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH,
    };
    use std::ffi::c_void;

    pub type PVOID = *mut c_void;

    /// `COMDLG_FILTERSPEC`, `ShObjIdl_core.h`.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ComDlgFilterSpec {
        pub name: *const u16,
        pub spec: *const u16,
    }

    /// `IFileDialog`'s vtable, with `IModalWindow::Show` in third place because
    /// `IFileDialog : public IModalWindow` (`ShObjIdl_core.h:19566`), and
    /// `IFileOpenDialog::GetResults` appended as the 28th entry: the open dialog
    /// is that same prefix plus two methods, and the probe confirmed inherited
    /// slot 10 answers identically through a QI'd `IFileOpenDialog` pointer.
    ///
    /// `Advise`/`Unadvise`/`AddPlace`/`Close`/`SetClientGuid`/`ClearClientData`/
    /// `SetFilter` are declared so the slots after them do not shift — an
    /// omitted method here is an off-by-one there, and an off-by-one at index 3
    /// opens a modal dialog from code that was only meant to read options.
    #[repr(C)]
    pub struct IFileDialogVtbl {
        pub query_interface:
            unsafe extern "system" fn(this: PVOID, riid: *const Guid, ppv: *mut PVOID) -> i32,
        pub add_ref: unsafe extern "system" fn(this: PVOID) -> u32,
        pub release: unsafe extern "system" fn(this: PVOID) -> u32,
        /// `IModalWindow::Show(HWND hwndOwner)`.  NEVER call from a probe.
        pub show: unsafe extern "system" fn(this: PVOID, owner: PVOID) -> i32,
        pub set_file_types: unsafe extern "system" fn(
            this: PVOID,
            count: u32,
            specs: *const ComDlgFilterSpec,
        ) -> i32,
        pub set_file_type_index: unsafe extern "system" fn(this: PVOID, index: u32) -> i32,
        pub get_file_type_index: unsafe extern "system" fn(this: PVOID, out: *mut u32) -> i32,
        pub advise:
            unsafe extern "system" fn(this: PVOID, events: PVOID, cookie: *mut u32) -> i32,
        pub unadvise: unsafe extern "system" fn(this: PVOID, cookie: u32) -> i32,
        pub set_options: unsafe extern "system" fn(this: PVOID, options: u32) -> i32,
        pub get_options: unsafe extern "system" fn(this: PVOID, out: *mut u32) -> i32,
        pub set_default_folder: unsafe extern "system" fn(this: PVOID, item: PVOID) -> i32,
        pub set_folder: unsafe extern "system" fn(this: PVOID, item: PVOID) -> i32,
        pub get_folder: unsafe extern "system" fn(this: PVOID, out: *mut PVOID) -> i32,
        pub get_current_selection: unsafe extern "system" fn(this: PVOID, out: *mut PVOID) -> i32,
        pub set_file_name: unsafe extern "system" fn(this: PVOID, name: *const u16) -> i32,
        pub get_file_name: unsafe extern "system" fn(this: PVOID, out: *mut *mut u16) -> i32,
        pub set_title: unsafe extern "system" fn(this: PVOID, title: *const u16) -> i32,
        pub set_ok_button_label: unsafe extern "system" fn(this: PVOID, label: *const u16) -> i32,
        pub set_file_name_label: unsafe extern "system" fn(this: PVOID, label: *const u16) -> i32,
        pub get_result: unsafe extern "system" fn(this: PVOID, out: *mut PVOID) -> i32,
        pub add_place:
            unsafe extern "system" fn(this: PVOID, item: PVOID, label: *const u16) -> i32,
        pub set_default_extension: unsafe extern "system" fn(this: PVOID, ext: *const u16) -> i32,
        pub close: unsafe extern "system" fn(this: PVOID, reason: u32) -> i32,
        pub set_client_guid: unsafe extern "system" fn(this: PVOID, guid: *const Guid) -> i32,
        pub clear_client_data: unsafe extern "system" fn(this: PVOID) -> i32,
        pub set_filter: unsafe extern "system" fn(this: PVOID, filter: PVOID) -> i32,
        /// Slot 27: `IFileOpenDialog::GetResults`, valid only through an
        /// `IID_IFileOpenDialog` pointer.
        pub get_results: unsafe extern "system" fn(this: PVOID, out: *mut PVOID) -> i32,
    }

    /// `IShellItem`, `ShObjIdl_core.h:8747`: three `IUnknown` slots, then
    /// `BindToHandler`, `GetParent`, `GetDisplayName`, `GetAttributes`, `Compare`.
    #[repr(C)]
    pub struct IShellItemVtbl {
        pub query_interface:
            unsafe extern "system" fn(this: PVOID, riid: *const Guid, ppv: *mut PVOID) -> i32,
        pub add_ref: unsafe extern "system" fn(this: PVOID) -> u32,
        pub release: unsafe extern "system" fn(this: PVOID) -> u32,
        pub bind_to_handler: unsafe extern "system" fn(
            this: PVOID,
            bind_ctx: PVOID,
            handler: *const Guid,
            riid: *const Guid,
            out: *mut PVOID,
        ) -> i32,
        pub get_parent: unsafe extern "system" fn(this: PVOID, out: *mut PVOID) -> i32,
        pub get_display_name:
            unsafe extern "system" fn(this: PVOID, sigdn: u32, out: *mut *mut u16) -> i32,
        pub get_attributes:
            unsafe extern "system" fn(this: PVOID, flags: u32, mask: u32, out: *mut u32) -> i32,
        pub compare:
            unsafe extern "system" fn(this: PVOID, other: PVOID, hint: u32, out: *mut i32) -> i32,
    }

    /// `IShellItemArray`, `ShObjIdl_core.h:10889`.  The order here is load
    /// bearing and counter-intuitive: `GetCount` is slot 7 and `GetItemAt` slot 8,
    /// because the first four methods are `BindToHandler`/`GetPropertyStore`/
    /// `GetPropertyDescriptionList`/`GetAttributes`.
    #[repr(C)]
    pub struct IShellItemArrayVtbl {
        pub query_interface:
            unsafe extern "system" fn(this: PVOID, riid: *const Guid, ppv: *mut PVOID) -> i32,
        pub add_ref: unsafe extern "system" fn(this: PVOID) -> u32,
        pub release: unsafe extern "system" fn(this: PVOID) -> u32,
        pub bind_to_handler: unsafe extern "system" fn(
            this: PVOID,
            bind_ctx: PVOID,
            handler: *const Guid,
            riid: *const Guid,
            out: *mut PVOID,
        ) -> i32,
        pub get_property_store:
            unsafe extern "system" fn(this: PVOID, flags: u32, riid: *const Guid, out: *mut PVOID) -> i32,
        pub get_property_description_list:
            unsafe extern "system" fn(this: PVOID, key: *const c_void, riid: *const Guid, out: *mut PVOID) -> i32,
        pub get_attributes:
            unsafe extern "system" fn(this: PVOID, flags: u32, mask: u32, out: *mut u32) -> i32,
        pub get_count: unsafe extern "system" fn(this: PVOID, out: *mut u32) -> i32,
        /// `(DWORD index, IShellItem **out)` — no REFIID.  With one, the call
        /// still returns S_OK and leaves the pointer NULL.
        pub get_item_at: unsafe extern "system" fn(this: PVOID, index: u32, out: *mut PVOID) -> i32,
        pub enum_items: unsafe extern "system" fn(this: PVOID, out: *mut PVOID) -> i32,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, model: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(
            class: *const Guid,
            outer: PVOID,
            context: u32,
            riid: *const Guid,
            out: *mut PVOID,
        ) -> i32;
        fn CoTaskMemFree(block: *mut c_void);
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> PVOID;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemFromParsingName(
            path: *const u16,
            bind_ctx: PVOID,
            riid: *const Guid,
            out: *mut PVOID,
        ) -> i32;
    }

    /// Same `to_wide` shape as `crypto.rs` and `native_system.rs`.
    pub fn to_wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Read a callee-allocated `LPWSTR` and release it with `CoTaskMemFree`.
    ///
    /// The free is unconditional once the pointer is non-null: `GetDisplayName`
    /// documents the buffer as task-memory, and skipping it leaks on every pick,
    /// while freeing anything else faults in the heap (the probe found that out
    /// by dying mid-log, which is why every evidence file here ends in `rc=`).
    unsafe fn take_wide(block: *mut u16) -> Option<String> {
        if block.is_null() {
            return None;
        }
        let mut length = 0usize;
        while *block.add(length) != 0 {
            length += 1;
            if length > 32_768 {
                // A pathological buffer must not hang the request thread.
                break;
            }
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(block, length));
        CoTaskMemFree(block as *mut c_void);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// `IUnknown`, the only vtable shape every COM object is guaranteed to have.
    /// Used to reach `Release` for an object whose interface we no longer care
    /// about, which is why it is a three-field struct and not a `transmute`.
    #[repr(C)]
    struct IUnknownVtbl {
        query_interface:
            unsafe extern "system" fn(this: PVOID, riid: *const Guid, ppv: *mut PVOID) -> i32,
        add_ref: unsafe extern "system" fn(this: PVOID) -> u32,
        release: unsafe extern "system" fn(this: PVOID) -> u32,
    }

    /// An COM object pointer that releases itself.  Raw pointer inside, so this
    /// type is neither `Send` nor `Sync` — which is the apartment rule, checked
    /// by the compiler rather than by comment.
    pub struct Obj {
        raw: PVOID,
    }

    impl Obj {
        /// Takes one reference count, which for the pointers produced below is
        /// the count the factory call already gave us.
        #[must_use]
        pub fn new(raw: PVOID) -> Obj {
            Obj { raw }
        }

        #[must_use]
        pub fn get(&self) -> PVOID {
            self.raw
        }
    }

    impl Drop for Obj {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                // SAFETY: `Release` is slot 2 of every vtable in existence, and
                // the pointer came from a successful CoCreateInstance,
                // QueryInterface or shell factory call on this same thread that
                // we have not released anywhere else — each `Obj` is the sole
                // owner of exactly one reference.
                unsafe {
                    let vtable = &**(self.raw as *mut *const IUnknownVtbl);
                    (vtable.release)(self.raw);
                }
            }
        }
    }

    /// `CoInitializeEx` for the duration of one dialog.  `Drop` pairs the
    /// `CoUninitialize` on every path, including an early `return`.
    pub struct ComGuard {
        owns: bool,
    }

    impl ComGuard {
        /// `None` means COM refused to start and no dialog can be shown.
        #[must_use]
        pub fn init() -> Option<ComGuard> {
            // SAFETY: `CoInitializeEx(NULL, …)` touches only this thread's
            // apartment state.
            let hr = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
            if hr == S_OK || hr == S_FALSE {
                // S_OK: we brought the apartment up.  S_FALSE: it was already an
                // STA, and we took a count on it — both need a CoUninitialize to
                // stay balanced.
                return Some(ComGuard { owns: true });
            }
            if hr == RPC_E_CHANGED_MODE {
                // Another apartment model is already installed on this thread.
                // The probe confirmed the dialog is still creatable (log line 76)
                // and that the apartment is not ours to tear down.
                return Some(ComGuard { owns: false });
            }
            None
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.owns {
                // SAFETY: paired with the CoInitializeEx in `init`, on this
                // thread, after every COM object this call created is released.
                unsafe { CoUninitialize() }
            }
        }
    }

    #[inline]
    unsafe fn dialog_vtable(this: PVOID) -> &'static IFileDialogVtbl {
        &**(this as *mut *const IFileDialogVtbl)
    }

    #[inline]
    unsafe fn item_vtable(this: PVOID) -> &'static IShellItemVtbl {
        &**(this as *mut *const IShellItemVtbl)
    }

    #[inline]
    unsafe fn array_vtable(this: PVOID) -> &'static IShellItemArrayVtbl {
        &**(this as *mut *const IShellItemArrayVtbl)
    }

    /// `GetDisplayName` with the same fallback chain WinForms ends up with:
    /// the filesystem path if there is one, otherwise the desktop parsing name.
    unsafe fn display_path(item: PVOID) -> Option<String> {
        for sigdn in [SIGDN_FILESYSPATH, SIGDN_DESKTOPABSOLUTEPARSING] {
            let mut block: *mut u16 = std::ptr::null_mut();
            if (item_vtable(item).get_display_name)(item, sigdn, &mut block) == S_OK {
                if let Some(text) = take_wide(block) {
                    return Some(text);
                }
            } else if !block.is_null() {
                take_wide(block);
            }
        }
        None
    }

    /// Create, configure and show one dialog.  The whole COM lifetime lives in
    /// this function; nothing escapes it.
    pub fn run(request: &DialogRequest) -> DialogOutcome {
        // The guard is declared first so it drops last: every `Obj` below is
        // released before `CoUninitialize` closes the apartment they live in.
        let _com = match ComGuard::init() {
            Some(guard) => guard,
            None => return DialogOutcome::Unavailable,
        };

        let class = match request.shape {
            DialogShape::ChooseFolder
            | DialogShape::ChooseFile
            | DialogShape::ChooseAnyFile
            | DialogShape::ChooseManyFiles => &CLSID_FILE_OPEN_DIALOG,
            DialogShape::SaveFile | DialogShape::SaveAs | DialogShape::Export => {
                &CLSID_FILE_SAVE_DIALOG
            }
        };

        let mut raw: PVOID = std::ptr::null_mut();
        // SAFETY: `class` and `&IID_IFILE_DIALOG` are 'static consts; `&mut raw`
        // outlives the call; on any failure the out-param stays NULL and
        // `Obj::new(NULL)` drops as a no-op.
        let hr = unsafe {
            CoCreateInstance(
                class as *const Guid,
                std::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IFILE_DIALOG as *const Guid,
                &mut raw,
            )
        };
        if hr != S_OK || raw.is_null() {
            return DialogOutcome::Unavailable;
        }
        let dialog = Obj::new(raw);

        let owner = match request.owner_hwnd {
            Some(hwnd) if hwnd != 0 => hwnd as PVOID,
            // SAFETY: GetForegroundWindow takes no arguments and returns either
            // NULL or a handle valid while the dialog is modal; the PowerShell
            // scripts' invisible topmost `Form` was a stand-in for exactly this
            // "land in front of what the user is doing" behaviour.
            _ => unsafe { GetForegroundWindow() },
        };

        // SAFETY: `dialog` is a live COM object this thread created inside the
        // apartment `ComGuard` opened, and it outlives the call.
        unsafe { configure(&dialog, request) };

        // SAFETY: `dialog` owns the only live reference, `Show` is slot 3 (the
        // probe identified it and never called it), and `owner` is either NULL or
        // a window handle.
        let hr = unsafe { (dialog_vtable(dialog.get()).show)(dialog.get(), owner) };
        if hr == ERROR_CANCELLED {
            return DialogOutcome::Canceled;
        }
        if hr != S_OK {
            return DialogOutcome::Unavailable;
        }
        // SAFETY: `Show` returned S_OK, so the dialog holds a selection to read,
        // and `dialog` is still the sole live owner on this thread.
        unsafe { read_result(&dialog, request) }
    }

    /// Everything before `Show`.
    ///
    /// Only `SetOptions` is treated as fatal, and it is fatal for a specific
    /// reason: it carries `FOS_PICKFOLDERS`, so a dialog that refused the mask
    /// would answer a *file* where the route asked for a *folder*.  A title,
    /// filter or suggested name the shell refuses is cosmetic — better to show a
    /// slightly plainer dialog than to answer `Unavailable`, which today's
    /// handlers cannot distinguish from a cancel.
    ///
    /// Order matters only in that `SetFileTypes` must precede
    /// `SetFileTypeIndex` for the latter to select an entry that exists.
    unsafe fn configure(dialog: &Obj, request: &DialogRequest) {
        let vt = dialog_vtable(dialog.get());

        if (vt.set_options)(dialog.get(), request.options) != S_OK {
            return;
        }

        // The wide strings have to outlive SetFileTypes, which stores pointers
        // rather than copies, so they are all bound here for the whole call.
        let names: Vec<Vec<u16>> =
            request.filters.iter().map(|filter| to_wide(&filter.name)).collect();
        let specs: Vec<Vec<u16>> = request
            .filters
            .iter()
            .map(|filter| to_wide(&filter.patterns))
            .collect();
        let entries: Vec<ComDlgFilterSpec> = names
            .iter()
            .zip(specs.iter())
            .map(|(name, spec)| ComDlgFilterSpec {
                name: name.as_ptr(),
                spec: spec.as_ptr(),
            })
            .collect();
        if !entries.is_empty() {
            (vt.set_file_types)(dialog.get(), entries.len() as u32, entries.as_ptr());
        }

        let title = to_wide(&request.title);
        (vt.set_title)(dialog.get(), title.as_ptr());

        // WinForms' `$dialog.FileName = '…'`, and only for the save shapes: on an
        // open dialog it would pre-select a file the user never asked for, which
        // is also why `DialogRequest::default_name` is left None by every choose
        // route.
        if matches!(
            request.shape,
            DialogShape::SaveFile | DialogShape::SaveAs | DialogShape::Export
        ) {
            if let Some(name) = request.default_name.as_deref() {
                let wide = to_wide(name);
                (vt.set_file_name)(dialog.get(), wide.as_ptr());
            }
        }

        if let Some(dir) = request.initial_dir.as_deref() {
            let wide = to_wide(dir);
            let mut item: PVOID = std::ptr::null_mut();
            if SHCreateItemFromParsingName(
                wide.as_ptr(),
                std::ptr::null_mut(),
                &IID_ISHELL_ITEM as *const Guid,
                &mut item,
            ) == S_OK
                && !item.is_null()
            {
                let folder = Obj::new(item);
                // SetDefaultFolder, not SetFolder: the user may still navigate
                // away, which is what a "start here" hint should do.
                (vt.set_default_folder)(dialog.get(), folder.get());
            }
        }
    }

    unsafe fn read_result(dialog: &Obj, request: &DialogRequest) -> DialogOutcome {
        if request.shape.result_mode() == ResultMode::Many {
            // GetResults lives on IFileOpenDialog, so ask for that interface by
            // its IID rather than assuming the pointer already is one.
            let mut open: PVOID = std::ptr::null_mut();
            let hr = (dialog_vtable(dialog.get()).query_interface)(
                dialog.get(),
                &IID_IFILE_OPEN_DIALOG as *const Guid,
                &mut open,
            );
            if hr == S_OK && !open.is_null() {
                let open = Obj::new(open);
                let mut array: PVOID = std::ptr::null_mut();
                if (dialog_vtable(open.get()).get_results)(open.get(), &mut array) == S_OK
                    && !array.is_null()
                {
                    let array = Obj::new(array);
                    let mut count: u32 = 0;
                    if (array_vtable(array.get()).get_count)(array.get(), &mut count) != S_OK {
                        return DialogOutcome::Unavailable;
                    }
                    let mut paths = Vec::with_capacity(count as usize);
                    for index in 0..count {
                        let mut item: PVOID = std::ptr::null_mut();
                        if (array_vtable(array.get()).get_item_at)(
                            array.get(),
                            index,
                            &mut item,
                        ) != S_OK
                        {
                            continue;
                        }
                        if item.is_null() {
                            continue;
                        }
                        let item = Obj::new(item);
                        if let Some(text) = display_path(item.get()) {
                            paths.push(text);
                        }
                    }
                    return if paths.is_empty() {
                        DialogOutcome::Unavailable
                    } else {
                        DialogOutcome::Picked(paths)
                    };
                }
            }
            // Multi-select is the whole point of the route; falling back to
            // GetResult would silently answer with one file out of many.
            return DialogOutcome::Unavailable;
        }

        let mut item: PVOID = std::ptr::null_mut();
        if (dialog_vtable(dialog.get()).get_result)(dialog.get(), &mut item) != S_OK
            || item.is_null()
        {
            return DialogOutcome::Unavailable;
        }
        let item = Obj::new(item);
        match display_path(item.get()) {
            Some(text) => DialogOutcome::Picked(vec![text]),
            None => DialogOutcome::Unavailable,
        }
    }

    /// The filter list this module would hand the OS, exposed for the tests that
    /// assert the mapping without a desktop.
    #[must_use]
    pub fn filter_debug(filters: &[WinFilter]) -> String {
        filters
            .iter()
            .map(|filter| format!("{}={}", filter.name, filter.patterns))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ---------------------------------------------------------------------------
// MANUAL SMOKE — a human runs this, `cargo test` never does
// ---------------------------------------------------------------------------

/// Open one real file dialog and return what was chosen.
///
/// `#[cfg(test)]`-free on purpose, and unreachable from the test binary because
/// nothing in `#[cfg(test)] mod tests` calls it.  To use it, temporarily call it
/// from a debug build of the app (or a scratch `fn main`) — it needs an
/// interactive desktop and it will block until a person clicks something.
///
/// ```text
/// // temporary, in a debug build only:
/// let chosen = readmd_kernel::win_dialogs::manual_smoke_open_file();
/// eprintln!("manual smoke: {chosen:?}");
/// ```
#[must_use]
pub fn manual_smoke_open_file() -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::ChooseFile))
}

/// The same for the folder shape, which is the one that needed a design
/// decision (`FOS_PICKFOLDERS` instead of a non-existent `FileBrowserDialog`).
#[must_use]
pub fn manual_smoke_choose_folder() -> DialogOutcome {
    run(&DialogRequest::for_shape(DialogShape::ChooseFolder))
}

// ---------------------------------------------------------------------------
// Tests: pure, offline, non-interactive, deterministic, no spawns, no clock
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The literal from `h_dialog_choose_file`'s PowerShell `Filter` assignment.
    const CHOOSE_FILE_FILTER: &str =
        "Markdown 文件 (*.md;*.markdown)|*.md;*.markdown|所有文件 (*.*)|*.*";
    /// The literal from `h_dialog_save_file` / `h_dialog_save_as`.
    const SAVE_FILTER: &str = "Markdown 文件 (*.md)|*.md|所有文件 (*.*)|*.*";

    #[test]
    fn filter_parser_round_trips_every_script_literal() {
        for text in [
            CHOOSE_FILE_FILTER,
            SAVE_FILTER,
            "所有文件 (*.*)|*.*",
            "所有支持的文件 (*.*)|*.*",
            "PDF 文档 (*.pdf)|*.pdf|所有文件 (*.*)|*.*",
        ] {
            let parsed = parse_win_filter(text);
            assert!(!parsed.is_empty(), "{text} parsed to nothing");
            assert_eq!(render_win_filter(&parsed), text);
        }
    }

    #[test]
    fn filter_parser_builds_the_com_spec_entries() {
        let parsed = parse_win_filter(CHOOSE_FILE_FILTER);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Markdown 文件 (*.md;*.markdown)");
        assert_eq!(parsed[0].patterns, "*.md;*.markdown");
        assert_eq!(parsed[1].name, "所有文件 (*.*)");
        assert_eq!(parsed[1].patterns, "*.*");
    }

    #[test]
    fn filter_parser_ignores_dangling_separators() {
        assert!(parse_win_filter("").is_empty());
        assert!(parse_win_filter("名字").is_empty());
        // A patternless group would show a selectable filter that matches
        // nothing, so it is dropped rather than guessed at.
        assert!(parse_win_filter("名字|").is_empty());
        assert_eq!(parse_win_filter("a|*.*|b|").len(), 1);
        // An unnamed group still gets a visible label.
        let only = parse_win_filter("|*.md");
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].name, "*.md");
    }

    #[test]
    fn every_shape_reproduces_todays_strings() {
        assert_eq!(DialogShape::ChooseFolder.title(), "选择文件夹");
        assert_eq!(DialogShape::ChooseFile.title(), "打开文件");
        assert_eq!(DialogShape::ChooseAnyFile.title(), "选择文件");
        assert_eq!(DialogShape::ChooseManyFiles.title(), "选择文件（可多选）");
        assert_eq!(DialogShape::SaveFile.title(), "另存为");
        assert_eq!(DialogShape::SaveAs.title(), "另存为");
        assert_eq!(DialogShape::Export.title(), "导出文档");

        assert_eq!(DialogShape::ChooseFolder.filter_text(), None);
        assert_eq!(DialogShape::ChooseFile.filter_text(), Some(CHOOSE_FILE_FILTER));
        assert_eq!(DialogShape::ChooseAnyFile.filter_text(), Some("所有文件 (*.*)|*.*"));
        assert_eq!(
            DialogShape::ChooseManyFiles.filter_text(),
            Some("所有支持的文件 (*.*)|*.*")
        );
        assert_eq!(DialogShape::SaveFile.filter_text(), Some(SAVE_FILTER));
        assert_eq!(DialogShape::SaveAs.filter_text(), Some(SAVE_FILTER));
    }

    #[test]
    fn export_filters_cover_the_five_accepted_formats() {
        for (format, extension) in
            [("pdf", "pdf"), ("docx", "docx"), ("epub", "epub"), ("html", "html"), ("tex", "tex")]
        {
            let (ext, text) = DialogShape::export_filter_text(format)
                .unwrap_or_else(|| panic!("{format} has no filter"));
            assert_eq!(ext, extension);
            let parsed = parse_win_filter(text);
            assert_eq!(parsed[0].patterns, format!("*.{}", extension));
            assert_eq!(parsed.last().unwrap().patterns, "*.*");
        }
        assert!(DialogShape::export_filter_text("rtf").is_none());
    }

    #[test]
    fn options_match_the_measured_masks() {
        // Probe section F: the save dialog accepted and reported exactly 0x842.
        assert_eq!(DialogShape::SaveAs.options(), 0x842);
        assert_eq!(DialogShape::SaveFile.options(), 0x842);
        assert_eq!(DialogShape::Export.options(), 0x842);
        // Probe section G: the folder shape is the open coclass + PICKFOLDERS.
        let folder = DialogShape::ChooseFolder.options();
        assert_eq!(folder, FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
        assert_eq!(folder, 0x860);
        // Only the multi route may select more than one thing.
        assert_eq!(
            DialogShape::ChooseManyFiles.options(),
            DialogShape::ChooseAnyFile.options() | FOS_ALLOWMULTISELECT
        );
        assert_eq!(DialogShape::ChooseAnyFile.options() & FOS_ALLOWMULTISELECT, 0);
        // A folder picker that demanded an existing *file* could never confirm.
        assert_eq!(folder & FOS_FILEMUSTEXIST, 0);
    }

    #[test]
    fn result_mode_is_many_only_for_the_multi_route() {
        assert_eq!(DialogShape::ChooseManyFiles.result_mode(), ResultMode::Many);
        for shape in [
            DialogShape::ChooseFolder,
            DialogShape::ChooseFile,
            DialogShape::ChooseAnyFile,
            DialogShape::SaveFile,
            DialogShape::SaveAs,
            DialogShape::Export,
        ] {
            assert_eq!(shape.result_mode(), ResultMode::Single, "{shape:?}");
        }
    }

    #[test]
    fn request_builder_is_the_scripts_say_so() {
        let request = DialogRequest::for_shape(DialogShape::ChooseManyFiles);
        assert_eq!(request.title, "选择文件（可多选）");
        assert_eq!(
            render_win_filter(&request.filters),
            "所有支持的文件 (*.*)|*.*"
        );
        assert_eq!(request.options, DialogShape::ChooseManyFiles.options());
        assert_eq!(request.default_name, None);
        assert_eq!(request.owner_hwnd, None);

        let saved = DialogRequest::for_shape(DialogShape::SaveFile).with_default_name("note.md");
        assert_eq!(saved.default_name.as_deref(), Some("note.md"));
        assert_eq!(saved.shape, DialogShape::SaveFile);
    }

    #[test]
    fn apostrophes_survive_verbatim_because_there_is_no_quoting_context() {
        // The PowerShell path had to double `'` to survive its own single-quoted
        // literal.  Passing the name straight to SetFileName must not repeat it.
        let name = "Bob's 笔记's.md";
        let request = DialogRequest::for_shape(DialogShape::SaveAs).with_default_name(name);
        assert_eq!(request.default_name.as_deref(), Some(name));
        assert!(!name.contains("''"));
    }

    #[test]
    fn default_names_follow_the_handlers_defaults() {
        assert_eq!(DialogShape::SaveAs.default_name(None, None), "document.md");
        assert_eq!(
            DialogShape::SaveFile.default_name(Some("  "), None),
            "document.md"
        );
        assert_eq!(
            DialogShape::SaveAs.default_name(Some("readme.md"), None),
            "readme.md"
        );
        // h_export: only the lowercase-suffix test, never a doubled extension.
        assert_eq!(
            DialogShape::Export.default_name(Some("deck"), Some("pdf")),
            "deck.pdf"
        );
        assert_eq!(
            DialogShape::Export.default_name(Some("deck.PDF"), Some("pdf")),
            "deck.PDF"
        );
        assert_eq!(
            DialogShape::Export.default_name(Some("a.b.epub"), Some("epub")),
            "a.b.epub"
        );
        assert_eq!(with_extension("x", "tex"), "x.tex");
    }

    #[test]
    fn export_request_keeps_its_own_filter_table() {
        let request = DialogRequest::for_export("epub", "book.epub");
        assert_eq!(request.shape, DialogShape::Export);
        assert_eq!(request.title, "导出文档");
        assert_eq!(request.filters[0].patterns, "*.epub");
        assert_eq!(request.default_name.as_deref(), Some("book.epub"));
    }

    #[test]
    fn cancel_and_failure_read_as_no_path() {
        assert_eq!(DialogOutcome::Canceled.single(), None);
        assert_eq!(DialogOutcome::Unavailable.single(), None);
        assert!(DialogOutcome::Canceled.many().is_empty());
        assert!(!DialogOutcome::Canceled.is_pick());
        assert!(!DialogOutcome::Unavailable.is_pick());
        // Confirmed but empty is the one state the PowerShell runner turned into
        // `None` via `if !res.is_empty()`; it must keep reading as no path.
        assert_eq!(DialogOutcome::Picked(Vec::new()).single(), None);
        assert!(!DialogOutcome::Picked(Vec::new()).is_pick());
    }

    #[test]
    fn picked_results_map_onto_the_two_envelope_shapes() {
        let one = DialogOutcome::Picked(vec!["C:\\a\\b.md".to_string()]);
        assert_eq!(one.single(), Some("C:\\a\\b.md"));
        assert!(one.is_pick());

        let many = DialogOutcome::Picked(vec![
            "C:\\a.md".to_string(),
            "D:\\中文 文件.md".to_string(),
        ]);
        assert_eq!(many.many().len(), 2);
        // `single()` is first-wins, which is also what a mis-wired multi route
        // would have produced through the old `Write-Output` loop.
        assert_eq!(many.single(), Some("C:\\a.md"));
    }

    #[test]
    fn only_one_hresult_means_the_user_canceled() {
        assert!(is_cancel(ERROR_CANCELLED));
        assert!(is_failure(ERROR_CANCELLED));
        // The whole point of the distinction: reading a dialog that was never
        // shown must not be reported as a cancel.
        assert!(!is_cancel(E_UNEXPECTED));
        assert!(!is_cancel(E_INVALIDARG));
        assert!(!is_cancel(REGDB_E_CLASSNOTREG));
        assert!(!is_failure(S_OK));
        // Win32 1223, i.e. `MAKE_HRESULT(SEVERITY_ERROR, FACILITY_WIN32, 1223)`.
        assert_eq!(ERROR_CANCELLED as u32, 0x8007_04C7);
    }

    #[test]
    fn guid_literals_keep_the_mixed_endian_split() {
        // Data1/2/3 are native-endian nibble groups, Data4 is the trailing text
        // verbatim.  A single wrong byte is invisible to rustc, so the parts of
        // the canonical form are asserted here as a cheap second witness to the
        // live-OS layout proof in probe_guids.py.
        let text = |guid: &Guid| -> String {
            let tail = guid
                .data4
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<String>();
            // Canonical text form is 8-4-4-4-12: Data4 spans the last two
            // groups, so it renders as 4 hex, a dash, then 12 hex.
            format!(
                "{:08X}-{:04X}-{:04X}-{}-{}",
                guid.data1,
                guid.data2,
                guid.data3,
                &tail[..4],
                &tail[4..]
            )
        };
        assert_eq!(text(&CLSID_FILE_OPEN_DIALOG), "DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7");
        assert_eq!(text(&CLSID_FILE_SAVE_DIALOG), "C0B4E2F3-BA21-4773-8DBA-335EC946EB8B");
        assert_eq!(text(&IID_IFILE_DIALOG).to_lowercase(), "42f85136-db7e-439c-85f1-e4075d135fc8");
        assert_eq!(text(&IID_ISHELL_ITEM).to_lowercase(), "43826d1e-e718-42ee-bc55-a1e261c37bfe");
        assert_eq!(
            text(&IID_ISHELL_ITEM_ARRAY).to_lowercase(),
            "b63ea76d-1f85-456f-a19c-48159efa858b"
        );
        assert_eq!(
            text(&IID_IFILE_OPEN_DIALOG).to_lowercase(),
            "d57c7288-d4ad-4768-be02-9d969532d960"
        );
        assert_eq!(
            text(&IID_IMODAL_WINDOW).to_lowercase(),
            "b4db1657-70d7-485e-8e3e-6fcb5a5c1802"
        );
    }

    #[test]
    fn the_open_coclass_is_the_one_folder_shaper() {
        // The brief's CLSID_FileBrowserDialog does not exist on this OS, so a
        // folder pick must be asked of the *open* dialog or the class check here
        // would silently start pointing at the save dialog.
        for shape in [
            DialogShape::ChooseFolder,
            DialogShape::ChooseFile,
            DialogShape::ChooseAnyFile,
            DialogShape::ChooseManyFiles,
        ] {
            assert_eq!(shape.options() & FOS_PICKFOLDERS != 0, shape == DialogShape::ChooseFolder);
        }
        for shape in [
            DialogShape::SaveFile,
            DialogShape::SaveAs,
            DialogShape::Export,
        ] {
            assert_eq!(shape.options() & FOS_PICKFOLDERS, 0);
        }
    }

    #[test]
    fn wide_strings_are_nul_terminated_utf16() {
        // Same helper the interactive layer uses; a missing terminator makes the
        // OS read past the buffer.
        #[cfg(windows)]
        {
            let made = win::to_wide("中文a");
            assert_eq!(made.last(), Some(&0u16));
            assert_eq!(made.len(), "中文a".encode_utf16().count() + 1);
            assert_eq!(win::to_wide(""), vec![0u16]);
            // A non-BMP character must stay one surrogate pair, not be split.
            assert_eq!(win::to_wide("📄").len(), 3);
        }
    }

    #[cfg(windows)]
    #[test]
    fn filter_spec_array_layout_is_two_pointers() {
        // `COMDLG_FILTERSPEC` is `{LPCWSTR;LPCWSTR}` and `SetFileTypes` takes a
        // count plus that array; a different stride would make the OS walk off
        // the end of the `entries` vector in `configure`.
        assert_eq!(std::mem::size_of::<win::ComDlgFilterSpec>(), 16);
        assert_eq!(std::mem::align_of::<win::ComDlgFilterSpec>(), 8);
    }

    #[cfg(windows)]
    #[test]
    fn vtable_slots_are_where_the_header_says() {
        // Every number below is a slot the live probe identified by calling it
        // (`probe_guids.log` sections D/E/F/G2), restated here so a field added,
        // dropped or reordered in a `#[repr(C)]` vtable fails offline instead of
        // opening a modal dialog from code that only meant to read options.
        use std::mem::offset_of;
        type Dlg = win::IFileDialogVtbl;
        type Item = win::IShellItemVtbl;
        type Array = win::IShellItemArrayVtbl;
        let slot = |byte: usize| byte / std::mem::size_of::<usize>();
        // IUnknown
        assert_eq!(slot(offset_of!(Dlg, query_interface)), 0);
        assert_eq!(slot(offset_of!(Dlg, add_ref)), 1);
        assert_eq!(slot(offset_of!(Dlg, release)), 2);
        // IModalWindow::Show -- the one that must never be reached by accident.
        assert_eq!(slot(offset_of!(Dlg, show)), 3);
        assert_eq!(slot(offset_of!(Dlg, set_file_types)), 4);
        assert_eq!(slot(offset_of!(Dlg, set_file_type_index)), 5);
        assert_eq!(slot(offset_of!(Dlg, get_file_type_index)), 6);
        assert_eq!(slot(offset_of!(Dlg, set_options)), 9);
        assert_eq!(slot(offset_of!(Dlg, get_options)), 10);
        assert_eq!(slot(offset_of!(Dlg, set_default_folder)), 11);
        assert_eq!(slot(offset_of!(Dlg, get_folder)), 13);
        assert_eq!(slot(offset_of!(Dlg, set_file_name)), 15);
        assert_eq!(slot(offset_of!(Dlg, get_file_name)), 16);
        assert_eq!(slot(offset_of!(Dlg, set_title)), 17);
        assert_eq!(slot(offset_of!(Dlg, get_result)), 20);
        assert_eq!(slot(offset_of!(Dlg, set_default_extension)), 22);
        assert_eq!(slot(offset_of!(Dlg, set_client_guid)), 24);
        assert_eq!(slot(offset_of!(Dlg, clear_client_data)), 25);
        // IFileOpenDialog::GetResults, the one entry past the shared prefix.
        assert_eq!(slot(offset_of!(Dlg, get_results)), 27);
        assert_eq!(std::mem::size_of::<Dlg>() / std::mem::size_of::<usize>(), 28);
        // IShellItem / IShellItemArray
        assert_eq!(slot(offset_of!(Item, get_display_name)), 5);
        assert_eq!(std::mem::size_of::<Item>() / std::mem::size_of::<usize>(), 8);
        assert_eq!(slot(offset_of!(Array, get_count)), 7);
        assert_eq!(slot(offset_of!(Array, get_item_at)), 8);
        assert_eq!(std::mem::size_of::<Array>() / std::mem::size_of::<usize>(), 10);
    }

    #[cfg(windows)]
    #[test]
    fn filter_debug_labels_the_groups() {
        let parsed = parse_win_filter(CHOOSE_FILE_FILTER);
        assert_eq!(
            win::filter_debug(&parsed),
            "Markdown 文件 (*.md;*.markdown)=*.md;*.markdown, 所有文件 (*.*)=*.*"
        );
        assert_eq!(win::filter_debug(&[]), "");
    }

    #[test]
    fn capability_flag_matches_the_platform() {
        assert_eq!(dialog_supported(), cfg!(windows));
    }
}
