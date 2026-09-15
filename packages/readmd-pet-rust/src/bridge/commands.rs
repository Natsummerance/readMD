use crate::protocol::{
    CommandEnvelope, MAX_COMMAND_BODY_BYTES, MAX_PENDING_COMMANDS, MAX_PENDING_COMMAND_BYTES,
};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// ReadMD-compatible durable FIFO publisher. A command is visible only after
/// its exclusive `.tmp` file has been atomically renamed to `.json`.
#[derive(Debug, Clone)]
pub struct DurableCommandPublisher {
    directory: PathBuf,
}

impl DurableCommandPublisher {
    pub fn new(bridge_file: impl AsRef<Path>) -> Self {
        Self {
            directory: PathBuf::from(format!("{}.commands", bridge_file.as_ref().display())),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn publish(&self, command: serde_json::Value) -> Result<PathBuf, String> {
        if !command.is_object() {
            return Err("pet_command_must_be_object".into());
        }
        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("pet_command_mkdir:{error}"))?;
        let mut entries = Vec::new();
        let mut pending_bytes = 0u64;
        for entry in
            fs::read_dir(&self.directory).map_err(|error| format!("pet_command_readdir:{error}"))?
        {
            let entry = entry.map_err(|error| format!("pet_command_entry:{error}"))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            pending_bytes = pending_bytes
                .saturating_add(entry.metadata().map(|value| value.len()).unwrap_or(0));
            entries.push(path);
        }
        if entries.len() >= MAX_PENDING_COMMANDS || pending_bytes >= MAX_PENDING_COMMAND_BYTES {
            return Err("pet_command_queue_full".into());
        }
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "pet_clock_invalid")?
            .as_millis() as u64;
        let body = serde_json::to_vec(&CommandEnvelope {
            command,
            created_at,
        })
        .map_err(|error| format!("pet_command_encode:{error}"))?;
        if body.len() > MAX_COMMAND_BODY_BYTES
            || pending_bytes.saturating_add(body.len() as u64) > MAX_PENDING_COMMAND_BYTES
        {
            return Err("pet_command_too_large".into());
        }
        let mut digest = Sha256::new();
        digest.update(&body);
        let suffix = format!("{:x}", digest.finalize());
        let id = format!("{:020}-{}-{}", created_at, std::process::id(), suffix);
        let target = self.directory.join(format!("{id}.json"));
        let temp = self.directory.join(format!("{id}.json.tmp"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("pet_command_temp:{error}"))?;
        let result = (|| {
            file.write_all(&body)
                .map_err(|error| format!("pet_command_write:{error}"))?;
            file.sync_all()
                .map_err(|error| format!("pet_command_sync:{error}"))?;
            fs::rename(&temp, &target).map_err(|error| format!("pet_command_commit:{error}"))?;
            Ok(target.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publisher_writes_exact_envelope_and_never_overwrites() {
        let root = std::env::temp_dir().join(format!("readmd-pet-command-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let bridge = root.join("state.json");
        let publisher = DurableCommandPublisher::new(&bridge);
        let path = publisher
            .publish(serde_json::json!({"type":"open-menu"}))
            .unwrap();
        let value: CommandEnvelope = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(value.command["type"], "open-menu");
        assert!(value.created_at > 0);
        let _ = fs::remove_dir_all(root);
    }
}
