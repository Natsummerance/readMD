use crate::protocol::{parse_snapshot, PetSnapshot, MAX_SNAPSHOT_BYTES};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct SnapshotUpdate {
    pub snapshot: PetSnapshot,
    pub signature: String,
}

/// Reads the ReadMD snapshot without allowing a malformed write to suppress a
/// later repaired write. The signature is updated only after JSON validation.
pub struct SnapshotReader {
    path: PathBuf,
    signature: Option<String>,
}

impl SnapshotReader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            signature: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&mut self) -> Result<Option<SnapshotUpdate>, String> {
        let metadata = match fs::metadata(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("snapshot_stat_failed:{error}")),
        };
        if !metadata.is_file() {
            return Err("invalid_pet_snapshot_file".into());
        }
        if metadata.len() > MAX_SNAPSHOT_BYTES as u64 {
            return Err("pet_snapshot_too_large".into());
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let created = metadata
            .created()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let bytes =
            fs::read(&self.path).map_err(|error| format!("snapshot_read_failed:{error}"))?;
        // Include a content digest as a fallback for filesystems with coarse
        // timestamps; this also distinguishes two atomic renames in one tick.
        let mut digest = Sha256::new();
        digest.update(&bytes);
        let digest = format!("{:x}", digest.finalize());
        let signature = format!(
            "{}:{}:{}:{}:{}",
            metadata.len(),
            modified,
            created,
            metadata.permissions().readonly(),
            digest
        );
        if self.signature.as_deref() == Some(signature.as_str()) {
            return Ok(None);
        }
        let snapshot = parse_snapshot(&bytes).map_err(str::to_string)?;
        self.signature = Some(signature.clone());
        Ok(Some(SnapshotUpdate {
            snapshot,
            signature,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn malformed_snapshot_does_not_poison_signature() {
        let dir = tempfile_dir();
        let path = dir.join("state.json");
        fs::write(&path, br#"{"format_version":1,"visible":true}"#).unwrap();
        let mut reader = SnapshotReader::new(&path);
        assert!(reader.read().unwrap().is_some());
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{").unwrap();
        assert!(reader.read().is_err());
        fs::write(&path, br#"{"format_version":1,"visible":false}"#).unwrap();
        assert!(reader.read().unwrap().is_some());
    }

    fn tempfile_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("readmd-pet-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&path);
        path
    }
}
