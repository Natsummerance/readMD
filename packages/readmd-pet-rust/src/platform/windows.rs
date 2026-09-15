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
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND,
};
use windows_sys::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, MonitorFromWindow, SetWindowRgn, HGDIOBJ, HRGN,
    MONITOR_DEFAULTTONEAREST, RGN_OR,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
    HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

const ALPHA_MAX: f64 = 255.0;

pub struct WindowsBackend {
    hwnd: Option<HWND>,
    mutex: Option<HANDLE>,
    click_through: bool,
    focusable: bool,
    interaction: InteractionRegionSnapshot,
    data_dir: PathBuf,
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
            focusable: false,
            interaction: InteractionRegionSnapshot::default(),
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
        // USERNAME is the stable fallback available in restricted packaged
        // processes; deployments that expose USER_SID get the actual SID
        // material. Hash the user and data directory independently so the
        // mutex follows the documented Local\ReadMDPetOverlay_<SID>_<DATA>
        // shape without putting either value in the global namespace.
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
            let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            style |= WS_EX_LAYERED | WS_EX_TOOLWINDOW;
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
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        Ok(())
    }

    fn effective_click_through(&self) -> bool {
        // A non-empty native input region is the authoritative hit-test
        // boundary.  The renderer may still request pointer pass-through while
        // the cursor is over transparent pixels, but retaining the region keeps
        // the browser able to receive the next move and re-arm interaction.
        self.click_through && self.interaction.rects.is_empty()
    }

    fn apply_input_region(
        &self,
        window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        let hwnd = self.hwnd()?;
        let scale = window.scale_factor().max(0.1);
        let mut combined: HRGN = std::ptr::null_mut();
        for rect in &regions.rects {
            let left = (rect.x.max(0.0) * scale).floor() as i32;
            let top = (rect.y.max(0.0) * scale).floor() as i32;
            let right = ((rect.x + rect.width).max(0.0) * scale).ceil() as i32;
            let bottom = ((rect.y + rect.height).max(0.0) * scale).ceil() as i32;
            if right <= left || bottom <= top {
                continue;
            }
            let region = unsafe { CreateRectRgn(left, top, right, bottom) };
            if region.is_null() {
                return Err(HostError::Backend(
                    "windows_input_region_alloc_failed".into(),
                ));
            }
            if combined.is_null() {
                combined = region;
            } else {
                let result = unsafe { CombineRgn(combined, combined, region, RGN_OR) };
                unsafe {
                    DeleteObject(region as HGDIOBJ);
                }
                if result == 0 {
                    unsafe {
                        DeleteObject(combined as HGDIOBJ);
                    }
                    return Err(HostError::Backend(
                        "windows_input_region_combine_failed".into(),
                    ));
                }
            }
        }

        let result = unsafe { SetWindowRgn(hwnd, combined, 1) };
        if result == 0 && !combined.is_null() {
            // Ownership transfers to the window on success only.
            unsafe {
                DeleteObject(combined as HGDIOBJ);
            }
            return Err(HostError::Backend(
                "windows_input_region_apply_failed".into(),
            ));
        }
        Ok(())
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
}

impl PlatformBackend for WindowsBackend {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn init(&mut self, window: &Window) -> HostResult<()> {
        let hwnd = Self::native_hwnd(window)?;
        self.hwnd = Some(hwnd);
        let mut security: SECURITY_ATTRIBUTES = unsafe { mem::zeroed() };
        security.nLength = mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
        // A named mutex is only a process-instance guard. It is local to the
        // interactive user session by design; Global\\ is intentionally not used.
        let name = self.mutex_name();
        let mutex = unsafe { CreateMutexW(&security, 0, name.as_ptr()) };
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
        self.set_style(self.click_through, self.focusable)
    }

    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()> {
        let bounds = bounds.clamp_host();
        window.set_outer_position(LogicalPosition::new(bounds.x, bounds.y));
        window.set_inner_size(LogicalSize::new(bounds.width, bounds.height));
        // Touch monitor selection so a negative-origin/multi-monitor placement
        // is exercised by the native backend rather than being window-local.
        if let Some(hwnd) = self.hwnd {
            unsafe {
                let _ = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            }
        }
        Ok(())
    }

    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()> {
        window.set_visible(visible);
        Ok(())
    }

    fn set_opacity(&mut self, _window: &Window, opacity: f64) -> HostResult<()> {
        let hwnd = self.hwnd()?;
        let alpha = (opacity.clamp(0.0, 1.0) * ALPHA_MAX).round() as u8;
        let ok = unsafe { SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA) };
        if ok == 0 {
            return Err(HostError::Backend("windows_opacity_failed".into()));
        }
        Ok(())
    }

    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()> {
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
        window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        self.interaction = regions.clone();
        self.apply_input_region(window, regions)?;
        let effective = self.effective_click_through();
        window
            .set_ignore_cursor_events(effective)
            .map_err(|error| HostError::Backend(format!("ignore_cursor_events:{error}")))?;
        self.set_style(effective, self.focusable)
    }
}
