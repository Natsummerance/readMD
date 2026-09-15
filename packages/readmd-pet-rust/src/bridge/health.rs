use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct HealthState {
    pub engine: &'static str,
    pub state: String,
    pub renderer: String,
    pub code: String,
    pub pid: u32,
    pub updated_at: u128,
    pub engine_generation: u64,
    pub protocol_version: u32,
}

#[derive(Debug, Clone)]
pub struct HealthWriter {
    path: PathBuf,
}

impl HealthWriter {
    pub fn new(bridge_file: impl AsRef<Path>) -> Self {
        Self {
            path: PathBuf::from(format!(
                "{}.rust.health.json",
                bridge_file.as_ref().display()
            )),
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn write(&self, state: HealthState) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "health_parent_missing".to_string())?;
        fs::create_dir_all(parent).map_err(|error| format!("health_mkdir:{error}"))?;
        let bytes = serde_json::to_vec(&state).map_err(|error| format!("health_encode:{error}"))?;
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
            protocol_version: crate::protocol::PROTOCOL_VERSION,
        }
    }
}
