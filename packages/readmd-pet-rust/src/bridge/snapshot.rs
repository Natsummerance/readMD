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

    #[test]
    fn reader_rejects_all_invalid_shapes_and_size_limits() {
        let dir = tempfile_dir();
        let path = dir.join("invalid-state.json");
        let invalid = [
            b"".as_slice(),
            b"{".as_slice(),
            br#"[]"#.as_slice(),
            br#"null"#.as_slice(),
            br#"{}"#.as_slice(),
            br#"{"format_version":2}"#.as_slice(),
            br#"{"format_version":1,"bounds":{"x":0,"y":0,"width":1,"height":1}}"#.as_slice(),
        ];
        for bytes in invalid {
            fs::write(&path, bytes).unwrap();
            let mut reader = SnapshotReader::new(&path);
            assert!(
                reader.read().is_err(),
                "input should be rejected: {bytes:?}"
            );
        }
        fs::write(&path, vec![b'x'; MAX_SNAPSHOT_BYTES + 1]).unwrap();
        let mut reader = SnapshotReader::new(&path);
        assert_eq!(reader.read().unwrap_err(), "pet_snapshot_too_large");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reader_consumes_high_frequency_atomic_replacements_without_partial_json() {
        let dir = tempfile_dir();
        let path = dir.join("atomic-state.json");
        let mut reader = SnapshotReader::new(&path);
        for generation in 1..=10_000u64 {
            let temp = path.with_extension("tmp");
            let bytes =
                format!(r#"{{"format_version":1,"generation":{generation},"visible":true}}"#);
            fs::write(&temp, bytes).unwrap();
            // `rename` replaces an existing destination on Unix but not on
            // Windows.  The production Python writer uses ReplaceFile/os
            // replace; remove only the prior test file here before renaming.
            let _ = fs::remove_file(&path);
            fs::rename(&temp, &path).unwrap();
            let update = reader.read().unwrap().expect("new atomic snapshot");
            assert_eq!(update.snapshot.generation, generation);
        }
        assert!(reader.read().unwrap().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    fn tempfile_dir() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let path =
            std::env::temp_dir().join(format!("readmd-pet-test-{}-{nonce}", std::process::id()));
        let _ = fs::create_dir_all(&path);
        path
    }
}
