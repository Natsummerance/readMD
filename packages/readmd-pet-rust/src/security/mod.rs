use crate::error::{HostError, HostResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Constrain every renderer/model lookup to the verified runtime directory.
#[derive(Debug, Clone)]
pub struct AssetSandbox {
    root: PathBuf,
}

impl AssetSandbox {
    pub fn new(root: impl AsRef<Path>) -> HostResult<Self> {
        let root = root.as_ref().canonicalize().map_err(HostError::Io)?;
        if !root.is_dir() {
            return Err(HostError::UnsafeAssetPath);
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn resolve(&self, relative: &str) -> HostResult<PathBuf> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(HostError::UnsafeAssetPath);
        }
        let candidate = self.root.join(path);
        let canonical = candidate.canonicalize().map_err(HostError::Io)?;
        if !canonical.starts_with(&self.root) {
            return Err(HostError::UnsafeAssetPath);
        }
        Ok(canonical)
    }

    pub fn verify_digest(path: &Path, expected: &str, max_bytes: u64) -> HostResult<()> {
        let metadata = fs::symlink_metadata(path).map_err(HostError::Io)?;
        if !metadata.is_file() || metadata.len() > max_bytes {
            return Err(HostError::UnsafeAssetPath);
        }
        let mut digest = Sha256::new();
        let file = fs::File::open(path).map_err(HostError::Io)?;
        std::io::copy(
            &mut file.take(max_bytes.saturating_add(1)),
            &mut digest_sink(&mut digest),
        )
        .map_err(HostError::Io)?;
        let actual = format!("{:x}", digest.finalize());
        if actual != expected {
            return Err(HostError::UnsafeAssetPath);
        }
        Ok(())
    }
}

struct DigestSink<'a>(&'a mut Sha256);
impl<'a> std::io::Write for DigestSink<'a> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn digest_sink<'a>(digest: &'a mut Sha256) -> DigestSink<'a> {
    DigestSink(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_rejects_parent_escape() {
        let root = std::env::temp_dir().join(format!("readmd-sandbox-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let sandbox = AssetSandbox::new(&root).unwrap();
        assert!(sandbox.resolve("../outside").is_err());
        let _ = fs::remove_dir_all(root);
    }
}
