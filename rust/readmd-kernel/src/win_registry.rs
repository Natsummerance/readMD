//! Hand-rolled Win32 registry / shell FFI for the file-association path.
//!
//! Replication lane `port-registry-ffi-s14`.  Python authority:
//!   * `readmd.py:6006-6063` — `install_association()`, which the legacy code
//!     performs with eight `reg add` child processes plus one
//!     `ie4uinit.exe -show`.
//!   * `src/readmd_modules/windows_native.py:93-116` — the Edge WebView2
//!     client registration the kernel read back with `reg query`.
//!
//! None of those child processes survives here: every write is a
//! `RegCreateKeyExW` + `RegSetValueExW` pair in `advapi32`, every read a
//! `RegOpenKeyExW` + `RegQueryValueExW`, and the shell refresh is a single
//! `SHChangeNotify` in `shell32`.  That is the sanctioned upgrade for a spawn
//! the authority makes — the observable effect (the same keys, values and
//! types on disk, plus an association-changed broadcast) is preserved while
//! the process count goes to zero.
//!
//! Conventions follow `native_system.rs`, which this module deliberately does
//! not edit: `HKEY` is a `*mut c_void`, the predefined roots are `usize`
//! constants, every `LSTATUS` is an `i32` compared against `ERROR_SUCCESS`,
//! and wide strings come from a `to_wide` of the same shape as that module's
//! (and `crypto.rs`'s).
//!
//! Platform shape: the pure half — the write plan, the argument plumbing and
//! the UTF-16 helpers — compiles and is unit-testable everywhere, because that
//! is the half that can be tested without a registry.  Only the `extern`
//! blocks and the calls into them are `#[cfg(windows)]`; the same entry points
//! exist as inert stubs elsewhere so every call site still type-checks on a
//! non-Windows build.

// ------------------------------------------------------------------- constants

/// `winreg.HKEY_CURRENT_USER`, i.e. the `HKCU` `reg.exe` accepts: `0x80000001`.
pub const HKCU: usize = 0x8000_0001;
/// `winreg.HKEY_LOCAL_MACHINE` / `HKLM`: `0x80000002`.
pub const HKLM: usize = 0x8000_0002;

/// `KEY_READ`: `STANDARD_RIGHTS_READ | KEY_QUERY_VALUE |
/// KEY_ENUMERATE_SUB_KEYS | KEY_NOTIFY`.  This is the mask
/// `native_system.rs::win::open` already asks for, so a read through this
/// module and a read through that one run in the same registry view.
pub const KEY_READ: u32 = 0x2001_9;
/// `KEY_WRITE`: `STANDARD_RIGHTS_WRITE | KEY_SET_VALUE | KEY_CREATE_SUB_KEY`.
/// `reg add` needs the create-sub-key right because it creates every missing
/// parent key on the way to the leaf, and so does [`set_value`], which is the
/// only place this mask is used.
pub const KEY_WRITE: u32 = 0x2000_6;

/// `REG_SZ`, the type `reg add` uses when no `/t` is given.
pub const REG_SZ: u32 = 1;
/// `REG_EXPAND_SZ`, what `/t REG_EXPAND_SZ` selects.  The shell expands `%1`
/// only for this type, which is why the open command must not be a plain
/// `REG_SZ`.
pub const REG_EXPAND_SZ: u32 = 2;

/// `REG_OPTION_NON_VOLATILE`, the `RegCreateKeyExW` `dwOptions` that matches
/// `reg add`'s default: persist to disk rather than to a volatile key.
pub const REG_OPTION_NON_VOLATILE: u32 = 0;

/// `ERROR_SUCCESS`.  Every `LSTATUS` from these calls is compared to it.
pub const ERROR_SUCCESS: i32 = 0;

/// Component bits of the access masks above.  The tests recompose
/// [`KEY_READ`]/[`KEY_WRITE`] from them so a typo in a composed constant
/// cannot survive.
pub const KEY_QUERY_VALUE: u32 = 0x0001;
/// See [`KEY_QUERY_VALUE`].
pub const KEY_SET_VALUE: u32 = 0x0002;
/// See [`KEY_QUERY_VALUE`].
pub const KEY_CREATE_SUB_KEY: u32 = 0x0004;
/// See [`KEY_QUERY_VALUE`].
pub const KEY_ENUMERATE_SUB_KEYS: u32 = 0x0008;
/// See [`KEY_QUERY_VALUE`].
pub const KEY_NOTIFY: u32 = 0x0010;
/// See [`KEY_QUERY_VALUE`]: the `READ_CONTROL` half shared by both masks.
pub const STANDARD_RIGHTS: u32 = 0x2000_0;
/// `KEY_WOW64_64KEY`.  Deliberately *not* requested anywhere: a 64-bit process
/// reading without a view flag is already in the view a 64-bit `reg.exe`
/// printed, so passing one would be the change.
pub const KEY_WOW64_64KEY: u32 = 0x0100;
/// `KEY_WOW64_32KEY`, likewise unused, kept so the choice is documented rather
/// than accidental.
pub const KEY_WOW64_32KEY: u32 = 0x0200;

/// `SHCNE_ASSOCCHANGED`, the broadcast `ie4uinit.exe -show` performs.
pub const SHCNE_ASSOCCHANGED: u32 = 0x0800_0000;
/// `SHCNF_FLUSH`: wait for the notification to be delivered.
pub const SHCNF_FLUSH: u32 = 0x1000;
/// `SHCNF_DWORD`, i.e. "both item arguments are plain values" — `0`, the only
/// correct selector to pair with two `NULL` items.
pub const SHCNF_DWORD: u32 = 0x0000;
/// The `uFlags` [`notify_association_changed`] passes.
pub const SHCNF_ASSOCIATION_FLAGS: u32 = SHCNF_FLUSH | SHCNF_DWORD;

/// Stand-in `LSTATUS` for the write stubs on a platform with no registry.
/// `i32::MIN` cannot collide with a real `LSTATUS`.
pub const LSTATUS_UNSUPPORTED: i32 = i32::MIN;

// ------------------------------------------------------------ write planning

/// One `reg add <root>\<sub_key> [/v <name>|/ve] /t <kind> /d <data> /f`.
///
/// `value_name: None` is `/ve`, the key's default (unnamed) value — every
/// value `install_association()` writes is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryWrite {
    /// [`HKCU`] or [`HKLM`].
    pub root: usize,
    /// Path below the root, backslash separated, without the `HKCU\` prefix.
    pub sub_key: String,
    /// `Some(name)` is `/v <name>`; `None` is `/ve`.
    pub value_name: Option<String>,
    /// [`REG_SZ`] or [`REG_EXPAND_SZ`].
    pub kind: u32,
    /// The literal payload, without its NUL terminator.
    pub data: String,
}

/// A write whose `LSTATUS` was not [`ERROR_SUCCESS`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFailure {
    /// The key the call was made against, for diagnostics only.
    pub sub_key: String,
    /// Raw `LSTATUS`; [`LSTATUS_UNSUPPORTED`] off Windows.
    pub code: i32,
}

/// `readmd.py:6002-6003`: `_quote(s) = '"%s"' % s`.
pub fn py_quote(text: &str) -> String {
    format!("\"{}\"", text)
}

/// The four extensions `readmd.py:6044` loops over, in that order.
pub const MARKDOWN_EXTENSIONS: [&str; 4] = [".md", ".markdown", ".mdown", ".mkd"];

/// `install_association()`'s `icon = '%s,0' % _quote(icon_file)`
/// (`readmd.py:6025`): a `,0`-suffixed, quoted `DefaultIcon` payload.
pub fn default_icon_data(icon_file: &str) -> String {
    format!("{},0", py_quote(icon_file))
}

/// `install_association()`'s `cmd = '%s "%%1"' % _quote(pyw)`
/// (`readmd.py:6028`): the quoted executable, then the literal `"%1"` the
/// shell substitutes.  Stored as [`REG_EXPAND_SZ`] so `%1` survives the write.
pub fn open_command_data(exe: &str) -> String {
    format!("{} \"%1\"", py_quote(exe))
}

/// The class name `readmd.py:6046-6047` writes into every extension key.
pub const MARKDOWN_CLASS: &str = "ReadMD.markdown";
/// The class description, `readmd.py:6048-6049`, byte-for-byte including the
/// space-separated Chinese suffix.
pub const MARKDOWN_CLASS_DESCRIPTION: &str = "ReadMD Markdown 阅读器";
/// `readmd.py:6054-6055` registers the *legacy* leaf `readmd.py` under
/// `Applications`; that literal is what the installer has always written, so
/// the kernel writes the same string rather than its own executable name.
pub const APPLICATIONS_LEAF: &str = r"Applications\readmd.py";

/// The complete ordered write set behind `install_association()`'s eight
/// `reg add` calls (`readmd.py:6044-6055`).
///
/// Order is preserved because it is observable: the old code ran the four
/// extension assignments first, so a machine interrupted mid-list had `.md`
/// pointing at a class that did not exist yet.
pub fn association_writes(exe: &str, icon_file: &str) -> Vec<RegistryWrite> {
    let command = open_command_data(exe);
    let icon = default_icon_data(icon_file);
    let default = |sub_key: String, kind: u32, data: String| RegistryWrite {
        root: HKCU,
        sub_key,
        value_name: None,
        kind,
        data,
    };
    let mut writes = Vec::with_capacity(8);
    for ext in MARKDOWN_EXTENSIONS {
        writes.push(default(
            format!(r"Software\Classes\{}", ext),
            REG_SZ,
            MARKDOWN_CLASS.to_string(),
        ));
    }
    writes.push(default(
        r"Software\Classes\ReadMD.markdown".to_string(),
        REG_SZ,
        MARKDOWN_CLASS_DESCRIPTION.to_string(),
    ));
    writes.push(default(
        r"Software\Classes\ReadMD.markdown\DefaultIcon".to_string(),
        REG_SZ,
        icon,
    ));
    writes.push(default(
        r"Software\Classes\ReadMD.markdown\shell\open\command".to_string(),
        REG_EXPAND_SZ,
        command.clone(),
    ));
    writes.push(default(
        format!(r"Software\Classes\{}\shell\open\command", APPLICATIONS_LEAF),
        REG_EXPAND_SZ,
        command,
    ));
    writes
}

/// Exactly what the two `advapi32` calls receive for one [`RegistryWrite`],
/// computed without a registry: the wide sub-key, the wide value name (an
/// empty `Vec` standing in for the `NULL` that `/ve` maps to), the type, and
/// the byte payload whose length becomes `cbData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiPlan {
    /// `lpSubKey` for `RegCreateKeyExW`.
    pub sub_key: Vec<u16>,
    /// `lpValueName` for `RegSetValueExW`; empty means pass `NULL`.
    pub value_name: Vec<u16>,
    /// `dwType`.
    pub kind: u32,
    /// `lpData`, NUL-terminated little-endian UTF-16.
    pub data: Vec<u8>,
    /// `cbData`, in bytes.
    pub data_len: u32,
    /// `samDesired` for `RegCreateKeyExW`.
    pub access: u32,
    /// `dwOptions` for `RegCreateKeyExW`.
    pub options: u32,
}

/// Turn a [`RegistryWrite`] into an [`FfiPlan`].  Pure, and the only place the
/// argument plumbing is decided, so [`apply`] cannot drift from what the tests
/// pin.
pub fn plan_write(write: &RegistryWrite) -> FfiPlan {
    let units = to_wide(&write.data);
    FfiPlan {
        sub_key: to_wide(&write.sub_key),
        // `/ve` addresses the unnamed value: `RegSetValueExW` takes `NULL`,
        // which is what `reg add /ve` passes.
        value_name: write
            .value_name
            .as_deref()
            .map(to_wide)
            .unwrap_or_default(),
        kind: write.kind,
        data_len: (units.len() * 2) as u32,
        data: wide_bytes_of(&units),
        access: KEY_WRITE,
        options: REG_OPTION_NON_VOLATILE,
    }
}

/// The [`FfiPlan`] counterpart for a value deletion: which key to open, which
/// value name to drop, with which access mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfiDelete {
    /// `lpSubKey` for `RegOpenKeyExW`.
    pub sub_key: Vec<u16>,
    /// `lpValueName` for `RegDeleteValueW`; empty means `NULL`.
    pub value_name: Vec<u16>,
    /// A delete needs write access to the value's *key*.
    pub access: u32,
}

/// Plan a `RegDeleteValueW`.  Pure, see [`plan_write`].
pub fn plan_delete(sub_key: &str, value_name: Option<&str>) -> FfiDelete {
    FfiDelete {
        sub_key: to_wide(sub_key),
        value_name: value_name.map(to_wide).unwrap_or_default(),
        access: KEY_WRITE,
    }
}

// -------------------------------------------------------------- UTF-16 helpers

/// A Win32 wide string: UTF-16 code units plus a terminating NUL.  Same shape
/// as `native_system.rs::win::to_wide` and `crypto.rs`.
pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Little-endian bytes of an already-[`to_wide`] code-unit slice, i.e. the
/// `lpData` buffer for a `REG_SZ`/`REG_EXPAND_SZ` value.
pub fn wide_bytes_of(units: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(units.len() * 2);
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

/// Inverse of [`wide_bytes_of`] for a possibly unaligned buffer: every two
/// little-endian bytes become one code unit, a trailing lone byte is dropped,
/// and invalid units become U+FFFD the way `native_system::decode_reg_sz`
/// makes them.
pub fn decode_utf16_bytes(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() == 2 {
                Some(u16::from_le_bytes([pair[0], pair[1]]))
            } else {
                None
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Cut a registry string payload at its first NUL, which is where `reg.exe`
/// stops printing and where the shell stops reading; anything past the
/// terminator is padding, not data.
pub fn trim_at_nul(text: &str) -> String {
    match text.find('\0') {
        Some(index) => text[..index].to_string(),
        None => text.to_string(),
    }
}

// ------------------------------------------------------------------ read paths

/// Open `sub_key` for reading and return `(type, raw bytes)` for one value.
/// `value_name: None` addresses the key's default value.
///
/// The `Option` is the `FileNotFoundError` Python catches with a bare
/// `except Exception`, and equally the "key not found" that made the retired
/// `reg query` path fall through to its next candidate.
pub fn query_value(
    root: usize,
    sub_key: &str,
    value_name: Option<&str>,
) -> Option<(u32, Vec<u8>)> {
    #[cfg(windows)]
    {
        unsafe {
            let handle = win::open(root, sub_key)?;
            let name = value_name.map(to_wide).unwrap_or_default();
            let found = win::query(handle, &name);
            win::close(handle);
            found
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (root, sub_key, value_name);
        None
    }
}

/// Any string-typed value: [`REG_SZ`] or [`REG_EXPAND_SZ`], NUL-trimmed.
pub fn query_string(root: usize, sub_key: &str, value_name: &str) -> Option<String> {
    let (kind, bytes) = query_value(root, sub_key, Some(value_name))?;
    match kind {
        REG_SZ | REG_EXPAND_SZ => Some(trim_at_nul(&decode_utf16_bytes(&bytes))),
        _ => None,
    }
}

/// [`REG_SZ`] only.  This is the parity-exact reader for the retired
/// `reg query` text parser, which collected the lines containing `REG_SZ` and
/// ignored every other type; the WebView2 probe goes through it so a value
/// stored as `REG_EXPAND_SZ` still reads as absent, exactly as before.
pub fn query_reg_sz(root: usize, sub_key: &str, value_name: &str) -> Option<String> {
    let (kind, bytes) = query_value(root, sub_key, Some(value_name))?;
    match kind {
        REG_SZ => Some(trim_at_nul(&decode_utf16_bytes(&bytes))),
        _ => None,
    }
}

// ----------------------------------------------------------------- mutations

/// `reg add <root>\<sub_key> [/v <name>|/ve] /t <kind> /d <data> /f`: create
/// the key chain with [`KEY_WRITE`], set the value, close the handle on every
/// path.
pub fn set_value(
    root: usize,
    sub_key: &str,
    value_name: Option<&str>,
    kind: u32,
    data: &str,
) -> Result<(), i32> {
    let write = RegistryWrite {
        root,
        sub_key: sub_key.to_string(),
        value_name: value_name.map(str::to_string),
        kind,
        data: data.to_string(),
    };
    let plan = plan_write(&write);
    #[cfg(windows)]
    {
        unsafe {
            let handle = match win::create(root, &plan.sub_key, plan.options, plan.access) {
                Ok(handle) => handle,
                Err(code) => return Err(code),
            };
            let set = win::set(handle, &plan.value_name, plan.kind, &plan.data);
            win::close(handle);
            set
        }
    }
    #[cfg(not(windows))]
    {
        let _ = plan;
        Err(LSTATUS_UNSUPPORTED)
    }
}

/// `reg delete <root>\<sub_key> /v <name> /f`.  Nothing in the association
/// path deletes anything — the ported Python does not either — so this exists
/// to complete the `advapi32` surface an uninstall needs, and is deliberately
/// not wired to a call site yet.
#[allow(dead_code)]
pub fn delete_value(root: usize, sub_key: &str, value_name: &str) -> Result<(), i32> {
    let plan = plan_delete(sub_key, Some(value_name));
    #[cfg(windows)]
    {
        unsafe {
            let handle = match win::open_with(root, &plan.sub_key, KEY_WRITE) {
                Ok(handle) => handle,
                Err(code) => return Err(code),
            };
            let code = win::delete(handle, &plan.value_name);
            win::close(handle);
            if code == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(code)
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (plan, root);
        Err(LSTATUS_UNSUPPORTED)
    }
}

/// Runs every write, collecting the `LSTATUS` failures rather than stopping at
/// the first one.
///
/// `subprocess.run(capture_output=True)` looks at a non-zero `reg add` exit
/// code and *discards* it, so the authority's eight writes are
/// fire-and-forget and it still returns `True`.  The failures are therefore
/// reported here and deliberately swallowed by the kernel counterpart of
/// `install_association`, which keeps the authority's observable
/// `(True, "OK")` / `(False, "<error string>")` contract instead of making the
/// kernel stricter than the thing it is replacing.
pub fn apply(writes: &[RegistryWrite]) -> Vec<WriteFailure> {
    let mut failures = Vec::new();
    for write in writes {
        let result = set_value(
            write.root,
            &write.sub_key,
            write.value_name.as_deref(),
            write.kind,
            &write.data,
        );
        if let Err(code) = result {
            failures.push(WriteFailure {
                sub_key: write.sub_key.clone(),
                code,
            });
        }
    }
    failures
}

/// `ie4uinit.exe -show` without the child process: broadcast
/// [`SHCNE_ASSOCCHANGED`] so the shell rebuilds its association and icon
/// caches.
///
/// `SHChangeNotify` returns `VOID`, so there is no failure channel to report —
/// which is exactly `readmd.py:6056-6059`, whose `try/except: pass` around the
/// spawn swallowed whatever the child could have done.
pub fn notify_association_changed() {
    #[cfg(windows)]
    unsafe {
        win::sh_change_notify(SHCNE_ASSOCCHANGED, SHCNF_ASSOCIATION_FLAGS);
    }
    #[cfg(not(windows))]
    {}
}

// -------------------------------------------------------------------- win32

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    use super::{ERROR_SUCCESS, KEY_READ, to_wide};

    type HKey = *mut c_void;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            key: HKey,
            sub_key: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            sam_desired: u32,
            security_attributes: *mut c_void,
            result: *mut HKey,
            disposition: *mut u32,
        ) -> i32;
        fn RegOpenKeyExW(
            key: HKey,
            sub_key: *const u16,
            options: u32,
            sam_desired: u32,
            result: *mut HKey,
        ) -> i32;
        fn RegSetValueExW(
            key: HKey,
            value_name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            data_len: u32,
        ) -> i32;
        fn RegDeleteValueW(key: HKey, value_name: *const u16) -> i32;
        fn RegQueryValueExW(
            key: HKey,
            value_name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: HKey) -> i32;
    }

    // `SHChangeNotify`, *not* `SHChangeNotifyW`: read off the host
    // `C:\Windows\System32\shell32.dll` export table, the `SHChangeNotify*`
    // names present are `SHChangeNotify`, `...Register`, `...Deregister`,
    // `...RegisterThread` and `...SuspendResume`.  There is no wide form, so
    // declaring one would be an unresolved import at link time.
    #[link(name = "shell32")]
    extern "system" {
        fn SHChangeNotify(event_id: u32, flags: u32, item1: *const c_void, item2: *const c_void);
    }

    /// `RegOpenKeyExW` with a caller-chosen access mask and `options = 0`, in
    /// the process' default registry view (no `KEY_WOW64_*` bit), which is the
    /// view a 64-bit `reg.exe` read.
    pub unsafe fn open_with(root: usize, sub_key: &[u16], access: u32) -> Result<HKey, i32> {
        let mut handle: HKey = std::ptr::null_mut();
        let code = RegOpenKeyExW(root as HKey, sub_key.as_ptr(), 0, access, &mut handle);
        if code == ERROR_SUCCESS {
            Ok(handle)
        } else {
            Err(code)
        }
    }

    /// [`open_with`] with [`KEY_READ`] and an `Option` for the probe sites.
    pub unsafe fn open(root: usize, sub_key: &str) -> Option<HKey> {
        open_with(root, &to_wide(sub_key), KEY_READ).ok()
    }

    pub unsafe fn close(handle: HKey) {
        RegCloseKey(handle);
    }

    /// `RegCreateKeyExW`, creating missing parents the way `reg add` does.
    /// `lpClass`, `lpSecurityAttributes` and `lpdwDisposition` are all `NULL`:
    /// no class name is written, default security is what `reg add` gets, and
    /// the created-versus-opened disposition is not observable here.
    pub unsafe fn create(
        root: usize,
        sub_key: &[u16],
        options: u32,
        access: u32,
    ) -> Result<HKey, i32> {
        let mut handle: HKey = std::ptr::null_mut();
        let code = RegCreateKeyExW(
            root as HKey,
            sub_key.as_ptr(),
            0,
            std::ptr::null(),
            options,
            access,
            std::ptr::null_mut(),
            &mut handle,
            std::ptr::null_mut(),
        );
        if code == ERROR_SUCCESS {
            Ok(handle)
        } else {
            Err(code)
        }
    }

    /// `RegSetValueExW` for one planned value; an empty name is the `NULL`
    /// that addresses the key's default value.
    pub unsafe fn set(
        handle: HKey,
        value_name: &[u16],
        kind: u32,
        data: &[u8],
    ) -> Result<(), i32> {
        let name = if value_name.is_empty() {
            std::ptr::null()
        } else {
            value_name.as_ptr()
        };
        let code = RegSetValueExW(handle, name, 0, kind, data.as_ptr(), data.len() as u32);
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(code)
        }
    }

    /// `RegDeleteValueW`.  An empty planned name becomes the `NULL` that
    /// addresses the key's default value — passing `Vec::new().as_ptr()`
    /// instead would hand a dangling pointer to an `LPCWSTR`.  The public
    /// [`super::delete_value`] requires a real name, so no live path reaches
    /// the `NULL` arm; it exists for the planned `plan_delete(.., None)` form.
    pub unsafe fn delete(handle: HKey, value_name: &[u16]) -> i32 {
        let name = if value_name.is_empty() {
            std::ptr::null()
        } else {
            value_name.as_ptr()
        };
        RegDeleteValueW(handle, name)
    }

    /// `RegQueryValueExW` twice (size probe, then read), the same two-call
    /// shape and `+2` slack `native_system.rs::win::query` uses.
    pub unsafe fn query(handle: HKey, name: &[u16]) -> Option<(u32, Vec<u8>)> {
        let name_ptr = if name.is_empty() {
            std::ptr::null()
        } else {
            name.as_ptr()
        };
        let mut kind = 0u32;
        let mut length = 0u32;
        if RegQueryValueExW(
            handle,
            name_ptr,
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut length,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut buffer = vec![0u8; length as usize + 2];
        let mut size = buffer.len() as u32;
        if RegQueryValueExW(
            handle,
            name_ptr,
            std::ptr::null_mut(),
            &mut kind,
            buffer.as_mut_ptr(),
            &mut size,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        buffer.truncate(size as usize);
        Some((kind, buffer))
    }

    pub unsafe fn sh_change_notify(event_id: u32, flags: u32) {
        SHChangeNotify(event_id, flags, std::ptr::null(), std::ptr::null());
    }
}

// ----------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn write_of(sub_key: &str, kind: u32, data: &str) -> RegistryWrite {
        RegistryWrite {
            root: HKCU,
            sub_key: sub_key.to_string(),
            value_name: None,
            kind,
            data: data.to_string(),
        }
    }

    fn sample() -> Vec<RegistryWrite> {
        association_writes(r"C:\R\ReadMD.exe", r"D:\d\markdown-file.ico")
    }

    // ----------------------------------------------------------- constants

    #[test]
    fn predefined_root_values() {
        assert_eq!(HKCU, 0x8000_0001usize);
        assert_eq!(HKLM, 0x8000_0002usize);
        // Both are kernel handles, and they differ only in the low byte.
        assert_ne!(HKCU & 0x8000_0000, 0);
        assert_eq!(HKCU ^ HKLM, 3);
    }

    #[test]
    fn access_masks_compose_from_their_bits() {
        assert_eq!(
            KEY_READ,
            STANDARD_RIGHTS | KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS | KEY_NOTIFY
        );
        assert_eq!(KEY_READ, 0x20019);
        assert_eq!(
            KEY_WRITE,
            STANDARD_RIGHTS | KEY_SET_VALUE | KEY_CREATE_SUB_KEY
        );
        assert_eq!(KEY_WRITE, 0x20006);
        // `reg add` creates parents, so the write mask really does carry
        // KEY_CREATE_SUB_KEY; `reg query` never needs a set right.
        assert_ne!(KEY_WRITE & KEY_CREATE_SUB_KEY, 0);
        assert_eq!(KEY_READ & KEY_SET_VALUE, 0);
        assert_eq!(KEY_WOW64_64KEY, 0x0100);
        assert_eq!(KEY_WOW64_32KEY, 0x0200);
        // No view flag is requested anywhere, which is the deliberate choice
        // documented on KEY_WOW64_64KEY.
        assert_eq!(KEY_READ & (KEY_WOW64_64KEY | KEY_WOW64_32KEY), 0);
        assert_eq!(KEY_WRITE & (KEY_WOW64_64KEY | KEY_WOW64_32KEY), 0);
    }

    #[test]
    fn value_type_ids_match_winreg() {
        assert_eq!(REG_SZ, 1);
        assert_eq!(REG_EXPAND_SZ, 2);
        assert_eq!(REG_OPTION_NON_VOLATILE, 0);
        assert_eq!(ERROR_SUCCESS, 0);
        assert_eq!(LSTATUS_UNSUPPORTED, i32::MIN);
    }

    #[test]
    fn shell_notification_flags_are_assoc_changed_plus_flush() {
        assert_eq!(SHCNE_ASSOCCHANGED, 0x0800_0000);
        assert_eq!(SHCNF_FLUSH, 0x1000);
        assert_eq!(SHCNF_DWORD, 0);
        assert_eq!(SHCNF_ASSOCIATION_FLAGS, 0x1000);
        // `SHCNF_TYPE` is the low-byte item selector; with SHCNF_DWORD the two
        // item arguments are plain values, which is why both are NULL.
        assert_eq!(SHCNF_ASSOCIATION_FLAGS & 0xFF, SHCNF_DWORD);
        // The event id must not collide with the flag space.
        assert_eq!(SHCNE_ASSOCCHANGED & SHCNF_FLUSH, 0);
    }

    // -------------------------------------------------------------- utf-16

    #[test]
    fn to_wide_appends_exactly_one_terminator() {
        assert_eq!(to_wide(""), vec![0u16]);
        assert_eq!(to_wide("a"), vec![0x61u16, 0]);
        assert_eq!(to_wide("pv"), vec![0x70, 0x76, 0]);
        let text = "ReadMD Markdown 阅读器";
        // 19 characters (6 + 1 + 8 + 1 + 3 CJK), all BMP, so one code unit
        // each plus the terminator: 20 units where `str::len()` says 25 bytes.
        assert_eq!(text.chars().count(), 19);
        assert_eq!(text.len(), 25);
        assert_eq!(to_wide(text).len(), 20);
    }

    #[test]
    fn to_wide_handles_surrogate_pairs() {
        // U+1F4C4 is two UTF-16 code units, not one, so a naive `as u16` cast
        // per char would have truncated it.
        let wide = to_wide("\u{1F4C4}");
        assert_eq!(wide.len(), 3);
        assert_eq!(&wide[..2], &[0xD83D, 0xDCC4]);
        assert_eq!(wide[2], 0);
    }

    #[test]
    fn wide_bytes_is_little_endian_and_round_trips() {
        let units = to_wide("ab");
        assert_eq!(wide_bytes_of(&units), vec![b'a', 0, b'b', 0, 0, 0]);
        assert_eq!(decode_utf16_bytes(&wide_bytes_of(&units)), "ab\0");
        let cjk = to_wide("阅读器");
        assert_eq!(decode_utf16_bytes(&wide_bytes_of(&cjk)), "阅读器\0");
        assert_eq!(trim_at_nul(&decode_utf16_bytes(&wide_bytes_of(&cjk))), "阅读器");
    }

    #[test]
    fn decode_utf16_bytes_tolerates_odd_and_empty_buffers() {
        assert_eq!(decode_utf16_bytes(&[]), "");
        // A truncated final code unit is dropped rather than panicking.
        assert_eq!(decode_utf16_bytes(&[b'a', 0, b'b']), "a");
        // An unpaired surrogate becomes U+FFFD, the same lossy choice
        // `native_system::decode_reg_sz` makes.
        assert_eq!(decode_utf16_bytes(&[0x00, 0xD8]), "\u{FFFD}");
    }

    #[test]
    fn trim_at_nul_stops_at_the_first_terminator() {
        assert_eq!(trim_at_nul("abc"), "abc");
        assert_eq!(trim_at_nul("abc\0"), "abc");
        assert_eq!(trim_at_nul("abc\0trailing padding"), "abc");
        assert_eq!(trim_at_nul("\0"), "");
        assert_eq!(trim_at_nul(""), "");
    }

    // ------------------------------------------------------- write planning

    #[test]
    fn quote_matches_the_python_helper() {
        assert_eq!(py_quote(r"C:\a b\ReadMD.exe"), r#""C:\a b\ReadMD.exe""#);
        assert_eq!(py_quote(""), r#""""#);
    }

    #[test]
    fn open_command_keeps_percent_one_literal_and_quoted() {
        assert_eq!(
            open_command_data(r"C:\Program Files\ReadMD\ReadMD.exe"),
            r#""C:\Program Files\ReadMD\ReadMD.exe" "%1""#
        );
    }

    #[test]
    fn default_icon_appends_the_index_outside_the_quotes() {
        assert_eq!(
            default_icon_data(r"D:\data\icons\markdown-file.ico"),
            r#""D:\data\icons\markdown-file.ico",0"#
        );
    }

    #[test]
    fn association_writes_are_the_eight_reg_add_calls_in_order() {
        let writes = sample();
        assert_eq!(writes.len(), 8);
        let keys: Vec<&str> = writes.iter().map(|w| w.sub_key.as_str()).collect();
        assert_eq!(
            keys,
            [
                r"Software\Classes\.md",
                r"Software\Classes\.markdown",
                r"Software\Classes\.mdown",
                r"Software\Classes\.mkd",
                r"Software\Classes\ReadMD.markdown",
                r"Software\Classes\ReadMD.markdown\DefaultIcon",
                r"Software\Classes\ReadMD.markdown\shell\open\command",
                r"Software\Classes\Applications\readmd.py\shell\open\command",
            ]
        );
        // Every one is HKCU — `install_association`'s "no admin" promise — and
        // every one is the *default* value, i.e. `/ve`.
        assert!(writes.iter().all(|w| w.root == HKCU));
        assert!(writes.iter().all(|w| w.value_name.is_none()));
    }

    #[test]
    fn association_write_types_and_payloads_match_the_authority() {
        let writes = sample();
        for write in &writes[..4] {
            assert_eq!(
                (write.kind, write.data.as_str()),
                (REG_SZ, "ReadMD.markdown"),
                "{}",
                write.sub_key
            );
        }
        assert_eq!(writes[4].kind, REG_SZ);
        assert_eq!(writes[4].data, "ReadMD Markdown 阅读器");
        assert_eq!(writes[5].kind, REG_SZ);
        assert_eq!(writes[5].data, r#""D:\d\markdown-file.ico",0"#);
        // Only the two open commands are REG_EXPAND_SZ; as a plain REG_SZ the
        // shell would not expand `%1`.
        assert_eq!(writes[6].kind, REG_EXPAND_SZ);
        assert_eq!(writes[7].kind, REG_EXPAND_SZ);
        let command = r#""C:\R\ReadMD.exe" "%1""#;
        assert_eq!(writes[6].data, command);
        assert_eq!(writes[7].data, command);
        assert_ne!(writes[6].sub_key, writes[7].sub_key);
    }

    #[test]
    fn association_writes_reuse_the_legacy_applications_leaf() {
        let writes = sample();
        assert!(writes[7]
            .sub_key
            .starts_with(r"Software\Classes\Applications\readmd.py"));
        assert_eq!(APPLICATIONS_LEAF, r"Applications\readmd.py");
        assert_eq!(MARKDOWN_CLASS, "ReadMD.markdown");
    }

    #[test]
    fn extension_list_is_the_python_tuple() {
        assert_eq!(MARKDOWN_EXTENSIONS, [".md", ".markdown", ".mdown", ".mkd"]);
        let leaves: Vec<String> = sample()[..4]
            .iter()
            .map(|w| w.sub_key.rsplit('\\').next().unwrap().to_string())
            .collect();
        assert_eq!(leaves, MARKDOWN_EXTENSIONS.map(str::to_string));
    }

    #[test]
    fn association_writes_are_independent_of_previous_calls() {
        // The planner must not accumulate: two calls produce equal, not
        // doubling, plans.
        assert_eq!(sample(), sample());
        assert_eq!(association_writes("A", "B").len(), 8);
    }

    // ---------------------------------------------------- ffi arg plumbing

    #[test]
    fn plan_write_produces_nul_terminated_utf16_payload() {
        let plan = plan_write(&write_of(r"Software\Classes\.md", REG_SZ, "ReadMD.markdown"));
        assert_eq!(plan.kind, REG_SZ);
        assert_eq!(plan.access, KEY_WRITE);
        assert_eq!(plan.options, REG_OPTION_NON_VOLATILE);
        assert_eq!(plan.sub_key, to_wide(r"Software\Classes\.md"));
        // `/ve` -> empty name -> NULL at the call site.
        assert!(plan.value_name.is_empty());
        // cbData counts bytes with the terminator: 15 chars + NUL -> 32.
        assert_eq!(plan.data_len, 32);
        assert_eq!(plan.data.len() as u32, plan.data_len);
        assert_eq!(&plan.data[plan.data.len() - 2..], &[0, 0]);
        assert_eq!(
            trim_at_nul(&decode_utf16_bytes(&plan.data)),
            "ReadMD.markdown"
        );
    }

    #[test]
    fn plan_write_names_a_named_value_and_keeps_the_expand_type() {
        let mut write = write_of(r"Software\Classes\X", REG_EXPAND_SZ, "%1");
        write.value_name = Some("command".to_string());
        let plan = plan_write(&write);
        assert_eq!(plan.value_name, to_wide("command"));
        assert_eq!(plan.kind, REG_EXPAND_SZ);
        assert_eq!(plan.data_len, 6); // '%', '1', NUL = 3 units = 6 bytes
        assert_eq!(plan.data, wide_bytes_of(&to_wide("%1")));
    }

    #[test]
    fn plan_write_data_length_is_bytes_not_units() {
        let plan = plan_write(&write_of("K", REG_SZ, "abc"));
        assert_eq!(plan.data_len, 8); // 3 chars + NUL = 4 units * 2 bytes
        assert_eq!(plan.data.len(), 8);
        let empty = plan_write(&write_of("K", REG_SZ, ""));
        assert_eq!(empty.data_len, 2); // the terminator alone
        assert_eq!(empty.data, vec![0, 0]);
    }

    #[test]
    fn plan_write_carries_the_chinese_description_without_mangling() {
        let plan = plan_write(&write_of(
            r"Software\Classes\ReadMD.markdown",
            REG_SZ,
            MARKDOWN_CLASS_DESCRIPTION,
        ));
        let text = trim_at_nul(&decode_utf16_bytes(&plan.data));
        assert_eq!(text, "ReadMD Markdown 阅读器");
        assert_eq!(plan.data_len, (text.encode_utf16().count() + 1) as u32 * 2);
    }

    #[test]
    fn plan_write_length_fits_a_u32_cb_data() {
        // `cbData` is a DWORD, so a value near the 64 KiB REG_SZ ceiling still
        // has to be expressible: check the conversion cannot wrap.
        let big = "x".repeat(30_000);
        let plan = plan_write(&write_of("K", REG_SZ, &big));
        assert_eq!(plan.data_len, 60_002);
        assert_eq!(plan.data.len() as u32, plan.data_len);
    }

    #[test]
    fn plan_write_leaves_a_zero_len_payload_impossible() {
        // Even the empty string writes 2 bytes, so `lpData` is never a dangling
        // pointer paired with a zero size.
        for data in ["", "a"] {
            let plan = plan_write(&write_of("K", REG_SZ, data));
            assert!(!plan.data.is_empty());
            assert_ne!(plan.data_len, 0);
            assert_eq!(plan.data_len % 2, 0);
        }
    }

    #[test]
    fn plan_delete_uses_the_write_mask_and_names_the_value() {
        let plan = plan_delete(r"Software\Classes\.md", Some(""));
        assert_eq!(plan.access, KEY_WRITE);
        assert_eq!(plan.sub_key, to_wide(r"Software\Classes\.md"));
        assert_eq!(plan.value_name, to_wide(""));
        let default_value = plan_delete("K", None);
        // An empty planned name is the condition `win::delete` maps to a NULL
        // `lpValueName`; `Some("")` is not — it stays a wide empty string,
        // which is what `reg delete … /v ""` passes.
        assert!(default_value.value_name.is_empty());
    }

    #[test]
    fn apply_over_an_empty_plan_touches_nothing() {
        // Exercises the loop plumbing without a registry: no writes in, no
        // failures out.  This is the only `apply` call any test makes on
        // Windows, because a unit test must not write to HKCU.
        assert!(apply(&[]).is_empty());
    }

    #[test]
    fn write_failure_keeps_key_and_code() {
        let failure = WriteFailure {
            sub_key: r"Software\Classes\.md".to_string(),
            code: 5, // ERROR_ACCESS_DENIED
        };
        assert_eq!(failure.code, 5);
        assert_eq!(failure.sub_key, r"Software\Classes\.md");
        assert!(apply(&[]).iter().ne(&[failure]));
    }

    // ----------------------------------------------------------- read paths

    #[test]
    fn reader_stubs_are_inert_off_windows() {
        #[cfg(not(windows))]
        {
            assert_eq!(query_value(HKCU, "Software", Some("nope")), None);
            assert_eq!(query_string(HKCU, "Software", "nope"), None);
            assert_eq!(query_reg_sz(HKCU, "Software", "nope"), None);
            assert_eq!(
                set_value(HKCU, "Software", None, REG_SZ, "x"),
                Err(LSTATUS_UNSUPPORTED)
            );
            assert_eq!(
                delete_value(HKCU, "Software", "x"),
                Err(LSTATUS_UNSUPPORTED)
            );
            let failures = apply(&[write_of("K", REG_SZ, "v")]);
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].code, LSTATUS_UNSUPPORTED);
        }
    }

    #[test]
    #[cfg(windows)]
    fn reads_drive_the_ffi_against_an_absent_key_and_write_nothing() {
        // A sub-key no installer creates.  Opening it is read-only and cannot
        // change any registry state, but it does drive RegOpenKeyExW +
        // RegQueryValueExW + RegCloseKey for real, which is what proves the
        // advapi32 declarations bind to live exports.
        let absent = r"Software\ReadMD-Kernel-Lane-S14-Absent";
        assert_eq!(query_value(HKCU, absent, Some("pv")), None);
        assert_eq!(query_string(HKCU, absent, "pv"), None);
        assert_eq!(query_reg_sz(HKCU, absent, "pv"), None);
        // The default value of the same absent key, and a delete that never
        // gets as far as a handle: both must be `None`/`Err`, not a panic.
        assert_eq!(query_value(HKCU, absent, None), None);
        assert!(delete_value(HKCU, absent, "pv").is_err());
    }

    #[test]
    fn notify_association_changed_is_infallible_by_design() {
        // `SHChangeNotify` returns VOID, so this call has no observable
        // result to assert beyond "it cannot fail the caller", which is the
        // same contract as the swallowed `ie4uinit.exe -show` spawn.  The test
        // deliberately does *not* broadcast during `cargo test` (a flush makes
        // every shell rebuild its caches); the binding itself is proven by the
        // test binary linking, and the real call site is `--assoc`.
        assert_eq!(SHCNF_ASSOCIATION_FLAGS, SHCNF_FLUSH);
        assert!(std::mem::size_of::<usize>() >= 4);
    }

    #[test]
    fn pure_half_is_platform_independent() {
        // Nothing in the planner is `#[cfg]`-gated, so a Linux or macOS build
        // produces the identical eight-item plan; only the FFI arms differ.
        let writes = association_writes("/usr/bin/readmd", "/tmp/markdown-file.ico");
        assert_eq!(writes.len(), 8);
        assert_eq!(writes[5].data, "\"/tmp/markdown-file.ico\",0");
        assert_eq!(writes[6].data, "\"/usr/bin/readmd\" \"%1\"");
    }
}
