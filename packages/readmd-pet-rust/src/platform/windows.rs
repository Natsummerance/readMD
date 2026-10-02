use super::{InteractionRegionSnapshot, PlatformBackend};
use crate::error::{HostError, HostResult};
use crate::protocol::SnapshotBounds;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use sha2::{Digest, Sha256};
use std::mem;
use std::path::PathBuf;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::window::Window;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, RECT,
};
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
#[allow(non_snake_case)]
struct MARGINS {
    cxLeftWidth: i32,
    cxRightWidth: i32,
    cyTopHeight: i32,
    cyBottomHeight: i32,
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmExtendFrameIntoClientArea(hwnd: HWND, pMarInset: *const MARGINS) -> i32;
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwAttribute: u32,
        pvAttribute: *const std::ffi::c_void,
        cbAttribute: u32,
    ) -> i32;
}
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    FillRect, GetMonitorInfoW, GetStockObject, MonitorFromWindow, RedrawWindow, BLACK_BRUSH,
    HBRUSH, HDC, MONITORINFO, MONITOR_DEFAULTTONEAREST, NULL_BRUSH, RDW_ERASE, RDW_FRAME,
    RDW_INVALIDATE,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetClientRect, GetWindowLongPtrW, SetClassLongPtrW, SetWindowLongPtrW,
    SetWindowPos, SystemParametersInfoW, GCLP_HBRBACKGROUND, GWLP_WNDPROC, GWL_EXSTYLE, GWL_STYLE,
    HWND_TOPMOST, HWND_NOTOPMOST, SPI_GETWORKAREA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    WM_ERASEBKGND, WM_NCACTIVATE, WM_NCPAINT, WNDPROC, WS_BORDER, WS_CAPTION,
    WS_DLGFRAME, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};

/// Undocumented uxtheme messages that draw the "basic" caption/frame.
const WM_NCUAHDRAWCAPTION: u32 = 0x00AE;
const WM_NCUAHDRAWFRAME: u32 = 0x00AF;
/// The pet host owns exactly one overlay window per process.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);
static NATIVE_DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);
static COMPLETED_DRAG: AtomicBool = AtomicBool::new(false);

pub fn native_drag_active() -> bool {
    NATIVE_DRAG_ACTIVE.load(Ordering::Acquire)
}
pub fn take_completed_drag() -> bool {
    COMPLETED_DRAG.swap(false, Ordering::AcqRel)
}

/// The overlay is a frameless DWM "sheet of glass".  DWM does not render
/// non-client chrome for it, so DefWindowProc falls back to painting a
/// Windows-basic caption with min/max/close into the surface on
/// WM_NCACTIVATE / WM_NCPAINT.  With a NULL background brush those pixels are
/// never erased, leaving a title bar framing the pet.  Swallow every
/// non-client paint and erase the client to black (= transparent on glass).
unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let original: WNDPROC =
        mem::transmute::<isize, WNDPROC>(ORIGINAL_WNDPROC.load(Ordering::Relaxed));
    match msg {
        windows_sys::Win32::UI::WindowsAndMessaging::WM_ENTERSIZEMOVE => {
            NATIVE_DRAG_ACTIVE.store(true, Ordering::Release);
        }
        windows_sys::Win32::UI::WindowsAndMessaging::WM_EXITSIZEMOVE => {
            NATIVE_DRAG_ACTIVE.store(false, Ordering::Release);
            COMPLETED_DRAG.store(true, Ordering::Release);
        }
        WM_NCPAINT | WM_NCUAHDRAWCAPTION | WM_NCUAHDRAWFRAME => return 0,
        // lParam = -1 keeps the activation bookkeeping but skips the NC repaint.
        WM_NCACTIVATE => return CallWindowProcW(original, hwnd, msg, wparam, -1),
        WM_ERASEBKGND => {
            let mut rect: RECT = mem::zeroed();
            if GetClientRect(hwnd, &mut rect) != 0 {
                FillRect(wparam as HDC, &rect, GetStockObject(BLACK_BRUSH) as HBRUSH);
            }
            return 1;
        }
        _ => {}
    }
    CallWindowProcW(original, hwnd, msg, wparam, lparam)
}

pub struct WindowsBackend {
    hwnd: Option<HWND>,
    mutex: Option<HANDLE>,
    click_through: bool,
    always_on_top: bool,
    focusable: bool,
    interaction: InteractionRegionSnapshot,
    applied: Option<SnapshotBounds>,
    data_dir: PathBuf,
}

/// Work area in the same logical units as [`SnapshotBounds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

const EDGE_PADDING: f64 = 24.0;

/// Geometry the native window will really have after the work-area guard.
///
/// The application owns the pet size (it publishes `bounds` and `scale`); the
/// host may only keep the window on screen.  Returning the applied rect is what
/// lets `PetSnapshot` acknowledgements report where the pet actually is.
pub fn place_in_work_area(bounds: SnapshotBounds, work: Option<WorkArea>) -> SnapshotBounds {
    let Some(work) = work else {
        return bounds;
    };
    let mut placed = bounds;
    let max_x = work.x + (work.width - placed.width).max(0.0);
    let max_y = work.y + (work.height - placed.height).max(0.0);
    if placed.x == 0.0 && placed.y == 0.0 {
        // Nothing has been persisted yet: park the pet bottom-right, clear of
        // the taskbar, instead of stacking it under the title bar.
        placed.x = work.x + (work.width - placed.width - EDGE_PADDING).max(0.0);
        placed.y = work.y + (work.height - placed.height - EDGE_PADDING).max(0.0);
        return placed;
    }
    placed.x = placed.x.clamp(work.x, max_x);
    placed.y = placed.y.clamp(work.y, max_y);
    placed
}

impl Default for WindowsBackend {
    fn default() -> Self {
        let data_dir = std::env::var_os("READMD_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("readmd"));
        Self {
            hwnd: None,
            mutex: None,
            click_through: true,
            always_on_top: true,
            focusable: false,
            interaction: InteractionRegionSnapshot::default(),
            applied: None,
            data_dir,
        }
    }
}

impl Drop for WindowsBackend {
    fn drop(&mut self) {
        if let Some(handle) = self.mutex.take() {
            unsafe {
                CloseHandle(handle);
            }
        }
    }
}

impl WindowsBackend {
    fn hwnd(&self) -> HostResult<HWND> {
        self.hwnd
            .ok_or_else(|| HostError::Backend("windows_hwnd_unavailable".into()))
    }

    fn mutex_name(&self) -> Vec<u16> {
        let sid = std::env::var("USER_SID")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown-user".into());
        let user_hash = format!("{:x}", Sha256::digest(sid.as_bytes()));
        let data_hash = format!(
            "{:x}",
            Sha256::digest(self.data_dir.to_string_lossy().as_bytes())
        );
        format!(
            "Local\\ReadMDPetOverlay_{}_{}",
            &user_hash[..32],
            &data_hash[..32]
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
    }

    fn set_style(&self, click_through: bool, focusable: bool) -> HostResult<()> {
        let hwnd = self.hwnd()?;
        unsafe {
            // 1. Strip ALL frame styles from GWL_STYLE: caption, sizing frame, system menu, min/max boxes
            let mut win_style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            win_style &= !(WS_CAPTION
                | WS_THICKFRAME
                | WS_BORDER
                | WS_DLGFRAME
                | WS_SYSMENU
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX);
            win_style |= WS_POPUP;
            SetWindowLongPtrW(hwnd, GWL_STYLE, win_style as isize);

            // 2. Configure GWL_EXSTYLE: strip WS_EX_LAYERED to protect DirectComposition transparency
            let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            style &= !WS_EX_LAYERED;
            style |= WS_EX_TOOLWINDOW;
            if click_through {
                style |= WS_EX_TRANSPARENT;
            } else {
                style &= !WS_EX_TRANSPARENT;
            }
            if focusable {
                style &= !WS_EX_NOACTIVATE;
            } else {
                style |= WS_EX_NOACTIVATE;
            }
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style as isize);
            SetWindowPos(
                hwnd,
                if self.always_on_top { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        Ok(())
    }

    fn effective_click_through(&self) -> bool {
        self.click_through
    }

    fn native_hwnd(window: &Window) -> HostResult<HWND> {
        let handle = window
            .window_handle()
            .map_err(|error| HostError::Backend(format!("window_handle:{error}")))?;
        match handle.as_raw() {
            RawWindowHandle::Win32(value) => Ok(value.hwnd.get() as HWND),
            _ => Err(HostError::Backend("not_a_windows_window".into())),
        }
    }
    /// Work area of the monitor that actually holds this window, converted to
    /// the window's own logical units.  `SPI_GETWORKAREA` only ever describes
    /// the primary monitor in physical pixels, so using it for a pet parked on
    /// a secondary monitor at a different DPI mis-clamps every edge.
    fn work_area(&self, window: &Window) -> Option<WorkArea> {
        let scale = window.scale_factor().max(0.1);
        if let Some(hwnd) = self.hwnd {
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            if !monitor.is_null() {
                let mut info = MONITORINFO {
                    cbSize: mem::size_of::<MONITORINFO>() as u32,
                    rcMonitor: unsafe { mem::zeroed() },
                    rcWork: unsafe { mem::zeroed() },
                    dwFlags: 0,
                };
                let ok = unsafe { GetMonitorInfoW(monitor, &mut info) } != 0;
                let work = info.rcWork;
                if ok && work.right > work.left && work.bottom > work.top {
                    return Some(WorkArea {
                        x: work.left as f64 / scale,
                        y: work.top as f64 / scale,
                        width: (work.right - work.left) as f64 / scale,
                        height: (work.bottom - work.top) as f64 / scale,
                    });
                }
            }
        }
        let mut primary: RECT = unsafe { mem::zeroed() };
        let found = unsafe {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut primary as *mut _ as *mut _, 0) != 0
        };
        if found && primary.right > primary.left && primary.bottom > primary.top {
            return Some(WorkArea {
                x: primary.left as f64 / scale,
                y: primary.top as f64 / scale,
                width: (primary.right - primary.left) as f64 / scale,
                height: (primary.bottom - primary.top) as f64 / scale,
            });
        }
        let monitor = window
            .current_monitor()
            .or_else(|| window.primary_monitor())?;
        let size = monitor.size();
        Some(WorkArea {
            x: monitor.position().x as f64 / scale,
            y: monitor.position().y as f64 / scale,
            width: size.width as f64 / scale,
            height: (size.height as f64 / scale - 50.0).max(0.0),
        })
    }
}

impl PlatformBackend for WindowsBackend {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn win32_hwnd(&self) -> Option<isize> {
        self.hwnd.map(|h| h as isize)
    }

    fn applied_bounds(&self) -> Option<SnapshotBounds> {
        self.applied
    }

    fn init(&mut self, window: &Window) -> HostResult<()> {
        let hwnd = Self::native_hwnd(window)?;
        self.hwnd = Some(hwnd);

        // 1. Extend DWM glass frame into the entire client area for true desktop transparency
        unsafe {
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

            // Disable Windows 11 rounded corner preference which draws a 1px border around popup windows
            const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
            const DWMWCP_DONOTROUND: u32 = 1;
            let corner: u32 = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 2. Set window background brush to NULL_BRUSH so GDI never paints white on erase/paint
            SetClassLongPtrW(
                hwnd,
                GCLP_HBRBACKGROUND,
                GetStockObject(NULL_BRUSH as i32) as isize,
            );

            // 3. Never paint caption chrome onto the glass (see overlay_wndproc).
            if ORIGINAL_WNDPROC.load(Ordering::Relaxed) == 0 {
                let previous =
                    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, overlay_wndproc as *const () as isize);
                ORIGINAL_WNDPROC.store(previous, Ordering::Relaxed);
            }
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null_mut(),
                RDW_INVALIDATE | RDW_ERASE | RDW_FRAME,
            );
        }

        let mut security: SECURITY_ATTRIBUTES = unsafe { mem::zeroed() };
        security.nLength = mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
        let name = self.mutex_name();
        let mut mutex = unsafe { CreateMutexW(&security, 0, name.as_ptr()) };
        if !mutex.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            for _ in 0..30 {
                unsafe {
                    CloseHandle(mutex);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
                mutex = unsafe { CreateMutexW(&security, 0, name.as_ptr()) };
                if mutex.is_null() || unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
                    break;
                }
            }
        }
        if mutex.is_null() {
            return Err(HostError::Backend(
                "pet_single_instance_mutex_failed".into(),
            ));
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(mutex);
            }
            return Err(HostError::Backend("pet_single_instance_exists".into()));
        }
        self.mutex = Some(mutex);
        self.set_style(self.click_through, self.focusable)?;
        Ok(())
    }

    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()> {
        let placed = place_in_work_area(bounds.clamp_host(), self.work_area(window));
        self.applied = Some(placed);
        window.set_outer_position(LogicalPosition::new(placed.x, placed.y));
        window.set_inner_size(LogicalSize::new(placed.width, placed.height));
        let _ = self.set_style(self.click_through, self.focusable);
        Ok(())
    }

    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()> {
        window.set_visible(visible);
        // tao rewrites GWL_STYLE from its own flags on every flag change and
        // always adds WS_CAPTION | WS_SYSMENU; with the DWM frame extended over
        // the whole client that paints a full title bar with min/max/close.
        // Strip it again after each show. Style changes must preserve visibility.
        if visible {
            self.set_style(self.effective_click_through(), self.focusable)?;
        }
        Ok(())
    }

    fn set_opacity(&mut self, _window: &Window, _opacity: f64) -> HostResult<()> {
        // Preserved without WS_EX_LAYERED to protect DirectComposition desktop transparency.
        // Opaque box / frame artifacts occur when WS_EX_LAYERED forces GDI backing store.
        Ok(())
    }

    fn drag_window(&self, window: &Window) -> HostResult<()> {
        if native_drag_active() {
            return Ok(());
        }
        let _ = window;
        let hwnd = self.hwnd()?;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, PostMessageW, HTCAPTION, WM_NCLBUTTONDOWN,
        };
        // BongoCat delegates placement to user32's caption drag. Do not also
        // drive SetWindowPos from an input-polling thread. The asynchronous
        // request lets the renderer IPC callback return before the move loop.
        // SAFETY: the HWND belongs to this event-loop thread; capture is released
        // only for an active left-button gesture, and no pointer escapes.
        unsafe {
            if GetAsyncKeyState(1) as u16 & 0x8000 == 0 {
                return Ok(());
            }
            let mut cursor = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
            if GetCursorPos(&mut cursor) == 0 {
                return Err(HostError::Backend("native_drag_cursor_unavailable".into()));
            }
            let position = ((cursor.y as u16 as u32) << 16) | cursor.x as u16 as u32;
            ReleaseCapture();
            if PostMessageW(
                hwnd,
                WM_NCLBUTTONDOWN,
                HTCAPTION as usize,
                position as isize,
            ) == 0
            {
                return Err(HostError::Backend("native_drag_start_failed".into()));
            }
        }
        Ok(())
    }

    fn set_always_on_top(&mut self, _window: &Window, enabled: bool) -> HostResult<()> {
        if self.always_on_top != enabled {
            self.always_on_top = enabled;
            self.set_style(self.effective_click_through(), self.focusable)?;
        }
        Ok(())
    }

    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()> {
        if self.click_through == click_through {
            return Ok(());
        }
        self.click_through = click_through;
        let effective = self.effective_click_through();
        window
            .set_ignore_cursor_events(effective)
            .map_err(|error| HostError::Backend(format!("ignore_cursor_events:{error}")))?;
        self.set_style(effective, self.focusable)
    }

    fn set_focusable(&mut self, window: &Window, focusable: bool) -> HostResult<()> {
        self.focusable = focusable;
        window.set_focusable(focusable);
        self.set_style(self.effective_click_through(), focusable)
    }

    fn update_interaction_regions(
        &mut self,
        _window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        self.interaction = regions.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(x: f64, y: f64, width: f64, height: f64) -> Option<WorkArea> {
        Some(WorkArea {
            x,
            y,
            width,
            height,
        })
    }

    fn bounds(x: f64, y: f64, width: f64, height: f64) -> SnapshotBounds {
        SnapshotBounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn an_unmeasurable_work_area_leaves_the_request_untouched() {
        let requested = bounds(120.0, 80.0, 320.0, 500.0);
        assert_eq!(place_in_work_area(requested, None), requested);
    }

    #[test]
    fn a_never_persisted_pet_is_parked_clear_of_the_taskbar() {
        // Nothing has been stored yet, so x/y are still the default (0, 0);
        // stacking the overlay under the title bar is what used to happen.
        let placed = place_in_work_area(
            bounds(0.0, 0.0, 320.0, 420.0).clamp_host(),
            work(0.0, 0.0, 1920.0, 1040.0),
        );
        assert_eq!(placed.x, 1920.0 - 320.0 - EDGE_PADDING);
        assert_eq!(placed.y, 1040.0 - 420.0 - EDGE_PADDING);
    }

    #[test]
    fn a_persisted_position_is_only_pulled_back_inside_the_work_area() {
        let placed = place_in_work_area(
            bounds(1900.0, 1030.0, 320.0, 420.0).clamp_host(),
            work(0.0, 0.0, 1920.0, 1040.0),
        );
        assert_eq!(placed.x, 1920.0 - 320.0);
        assert_eq!(placed.y, 1040.0 - 420.0);
        // An in-range position must survive byte for byte.
        let kept = place_in_work_area(
            bounds(640.0, 300.0, 320.0, 420.0),
            work(0.0, 0.0, 1920.0, 1040.0),
        );
        assert_eq!(kept, bounds(640.0, 300.0, 320.0, 420.0));
    }

    #[test]
    fn negative_origins_on_a_left_or_above_monitor_are_legitimate() {
        // A monitor above/left of the primary one has negative work-area
        // coordinates; the guard must not pull an in-range pet back to (0, 0).
        let placed = place_in_work_area(
            bounds(-1930.0, -500.0, 320.0, 420.0).clamp_host(),
            work(-2560.0, -1080.0, 1920.0, 1040.0),
        );
        assert_eq!(placed, bounds(-1930.0, -500.0, 320.0, 420.0));
        // The same y on a work area that starts at 0 *is* off screen and has to
        // come back down to the monitor edge.
        let clamped = place_in_work_area(
            bounds(-1930.0, -100.0, 320.0, 420.0).clamp_host(),
            work(-2560.0, 0.0, 1920.0, 1040.0),
        );
        assert_eq!(clamped.x, -1930.0);
        assert_eq!(clamped.y, 0.0);
    }

    #[test]
    fn a_tall_pet_keeps_the_height_the_application_asked_for() {
        // Regression: `set_bounds` used to clamp the height to an invented 380
        // logical pixels, so a scaled-up pet was silently cut off and the
        // reported bounds lied about the drawn rect.
        let placed = place_in_work_area(
            bounds(100.0, 100.0, 640.0, 700.0).clamp_host(),
            work(0.0, 0.0, 1920.0, 1040.0),
        );
        assert_eq!(placed.width, 640.0);
        assert_eq!(placed.height, 700.0);
    }

    #[test]
    fn a_window_larger_than_the_work_area_stays_at_its_origin() {
        let placed = place_in_work_area(
            bounds(500.0, 500.0, 640.0, 700.0).clamp_host(),
            work(0.0, 0.0, 600.0, 500.0),
        );
        assert_eq!(placed.x, 0.0);
        assert_eq!(placed.y, 0.0);
    }

    #[test]
    fn applied_bounds_report_the_geometry_the_backend_really_used() {
        let mut backend = WindowsBackend::default();
        assert_eq!(backend.applied_bounds(), None);
        backend.applied = Some(bounds(7.0, 11.0, 320.0, 420.0));
        assert_eq!(
            backend.applied_bounds(),
            Some(bounds(7.0, 11.0, 320.0, 420.0))
        );
    }
}
