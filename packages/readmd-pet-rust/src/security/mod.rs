use crate::error::{HostError, HostResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Largest single file the custom protocol will read into memory.
///
/// The installer caps the whole runtime tree at `MAX_EXPANDED_BYTES`
/// (1500 MiB, `runtime.py:45`); without a per-file cap a corrupted tree could
/// make the pet host allocate unboundedly from a renderer `fetch()`.
pub const MAX_SERVED_ASSET_BYTES: u64 = 256 * 1024 * 1024;

/// Native mirror of `RustPetRuntime._safe_name` (`runtime.py:104-120`).
///
/// The Python side validates every ZIP member and every walked relative path
/// with this rule before it will extract or hash anything.  It rejects NUL
/// bytes, alternate separators, leading `/` or `~`, *drive-relative* first
/// components (`C:foo`, which Windows resolves against the current drive),
/// absolute paths and `.`/`..` components.  Rust's `Path::components()` collapses
/// a mid-path `.` exactly like `pathlib` does, so both languages agree.
pub fn is_safe_relative_path(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') || name.contains('\\') {
        return false;
    }
    if name.starts_with('/') || name.starts_with('~') {
        return false;
    }
    // A first component that looks like `C:foo` is drive-relative, not a plain
    // directory name, even though `Path::is_absolute()` reports false for it.
    if let Some(first) = name.split('/').next() {
        let bytes = first.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' {
            return false;
        }
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return false;
    }
    // `_safe_name` inherits pathlib's collapsing of a mid-path `.`, so Python
    // accepts `./a`; the host is deliberately stricter.  A request path that
    // still contains `.` has already escaped URL normalisation, and Chromium
    // never sends one, so refusing it costs nothing and removes an ambiguity
    // between the two `.`-collapsing implementations.
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return false;
    }
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

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

    /// Same as [`AssetSandbox::new`] but tolerant of an absent directory, which
    /// is normal for optional runtime siblings such as `models/`.
    pub fn try_new(root: impl AsRef<Path>) -> Option<Self> {
        match Self::new(root) {
            Ok(sandbox) => Some(sandbox),
            Err(_) => None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve `relative` inside the sandbox or explain why it is refused.
    pub fn resolve(&self, relative: &str) -> HostResult<PathBuf> {
        if !is_safe_relative_path(relative) {
            return Err(HostError::UnsafeAssetPath);
        }
        let candidate = self.root.join(relative);
        // `runtime.py:_verify_tree` refuses a symlinked member outright, so the
        // host must not follow one either: `canonicalize` alone would happily
        // resolve a planted link out of the sandbox.
        let link = fs::symlink_metadata(&candidate).map_err(HostError::Io)?;
        if link.file_type().is_symlink() {
            return Err(HostError::UnsafeAssetPath);
        }
        let canonical = candidate.canonicalize().map_err(HostError::Io)?;
        if !canonical.starts_with(&self.root) {
            return Err(HostError::UnsafeAssetPath);
        }
        if !canonical.is_file() || canonical.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > MAX_SERVED_ASSET_BYTES {
            return Err(HostError::UnsafeAssetPath);
        }
        Ok(canonical)
    }

    /// Verify a file against a manifest digest before it is trusted.
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

/// Which sandbox a renderer request belongs to.
///
/// The shipped runtime keeps large Live2D models and the Cubism vendor runtime
/// as *siblings* of `renderer/` rather than inside it, so the protocol needs
/// more than one root — but it must never become a general filesystem server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetRoute {
    Bundle(String),
    /// `(sibling, path relative to that sibling)`
    Sibling(&'static str, String),
    /// `(sibling, path relative to the bundle, path relative to the sibling)`.
    /// `assets/x.png` is `renderer/assets/x.png` first and only then the
    /// package-level `assets/x.png`; a bare `index.html` is
    /// `renderer/index.html` first and the package `assets/index.html` second.
    BundleThenSibling(&'static str, String, String),
}

/// Sibling directories the renderer bundle is allowed to reach.
pub const ASSET_SIBLINGS: &[&str] = &["vendor", "models", "assets"];

/// Pure half of the custom-protocol routing: decide which root answers a
/// percent-decoded request path.
pub fn route_asset(relative: &str) -> Result<AssetRoute, &'static str> {
    if !is_safe_relative_path(relative) {
        return Err("unsafe_asset_path");
    }
    for prefix in ASSET_SIBLINGS {
        let needle = format!("{prefix}/");
        if let Some(rest) = relative.strip_prefix(needle.as_str()) {
            let rest = rest.to_string();
            if rest.is_empty() {
                return Err("unsafe_asset_path");
            }
            return Ok(match *prefix {
                "vendor" | "models" => AssetRoute::Sibling(prefix, rest),
                // `assets/` prefers the bundle copy so a renderer can ship an
                // override for a package-level file.
                _ => AssetRoute::BundleThenSibling(
                    prefix,
                    relative.to_string(),
                    rest.clone(),
                ),
            });
        }
    }
    // A bare filename is a bundle file, falling back to the package `assets/`
    // directory for the shared fonts/images the install layout keeps outside
    // `renderer/`.
    Ok(AssetRoute::BundleThenSibling(
        "assets",
        relative.to_string(),
        relative.to_string(),
    ))
}

/// Resolve renderer asset requests against the verified runtime layout.
#[derive(Debug, Clone)]
pub struct RendererAssetRouter {
    bundle: AssetSandbox,
    package_root: PathBuf,
}

impl RendererAssetRouter {
    pub fn new(renderer_root: &Path) -> HostResult<Self> {
        let bundle = AssetSandbox::new(renderer_root)?;
        let package_root = bundle
            .root()
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| bundle.root().to_path_buf());
        Ok(Self {
            bundle,
            package_root,
        })
    }

    pub fn serve(&self, relative: &str) -> HostResult<PathBuf> {
        let route = route_asset(relative).map_err(|_| HostError::UnsafeAssetPath)?;
        self.resolve_route(&route)
    }

    fn resolve_route(&self, route: &AssetRoute) -> HostResult<PathBuf> {
        match route {
            AssetRoute::Bundle(rest) => self.bundle.resolve(rest),
            AssetRoute::Sibling(prefix, rest) => self.sibling(prefix)?.resolve(rest),
            AssetRoute::BundleThenSibling(prefix, bundle_path, sibling_path) => {
                match self.bundle.resolve(bundle_path) {
                    Ok(path) => Ok(path),
                    Err(_) => self.sibling(prefix)?.resolve(sibling_path),
                }
            }
        }
    }

    fn sibling(&self, prefix: &str) -> HostResult<AssetSandbox> {
        if !ASSET_SIBLINGS.contains(&prefix) {
            return Err(HostError::UnsafeAssetPath);
        }
        AssetSandbox::new(self.package_root.join(prefix))
    }
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

    #[test]
    fn sandbox_rejects_absolute_and_alternate_separator_paths() {
        let root = temp_tree("shape");
        fs::write(root.join("ok.txt"), b"ok").unwrap();
        let sandbox = AssetSandbox::new(&root).unwrap();
        assert!(sandbox.resolve("/etc/passwd").is_err());
        assert!(sandbox.resolve("..\\outside").is_err());
        assert!(sandbox.resolve("ok.txt").is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn relative_path_rule_matches_python_safe_name() {
        // Cases read straight off `runtime.py:104-120`.
        for accepted in ["index.html", "renderer/index.html", "a/b/c.png"] {
            assert!(is_safe_relative_path(accepted), "{accepted}");
        }
        for refused in [
            "",
            "x\0y",
            "assets\\bundle.js",
            "/etc/passwd",
            "~/.ssh/id_ed25519",
            "C:foo/bar",
            "C:/Windows/win.ini",
            ".",
            "..",
            "./a",
            "a/./b",
            "../secrets",
            "a/../../b",
            "a/../b",
        ] {
            assert!(!is_safe_relative_path(refused), "{refused}");
        }
    }

    #[test]
    fn routing_prefers_the_bundle_and_only_reaches_named_siblings() {
        assert_eq!(
            route_asset("index.html").unwrap(),
            AssetRoute::BundleThenSibling("assets", "index.html".into(), "index.html".into())
        );
        assert_eq!(
            route_asset("vendor/cubm-core.js").unwrap(),
            AssetRoute::Sibling("vendor", "cubm-core.js".into())
        );
        assert_eq!(
            route_asset("models/dragon/dragon.model3.json").unwrap(),
            AssetRoute::Sibling("models", "dragon/dragon.model3.json".into())
        );
        assert_eq!(
            route_asset("assets/sheet.png").unwrap(),
            AssetRoute::BundleThenSibling("assets", "assets/sheet.png".into(), "sheet.png".into())
        );
        assert_eq!(route_asset("vendor/"), Err("unsafe_asset_path"));
        assert_eq!(route_asset("../../etc/passwd"), Err("unsafe_asset_path"));
        assert_eq!(route_asset("~/.ssh/id"), Err("unsafe_asset_path"));
        assert_eq!(route_asset("C:Windows/win.ini"), Err("unsafe_asset_path"));
    }

    #[test]
    fn router_serves_the_shipped_runtime_layout_and_nothing_else() {
        let package = temp_tree("layout");
        let renderer = package.join("renderer");
        fs::create_dir_all(renderer.join("assets")).unwrap();
        fs::create_dir_all(package.join("vendor")).unwrap();
        fs::create_dir_all(package.join("models")).unwrap();
        fs::create_dir_all(package.join("assets")).unwrap();
        fs::write(renderer.join("index.html"), b"<html>").unwrap();
        fs::write(renderer.join("assets").join("local.png"), b"local").unwrap();
        fs::write(package.join("assets").join("local.png"), b"shadow").unwrap();
        fs::write(package.join("assets").join("shared.svg"), b"shared").unwrap();
        fs::write(package.join("vendor").join("cubm.js"), b"vendor").unwrap();
        fs::write(package.join("models").join("m.model3.json"), b"model").unwrap();
        fs::write(package.join("secret.txt"), b"nope").unwrap();

        let router = RendererAssetRouter::new(&renderer).unwrap();
        assert_eq!(
            router.serve("index.html").unwrap().file_name().unwrap(),
            "index.html"
        );
        // The bundle copy wins over the package copy.
        assert_eq!(fs::read(router.serve("assets/local.png").unwrap()).unwrap(), b"local");
        assert_eq!(fs::read(router.serve("assets/shared.svg").unwrap()).unwrap(), b"shared");
        assert_eq!(fs::read(router.serve("vendor/cubm.js").unwrap()).unwrap(), b"vendor");
        assert_eq!(
            fs::read(router.serve("models/m.model3.json").unwrap()).unwrap(),
            b"model"
        );
        // A bare name is not allowed to climb out of renderer/ into the package.
        assert!(router.serve("secret.txt").is_err());
        assert!(router.serve("../secret.txt").is_err());
        assert!(router.serve("vendor/../secret.txt").is_err());
        assert!(router.serve("assets/../../../../secret.txt").is_err());
        let _ = fs::remove_dir_all(package);
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_refuses_to_follow_a_planted_symlink() {
        let root = temp_tree("symlink");
        let outside = temp_tree("outside");
        fs::write(outside.join("secret"), b"secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), root.join("link")).unwrap();
        let sandbox = AssetSandbox::new(&root).unwrap();
        assert!(sandbox.resolve("link").is_err());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    fn temp_tree(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "readmd-sandbox-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
