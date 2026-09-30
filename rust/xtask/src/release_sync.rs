//! `cargo xtask release-asset-sync --assets-dir DIR --tag TAG --commit SHA [--repo OWNER/NAME]`
//!
//! Stage and switch GitHub Release assets without ever publishing a partial
//! set (port of `tools/release_asset_sync.py`):
//!
//! 1. the candidate directory must hold exactly the expected payload; a
//!    `SHA256SUMS.txt` is written next to it;
//! 2. every file is uploaded under a per-commit staging prefix and verified;
//! 3. only then is each public asset replaced (delete + rename), checksum last;
//! 4. anything that is not an expected final name is removed.
//!
//! All GitHub traffic goes through the `gh` CLI (`GH_TOKEN` / `GH_REPO`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const CHECKSUM_NAME: &str = "SHA256SUMS.txt";
pub const STAGING_MARKER: &str = "__release_sync__";

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub size: u64,
    pub state: String,
    pub url: String,
}

pub struct GhOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// `gh` runner; tests substitute an in-memory fake.
pub trait Gh {
    fn run(&mut self, args: &[String], input: Option<&str>) -> GhOutput;
}

pub fn expected_assets(version: &str) -> BTreeSet<String> {
    [
        format!("ReadMDSetup-v{version}.exe"),
        format!("ReadMD-portable-v{version}.exe"),
        format!("readmd-vscode-{version}.vsix"),
        format!("readmd-mcp-server-{version}.zip"),
        format!("ReadMD-macos-x64-v{version}.zip"),
        format!("ReadMD-macos-arm64-v{version}.zip"),
        format!("ReadMD-linux-x86_64-v{version}.AppImage"),
        format!("readmd_{version}_amd64.deb"),
        format!("ReadMD-linux-aarch64-v{version}.AppImage"),
        format!("readmd_{version}_arm64.deb"),
        CHECKSUM_NAME.to_string(),
    ]
    .into_iter()
    .collect()
}

pub fn payload_assets(version: &str) -> BTreeSet<String> {
    let mut s = expected_assets(version);
    s.remove(CHECKSUM_NAME);
    s
}

fn files_in(dir: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut out = BTreeMap::new();
    for e in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            out.insert(e.file_name().to_string_lossy().into_owned(), e.path());
        }
    }
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Validate the candidate set and write `SHA256SUMS.txt` (`<sha>  <name>`).
pub fn prepare_assets(dir: &Path, version: &str) -> Result<PathBuf, String> {
    let files = files_in(dir)?;
    let required = payload_assets(version);
    let have: BTreeSet<String> = files.keys().cloned().collect();
    if have != required {
        let missing: Vec<_> = required.difference(&have).collect();
        let extra: Vec<_> = have.difference(&required).collect();
        return Err(format!("asset mismatch: missing={missing:?}, extra={extra:?}"));
    }
    let mut lines = String::new();
    for name in &required {
        let data = fs::read(&files[name]).map_err(|e| format!("{name}: {e}"))?;
        lines.push_str(&format!("{}  {name}\n", hex(&Sha256::digest(&data))));
    }
    let checksum = dir.join(CHECKSUM_NAME);
    fs::write(&checksum, lines).map_err(|e| e.to_string())?;
    Ok(checksum)
}

pub fn clean_commit(commit: &str) -> Result<String, String> {
    let clean: String = commit.chars().filter(|c| c.is_ascii_alphanumeric()).take(40).collect();
    if clean.is_empty() {
        return Err("commit is required".into());
    }
    Ok(clean)
}

pub fn staging_prefix(commit: &str) -> Result<String, String> {
    Ok(format!("{STAGING_MARKER}{}_", clean_commit(commit)?))
}

fn asset_from_json(v: &Value) -> Result<Asset, String> {
    Ok(Asset {
        id: v["id"].as_i64().ok_or("asset without id")?,
        name: v["name"].as_str().ok_or("asset without name")?.to_string(),
        size: v["size"].as_u64().unwrap_or(0),
        state: v["state"].as_str().unwrap_or("").to_string(),
        url: v["url"].as_str().ok_or("asset without url")?.to_string(),
    })
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// `None` when the release does not exist yet (HTTP 404).
pub fn fetch_release(gh: &mut dyn Gh, tag: &str, repo: &str) -> Result<Option<Value>, String> {
    let out = gh.run(&args(&["api", &format!("repos/{repo}/releases/tags/{tag}")]), None);
    if out.code != 0 && out.stderr.contains("HTTP 404") {
        return Ok(None);
    }
    if out.code != 0 {
        let msg = out.stderr.trim();
        return Err(if msg.is_empty() { "failed to fetch release".into() } else { msg.to_string() });
    }
    serde_json::from_str(&out.stdout).map(Some).map_err(|e| format!("release JSON: {e}"))
}

fn release_assets(gh: &mut dyn Gh, tag: &str, repo: &str) -> Result<Vec<Asset>, String> {
    let release = fetch_release(gh, tag, repo)?.ok_or_else(|| format!("release {tag} disappeared"))?;
    release["assets"].as_array().map(|a| a.iter().map(asset_from_json).collect()).unwrap_or(Ok(Vec::new()))
}

fn checked(out: GhOutput, what: &str) -> Result<(), String> {
    if out.code == 0 {
        Ok(())
    } else {
        let msg = out.stderr.trim();
        Err(if msg.is_empty() { format!("gh command failed: {what}") } else { msg.to_string() })
    }
}

fn delete_asset(gh: &mut dyn Gh, a: &Asset) -> Result<(), String> {
    checked(gh.run(&args(&["api", "--method", "DELETE", &a.url]), None), "delete")
}

fn rename_asset(gh: &mut dyn Gh, a: &Asset, name: &str) -> Result<(), String> {
    let body = json!({ "name": name }).to_string();
    checked(gh.run(&args(&["api", "--method", "PATCH", &a.url, "--input", "-"]), Some(&body)), "rename")
}

pub fn upload_staged_assets(
    gh: &mut dyn Gh,
    tag: &str,
    dir: &Path,
    version: &str,
    commit: &str,
    repo: &str,
) -> Result<(String, BTreeMap<String, Asset>), String> {
    let prefix = staging_prefix(commit)?;
    let files = files_in(dir)?;
    let required = expected_assets(version);
    if files.keys().cloned().collect::<BTreeSet<_>>() != required {
        return Err("release assets changed during synchronization".into());
    }
    let staging_dir = dir.join(format!(".staging-{}", clean_commit(commit)?));
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir(&staging_dir).map_err(|e| e.to_string())?;
    let mut staged_paths = Vec::new();
    for (name, src) in &files {
        let dst = staging_dir.join(format!("{prefix}{name}"));
        fs::copy(src, &dst).map_err(|e| format!("{name}: {e}"))?;
        staged_paths.push(dst);
    }
    for p in &staged_paths {
        let path = p.to_string_lossy().into_owned();
        checked(gh.run(&args(&["release", "upload", tag, &path, "--clobber"]), None), "upload")?;
    }

    let assets: BTreeMap<String, Asset> =
        release_assets(gh, tag, repo)?.into_iter().map(|a| (a.name.clone(), a)).collect();
    let mut staged = BTreeMap::new();
    for name in &required {
        let staged_name = format!("{prefix}{name}");
        let size = fs::metadata(&files[name]).map(|m| m.len()).unwrap_or(u64::MAX);
        match assets.get(&staged_name) {
            Some(a) if a.state == "uploaded" && a.size == size => {
                staged.insert(name.clone(), a.clone());
            }
            _ => return Err(format!("staged asset was not accepted: {staged_name}")),
        }
    }
    let _ = fs::remove_dir_all(&staging_dir);
    Ok((prefix, staged))
}

pub fn swap_staged_assets(gh: &mut dyn Gh, tag: &str, version: &str, commit: &str, repo: &str) -> Result<(), String> {
    let prefix = staging_prefix(commit)?;
    let mut assets: BTreeMap<String, Asset> =
        release_assets(gh, tag, repo)?.into_iter().map(|a| (a.name.clone(), a)).collect();
    let expected = expected_assets(version);

    // Uploads are complete before any public name changes.  Checksum goes last.
    let mut ordered: Vec<String> = expected.iter().filter(|n| *n != CHECKSUM_NAME).cloned().collect();
    ordered.push(CHECKSUM_NAME.to_string());
    for name in &ordered {
        let staged_name = format!("{prefix}{name}");
        let staged = assets.get(&staged_name).cloned().ok_or_else(|| format!("missing staged asset: {staged_name}"))?;
        if let Some(current) = assets.get(name).cloned() {
            delete_asset(gh, &current)?;
        }
        rename_asset(gh, &staged, name)?;
        assets.remove(&staged_name);
        assets.insert(name.clone(), Asset { name: name.clone(), ..staged });
    }

    // Every public name must be live before anything else is touched …
    let live: BTreeSet<String> = release_assets(gh, tag, repo)?.into_iter().map(|a| a.name).collect();
    let missing: Vec<_> = expected.difference(&live).collect();
    if !missing.is_empty() {
        return Err(format!("final asset mismatch: missing {missing:?}"));
    }
    // … then obsolete finals and staging left by cancelled older runs go.
    // (The Python original compared for equality *before* this cleanup, so any
    // leftover asset failed the run after the swap had already happened.)
    for a in release_assets(gh, tag, repo)? {
        if !expected.contains(&a.name) {
            delete_asset(gh, &a)?;
        }
    }
    let final_names: BTreeSet<String> = release_assets(gh, tag, repo)?.into_iter().map(|a| a.name).collect();
    if final_names != expected {
        let diff: Vec<_> = final_names.symmetric_difference(&expected).collect();
        return Err(format!("final asset mismatch: {diff:?}"));
    }
    Ok(())
}

struct GhCli;

impl Gh for GhCli {
    fn run(&mut self, args: &[String], input: Option<&str>) -> GhOutput {
        let mut cmd = Command::new("gh");
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return GhOutput { code: -1, stdout: String::new(), stderr: format!("cannot run gh: {e}") },
        };
        if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
            let _ = stdin.write_all(text.as_bytes());
        }
        match child.wait_with_output() {
            Ok(o) => GhOutput {
                code: o.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
            },
            Err(e) => GhOutput { code: -1, stdout: String::new(), stderr: e.to_string() },
        }
    }
}

pub fn main(root: &Path, argv: &[String]) -> Result<(), String> {
    let mut opts: BTreeMap<&str, String> = BTreeMap::new();
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        let key = match a.as_str() {
            "--assets-dir" | "--tag" | "--commit" | "--repo" => a.as_str(),
            other => return Err(format!("unknown argument: {other}")),
        };
        opts.insert(key, it.next().ok_or_else(|| format!("{key} needs a value"))?.clone());
    }
    let need = |k: &str| opts.get(k).cloned().ok_or_else(|| format!("{k} is required"));
    let assets_dir = PathBuf::from(need("--assets-dir")?);
    let tag = need("--tag")?;
    let commit = need("--commit")?;
    let repo = opts.get("--repo").cloned().or_else(|| std::env::var("GH_REPO").ok()).filter(|s| !s.is_empty());
    if std::env::var("GH_TOKEN").map(|s| s.is_empty()).unwrap_or(true) {
        return Err("GH_TOKEN is required".into());
    }
    let repo = repo.ok_or("GH_REPO or --repo is required")?;
    let version = fs::read_to_string(root.join("VERSION")).map_err(|e| format!("VERSION: {e}"))?.trim().to_string();

    let mut gh = GhCli;
    if fetch_release(&mut gh, &tag, &repo)?.is_none() {
        println!("release {tag} does not exist yet; skipping asset sync");
        return Ok(());
    }
    prepare_assets(&assets_dir, &version)?;
    let (prefix, staged) = upload_staged_assets(&mut gh, &tag, &assets_dir, &version, &commit, &repo)?;
    swap_staged_assets(&mut gh, &tag, &version, &commit, &repo)?;
    println!("release assets synced: {tag} / {commit} ({} staged via {prefix})", staged.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = "9.9.9";
    const TAG: &str = "v9.9.9";
    const COMMIT: &str = "abc123def4567890abcdef1234567890abcdef12";
    const REPO: &str = "Natsummerance/readMD";

    #[derive(Default)]
    struct FakeGh {
        assets: BTreeMap<String, Value>,
        next_id: i64,
        calls: Vec<Vec<String>>,
    }

    impl FakeGh {
        fn new() -> Self {
            FakeGh { next_id: 1000, ..Default::default() }
        }
        fn upsert(&mut self, name: &str, size: u64, id: Option<i64>) {
            let id = id.unwrap_or_else(|| {
                self.next_id += 1;
                self.next_id - 1
            });
            self.assets.insert(
                name.to_string(),
                json!({"id": id, "name": name, "size": size, "state": "uploaded", "url": format!("https://api.github.com/assets/{id}")}),
            );
        }
        fn remove_id(&mut self, id: i64) -> Value {
            let key = self.assets.iter().find(|(_, v)| v["id"] == id).map(|(k, _)| k.clone()).expect("known id");
            self.assets.remove(&key).unwrap()
        }
        fn ok(v: Value) -> GhOutput {
            GhOutput { code: 0, stdout: v.to_string(), stderr: String::new() }
        }
    }

    impl Gh for FakeGh {
        fn run(&mut self, a: &[String], input: Option<&str>) -> GhOutput {
            self.calls.push(a.to_vec());
            let id_of = |url: &str| url.rsplit('/').next().unwrap().parse::<i64>().unwrap();
            if a[0] == "api" && a[1].contains("releases/tags/") {
                return Self::ok(json!({ "assets": self.assets.values().cloned().collect::<Vec<_>>() }));
            }
            if a[0] == "release" && a[1] == "upload" {
                let p = Path::new(&a[3]);
                let size = fs::metadata(p).unwrap().len();
                self.upsert(&p.file_name().unwrap().to_string_lossy(), size, None);
                return Self::ok(json!({}));
            }
            if a[..3] == ["api", "--method", "DELETE"] {
                self.remove_id(id_of(&a[3]));
                return Self::ok(json!({}));
            }
            if a[..3] == ["api", "--method", "PATCH"] {
                let id = id_of(&a[3]);
                let name: Value = serde_json::from_str(input.unwrap()).unwrap();
                let old = self.remove_id(id);
                self.upsert(name["name"].as_str().unwrap(), old["size"].as_u64().unwrap(), Some(id));
                return Self::ok(json!({}));
            }
            panic!("unsupported gh command: {a:?}");
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("readmd-xtask-sync-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        for (i, name) in payload_assets(VERSION).iter().enumerate() {
            fs::write(d.join(name), format!("payload {i}")).unwrap();
        }
        d
    }

    #[test]
    fn prepare_validates_and_builds_plain_checksums() {
        let d = scratch("prepare");
        let sums = fs::read_to_string(prepare_assets(&d, VERSION).unwrap()).unwrap();
        let lines: Vec<&str> = sums.lines().collect();
        assert_eq!(lines.len(), 10);
        for l in lines {
            let (digest, name) = l.split_once("  ").unwrap();
            assert_eq!(digest, hex(&Sha256::digest(fs::read(d.join(name)).unwrap())));
        }
        fs::write(d.join("unexpected.txt"), "bad").unwrap();
        assert!(prepare_assets(&d, VERSION).unwrap_err().contains("extra"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn upload_requires_exact_candidate_set() {
        let d = scratch("exact");
        fs::write(d.join("unexpected.txt"), "bad").unwrap();
        assert!(upload_staged_assets(&mut FakeGh::new(), TAG, &d, VERSION, COMMIT, REPO).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_release_is_reported_as_none() {
        struct Missing(Vec<Vec<String>>);
        impl Gh for Missing {
            fn run(&mut self, a: &[String], _: Option<&str>) -> GhOutput {
                self.0.push(a.to_vec());
                GhOutput { code: 1, stdout: String::new(), stderr: "gh: Not Found (HTTP 404)".into() }
            }
        }
        let mut gh = Missing(Vec::new());
        assert_eq!(fetch_release(&mut gh, TAG, REPO).unwrap(), None);
        assert_eq!(gh.0, vec![args(&["api", &format!("repos/{REPO}/releases/tags/{TAG}")])]);
    }

    #[test]
    fn stages_before_replacing_public_assets() {
        let d = scratch("swap");
        let mut gh = FakeGh::new();
        for (i, name) in expected_assets(VERSION).iter().enumerate() {
            gh.upsert(name, 1, Some(900 + i as i64));
        }
        gh.upsert("ReadMD-old.zip", 1, Some(890));
        prepare_assets(&d, VERSION).unwrap();
        let (prefix, staged) = upload_staged_assets(&mut gh, TAG, &d, VERSION, COMMIT, REPO).unwrap();
        assert_eq!(prefix, staging_prefix(COMMIT).unwrap());
        assert_eq!(staged.len(), 11);
        let public: BTreeSet<String> = gh.assets.keys().filter(|n| !n.starts_with(STAGING_MARKER)).cloned().collect();
        let mut before = expected_assets(VERSION);
        before.insert("ReadMD-old.zip".into());
        assert_eq!(public, before, "no public asset changes before the swap");

        swap_staged_assets(&mut gh, TAG, VERSION, COMMIT, REPO).unwrap();
        let names: BTreeSet<String> = gh.assets.keys().cloned().collect();
        assert_eq!(names, expected_assets(VERSION));
        // The checksum is the last public name to switch.
        let renames: Vec<&Vec<String>> = gh.calls.iter().filter(|c| c.get(2).map(String::as_str) == Some("PATCH")).collect();
        assert_eq!(renames.len(), 11);
        let last_id: i64 = renames.last().unwrap()[3].rsplit('/').next().unwrap().parse().unwrap();
        assert_eq!(gh.assets[CHECKSUM_NAME]["id"], last_id);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn commit_is_sanitised() {
        assert_eq!(clean_commit("ab-cd/ef").unwrap(), "abcdef");
        assert!(clean_commit("--").is_err());
        assert_eq!(staging_prefix("x").unwrap(), "__release_sync__x_");
    }
}
