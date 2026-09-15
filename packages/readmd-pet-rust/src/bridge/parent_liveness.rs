use std::thread;

/// Notify the Tao thread as soon as Python closes the inherited anonymous
/// pipe. This is intentionally a blocking reader; no grace-period sleep is
/// used for EOF.
pub fn spawn_parent_watcher<F>(pipe_handle: Option<String>, parent_pid: Option<u32>, on_gone: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::Builder::new()
        .name("readmd-pet-parent".into())
        .spawn(move || {
            if let Some(value) = pipe_handle {
                if wait_for_pipe(&value) {
                    on_gone();
                    return;
                }
            }
            if let Some(pid) = parent_pid {
                wait_for_parent_pid(pid);
                on_gone();
            }
        })
        .ok();
}

#[cfg(windows)]
fn wait_for_pipe(value: &str) -> bool {
    use std::fs::File;
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;
    let raw = match value.parse::<usize>() {
        Ok(value) => value,
        Err(_) => return false,
    };
    if raw == 0 {
        return false;
    }
    // The child owns the read end and therefore closes it exactly once when
    // this File is dropped; Python owns the write end.
    let mut file = unsafe { File::from_raw_handle(raw as *mut std::ffi::c_void) };
    let mut byte = [0u8; 1];
    loop {
        match file.read(&mut byte) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(_) => return true,
        }
    }
}

#[cfg(unix)]
fn wait_for_pipe(value: &str) -> bool {
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::io::FromRawFd;
    let fd = match value.parse::<i32>() {
        Ok(value) => value,
        Err(_) => return false,
    };
    if fd < 0 {
        return false;
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut byte = [0u8; 1];
    loop {
        match file.read(&mut byte) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(_) => return true,
        }
    }
}

#[cfg(not(any(windows, unix)))]
fn wait_for_pipe(_value: &str) -> bool {
    false
}

#[cfg(windows)]
fn wait_for_parent_pid(pid: u32) {
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        );
        if handle.is_null() {
            return;
        }
        let result = WaitForSingleObject(handle, u32::MAX);
        CloseHandle(handle);
        if result == WAIT_OBJECT_0 {
            return;
        }
    }
    thread::sleep(Duration::from_millis(100));
}

#[cfg(unix)]
fn wait_for_parent_pid(pid: u32) {
    use std::thread;
    use std::time::Duration;
    loop {
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(not(any(windows, unix)))]
fn wait_for_parent_pid(_pid: u32) {}
