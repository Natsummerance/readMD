//! Cooperative cancellation for long requests (export, single-file convert).
//!
//! A request that carries an optional `task_id` registers a flag here for its
//! lifetime; `POST /api/task/cancel {"id": …}` raises it.  Work checks the flag
//! at its safe points and, for exports, once more right before the only commit
//! (`rename` of `<name>.readmd-part` onto the target), so a cancel that wins the
//! race never leaves a file behind and one that loses it reports `finished`.
//!
//! Batch conversion keeps its own job table (`batch2::CONVERT_JOBS`); the
//! cancel route forwards unknown ids there so one endpoint covers both.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const RUNNING: u8 = 0;
const COMMITTED: u8 = 1;

#[derive(Default)]
struct Entry {
    cancel: AtomicBool,
    phase: AtomicU8,
}

fn table() -> &'static Mutex<HashMap<String, Arc<Entry>>> {
    static T: OnceLock<Mutex<HashMap<String, Arc<Entry>>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// What `cancel` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelState {
    /// The flag was raised before the commit point.
    Cancelling,
    /// The task already committed its result (or finished).
    Finished,
    /// No task with this id is registered.
    Unknown,
}

impl CancelState {
    pub fn as_str(self) -> &'static str {
        match self {
            CancelState::Cancelling => "cancelling",
            CancelState::Finished => "finished",
            CancelState::Unknown => "unknown",
        }
    }
}

/// A registered task; removes itself from the table when dropped.
pub struct TaskGuard {
    id: String,
    entry: Arc<Entry>,
}

impl TaskGuard {
    pub fn is_cancelled(&self) -> bool {
        self.entry.cancel.load(Ordering::SeqCst)
    }

    /// Enter the commit point.  Returns `false` (and commits nothing) when the
    /// task was cancelled first; afterwards `cancel` answers `Finished`.
    pub fn try_commit(&self) -> bool {
        // The phase flip and the cancel check are ordered through the table
        // lock so a concurrent `cancel` sees exactly one of the two outcomes.
        let _lock = table().lock().unwrap_or_else(|e| e.into_inner());
        if self.entry.cancel.load(Ordering::SeqCst) {
            return false;
        }
        self.entry.phase.store(COMMITTED, Ordering::SeqCst);
        true
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        let mut map = table().lock().unwrap_or_else(|e| e.into_inner());
        if map.get(&self.id).is_some_and(|e| Arc::ptr_eq(e, &self.entry)) {
            map.remove(&self.id);
        }
    }
}

/// Accept only short, printable ids (the frontend sends `crypto.randomUUID()`).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Register `id`; `None` for an empty or malformed id (the request then runs
/// uncancellable, exactly as before `task_id` existed).
pub fn register(id: &str) -> Option<TaskGuard> {
    if !valid_id(id) {
        return None;
    }
    let entry = Arc::new(Entry::default());
    entry.phase.store(RUNNING, Ordering::SeqCst);
    table().lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), entry.clone());
    Some(TaskGuard { id: id.to_string(), entry })
}

/// Raise the cancel flag of a registered task.
pub fn cancel(id: &str) -> CancelState {
    let map = table().lock().unwrap_or_else(|e| e.into_inner());
    match map.get(id) {
        None => CancelState::Unknown,
        Some(e) if e.phase.load(Ordering::SeqCst) == COMMITTED => CancelState::Finished,
        Some(e) => {
            e.cancel.store(true, Ordering::SeqCst);
            CancelState::Cancelling
        }
    }
}

/// `<dir>/<name>.readmd-part` next to `target`.
pub fn part_path(target: &std::path::Path) -> std::path::PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "export".into());
    target.with_file_name(format!("{name}.readmd-part"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn register_cancel_and_drop() {
        let g = register("t-register-1").unwrap();
        assert!(!g.is_cancelled());
        assert_eq!(cancel("t-register-1"), CancelState::Cancelling);
        assert!(g.is_cancelled());
        assert!(!g.try_commit(), "a cancelled task must not commit");
        drop(g);
        assert_eq!(cancel("t-register-1"), CancelState::Unknown);
    }

    #[test]
    fn cancel_after_commit_reports_finished() {
        let g = register("t-commit-1").unwrap();
        assert!(g.try_commit());
        assert_eq!(cancel("t-commit-1"), CancelState::Finished);
        assert!(!g.is_cancelled());
    }

    #[test]
    fn bad_ids_are_not_registered() {
        for id in ["", "a b", "../x", &"x".repeat(129)] {
            assert!(register(id).is_none(), "{id:?}");
        }
        assert_eq!(cancel("never-registered"), CancelState::Unknown);
    }

    #[test]
    fn re_registering_an_id_keeps_the_newest() {
        let old = register("t-dup").unwrap();
        let new = register("t-dup").unwrap();
        drop(old); // must not remove the newer entry
        assert_eq!(cancel("t-dup"), CancelState::Cancelling);
        assert!(new.is_cancelled());
    }

    #[test]
    fn part_path_sits_next_to_the_target() {
        assert_eq!(part_path(Path::new("/a/b/doc.pdf")), Path::new("/a/b/doc.pdf.readmd-part"));
    }
}
