use std::thread;

/// Outcome of reading the inherited parent pipe.
///
/// The two failure modes must not collapse into one another: an *EOF* proves
/// Python closed its write end, while an *unusable handle* only proves the
/// handle we were handed is not a live pipe.  Treating the second as the first
/// makes the host commit suicide at startup (`stopped` / `parent_eof`) while the
/// parent is perfectly alive, which is how "the pet never opens" manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeRead {
    /// The write end is closed: the parent process is gone.
    Eof,
    /// The handle is absent, not a pipe, or no longer valid.  This says nothing
    /// about the parent.
    Unusable,
}

/// Win32 codes that mean "the peer is gone": `ERROR_BROKEN_PIPE` (109),
/// `ERROR_NO_DATA` (232), `ERROR_PIPE_NOT_CONNECTED` (233).
#[cfg(windows)]
const PIPE_CLOSED_ERRORS: &[i32] = &[109, 232, 233];
/// POSIX equivalents: `EPIPE` (32), `ESHUTDOWN` (108), `ENOTCONN` (107).
#[cfg(unix)]
const PIPE_CLOSED_ERRORS: &[i32] = &[32, 108, 107];
#[cfg(not(any(windows, unix)))]
const PIPE_CLOSED_ERRORS: &[i32] = &[];

/// Classify a failed pipe read.  Anything that is not an explicit
/// "peer closed" code is reported as unusable rather than as parent death.
pub fn classify_pipe_error(raw_os_error: Option<i32>) -> PipeRead {
    match raw_os_error {
        Some(code) if PIPE_CLOSED_ERRORS.contains(&code) => PipeRead::Eof,
        _ => PipeRead::Unusable,
    }
}

/// Decide whether the host should shut itself down.
///
/// `probe` is the PID fallback (`wait_for_parent_pid`) and is injected so the
/// policy is testable without killing anything.  The invariant encoded here is
/// that the host never exits without positive evidence: an unusable pipe with
/// no `READMD_PARENT_PID` keeps the overlay alive, because a leaked pet is
/// recoverable while a pet that vanishes at launch is not.
pub fn resolve_parent_liveness(
    pipe: Option<PipeRead>,
    parent_pid: Option<u32>,
    probe: impl FnOnce(u32) -> bool,
) -> bool {
    if pipe == Some(PipeRead::Eof) {
        return true;
    }
    match parent_pid {
        Some(pid) => probe(pid),
        None => false,
    }
}

/// Notify the Tao thread as soon as Python closes the inherited anonymous
/// pipe.  This is intentionally a blocking reader; no grace-period sleep is
/// used for EOF.
pub fn spawn_parent_watcher<F>(pipe_handle: Option<String>, parent_pid: Option<u32>, on_gone: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::Builder::new()
        .name("readmd-pet-parent".into())
        .spawn(move || {
            let pipe = pipe_handle.as_deref().and_then(wait_for_pipe);
            if resolve_parent_liveness(pipe, parent_pid, wait_for_parent_pid) {
                on_gone();
            }
        })
        .ok();
}

/// `None` when no usable handle was advertised at all, which is the ordinary
/// "Python did not create a pipe" case rather than evidence of anything.
#[cfg(windows)]
fn wait_for_pipe(value: &str) -> Option<PipeRead> {
    use std::fs::File;
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;

    let raw = value.parse::<usize>().ok().filter(|value| *value != 0)?;
    // The child owns the read end and therefore closes it exactly once when
    // this File is dropped; Python owns the write end.
    let mut file = unsafe { File::from_raw_handle(raw as *mut core::ffi::c_void) };
    let mut byte = [0u8; 1];
    loop {
        match file.read(&mut byte) {
            Ok(0) => return Some(PipeRead::Eof),
            Ok(_) => continue,
            Err(error) => return Some(classify_pipe_error(error.raw_os_error())),
        }
    }
}

#[cfg(unix)]
fn wait_for_pipe(value: &str) -> Option<PipeRead> {
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::io::FromRawFd;

    let raw = value.parse::<i32>().ok().filter(|value| *value >= 0)?;
    let mut file = unsafe { File::from_raw_fd(raw) };
    let mut byte = [0u8; 1];
    loop {
        match file.read(&mut byte) {
            Ok(0) => return Some(PipeRead::Eof),
            Ok(_) => continue,
            Err(error) => return Some(classify_pipe_error(error.raw_os_error())),
        }
    }
}

#[cfg(not(any(windows, unix)))]
fn wait_for_pipe(_value: &str) -> Option<PipeRead> {
    None
}

/// Block until the parent process is confirmed dead.
#[cfg(windows)]
fn wait_for_parent_pid(pid: u32) -> bool {
    poll_until_gone(pid)
}

/// A single liveness probe.
///
/// `OpenProcess` failing with anything other than `ERROR_INVALID_PARAMETER`
/// (no such process) proves nothing — `ERROR_ACCESS_DENIED` means the process
/// exists — so the probe reports "alive" and the caller keeps polling.  The
/// previous implementation blocked in `WaitForSingleObject` and treated
/// `WAIT_FAILED` as parent death, which killed a healthy host the moment the
/// handle lost `PROCESS_SYNCHRONIZE`.
#[cfg(windows)]
fn parent_process_gone(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // 87 == ERROR_INVALID_PARAMETER: the pid does not exist.
            return GetLastError() == 87;
        }
        let mut code = 0u32;
        let reported = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        reported != 0 && code != STILL_ACTIVE as u32
    }
}

#[cfg(windows)]
fn poll_until_gone(pid: u32) -> bool {
    loop {
        if parent_process_gone(pid) {
            return true;
        }
        thread::sleep(std::time::Duration::from_millis(250));
    }
}

#[cfg(unix)]
fn wait_for_parent_pid(pid: u32) -> bool {
    loop {
        if parent_process_gone(pid) {
            return true;
        }
        thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// `kill(pid, 0)` distinguishes the cases by `errno`: `ESRCH` (3) means the
/// process is gone, `EPERM` (1) means it exists but is not ours to signal.
#[cfg(unix)]
fn parent_process_gone(pid: u32) -> bool {
    unsafe {
        if libc::kill(pid as i32, 0) == 0 {
            return false;
        }
        *libc::__errno_location() == 3
    }
}

#[cfg(not(any(windows, unix)))]
fn wait_for_parent_pid(_pid: u32) -> bool {
    // Without a liveness primitive the host must not self-terminate.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unusable_pipe_never_ends_the_host_without_a_pid_verdict() {
        // Regression: the shipped watcher returned "parent gone" for every read
        // error, so a handle that was never inherited killed the host at once.
        assert!(!resolve_parent_liveness(Some(PipeRead::Unusable), None, |_| {
            panic!("must not probe without a pid")
        }));
        assert!(!resolve_parent_liveness(
            Some(PipeRead::Unusable),
            Some(4242),
            |_| false
        ));
        assert!(resolve_parent_liveness(
            Some(PipeRead::Unusable),
            Some(4242),
            |_| true
        ));
        assert!(!resolve_parent_liveness(None, None, |_| {
            panic!("must not probe without a pid")
        }));
    }

    #[test]
    fn a_pipe_eof_always_ends_the_host() {
        assert!(resolve_parent_liveness(Some(PipeRead::Eof), None, |_| false));
        assert!(resolve_parent_liveness(
            Some(PipeRead::Eof),
            Some(1),
            |_| panic!("eof is already conclusive")
        ));
    }

    #[test]
    fn only_peer_closed_codes_count_as_parent_death() {
        #[cfg(windows)]
        for code in [109, 232, 233] {
            assert_eq!(classify_pipe_error(Some(code)), PipeRead::Eof, "{code}");
        }
        #[cfg(unix)]
        for code in [32, 107, 108] {
            assert_eq!(classify_pipe_error(Some(code)), PipeRead::Eof, "{code}");
        }
        // ERROR_INVALID_HANDLE (6) and ERROR_ACCESS_DENIED (5) prove nothing.
        assert_eq!(classify_pipe_error(Some(6)), PipeRead::Unusable);
        assert_eq!(classify_pipe_error(Some(5)), PipeRead::Unusable);
        assert_eq!(classify_pipe_error(None), PipeRead::Unusable);
    }

    #[test]
    fn an_absent_handle_is_not_reported_as_a_pipe_at_all() {
        assert!(wait_for_pipe("").is_none());
        assert!(wait_for_pipe("not-a-number").is_none());
        assert!(wait_for_pipe("0").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn a_dead_handle_is_unusable_rather_than_parent_death() {
        // INVALID_HANDLE_VALUE is rejected by ReadFile with
        // ERROR_INVALID_HANDLE (6) immediately, so this cannot block: the
        // watcher must classify it as "no evidence" instead of "parent gone".
        let dead = usize::MAX;
        assert_eq!(
            wait_for_pipe(&dead.to_string()),
            Some(PipeRead::Unusable)
        );
    }
}
