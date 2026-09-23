use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static RUNTIME_GENERATION: OnceLock<u64> = OnceLock::new();

fn runtime_generation() -> u64 {
    *RUNTIME_GENERATION.get_or_init(|| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos() as u64)
            .unwrap_or_default();
        timestamp ^ (std::process::id() as u64).rotate_left(17)
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthState {
    pub engine: &'static str,
    pub state: String,
    pub renderer: String,
    pub code: String,
    pub pid: u32,
    pub updated_at: u128,
    pub engine_generation: u64,
    /// Stable for this host process and different from the navigation
    /// generation.  Python uses it to distinguish a replacement process from
    /// a stale `ready` report left by a crashed predecessor.
    pub runtime_generation: u64,
    pub protocol_version: u32,
}

#[derive(Debug, Clone)]
pub struct HealthWriter {
    path: PathBuf,
    /// Consecutive `write` failures.  Health is the only channel the app has to
    /// see this process, so a host that cannot publish it is already being torn
    /// down as `rust_health_timeout` by `RustPetRuntime` (`runtime.py:508-536`);
    /// the loop uses this counter to stop rendering an uncontrollable overlay.
    unwritable: std::cell::Cell<u32>,
}

impl HealthWriter {
    pub fn new(bridge_file: impl AsRef<Path>) -> Self {
        Self {
            path: PathBuf::from(format!(
                "{}.rust.health.json",
                bridge_file.as_ref().display()
            )),
            unwritable: std::cell::Cell::new(0),
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// How many `write` calls have failed in a row.
    pub fn consecutive_failures(&self) -> u32 {
        self.unwritable.get()
    }
    pub fn write(&self, state: HealthState) -> Result<(), String> {
        match self.commit(&state) {
            Ok(()) => {
                self.unwritable.set(0);
                Ok(())
            }
            Err(error) => {
                self.unwritable.set(self.unwritable.get().saturating_add(1));
                eprintln!("readmd-pet: health report failed ({error})");
                Err(error)
            }
        }
    }

    fn commit(&self, state: &HealthState) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "health_parent_missing".to_string())?;
        fs::create_dir_all(parent).map_err(|error| format!("health_mkdir:{error}"))?;
        let bytes =
            serde_json::to_vec(state).map_err(|error| format!("health_encode:{error}"))?;
        let temp = self.path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|error| format!("health_temp:{error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("health_write:{error}"))?;
        file.sync_all()
            .map_err(|error| format!("health_sync:{error}"))?;
        fs::rename(&temp, &self.path).map_err(|error| format!("health_commit:{error}"))?;
        Ok(())
    }

    pub fn new_state(
        state: impl Into<String>,
        renderer: impl Into<String>,
        code: impl Into<String>,
        generation: u64,
    ) -> HealthState {
        HealthState {
            engine: "rust",
            state: state.into(),
            renderer: renderer.into(),
            code: code.into(),
            pid: std::process::id(),
            updated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_millis())
                .unwrap_or_default(),
            engine_generation: generation,
            runtime_generation: runtime_generation(),
            protocol_version: crate::protocol::PROTOCOL_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn health_writer_replaces_complete_json_at_high_frequency() {
        let root = std::env::temp_dir().join(format!(
            "readmd-pet-health-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("bridge.json");
        let writer = HealthWriter::new(&path);
        let reader_path = writer.path().to_path_buf();
        let reader = thread::spawn(move || {
            for _ in 0..2_000 {
                if let Ok(bytes) = fs::read(&reader_path) {
                    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(value["engine"], "rust");
                }
                thread::yield_now();
            }
        });
        for generation in 0..2_000 {
            writer
                .write(HealthWriter::new_state(
                    if generation % 2 == 0 {
                        "loading"
                    } else {
                        "ready"
                    },
                    "hermes-sprite",
                    "ok",
                    generation,
                ))
                .unwrap();
        }
        reader.join().unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(writer.path()).unwrap()).unwrap();
        assert_eq!(value["engine"], "rust");
        assert!(value["runtime_generation"].as_u64().unwrap() > 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_unpublishable_health_report_is_reported_not_swallowed() {
        // `RustPetRuntime` decides the host is healthy by reading this file.
        // If the atomic commit cannot land, the host must know it: the old code
        // kept rendering a pet nobody could control while `available()` still
        // saw the previous `ready` report until it timed out.
        let root = std::env::temp_dir().join(format!(
            "readmd-pet-health-blocked-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let writer = HealthWriter::new(root.join("bridge.json"));
        fs::create_dir_all(writer.path()).unwrap();

        let error = writer
            .write(HealthWriter::new_state("ready", "hermes-sprite", "ok", 1))
            .expect_err("a directory cannot be replaced by the health report");
        assert!(
            error.starts_with("health_commit:"),
            "unexpected health error: {error}"
        );
        assert_eq!(writer.consecutive_failures(), 1);

        // Clearing the obstruction must restore reporting and reset the count.
        fs::remove_dir_all(writer.path()).unwrap();
        writer
            .write(HealthWriter::new_state("ready", "hermes-sprite", "ok", 2))
            .unwrap();
        assert_eq!(writer.consecutive_failures(), 0);
        let _ = fs::remove_dir_all(root);
    }
}
