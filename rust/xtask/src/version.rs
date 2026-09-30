//! Version propagation (port of `tools/sync_version.py::sync_all`).
//!
//! Every text file is edited with LF line endings internally and written
//! back with the line ending it had, so a Windows CRLF checkout stays CRLF.

use base64::Engine;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Semver {
    major: u64,
    minor: u64,
    patch: u64,
    extra: String,
}

fn parse_semver(ver: &str) -> Result<Semver, String> {
    let clean = ver.trim().trim_start_matches(['v', 'V']);
    let re = Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:[-.]([a-zA-Z0-9.]+))?$").unwrap();
    let c = re.captures(clean).ok_or_else(|| format!("invalid semantic version: {ver}"))?;
    Ok(Semver {
        major: c[1].parse().unwrap(),
        minor: c[2].parse().unwrap(),
        patch: c[3].parse().unwrap(),
        extra: c.get(4).map(|m| m.as_str().to_string()).unwrap_or_default(),
    })
}

impl Semver {
    fn triplet(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
    fn linglong(&self) -> String {
        let digits = Regex::new(r"\d+").unwrap();
        let extra = match digits.find(&self.extra) {
            Some(m) => m.as_str().to_string(),
            None if !self.extra.is_empty() => "1".to_string(),
            None => "0".to_string(),
        };
        format!("{}.{}", self.triplet(), extra)
    }
    fn harmony_code(&self) -> u64 {
        self.major * 10000 + self.minor * 100 + self.patch
    }
}

/// `READMD_VERSION=` / `VERSION=` in `.env`, then the `VERSION` file, then the
/// environment.
pub fn load_env_version(root: &Path) -> String {
    if let Ok(text) = fs::read_to_string(root.join(".env")) {
        for line in text.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("READMD_VERSION=").or_else(|| line.strip_prefix("VERSION=")) {
                let v = v.trim().trim_matches(['\'', '"']);
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("VERSION")) {
        let v = text.trim();
        if !v.is_empty() {
            return v.to_string();
        }
    }
    std::env::var("READMD_VERSION").unwrap_or_default()
}

const VERSION_KEYS: &[&str] = &[
    "READMD_VERSION",
    "READMD_VERSION_TAG",
    "READMD_VERSION_SEMVER",
    "READMD_VERSION_TRIPLET",
    "READMD_VERSION_MAJOR",
    "READMD_VERSION_MINOR",
    "READMD_VERSION_PATCH",
    "READMD_VERSION_PRERELEASE",
    "READMD_VERSION_LINGLONG",
    "READMD_VERSION_HARMONY_CODE",
    "READMD_VERSION_HARMONY_NAME",
    "READMD_VERSION_WINDOWS_FILEVER",
    "READMD_VERSION_VSCODE",
    "READMD_VERSION_RPM_REL",
    "READMD_VERSION_ARCH_REL",
    "READMD_VERSION_BUILD_DATE",
];

/// Today's date (UTC) as `YYYY-MM-DD`, from the civil-from-days algorithm.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// The version block for `.env` / `.env.example`, keeping every non-version
/// line of the existing file (credentials, URLs) after it.
fn env_block(target: &str, sv: &Semver, existing: &str) -> String {
    let ll = sv.linglong();
    let mut lines: Vec<String> = vec![
        "# ReadMD 全局统一版本配置 (Single Source of Truth for Global Versioning)".into(),
        "# 修改此文件中的 READMD_VERSION 或运行 cargo xtask sync-version <version> 即可一键同步全库所有文件".into(),
        String::new(),
        format!("READMD_VERSION={target}"),
        format!("READMD_VERSION_TAG=v{target}"),
        format!("READMD_VERSION_SEMVER={target}"),
        format!("READMD_VERSION_TRIPLET={}", sv.triplet()),
        format!("READMD_VERSION_MAJOR={}", sv.major),
        format!("READMD_VERSION_MINOR={}", sv.minor),
        format!("READMD_VERSION_PATCH={}", sv.patch),
        format!("READMD_VERSION_PRERELEASE={}", sv.extra),
        format!("READMD_VERSION_LINGLONG={ll}"),
        format!("READMD_VERSION_HARMONY_CODE={}", sv.harmony_code()),
        format!("READMD_VERSION_HARMONY_NAME={target}"),
        format!("READMD_VERSION_WINDOWS_FILEVER={ll}"),
        format!("READMD_VERSION_VSCODE={target}"),
        "READMD_VERSION_RPM_REL=1".into(),
        "READMD_VERSION_ARCH_REL=1".into(),
        format!("READMD_VERSION_BUILD_DATE={}", today()),
        String::new(),
    ];
    let custom: Vec<String> = existing
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| {
            let k = l.split('=').next().unwrap_or("").trim();
            !VERSION_KEYS.contains(&k) && k != "VERSION"
        })
        .map(str::to_string)
        .collect();
    if !custom.is_empty() {
        lines.push("# 自定义本地凭据与运行时环境".into());
        lines.extend(custom);
        lines.push(String::new());
    }
    lines.join("\n")
}

/// Ignore the build date and the tool hint when deciding whether `.env` drifted.
fn env_for_compare(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().starts_with("READMD_VERSION_BUILD_DATE=") && !l.starts_with("# 修改此文件"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// A text file held as LF internally, remembering its original ending.
struct Text {
    path: PathBuf,
    crlf: bool,
    original: String,
}

fn read_text(path: &Path) -> Option<Text> {
    let raw = fs::read_to_string(path).ok()?;
    let crlf = raw.contains("\r\n");
    Some(Text { path: path.to_path_buf(), crlf, original: raw.replace("\r\n", "\n") })
}

struct Change {
    path: PathBuf,
    crlf: bool,
    content: String,
}

#[derive(Default)]
struct Plan {
    changes: Vec<Change>,
    misses: Vec<String>,
}

impl Plan {
    fn push_if_changed(&mut self, t: &Text, new: String) {
        if new != t.original {
            self.changes.push(Change { path: t.path.clone(), crlf: t.crlf, content: new });
        }
    }
}

/// `re.sub` with `\g<n>` templates written as Rust `${n}`.
fn sub(text: &str, pattern: &str, rep: &str) -> String {
    Regex::new(pattern).unwrap().replace_all(text, rep).into_owned()
}

fn sub_counted(text: &str, pattern: &str, rep: &str) -> (String, usize) {
    let re = Regex::new(pattern).unwrap();
    let n = re.find_iter(text).count();
    (re.replace_all(text, rep).into_owned(), n)
}

fn artifact_names(text: &str, v: &str) -> String {
    let s = sub(text, r"(?P<prefix>-)v(?P<version>\d[0-9a-zA-Z.-]*)(?P<suffix>\.(?:exe|zip|AppImage|deb|vsix)\b)", &format!("${{prefix}}v{v}${{suffix}}"));
    let s = sub(&s, r"(_)[0-9a-zA-Z.-]+(_amd64\.deb)", &format!("${{1}}{v}${{2}}"));
    let s = sub(&s, r"(_)[0-9a-zA-Z.-]+(_arm64\.deb)", &format!("${{1}}{v}${{2}}"));
    let s = sub(&s, r"(vscode-)[0-9a-zA-Z.-]+(\.vsix)", &format!("${{1}}{v}${{2}}"));
    sub(&s, r"(server-)[0-9a-zA-Z.-]+(\.zip)", &format!("${{1}}{v}${{2}}"))
}

/// Rewrite `"version"` in a JSON manifest, keeping key order.
fn json_version(plan: &mut Plan, path: &Path, v: &str) {
    let Some(t) = read_text(path) else { return };
    let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&t.original) else { return };
    if data.get("version").and_then(|x| x.as_str()) == Some(v) {
        return;
    }
    if let Some(obj) = data.as_object_mut() {
        obj.insert("version".into(), serde_json::Value::String(v.into()));
    }
    let out = serde_json::to_string_pretty(&data).unwrap_or_default() + "\n";
    plan.push_if_changed(&t, out);
}

fn build_plan(root: &Path, v: &str) -> Result<Plan, String> {
    let sv = parse_semver(v)?;
    let triplet = sv.triplet();
    let ll = sv.linglong();
    let hc = sv.harmony_code();
    let mut plan = Plan::default();

    // .env / .env.example / VERSION
    let env_path = root.join(".env");
    let old_env = read_text(&env_path).unwrap_or(Text { path: env_path.clone(), crlf: false, original: String::new() });
    let new_env = env_block(v, &sv, &old_env.original);
    if env_for_compare(&old_env.original) != env_for_compare(&new_env) {
        plan.changes.push(Change { path: env_path, crlf: old_env.crlf, content: new_env });
    }
    let ex_path = root.join(".env.example");
    let old_ex = read_text(&ex_path).unwrap_or(Text { path: ex_path.clone(), crlf: false, original: String::new() });
    let new_ex = env_block(v, &sv, "");
    if env_for_compare(&old_ex.original) != env_for_compare(&new_ex) {
        plan.changes.push(Change { path: ex_path, crlf: old_ex.crlf, content: new_ex });
    }
    let vpath = root.join("VERSION");
    match read_text(&vpath) {
        Some(t) if t.original.trim() == v => {}
        Some(t) => plan.changes.push(Change { path: vpath, crlf: t.crlf, content: format!("{v}\n") }),
        None => plan.changes.push(Change { path: vpath, crlf: false, content: format!("{v}\n") }),
    }

    // Linux packaging
    for rel in ["scripts/linux/linglong.yaml", "packages/linglong/linglong.yaml"] {
        if let Some(t) = read_text(&root.join(rel)) {
            let new = sub(&t.original, r"(version:\s*)[0-9.]+", &format!("${{1}}{ll}"));
            plan.push_if_changed(&t, new);
        }
    }
    if let Some(t) = read_text(&root.join("scripts/linux/PKGBUILD")) {
        let new = sub(&t.original, r"(pkgver=)[^\n]+", &format!("${{1}}{triplet}"));
        plan.push_if_changed(&t, new);
    }

    // VS Code extension, HarmonyOS app, UI tests
    json_version(&mut plan, &root.join("packages/vscode-extension/package.json"), v);
    if let Some(t) = read_text(&root.join("packages/vscode-extension/README.md")) {
        let new = sub(&t.original, r"(# ReadMD for VS Code · V)[0-9a-zA-Z.-]+(?: [^\n]+)?", &format!("${{1}}{v}"));
        let new = sub(&new, r"(readmd-vscode-)[0-9a-zA-Z.-]+(?:\.vsix)", &format!("${{1}}{v}.vsix"));
        plan.push_if_changed(&t, new);
    }
    json_version(&mut plan, &root.join("packages/harmonyos-app/package.json"), v);
    for rel in ["packages/harmonyos-app/oh-package.json5", "packages/harmonyos-app/entry/oh-package.json5"] {
        if let Some(t) = read_text(&root.join(rel)) {
            let re = Regex::new(r#"("version"\s*:\s*")[^"]+("),"#).unwrap();
            let new = re.replacen(&t.original, 1, format!("${{1}}{v}${{2}},")).into_owned();
            plan.push_if_changed(&t, new);
        }
    }
    json_version(&mut plan, &root.join("ui-tests/package.json"), v);
    if let Some(t) = read_text(&root.join("packages/harmonyos-app/AppScope/app.json5")) {
        let new = sub(&t.original, r#""versionCode":\s*\d+"#, &format!("\"versionCode\": {hc}"));
        let new = sub(&new, r#""versionName":\s*"[^"]+""#, &format!("\"versionName\": \"{v}\""));
        plan.push_if_changed(&t, new);
    }

    // CI
    if let Some(t) = read_text(&root.join(".github/workflows/release.yml")) {
        let new = sub(&t.original, r"READMD_VERSION:\s*'[^']+'", &format!("READMD_VERSION: '{v}'"));
        plan.push_if_changed(&t, new);
    }

    // Front end: every anchor must match, a silent miss leaves an old version behind.
    if let Some(t) = read_text(&root.join("assets/index.html")) {
        let mut s = t.original.clone();
        let anchors: [(&str, String, &str); 5] = [
            (r#"<html([^>]*\bdata-version=")[^"]+""#, format!("<html${{1}}{v}\""), "assets/index.html :: data-version"),
            (r#"(<link[^>]*href="/assets/(?:css/[a-z-]+|style|workspace-ui|skill-workbench)\.css\?v=)[^"]+(")"#, format!("${{1}}{v}${{2}}"), "assets/index.html :: stylesheet ?v="),
            (r#"(<span id="status-version"[^>]*>)v[^<]+(</span>)"#, format!("${{1}}v{v}${{2}}"), "assets/index.html :: #status-version"),
            (r#"(id="menu-version-label">)当前版本 v[^<]+(</em>)"#, format!("${{1}}当前版本 v{v}${{2}}"), "assets/index.html :: #menu-version-label"),
            (r#"(<script[^>]*src="/assets/readmd\.boot\.js\?v=)[^"]+(")"#, format!("${{1}}{v}${{2}}"), "assets/index.html :: readmd.boot.js ?v="),
        ];
        for (pat, rep, label) in anchors {
            let (next, hits) = sub_counted(&s, pat, &rep);
            if hits == 0 {
                plan.misses.push(label.to_string());
            }
            s = next;
        }
        plan.push_if_changed(&t, s);
    }
    if let Some(t) = read_text(&root.join("assets/js/features/updater.js")) {
        let new = sub(&t.original, r"(typeof VERSION !== 'undefined' \? VERSION : ')[^']+'\)", &format!("${{1}}{v}')"));
        plan.push_if_changed(&t, new);
    }

    // READMEs and release notes
    for name in ["README.md", "README.en.md", "README.ja.md", "README.zh-TW.md"] {
        if let Some(t) = read_text(&root.join(name)) {
            let s = sub(&t.original, r"badge/version-v[0-9a-zA-Z.-]+-3b6ef5", &format!("badge/version-v{v}-3b6ef5"));
            let s = sub(&s, r"(releases/download/v)[0-9a-zA-Z.-]+", &format!("${{1}}{v}"));
            let s = sub(&s, r"(releases/tag/v)[0-9a-zA-Z.-]+", &format!("${{1}}{v}"));
            let s = artifact_names(&s, v);
            plan.push_if_changed(&t, s);
        }
    }
    if let Some(t) = read_text(&root.join("release/release_notes.md")) {
        let s = sub(&t.original, r"(# ReadMD v)[0-9a-zA-Z.-]+", &format!("${{1}}{v}"));
        plan.push_if_changed(&t, artifact_names(&s, v));
    }

    // Website
    let site = root.join("website/public");
    if site.is_dir() {
        let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&site)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("html" | "xml" | "txt" | "json")))
            .collect();
        files.sort();
        for p in files {
            let Some(t) = read_text(&p) else { continue };
            let s = artifact_names(&t.original, v);
            let s = sub(&s, r"(releases/tag/v)[0-9a-zA-Z.-]+", &format!("${{1}}{v}"));
            let s = sub(&s, r"(releases/download/v)[0-9a-zA-Z.-]+", &format!("${{1}}{v}"));
            let s = sub(&s, r#"("softwareVersion":\s*")[^"]+""#, &format!("${{1}}{v}\""));
            let s = sub(&s, r#"("artifactSection":\s*"v)[^"]+""#, &format!("${{1}}{v}\""));
            let s = sub(&s, r"2\.3\.7-beta\.\d+", v);
            plan.push_if_changed(&t, s);
        }
        // CSP hash of the JSON-LD block in index.html.
        let idx = site.join("index.html");
        let headers = site.join("_headers");
        if let (Some(it), Some(ht)) = (read_text(&idx), read_text(&headers)) {
            let idx_content = plan
                .changes
                .iter()
                .find(|c| c.path == idx)
                .map(|c| c.content.clone())
                .unwrap_or(it.original.clone());
            // Hash the LF form: that is what the Linux deploy job checks out and serves.
            let idx_bytes = idx_content;
            let _ = it.crlf;
            let re = Regex::new(r#"(?s)<script type="application/ld\+json">(.*?)</script>"#).unwrap();
            if let Some(c) = re.captures(&idx_bytes) {
                let digest = Sha256::digest(c[1].as_bytes());
                let hash = format!("sha256-{}", base64::prelude::BASE64_STANDARD.encode(digest));
                let new = sub(&ht.original, r"'sha256-[^']+'", &format!("'{hash}'"));
                plan.push_if_changed(&ht, new);
            }
        }
    }
    Ok(plan)
}

pub fn sync_all(root: &Path, ver: &str, check: bool) -> Result<bool, String> {
    let v = ver.trim().trim_start_matches(['v', 'V']).to_string();
    if v.is_empty() {
        return Err("no version given and none found in .env / VERSION".into());
    }
    let plan = build_plan(root, &v)?;
    if !plan.misses.is_empty() {
        println!("[WARN] {} 个版本锚点未命中，对应版本号可能停留在旧值：", plan.misses.len());
        for m in &plan.misses {
            println!(" - {m}");
        }
    }
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string().replace('\\', "/");
    if check {
        let stale_bundle = !crate::bundle::build(root).eq(&fs::read(root.join("assets/readmd.boot.js")).unwrap_or_default());
        if plan.changes.is_empty() && plan.misses.is_empty() && !stale_bundle {
            println!("[OK] All platform files are synchronized with version {v}");
            return Ok(true);
        }
        if !plan.changes.is_empty() {
            println!("[FAIL] {} files out of sync with version {v}:", plan.changes.len());
            for c in &plan.changes {
                println!(" - {}", rel(&c.path));
            }
        }
        if stale_bundle {
            println!("[FAIL] assets/readmd.boot.js is stale");
        }
        return Ok(false);
    }
    for c in &plan.changes {
        let content = if c.crlf { c.content.replace('\n', "\r\n") } else { c.content.clone() };
        fs::write(&c.path, content).map_err(|e| format!("write {}: {e}", c.path.display()))?;
        println!("[SYNC] Updated: {}", rel(&c.path));
    }
    crate::bundle::bundle(root, false)?;
    println!("\n[SUCCESS] Synchronized all platform version files to {v}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parts() {
        let s = parse_semver("v2.4.0-beta.3").unwrap();
        assert_eq!((s.major, s.minor, s.patch, s.extra.as_str()), (2, 4, 0, "beta.3"));
        assert_eq!(s.linglong(), "2.4.0.3");
        assert_eq!(s.harmony_code(), 20400);
        assert_eq!(parse_semver("2.4.0").unwrap().linglong(), "2.4.0.0");
        assert_eq!(parse_semver("2.4.0-rc").unwrap().linglong(), "2.4.0.1");
        assert!(parse_semver("2.4").is_err());
    }

    #[test]
    fn env_block_keeps_custom_lines_and_drops_old_versions() {
        let sv = parse_semver("3.0.0").unwrap();
        let out = env_block("3.0.0", &sv, "# c\nREADMD_VERSION=1.0.0\nAPI_URL=x\n\nVERSION=1\n");
        assert!(out.contains("READMD_VERSION=3.0.0\n"));
        assert!(out.contains("API_URL=x"));
        assert!(!out.contains("1.0.0"));
        assert!(!out.contains("VERSION=1\n"));
    }

    #[test]
    fn today_is_a_date() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
    }

    #[test]
    fn artifact_rewrites() {
        let s = artifact_names("ReadMD-v1.2.3.exe readmd_1.2.3_amd64.deb readmd-vscode-1.2.3.vsix", "9.9.9");
        assert_eq!(s, "ReadMD-v9.9.9.exe readmd_9.9.9_amd64.deb readmd-vscode-9.9.9.vsix");
    }
}
