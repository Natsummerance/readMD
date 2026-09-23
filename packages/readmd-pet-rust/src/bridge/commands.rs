use crate::protocol::{
    CommandEnvelope, MAX_COMMAND_BODY_BYTES, MAX_PENDING_COMMANDS, MAX_PENDING_COMMAND_BYTES,
};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

static COMMAND_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// ReadMD-compatible durable FIFO publisher. A command is visible only after
/// its exclusive `.tmp` file has been atomically renamed to `.json`.
#[derive(Debug, Clone)]
pub struct DurableCommandPublisher {
    directory: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl DurableCommandPublisher {
    pub fn new(bridge_file: impl AsRef<Path>) -> Self {
        Self {
            directory: PathBuf::from(format!("{}.commands", bridge_file.as_ref().display())),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn publish(&self, command: serde_json::Value) -> Result<PathBuf, String> {
        // Queue accounting and the exclusive temp/rename sequence must be a
        // single operation.  Without this lock concurrent renderer callbacks
        // could both pass the limit check or derive the same millisecond ID.
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "pet_command_lock_poisoned".to_string())?;
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
        let sequence = COMMAND_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let id = format!(
            "{:020}-{}-{:016x}-{}",
            created_at,
            std::process::id(),
            sequence,
            suffix
        );
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
    use std::collections::HashSet;
    use std::sync::Arc;

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

    #[test]
    fn concurrent_publishers_have_unique_fifo_names_and_no_temp_files() {
        let root = std::env::temp_dir().join(format!(
            "readmd-pet-command-stress-{}-{}",
            std::process::id(),
            COMMAND_SEQUENCE.load(Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        let publisher = Arc::new(DurableCommandPublisher::new(root.join("state.json")));
        let workers = (0..4)
            .map(|_| {
                let publisher = Arc::clone(&publisher);
                std::thread::spawn(move || {
                    (0..32)
                        .map(|_| publisher.publish(serde_json::json!({"type":"open-menu"})))
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect::<Vec<_>>();
        let paths = workers
            .into_iter()
            .flat_map(|worker| {
                worker
                    .join()
                    .expect("worker panicked")
                    .expect("publish failed")
            })
            .collect::<Vec<_>>();
        let names = paths
            .iter()
            .map(|path| path.file_name().unwrap().to_owned())
            .collect::<HashSet<_>>();
        assert_eq!(paths.len(), 128);
        assert_eq!(names.len(), 128);
        assert_eq!(fs::read_dir(publisher.directory()).unwrap().count(), 128);
        assert_eq!(
            fs::read_dir(publisher.directory())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("tmp"))
                .count(),
            0
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn publisher_enforces_queue_and_body_limits() {
        let root =
            std::env::temp_dir().join(format!("readmd-pet-command-limits-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let publisher = DurableCommandPublisher::new(root.join("state.json"));
        for index in 0..MAX_PENDING_COMMANDS {
            let path = publisher.directory().join(format!("{index:03}.json"));
            fs::create_dir_all(publisher.directory()).unwrap();
            fs::write(path, br#"{}"#).unwrap();
        }
        assert_eq!(
            publisher.publish(serde_json::json!({"type":"open-menu"})),
            Err("pet_command_queue_full".into())
        );
        let _ = fs::remove_dir_all(&root);

        let root =
            std::env::temp_dir().join(format!("readmd-pet-command-body-{}", std::process::id()));
        let publisher = DurableCommandPublisher::new(root.join("state.json"));
        let huge = "x".repeat(MAX_COMMAND_BODY_BYTES);
        assert_eq!(
            publisher.publish(serde_json::json!({"type":"open-menu", "data": huge})),
            Err("pet_command_too_large".into())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn publisher_supports_ten_thousand_consumed_commands_without_duplicates() {
        let root = std::env::temp_dir().join(format!(
            "readmd-pet-command-10k-{}-{}",
            std::process::id(),
            COMMAND_SEQUENCE.load(Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        let publisher = DurableCommandPublisher::new(root.join("state.json"));
        let mut names = HashSet::new();
        for _ in 0..10_000 {
            let path = publisher
                .publish(serde_json::json!({"type":"open-menu"}))
                .unwrap();
            assert!(names.insert(path.file_name().unwrap().to_owned()));
            let value: CommandEnvelope = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(value.command["type"], "open-menu");
            fs::remove_file(path).unwrap();
        }
        assert!(fs::read_dir(publisher.directory())
            .unwrap()
            .next()
            .is_none());
        let _ = fs::remove_dir_all(root);
    }
}
