//! `cargo xtask pet-package` — build the native ReadMD pet runtime package.
//!
//! Port of `packages/readmd-pet-rust/scripts/build-package.py`: build (or
//! reuse) the `readmd-pet-rust` executable for an explicit target, stage it
//! with the adapter's production renderer/assets/models/vendor payload,
//! record every byte in `runtime-manifest.json`, and write a deterministic
//! ZIP (sorted entries, fixed timestamp) for the managed Rust installer.
//!
//! Arguments: `[--platform windows|macos|linux] [--arch x86_64|aarch64]
//! [--skip-build] [--output DIR]`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub fn target_for(platform: &str, arch: &str) -> Option<&'static str> {
    Some(match (platform, arch) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        _ => return None,
    })
}

fn host_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn host_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    }
}

fn normalize_arch(raw: &str) -> &'static str {
    if matches!(raw.to_ascii_lowercase().as_str(), "arm64" | "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    }
}

pub fn package_version(cargo_toml: &str) -> String {
    let re = regex::Regex::new(r#"(?m)^version\s*=\s*"([^"]+)""#).expect("static regex");
    re.captures(cargo_toml).map(|c| c[1].to_string()).unwrap_or_else(|| "0.0.0".into())
}

pub fn role_of(relative: &str, executable: &str) -> &'static str {
    if relative == executable {
        "executable"
    } else if relative.starts_with("renderer/") {
        "renderer"
    } else if relative.starts_with("models/") {
        "model"
    } else if relative.starts_with("gnome-companion/") {
        "companion"
    } else {
        "asset"
    }
}

pub fn archive_name(platform: &str, arch: &str) -> String {
    if platform == "windows" && arch == "x86_64" {
        "ReadMD-Pet-Rust.zip".into()
    } else {
        format!("ReadMD-Pet-Rust-{platform}-{arch}.zip")
    }
}

fn copy_tree(source: &Path, dest: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!("required runtime asset directory is missing: {}", source.display()));
    }
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(source).map_err(|e| e.to_string())?;
        let to = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        } else if entry.file_type().is_file() {
            std::fs::copy(entry.path(), &to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    Ok(())
}

/// Sorted relative (`/`-separated) paths of every regular file under `root`.
fn files_sorted(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root).map_err(|e| e.to_string())?;
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push((rel, entry.path().to_path_buf()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Minimal deterministic ZIP writer (deflate, fixed 1980-01-01 timestamp,
/// Unix mode bits so the executable stays executable after extraction).
pub fn write_zip(entries: &[(String, Vec<u8>, u32)]) -> Result<Vec<u8>, String> {
    const DOS_TIME: u16 = 0;
    const DOS_DATE: u16 = (1 << 5) | 1; // 1980-01-01
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data, mode) in entries {
        let offset = u32::try_from(out.len()).map_err(|_| "archive exceeds 4 GiB")?;
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(data).map_err(|e| e.to_string())?;
        let packed = enc.finish().map_err(|e| e.to_string())?;
        let flags: u16 = if name.is_ascii() { 0 } else { 0x0800 };
        let (csize, usize_) = (packed.len() as u32, data.len() as u32);
        let header = |sig: u32, buf: &mut Vec<u8>, central: bool| {
            buf.extend_from_slice(&sig.to_le_bytes());
            if central {
                buf.extend_from_slice(&(3u16 << 8 | 20).to_le_bytes()); // made by: Unix
            }
            buf.extend_from_slice(&20u16.to_le_bytes());
            buf.extend_from_slice(&flags.to_le_bytes());
            buf.extend_from_slice(&8u16.to_le_bytes());
            buf.extend_from_slice(&DOS_TIME.to_le_bytes());
            buf.extend_from_slice(&DOS_DATE.to_le_bytes());
            buf.extend_from_slice(&crc.sum().to_le_bytes());
            buf.extend_from_slice(&csize.to_le_bytes());
            buf.extend_from_slice(&usize_.to_le_bytes());
            buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
        };
        header(0x0403_4b50, &mut out, false);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);
        header(0x0201_4b50, &mut central, true);
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&((0o100000 | mode) << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

fn flag_value(argv: &[String], name: &str) -> Result<Option<String>, String> {
    match argv.iter().position(|a| a == name) {
        Some(i) => argv.get(i + 1).cloned().map(Some).ok_or_else(|| format!("{name} needs a value")),
        None => Ok(None),
    }
}

pub fn main(root: &Path, argv: &[String]) -> Result<(), String> {
    let platform = flag_value(argv, "--platform")?.unwrap_or_else(|| host_platform().into());
    if !matches!(platform.as_str(), "windows" | "macos" | "linux") {
        return Err(format!("--platform must be windows, macos or linux (got {platform})"));
    }
    let arch = normalize_arch(&flag_value(argv, "--arch")?.unwrap_or_else(|| host_arch().into()));
    let target = target_for(&platform, arch).ok_or_else(|| format!("unsupported runtime target: {platform}/{arch}"))?;
    let skip_build = argv.iter().any(|a| a == "--skip-build");

    let crate_dir = root.join("packages").join("readmd-pet-rust");
    let adapter_dist = root.join("packages").join("readmd-hermes-pet-adapter").join("dist");
    let renderer = adapter_dist.join("renderer");

    if !skip_build {
        // Always name the target: a host-default build on a cross runner
        // would silently ship the wrong architecture.
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "--offline", "--release", "--manifest-path"])
            .arg(crate_dir.join("Cargo.toml"))
            .args(["--target", target])
            .current_dir(root)
            .status()
            .map_err(|e| format!("cargo build: {e}"))?;
        if !status.success() {
            return Err(format!("cargo build failed ({status})"));
        }
    }

    let exe_name = if platform == "windows" { "readmd-pet-rust.exe" } else { "readmd-pet-rust" };
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from).unwrap_or_else(|| crate_dir.join("target"));
    let mut executable = target_dir.join(target).join("release").join(exe_name);
    // Cargo's host release dir is only acceptable for a package matching the host.
    if !executable.is_file() && platform == host_platform() && arch == host_arch() {
        executable = target_dir.join("release").join(exe_name);
    }
    if !executable.is_file() {
        return Err(format!("built Rust executable is missing: {}", executable.display()));
    }
    if !renderer.join("index.html").is_file() {
        return Err(format!("renderer bundle is missing: {}", renderer.display()));
    }

    let output_dir = flag_value(argv, "--output")?.map(PathBuf::from).unwrap_or_else(|| crate_dir.join("dist"));
    let stage = output_dir.join(format!("ReadMD-Pet-Rust-{platform}-{arch}"));
    if stage.exists() {
        std::fs::remove_dir_all(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    }
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    std::fs::copy(&executable, stage.join(exe_name)).map_err(|e| e.to_string())?;
    // Compile ReadMD's tracked presentation, rather than shipping a stale dist
    // entry. Existing PIXI/Cubism chunks are reused without installing packages.
    let node = std::env::var("NODE").unwrap_or_else(|_| "node".into());
    let status = Command::new(node)
        .arg(crate_dir.join("scripts").join("build-renderer.mjs"))
        .arg("--renderer-cache").arg(&renderer)
        .arg("--output").arg(stage.join("renderer"))
        .current_dir(root).status().map_err(|e| format!("offline renderer build: {e}"))?;
    if !status.success() { return Err(format!("offline renderer build failed ({status})")); }
    // Without sprites/models/Cubism vendor files the window starts but never
    // becomes renderer-ready, so these are mandatory.
    for name in ["assets", "models", "vendor"] {
        copy_tree(&adapter_dist.join(name), &stage.join(name))?;
    }
    copy_tree(&crate_dir.join("models"), &stage.join("models"))?;
    let notice_dir = stage.join("licenses").join("bongocat");
    std::fs::create_dir_all(&notice_dir).map_err(|e| e.to_string())?;
    for name in ["LICENSE", "UPSTREAM.md"] {
        std::fs::copy(root.join("third_party").join("bongocat").join(name), notice_dir.join(name))
            .map_err(|e| format!("BongoCat attribution: {e}"))?;
    }
    if platform == "linux" && crate_dir.join("gnome-companion").is_dir() {
        copy_tree(&crate_dir.join("gnome-companion"), &stage.join("gnome-companion"))?;
    }

    let mut artifacts = Vec::new();
    for (rel, path) in files_sorted(&stage)? {
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        artifacts.push(json!({
            "path": rel,
            "sha256": format!("{:x}", Sha256::digest(&bytes)),
            "size": bytes.len(),
            "role": role_of(&rel, exe_name),
        }));
    }
    let cargo_toml = std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap_or_default();
    let manifest = json!({
        "runtime": "readmd-pet-rust",
        "version": package_version(&cargo_toml),
        // `protocol` is kept beside `protocol_version` for older local bundles.
        "protocol_version": 1,
        "protocol": 1,
        "platform": platform,
        "arch": arch,
        "artifacts": artifacts,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n";
    std::fs::write(stage.join("runtime-manifest.json"), text).map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for (rel, path) in files_sorted(&stage)? {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let executable_bit = rel == exe_name || rel.ends_with(".sh");
        entries.push((rel, data, if executable_bit { 0o755 } else { 0o644 }));
    }
    let archive = output_dir.join(archive_name(&platform, arch));
    std::fs::write(&archive, write_zip(&entries)?).map_err(|e| format!("{}: {e}", archive.display()))?;
    println!(
        "{}",
        json!({
            "archive": archive.to_string_lossy(),
            "stage": stage.to_string_lossy(),
            "platform": platform,
            "arch": arch,
            "files": entries.len() - 1,
        })
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_names_and_roles_match_the_python_script() {
        assert_eq!(target_for("windows", "x86_64"), Some("x86_64-pc-windows-msvc"));
        assert_eq!(target_for("macos", "aarch64"), Some("aarch64-apple-darwin"));
        assert_eq!(target_for("linux", "riscv"), None);
        assert_eq!(normalize_arch("ARM64"), "aarch64");
        assert_eq!(normalize_arch("AMD64"), "x86_64");
        assert_eq!(archive_name("windows", "x86_64"), "ReadMD-Pet-Rust.zip");
        assert_eq!(archive_name("linux", "aarch64"), "ReadMD-Pet-Rust-linux-aarch64.zip");
        assert_eq!(role_of("readmd-pet-rust", "readmd-pet-rust"), "executable");
        assert_eq!(role_of("renderer/index.html", "x"), "renderer");
        assert_eq!(role_of("models/a.moc3", "x"), "model");
        assert_eq!(role_of("gnome-companion/a.js", "x"), "companion");
        assert_eq!(role_of("vendor/cubism.js", "x"), "asset");
        assert_eq!(package_version("[package]\nname = \"p\"\nversion = \"1.2.3\"\n"), "1.2.3");
        assert_eq!(package_version("[package]\n"), "0.0.0");
    }

    #[test]
    fn zip_is_deterministic_and_readable() {
        let entries = vec![
            ("a.txt".to_string(), b"hello hello hello".to_vec(), 0o644),
            ("bin/run".to_string(), vec![0u8; 300], 0o755),
        ];
        let one = write_zip(&entries).unwrap();
        assert_eq!(one, write_zip(&entries).unwrap());
        // End-of-central-directory record: two entries.
        let eocd = one.len() - 22;
        assert_eq!(&one[eocd..eocd + 4], &0x0605_4b50u32.to_le_bytes());
        assert_eq!(u16::from_le_bytes([one[eocd + 10], one[eocd + 11]]), 2);
        // Round-trip the first member through a raw inflate.
        let name_len = u16::from_le_bytes([one[26], one[27]]) as usize;
        let csize = u32::from_le_bytes([one[18], one[19], one[20], one[21]]) as usize;
        let body = &one[30 + name_len..30 + name_len + csize];
        let mut text = String::new();
        std::io::Read::read_to_string(&mut flate2::read::DeflateDecoder::new(body), &mut text).unwrap();
        assert_eq!(text, "hello hello hello");
    }
}
