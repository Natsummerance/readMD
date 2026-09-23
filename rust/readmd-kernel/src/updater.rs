//! `src/readmd_modules/updater.py` — the update check's **tiers 2 and 3**, plus
//! `readmd.py::check_latest_release` (`readmd.py:150-179`), the startup upgrade
//! probe.
//!
//! ## What was missing
//!
//! [`crate::batch2`] already carries a faithful port of tier 1 of
//! `updater.check_update` (`updater.py:386-466`): the GitHub REST call, the
//! channel filter, the asset matcher and the response payload.  What it does
//! *not* do is stated in its own comment at `batch2.rs:264-269`: when tier 1
//! fails or rate-limits, Python falls back to
//!
//! ```text
//! _sniff_latest_tag_redirect()  updater.py:289-327   -> tag from a 302 Location
//! _fetch_manifest_assets()      updater.py:330-383   -> assets rebuilt from SHA256SUMS.txt
//! ```
//!
//! and answers `update_network_error` instead.  On a network where
//! `api.github.com` is rate-limited but `github.com` still redirects — the
//! exact situation the Chinese mirror prefixes exist for, and the situation the
//! docstring at `updater.py:388` describes — Python reports an update and Rust
//! reported a network failure.  This module closes that gap.
//!
//! ## Transport
//!
//! HTTPS egress runs **in-process** on the TLS stack this crate already links
//! and already drives from [`crate::server::http_request`], [`crate::ai`] and
//! [`crate::batch2`]: `ureq 2.12.1` + `rustls` + `ring` + `webpki-roots`, all
//! three of which are in `Cargo.lock` and vendored in `~/.cargo/registry`.  No
//! new dependency, no `curl`, no Python.
//!
//! Every piece of *logic* here is parameterised over a caller-supplied
//! transport, so the redirect parse, the manifest schema, the asset assembly
//! and the failure taxonomy are all pinned by tests that never open a socket.
//! The `*_via_ureq` functions are the thin live adapters.
//!
//! ## Honest scope
//!
//! Live network behaviour is **unverified in this build environment** (there is
//! no egress here).  What is verified is everything that is not the socket:
//! candidate-URL construction, `Location` handling, manifest grammar, asset
//! selection, version comparison and the error codes each failure path
//! produces.
//!
//! ## Single home for the shared helpers
//!
//! [`detect_app_flavor`] and [`match_release_asset`] used to exist twice: this
//! module had to be written without editing `batch2.rs`, which carried private
//! copies of the same Python lines.  `batch2::h_update_check` now delegates the
//! whole three-tier check to [`check_update`] (through [`check_update_live`]), so
//! the copies there are deleted and this module is the only port of
//! `updater.py:107-201`.

use serde_json::{json, Map, Value};
use std::time::Duration;

// ------------------------------------------------------------------ constants

/// `updater.GITHUB_REPO` (`updater.py:30`).
pub const GITHUB_REPO: &str = "Natsummerance/readMD";

/// `updater.GITHUB_API_LATEST` (`updater.py:31`).
pub const GITHUB_API_LATEST: &str = "https://api.github.com/repos/Natsummerance/readMD/releases/latest";

/// `updater.GITHUB_API_RELEASES` (`updater.py:32`).
pub const GITHUB_API_RELEASES: &str =
    "https://api.github.com/repos/Natsummerance/readMD/releases?per_page=100";

/// `updater.MIRROR_PREFIXES` (`updater.py:35-39`).  A mirror is a *prefix on the
/// official URL*, never a host of its own, which is why
/// `validate_update_source` still only ever trusts `github.com`.
pub const MIRROR_PREFIXES: [&str; 3] = [
    "https://ghfast.top/",
    "https://ghproxy.net/",
    "https://mirror.ghproxy.com/",
];

/// `req.add_header('User-Agent', 'ReadMD-Updater')` (`updater.py:237`).
pub const UPDATE_USER_AGENT: &str = "ReadMD-Updater";

/// `updater.TRANSIENT_FETCH_ATTEMPTS` (`updater.py:229`).
pub const TRANSIENT_FETCH_ATTEMPTS: usize = 2;

/// `updater.TRANSIENT_FETCH_BACKOFF = (0.3,)` (`updater.py:230`) — a *tuple*, so
/// there is exactly one sleep and the second attempt is not followed by one.
pub const TRANSIENT_FETCH_BACKOFF: [Duration; 1] = [Duration::from_millis(300)];

/// `updater.check_update(current_version, timeout=2.5)` (`updater.py:386`).
pub const DEFAULT_CHECK_TIMEOUT: Duration = Duration::from_millis(2500);

/// `readmd.py:137-138`, the startup probe's own pair of URLs.  They duplicate
/// `GITHUB_API_LATEST`/`GITHUB_API_RELEASES` verbatim in Python too.
pub const UPGRADE_RELEASE_URL: &str = GITHUB_API_LATEST;
pub const UPGRADE_RELEASES_URL: &str = GITHUB_API_RELEASES;

/// `readmd.py:164`, `urlopen(req, timeout=4)` — the startup probe is on the boot
/// path, so it gets a shorter budget than the in-app check and no retry.
pub const UPGRADE_CHECK_TIMEOUT: Duration = Duration::from_secs(4);

// ------------------------------------------------------------------- plumbing

/// One egress answer, *without* redirect following — which is the whole point of
/// `_NoRedirectHandler` (`updater.py:284-287`): the 302 must survive as an
/// answer so its `Location` can be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    /// `resp.headers.get('Location') or ''` (`updater.py:310`, `updater.py:312`).
    ///
    /// A 302 that urllib was told not to follow arrives as `HTTPError`, whose
    /// `.headers` is the very same header block — so the `try` arm and the
    /// `except HTTPError` arm differ only in where the headers came from.  One
    /// `Reply` per attempt collapses both arms into this lookup.
    pub fn location(&self) -> String {
        header_lookup(&self.headers, "Location")
            .unwrap_or("")
            .to_string()
    }
}

/// The two ways an attempt can fail, kept distinct because Python keeps them
/// distinct (`updater.py:238-246`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// DNS, refused connection, TLS reset, timeout — `urllib`'s `URLError` /
    /// `socket.timeout`.  `_open_with_retry` retries these once; the sniff and
    /// manifest loops skip the candidate and move on.
    Transport(String),
    /// The response arrived but could not be understood: invalid JSON, or a body
    /// that is not decodable.  Reported to the UI as `update_response_invalid`.
    Invalid(String),
}

/// `http.client.HTTPMessage.get(name)`: header names are **case-insensitive**
/// and a repeated header yields its **first** value (`get_all` is the only way
/// to see the rest).  Rust's default map behaviour would get both wrong, so the
/// lookup is explicit.
pub fn header_lookup<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn truthy(value: Option<&Value>) -> bool {
    value.map(crate::link_indexer::py_truthy).unwrap_or(false)
}

/// `^\s*` on its own: Python's 29-code-point whitespace set, leading side only.
/// [`crate::link_indexer::py_strip`] is both-ended, which is what `.strip()`
/// wants but not what a left-anchored `\s*` consumes.
fn py_strip_start(text: &str) -> &str {
    text.trim_start_matches(crate::link_indexer::py_isspace as fn(char) -> bool)
}

/// Python's `str(value or '')` (`versioning.py:14`): falsy → `''`, otherwise the
/// value's own `str()`.
fn py_str_or_empty(value: Option<&Value>) -> String {
    match value {
        None => String::new(),
        Some(v) if !truthy(Some(v)) => String::new(),
        Some(v) => crate::mdexport::py_str(v),
    }
}

/// `os.environ.get('CI') == 'true' or os.environ.get('GITHUB_ACTIONS') == 'true'`
/// (`updater.py:295`, `updater.py:336`).  Exact, case-sensitive string equality:
/// CI=1, CI=yes and CI=True all count as *not* CI here, in Python and here.
pub fn is_ci_environment() -> bool {
    std::env::var("CI").as_deref() == Ok("true")
        || std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
}

// ------------------------------------------------- _sniff_latest_tag_redirect

/// The tag pattern, `re.compile(r'/releases/tag/([^/?#\s]+)')` (`updater.py:302`).
///
/// Hand-rolled rather than run through the `regex` crate because Python's `\s`
/// in a `str` pattern is `Py_UNICODE_ISSPACE`, which is **29** code points and
/// includes `U+001C..U+001F`; the `regex` crate's `\s` is the Unicode
/// `White_Space` property, which is **25** and excludes them.  Measured with
/// `.qoder-scratch/updater-s1/oracle_ws.py`.  Using `\s` would let a Location
/// of `.../tag/v2.4\x1cjunk` sniff the bogus tag `v2.4\x1cjunk` where CPython
/// stops at `v2.4`.  [`crate::link_indexer::py_isspace`] is already the exact
/// 29-code-point set, so the stop test is that plus the three literal
/// delimiters.
fn tag_stop_char(c: char) -> bool {
    matches!(c, '/' | '?' | '#') || crate::link_indexer::py_isspace(c)
}

/// `tag_re.search(location)` + `.group(1).strip()` + the `if tag:` gate
/// (`updater.py:318-321`).
///
/// `search` takes the **leftmost** match, and `[^/?#\s]+` requires at least one
/// character — so an occurrence of the marker followed immediately by a stop
/// character fails to match and the scan continues to the next occurrence.  The
/// loop below reproduces that exactly.
///
/// Note two deliberate non-behaviours, both Python's:
/// * a **relative** `Location` still works.  The regex only searches, so
///   `/Natsummerance/readMD/releases/tag/v9.9.9` yields `v9.9.9`.  Nothing
///   resolves it against the request URL.
/// * the capture is **not** percent-decoded.  `.../tag/%20v2` yields the literal
///   string `%20v2`.
pub fn extract_release_tag(location: &str) -> Option<String> {
    const MARKER: &str = "/releases/tag/";
    let mut from = 0usize;
    while let Some(offset) = location[from..].find(MARKER) {
        let start = from + offset + MARKER.len();
        from = start;
        let tail = &location[start..];
        let end = tail
            .char_indices()
            .find(|(_, c)| tag_stop_char(*c))
            .map(|(i, _)| i)
            .unwrap_or(tail.len());
        let captured = &tail[..end];
        if captured.is_empty() {
            // `[^/?#\s]+` cannot match here; try the next occurrence.
            continue;
        }
        // `.group(1).strip()` — `str.strip()` is CPython's 29-code-point set, so
        // NBSP and U+3000 go too, not just ASCII space.
        let tag = crate::link_indexer::py_strip(captured);
        if tag.is_empty() {
            return None;
        }
        return Some(tag.to_string());
    }
    None
}

/// The candidate list of `updater.py:294-300`: the official endpoint always, and
/// the two mirror prefixes **in `MIRROR_PREFIXES` order** only off CI.  CI skips
/// mirrors because a shared runner must not depend on a third-party accelerator.
/// `MIRROR_PREFIXES[2]` (`mirror.ghproxy.com`) is not in this list — only the
/// first two are sniffed.
pub fn sniff_candidates(ci: bool) -> Vec<String> {
    let base = format!("https://github.com/{}/releases/latest", GITHUB_REPO);
    let mut candidates = vec![base.clone()];
    if !ci {
        candidates.push(format!("{}{}", MIRROR_PREFIXES[0], base));
        candidates.push(format!("{}{}", MIRROR_PREFIXES[1], base));
    }
    candidates
}

/// `updater._NoRedirectHandler` (`updater.py:284-287`) — urllib's
/// `HTTPRedirectHandler` subclass whose only override is `redirect_request()`
/// returning `None`, which means "do not follow, raise `HTTPError` instead".
///
/// It carries **no state** in Python, so it carries none here either; the type
/// exists because that is what the Python side names, and the observable
/// behaviour is entirely the [`Reply`] shape: a 3xx is an *answer*, not a
/// failure.  See [`redirect_request`], which is the override itself.
pub struct NoRedirectHandler;

impl NoRedirectHandler {
    /// `updater.py:285-286`.
    pub fn redirect_request() -> Option<Reply> {
        redirect_request()
    }
}

/// `updater._NoRedirectHandler.redirect_request()` (`updater.py:285-286`).
/// Returning `None` is urllib's "do not follow, raise `HTTPError` instead".
pub fn redirect_request() -> Option<Reply> {
    None
}

/// `updater._sniff_latest_tag_redirect(timeout=2.5)` (`updater.py:289-327`).
///
/// Failure taxonomy, per candidate:
/// * `Err(Transport)` — `updater.py:313-315`, `except Exception`: log at debug
///   and `continue` to the next mirror.  Not an error the caller sees.
/// * `Ok(_)` with no usable `Location`, or a `Location` with no tag — try the
///   next candidate.
/// * every candidate exhausted → `None`, which Python treats as "no tier-2
///   answer" and `check_update` then reports as `update_network_error`.
///
/// The `Request` is built with only `User-Agent` (`updater.py:306`); no `Accept`
/// header, unlike the JSON fetch.
pub fn sniff_latest_tag_redirect(
    ci: bool,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
) -> Option<String> {
    for url in sniff_candidates(ci) {
        let reply = match transport(&url) {
            Ok(reply) => reply,
            Err(_) => continue,
        };
        let location = reply.location();
        if location.is_empty() {
            continue;
        }
        if let Some(tag) = extract_release_tag(&location) {
            return Some(tag);
        }
    }
    None
}

// ---------------------------------------------------- _fetch_manifest_assets

/// `updater.py:335-341`, the manifest candidates for one tag: official first,
/// then the same two mirror prefixes, again skipped under CI.
pub fn manifest_candidates(tag_name: &str, ci: bool) -> Vec<String> {
    let base = format!(
        "https://github.com/{}/releases/download/{}/SHA256SUMS.txt",
        GITHUB_REPO, tag_name
    );
    let mut urls = vec![base.clone()];
    if !ci {
        urls.push(format!("{}{}", MIRROR_PREFIXES[0], base));
        urls.push(format!("{}{}", MIRROR_PREFIXES[1], base));
    }
    urls
}

/// The gate at `updater.py:355`: `if manifest_text and ('ReadMD' in manifest_text
/// or len(manifest_text) > 20)`.
///
/// `len()` on a `str` counts **code points**, not bytes — a 21-character
/// CJK manifest is 63 UTF-8 bytes and passes on `len`, and a 21-emoji manifest
/// (84 bytes) passes too.  Measured with
/// `.qoder-scratch/updater-s1/oracle_pure.py` section 3.  Using `str::len` here
/// would accept any 21-byte gibberish page (a 404 HTML stub, a captive-portal
/// notice) whose character count is well under 20.
pub fn manifest_text_is_acceptable(text: &str) -> bool {
    !text.is_empty() && (text.contains("ReadMD") || text.chars().count() > 20)
}

/// One line of `^\s*\*?([A-Fa-f0-9]{64})\s+\*?(.+?)\s*$` (`updater.py:366`),
/// returning the digest as written (the caller lowercases it) and the filename
/// after Python's `.strip()`.
///
/// Hand-rolled, and the backtracking is the interesting part.  Every case below
/// is a measured CPython answer from
/// `.qoder-scratch/updater-s1/oracle_manifest.py`:
///
/// | input (after the 64 hex chars) | CPython `group(2).strip()` |
/// |---|---|
/// | `" ReadMDSetup.exe"`           | `"ReadMDSetup.exe"` |
/// | `" *ReadMDSetup.exe"`          | `"ReadMDSetup.exe"` — `\*?` is *greedy*, so the binary-mode star is eaten |
/// | `" **x"`                       | `"*x"` — only the first star is eaten |
/// | `" *"`                         | `"*"` — eating it would leave `.+?` nothing to match, so the engine backtracks |
/// | `" "` (one space)              | **no match** — `\s+` eats it and `.+?` starves |
/// | `"  "` (two spaces)            | `""` — the shortest tail that can match at all |
/// | `"   "` (three spaces)         | `""` — `\s+` eats two, `.+?` takes the third |
/// | `"\t"` / `" \t"`               | **no match** / `""` — `\s` is a *class*; its run length decides |
/// | `""`                           | **no match** |
/// | 63 hex chars                   | no match |
/// | 64 hex chars then `123`        | no match (`\s+` must follow immediately) |
/// | `" x.exe\u3000"`               | `"x.exe"` — U+3000 is `\s`, so it is trailing whitespace |
/// | `"\xa0x.exe"`                  | `"x.exe"` — NBSP satisfies `\s+` |
///
/// The filename is **not** filtered when empty: `updater.py:367-374` appends
/// whatever `group(2).strip()` produced, so a `"digest   "` line really does
/// yield an asset named `""` — and since that list is then non-empty, the
/// synthetic `SHA256SUMS.txt` entry is appended after it too.  A one-space tail
/// yields nothing at all.  `resolve_expected_sha`'s twin in [`crate::validators`]
/// rejects empty names; this function may not.
pub fn manifest_line_parts(line: &str) -> Option<(String, String)> {
    // `^\s*`
    let body = py_strip_start(line);
    // `\*?` — greedy optional, and the digest can never start with `*`, so
    // eating it is always the branch that succeeds when a star is present.
    let body = body.strip_prefix('*').unwrap_or(body);
    let chars: Vec<char> = body.chars().collect();
    if chars.len() < 64 {
        return None;
    }
    let digest: String = chars[..64].iter().collect();
    if !digest
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f' | 'A'..='F'))
    {
        return None;
    }
    // `\s+` — at least one whitespace character must follow the digest.
    let rest: Vec<char> = chars[64..].to_vec();
    if rest.is_empty() || !crate::link_indexer::py_isspace(rest[0]) {
        return None;
    }
    let mut run = 0usize;
    while run < rest.len() && crate::link_indexer::py_isspace(rest[run]) {
        run += 1;
    }
    if run == rest.len() {
        // The whole tail is whitespace.  `(.+?)` still needs *one* character, so
        // `\s+` may only give one back when the run is at least two long:
        // CPython matches `"digest  "` (`group(2) == " "` -> `.strip()` -> `""`,
        // the real phantom asset) and does **not** match `"digest "` at all —
        // the single space is eaten by `\s+` and `.+?` starves.  Measured with
        // the live `re` in scratch/rust_parity/updater_fix_s12/REPORT.md (F-3);
        // the table above already documents the same split.
        if run < 2 {
            return None;
        }
        return Some((digest, String::new()));
    }
    let mut name: Vec<char> = rest[run..].to_vec();
    // `\*?`, greedy, then `(.+?)` needs >= 1 char.
    if name.len() > 1 && name[0] == '*' {
        name.remove(0);
    }
    // `\s*$` consumes the trailing whitespace run; `.+?` cannot go empty because
    // `name[0]` is not whitespace (the run above was maximal).
    let mut end = name.len();
    while end > 0 && crate::link_indexer::py_isspace(name[end - 1]) {
        end -= 1;
    }
    let raw: String = name[..end].iter().collect();
    // `.strip()` also clears a star-separated leading run, e.g. `" * x.exe"`.
    let stripped = crate::link_indexer::py_strip(&raw).to_string();
    Some((digest, stripped))
}

/// The asset list `updater.py:364-383` builds out of a manifest.
///
/// Order is Python's: every parsed line in file order, then — only when at least
/// one line parsed — a synthetic `SHA256SUMS.txt` entry carrying
/// `len(manifest_text)` as its size.  A manifest that lists `SHA256SUMS.txt`
/// itself therefore produces **two** assets of that name; that duplication is
/// real Python behaviour and is preserved.
///
/// The synthetic entry has no `expected_sha` key at all, so the payload's
/// `best_asset.get('expected_sha')` test (`updater.py:437`) is a *missing key*,
/// not a `null` — the three states stay distinct.
pub fn parse_manifest_assets(manifest_text: &str, tag_name: &str) -> Vec<Value> {
    let mut assets: Vec<Value> = Vec::new();
    for line in crate::link_indexer::py_splitlines(manifest_text) {
        if let Some((digest, filename)) = manifest_line_parts(line) {
            let mut asset = Map::new();
            asset.insert("name".to_string(), json!(filename));
            asset.insert(
                "browser_download_url".to_string(),
                json!(format!(
                    "https://github.com/{}/releases/download/{}/{}",
                    GITHUB_REPO, tag_name, filename
                )),
            );
            asset.insert("expected_sha".to_string(), json!(digest.to_lowercase()));
            asset.insert("size".to_string(), json!(0));
            assets.push(Value::Object(asset));
        }
    }
    if !assets.is_empty() {
        let mut sums = Map::new();
        sums.insert("name".to_string(), json!("SHA256SUMS.txt"));
        sums.insert(
            "browser_download_url".to_string(),
            json!(format!(
                "https://github.com/{}/releases/download/{}/SHA256SUMS.txt",
                GITHUB_REPO, tag_name
            )),
        );
        sums.insert("size".to_string(), json!(manifest_text.chars().count()));
        assets.push(Value::Object(sums));
    }
    assets
}

/// `updater._fetch_manifest_assets(tag_name, timeout=2.5)` (`updater.py:330-383`).
///
/// Returns `None` for "no manifest at all" and `Some(vec![])` for "a manifest
/// that yielded no assets" — Python's `return None` at `updater.py:361-362` versus
/// its `return assets` at `updater.py:383`.  The distinction is load-bearing:
/// `check_update` gates the tier-3 write-up on `if assets:` (`updater.py:411`),
/// which is false for both, but `resolve_expected_sha` distinguishes an absent
/// digest from an empty list and the caller reasons about the two differently.
///
/// A 200 from any candidate is *remembered*; the acceptance test
/// (`'ReadMD' in text or len(text) > 20`) only decides whether to stop looking
/// early.  A candidate that raises leaves the previous body untouched, and a
/// non-200 never writes one (`updater.py:348-362`).  The body is decoded
/// `utf-8, errors='replace'` — Rust's `String::from_utf8_lossy` is the same
/// U+FFFD policy.
pub fn fetch_manifest_assets(
    tag_name: &str,
    ci: bool,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
) -> Option<Vec<Value>> {
    if tag_name.is_empty() {
        return None;
    }
    let mut manifest_text: Option<String> = None;
    for url in manifest_candidates(tag_name, ci) {
        let reply = match transport(&url) {
            Ok(reply) => reply,
            Err(_) => continue,
        };
        // `status = getattr(resp, 'status', 200)` then `if status == 200`.  An
        // object with no `status` attribute defaults to 200 in Python; here the
        // 3xx case that urllib would have raised through arrives as a non-200
        // `Reply` and is rejected, which is the same observable outcome.
        if reply.status != 200 {
            continue;
        }
        // `updater.py:349-356` — the assignment is **not** gated by the
        // acceptance test, only the `break` is.  So a short or contentless 200
        // still overwrites whatever an earlier candidate produced, and a later
        // candidate that raises leaves the previous body in place rather than
        // clearing it.  Getting this wrong makes "every mirror 404s after the
        // origin served a 5-byte stub" return `None` where Python returns the
        // stub parsed into (zero) assets.
        let text = String::from_utf8_lossy(&reply.body).into_owned();
        manifest_text = Some(text.clone());
        if manifest_text_is_acceptable(&text) {
            break;
        }
    }
    let text = manifest_text?;
    // `updater.py:361-362` — `if not manifest_text: return None`.  A body that
    // decoded to the *empty string* is falsy in Python, so it means "no
    // manifest at all", not "a manifest with zero assets".  Only a non-empty
    // body that parses into nothing reaches `return assets == []`.
    if text.is_empty() {
        return None;
    }
    Some(parse_manifest_assets(&text, tag_name))
}

/// The key order of the release document `updater.py:412-419` writes when tier 3
/// recovers — CPython's dict literal order, measured as
/// `['tag_name','name','body','published_at','html_url','assets']`.
///
/// The container carries it: `serde_json` is built with `preserve_order`, so a
/// `Value` keeps the order it was built in.  Nothing in Python reads this
/// document by iteration
/// (only `data.get(...)`), so the observable contract is the key **set**; the
/// constant exists so that set, and the order it came from, are pinned in one
/// place rather than being folklore in a test.
pub const FALLBACK_DOC_KEYS: [&str; 6] =
    ["tag_name", "name", "body", "published_at", "html_url", "assets"];

/// The tier-2/tier-3 block of `updater.check_update` (`updater.py:405-421`):
/// sniff a tag, rebuild the assets from its `SHA256SUMS.txt`, and synthesise the
/// release document the payload builder expects.
///
/// The synthesised keys and their values are Python's, verbatim — including
/// `name` and `body` both being `f'ReadMD {tag}'`, `published_at` being `""`,
/// and `html_url` pointing at `/releases/tag/<tag>`.  `timeout` is threaded to
/// both calls in Python; here the transport closures carry it.
pub fn check_update_fallback(
    ci: bool,
    sniff: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    manifest: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
) -> Option<Value> {
    // `updater.py:285-286` — the handler is stateless; asserted here so the port
    // keeps the same shape a reader can check against the Python class.
    debug_assert!(NoRedirectHandler::redirect_request().is_none());
    let sniffed_tag = sniff_latest_tag_redirect(ci, sniff)?;
    let assets = fetch_manifest_assets(&sniffed_tag, ci, manifest)?;
    if assets.is_empty() {
        return None;
    }
    let mut data = Map::new();
    data.insert("tag_name".to_string(), json!(sniffed_tag));
    data.insert("name".to_string(), json!(format!("ReadMD {}", sniffed_tag)));
    data.insert("body".to_string(), json!(format!("ReadMD {}", sniffed_tag)));
    data.insert("published_at".to_string(), json!(""));
    data.insert(
        "html_url".to_string(),
        json!(format!(
            "https://github.com/{}/releases/tag/{}",
            GITHUB_REPO, sniffed_tag
        )),
    );
    data.insert("assets".to_string(), Value::Array(assets));
    Some(Value::Object(data))
}

// -------------------------------------------------------------- fetch helpers

/// `updater._open_with_retry(url, timeout)` (`updater.py:233-246`).
///
/// Python's retry taxonomy, reproduced exactly:
/// * `HTTPError` (any HTTP status, 4xx/5xx included) is **re-raised on the first
///   attempt** — "4xx/5xx 是服务端明确响应，重试无意义".  A 500 must not cost
///   the boot path another 2.5 s.
/// * anything else (`URLError`, TLS reset, timeout, DNS) is stored and retried,
///   sleeping `TRANSIENT_FETCH_BACKOFF[attempt]` first — one sleep, because the
///   tuple has one element.
/// * after the loop the *last* exception is re-raised.
///
/// The sleep only happens between attempts, so a test transport can assert the
/// attempt count without paying real time twice.
pub fn open_with_retry(
    url: &str,
    attempts: usize,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    sleep: &mut dyn FnMut(Duration),
) -> Result<Reply, FetchError> {
    let mut last: Option<FetchError> = None;
    for attempt in 0..attempts.max(1) {
        match transport(url) {
            // A status answer of any kind is a definitive answer.
            Ok(reply) => return Ok(reply),
            Err(err @ FetchError::Invalid(_)) => return Err(err),
            Err(err) => {
                if attempt + 1 < attempts {
                    sleep(TRANSIENT_FETCH_BACKOFF[attempt % TRANSIENT_FETCH_BACKOFF.len()]);
                }
                last = Some(err);
            }
        }
    }
    Err(last.unwrap_or_else(|| FetchError::Transport("update_network_error".to_string())))
}

/// `updater._fetch_release_json(url, timeout=5)` (`updater.py:249-255`): the
/// `Accept: application/vnd.github.v3+json` header, then `json.loads` **only**
/// when the status is 200 — a non-200 falls out of the `with` block and returns
/// `None` in Python, which the caller reads as "no data".
pub fn fetch_release_json(
    url: &str,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    sleep: &mut dyn FnMut(Duration),
) -> Result<Option<Value>, FetchError> {
    let reply = open_with_retry(url, TRANSIENT_FETCH_ATTEMPTS, transport, sleep)?;
    if reply.status != 200 {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&reply.body).into_owned();
    serde_json::from_str::<Value>(&text)
        .map(Some)
        .map_err(|e| FetchError::Invalid(format!("update_response_invalid: {e}")))
}

/// `updater._fetch_text(url, timeout=5)` (`updater.py:258-262`) — the plain-text
/// twin used for `SHA256SUMS.txt`, decoded `utf-8, errors='replace'`.
pub fn fetch_text(
    url: &str,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    sleep: &mut dyn FnMut(Duration),
) -> Result<Option<String>, FetchError> {
    let reply = open_with_retry(url, TRANSIENT_FETCH_ATTEMPTS, transport, sleep)?;
    if reply.status != 200 {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&reply.body).into_owned(),
    ))
}

// -------------------------------------------------------- flavor and matching

/// `updater.detect_app_flavor` (`updater.py:107-121`) as a **pure** decision over
/// the three values Python actually reads: `sys.platform`,
/// `getattr(sys, 'frozen', False)` and `sys.executable`.  All five of Python's
/// returns are reachable here; the rows are measured CPython answers:
///
/// | platform | frozen | `sys.executable` | return |
/// |---|---|---|---|
/// | `win32` | `False` | anything, incl. `''` | **`source`** (`updater.py:119`) |
/// | `win32` | `True` | `…\ReadMD-portable.exe` | `win_portable` |
/// | `win32` | `True` | `…\ReadMDSetup.exe` / `''` | `win_installer` |
/// | `darwin` | either | ignored | `macos` |
/// | `linux` / anything else | either | ignored | `linux` |
///
/// `source` is not cosmetic: `match_release_asset` folds it into the installer
/// arm (`updater.py:146`), but it *is* what `check_update` reports as `flavor` and
/// what `_validate_ready_update` compares a requested flavor against
/// (`updater.py:781-783`), where `win_installer` and `source` are different
/// answers.
pub fn detect_app_flavor_from(platform: &str, frozen: bool, executable: &str) -> String {
    if platform == "darwin" {
        return "macos".to_string();
    }
    if platform == "win32" {
        if !frozen {
            return "source".to_string();
        }
        // `os.path.basename(exe).lower()` — `ntpath.basename` splits on *both*
        // separators, and `''` basenames to `''`, which is not `portable` and so
        // lands on `win_installer`.
        let exe_name = executable
            .rsplit(|c| c == '/' || c == '\\')
            .next()
            .unwrap_or("")
            .to_lowercase();
        return if exe_name.contains("portable") {
            "win_portable".to_string()
        } else {
            "win_installer".to_string()
        };
    }
    "linux".to_string()
}

/// `updater.detect_app_flavor` (`updater.py:107-121`) for **this** process.
///
/// The kernel binary is the packaged app, so it reports `frozen = true`, which is
/// why this probe never answers `source` on Windows.  `source` is Python's answer
/// for a
/// win32 run from a checkout (`sys.frozen` absent) — measurable on this very box,
/// where the authority returns `source` — and it is pinned by
/// [`detect_app_flavor_from`], not hidden.
pub fn detect_app_flavor() -> String {
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(windows) {
        "win32"
    } else {
        "linux"
    };
    let executable = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    detect_app_flavor_from(platform, true, &executable)
}

/// `platform.machine().lower()` (`updater.py:129`).  CPython reports `AMD64` on
/// Windows x86-64 where `consts::ARCH` reports `x86_64`; the tokens below only
/// matter on the `linux` flavor, so the platform default is kept.
pub fn platform_machine() -> String {
    std::env::consts::ARCH.to_lowercase()
}

/// `name = a.get('name', '')` (`updater.py:136`, `:141`, `:190`) as CPython then
/// uses it: `name.upper()` (`:137`) / `name.lower()` (`:141`).  A mapping whose
/// `name` is **not a string** therefore raises `AttributeError` — the `.get`
/// default only covers an *absent* key, never an explicit `null`.  `None` here
/// means "CPython raises", and the caller turns it into
/// `update_response_invalid` (`updater.py:464-466`).
///
/// Measured with CPython 3.11.15 (`scratch/updater_parity_nonstring_name.py`):
///
/// | `asset['name']` | `match_release_asset([asset], 'win_installer')` |
/// |---|---|
/// | key absent / `''` | returns `(None, None)` — the `''` default |
/// | `'ReadMDSetup.exe'` | returns the asset |
/// | `5` / `0` | `AttributeError: 'int' object has no attribute 'upper'` |
/// | `null` | `AttributeError: 'NoneType' object has no attribute 'upper'` |
/// | `true` | `AttributeError: 'bool' object has no attribute 'upper'` |
/// | `2.5` | `AttributeError: 'float' object has no attribute 'upper'` |
/// | `['a']` / `{'k': 1}` | `AttributeError: 'list'/'dict' …` |
///
/// A non-mapping element is included too: `a.get` is itself an `AttributeError`
/// there, which is the same answer [`release_asset_list`] already gives earlier.
fn python_asset_name(asset: &Value) -> Option<String> {
    let map = match asset.as_object() {
        Some(map) => map,
        None => return None,
    };
    match map.get("name") {
        None => Some(String::new()),
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => None,
    }
}

/// `updater.match_release_asset(assets, flavor)` (`updater.py:124-201`).
///
/// Python's two passes and every early-out are kept, including the dead
/// `'.AppImage'` comparison at `updater.py:190` — Python lower-cases the name
/// first, so that branch can never fire and the fallback is `.deb` only.
///
/// The `Option` is CPython's exception channel: `None` means one of the asset
/// rows carried a non-string `name` and `name.upper()` raised inside the `try`
/// at `updater.py:430`, so the endpoint answers
/// `{'ok': False, 'error_code': 'update_response_invalid'}`.  Because pass 1
/// walks the whole list before pass 2 does, a poison row raises whatever its
/// position (`updater.py:135`, `:140`).
pub fn match_release_asset(
    assets: &[Value],
    flavor: &str,
) -> Option<(Option<Value>, Option<Value>)> {
    let machine = platform_machine();
    let is_arm = machine.contains("arm") || machine.contains("aarch64");
    let mut selected: Option<Value> = None;
    let mut sha_asset: Option<Value> = None;

    for asset in assets {
        let name = python_asset_name(asset)?.to_uppercase();
        if name == "SHA256SUMS.TXT" || name.contains("SHA256") {
            sha_asset = Some(asset.clone());
        }
    }

    for asset in assets {
        let name = python_asset_name(asset)?.to_lowercase();
        if flavor == "win_portable" {
            if name.contains("portable") && (name.ends_with(".exe") || name.ends_with(".zip")) {
                selected = Some(asset.clone());
                break;
            }
        } else if flavor == "win_installer" || flavor == "source" {
            if (name.contains("setup") || name.contains("readmdsetup")) && name.ends_with(".exe") {
                selected = Some(asset.clone());
                break;
            } else if selected.is_none() && name.ends_with(".exe") && !name.contains("portable") {
                selected = Some(asset.clone());
            }
        } else if flavor == "macos" {
            if name.ends_with(".zip") || name.ends_with(".dmg") {
                if is_arm && name.contains("arm64") {
                    selected = Some(asset.clone());
                    break;
                } else if !is_arm
                    && (name.contains("x64") || name.contains("x86_64") || name.contains("intel"))
                {
                    selected = Some(asset.clone());
                    break;
                } else if selected.is_none() && name.contains("macos") {
                    selected = Some(asset.clone());
                }
            }
        } else if flavor == "linux" {
            let (deb_token, appimage_token): (&str, &str) = if machine.contains("loongarch") {
                ("loongarch64", "loongarch64")
            } else if machine.contains("mips") {
                ("mips64el", "mips64el")
            } else if machine.contains("sw_64") || machine.contains("sw64") {
                ("sw64", "sw64")
            } else if machine.contains("riscv") {
                ("riscv64", "riscv64")
            } else if machine.starts_with("armv") || machine.contains("armhf") {
                ("armhf", "armhf")
            } else if is_arm || machine == "aarch64" || machine == "arm64" {
                ("arm64", "aarch64")
            } else {
                ("amd64", "x86_64")
            };
            if name.ends_with(&format!("_{}.deb", deb_token)) {
                selected = Some(asset.clone());
                break;
            } else if name.contains(&format!("-{}-", appimage_token)) && name.ends_with(".appimage")
            {
                selected = Some(asset.clone());
                break;
            } else if selected.is_none() && name.ends_with(".deb") {
                selected = Some(asset.clone());
            }
        }
    }

    // `updater.py:187-199` — "如果没匹配到精准架构，选取最接近的 exe 或 zip".
    if selected.is_none() && !assets.is_empty() {
        for asset in assets {
            let name = python_asset_name(asset)?.to_lowercase();
            if cfg!(windows) && name.ends_with(".exe") {
                selected = Some(asset.clone());
                break;
            } else if cfg!(target_os = "macos") && name.ends_with(".zip") {
                selected = Some(asset.clone());
                break;
            } else if cfg!(target_os = "linux") && name.ends_with(".deb") {
                selected = Some(asset.clone());
                break;
            }
        }
    }

    Some((selected, sha_asset))
}

/// `updater.resolve_expected_sha` (`updater.py:265-281`), with its single socket
/// call folded into `manifest`.  The `if not sha_url or not asset_name` guard
/// runs **before** any request (`updater.py:267-268`).
pub fn resolve_expected_sha(
    sha_url: Option<&str>,
    asset_name: Option<&str>,
    manifest: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<Value> {
    match (sha_url, asset_name) {
        (Some(url), Some(name)) if !url.is_empty() && !name.is_empty() => {
            let text = manifest(url)?;
            crate::validators::sha256sums_lookup(&text, name).map(Value::String)
        }
        _ => None,
    }
}

/// `updater._release_check_urls` (`updater.py:100-104`).
pub fn release_check_urls(current_version: &str) -> Vec<String> {
    let current = crate::validators::parse_version(current_version);
    let is_prerelease = current.map(|v| v.rank == 0).unwrap_or(false);
    vec![if is_prerelease {
        GITHUB_API_RELEASES.to_string()
    } else {
        GITHUB_API_LATEST.to_string()
    }]
}

/// `updater.parse_semver` (`updater.py:89-92`) — the 3-tuple form, with the
/// `(0, 0, 0)` fallback for anything unparseable.
pub fn parse_semver(value: Option<&Value>) -> [u64; 3] {
    let text = py_str_or_empty(value);
    match crate::validators::parse_version(&text) {
        Some(version) => version.core,
        None => [0, 0, 0],
    }
}

/// `assets = data.get('assets', [])` (`updater.py:434`) **as `match_release_asset`
/// then iterates it** (`updater.py:135`, `140`, `189`): `None` here means "the
/// loop raised", which `check_update`'s `except` (`updater.py:464-466`) answers as
/// `update_response_invalid`.  Every row below is a measured CPython answer (see
/// `scratch/rust_parity/updater_fix_s12/REPORT.md`, F-4):
///
/// | `data['assets']` | Python |
/// |---|---|
/// | key absent                          | `[]` default, no error |
/// | `[]`, `{}`, `""`                    | iterates to nothing → `ok: true`, `asset: null` |
/// | `[{'name': 'ReadMDSetup.exe'}]`     | normal path |
/// | `'abc'`                             | `AttributeError: 'str' object has no attribute 'get'` |
/// | `{'k': 1}`                          | same, on the key |
/// | `5` / `None` / `True`               | `TypeError: 'X' object is not iterable` |
/// | `['x']` / `[5]` / `[True]`          | `AttributeError` on the element (only mappings answer `.get`) |
///
/// Scope note: this answers the *container* question only.  The row question —
/// `[{'name': 5}]`, where `name.upper()` raises `AttributeError: 'int' object has
/// no attribute 'upper'` and `updater.py:464-466` answers the same two keys — is
/// answered by [`match_release_asset`], which is measured against CPython there.
pub(crate) fn release_asset_list(value: Option<&Value>) -> Option<Vec<Value>> {
    match value {
        None => Some(Vec::new()),
        Some(Value::Array(list)) => {
            if list.iter().all(Value::is_object) {
                Some(list.clone())
            } else {
                None
            }
        }
        // Empty containers iterate to nothing, so neither `for a in assets` ever
        // runs and `if not selected and assets:` (`updater.py:188`) is false too.
        Some(Value::Object(map)) if map.is_empty() => Some(Vec::new()),
        Some(Value::String(text)) if text.is_empty() => Some(Vec::new()),
        Some(_) => None,
    }
}

/// The pure tail of `updater.check_update` (`updater.py:423-466`).
pub fn release_check_payload(
    current_version: &str,
    data: Option<&Value>,
    last_error_code: &str,
    manifest: &mut dyn FnMut(&str) -> Option<String>,
) -> Value {
    let data = match data {
        None => {
            return json!({
                "ok": false,
                "error_code": if last_error_code.is_empty() {
                    "update_network_error".to_string()
                } else {
                    last_error_code.to_string()
                },
                "html_url": format!("https://github.com/{}/releases", GITHUB_REPO),
            })
        }
        Some(d) => d,
    };
    // `latest_tag = data.get('tag_name', '')` (`updater.py:431`) is a *raw* echo:
    // absent key is `""`, explicit `null` is `null`, a number stays a number.
    let latest_tag: Value = data.get("tag_name").cloned().unwrap_or_else(|| json!(""));
    let has_update = crate::validators::is_newer_version(&py_str_or_empty(Some(&latest_tag)), current_version);
    let flavor = detect_app_flavor();
    // `assets = data.get('assets', [])` (`updater.py:434`) is then *iterated* by
    // `match_release_asset` (`updater.py:135`), so a str/dict/int/None raises, and
    // so does a row whose `name` is not a string (`name.upper()`, `updater.py:137`).
    // Both raise inside the `try` at `updater.py:430`, and `updater.py:464-466`
    // answers with exactly two keys.  Absorbing either into `Vec::new()` / `""`
    // here hid that.
    let (best_asset, sha_asset) = match release_asset_list(data.get("assets"))
        .and_then(|assets| match_release_asset(&assets, &flavor))
    {
        Some(matched) => matched,
        None => return json!({"ok": false, "error_code": "update_response_invalid"}),
    };
    let sha_url = sha_asset
        .as_ref()
        .and_then(|a| a.get("browser_download_url"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let asset_name = best_asset
        .as_ref()
        .and_then(|a| a.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string);

    // `updater.py:436-444`: `if best_asset and best_asset.get('expected_sha')` is
    // a truthiness test on the *raw* value — the tier-3 assets are the only
    // producers of that key.
    let mut expected_sha = best_asset
        .as_ref()
        .and_then(|a| a.get("expected_sha"))
        .filter(|v| truthy(Some(v)))
        .cloned();
    if expected_sha.is_none() {
        expected_sha = resolve_expected_sha(sha_url.as_deref(), asset_name.as_deref(), manifest);
    }

    json!({
        "ok": true,
        "has_update": has_update,
        "current_version": current_version,
        "latest_version": latest_tag.clone(),
        "release_name": match data.get("name") {
            Some(name) if truthy(Some(name)) => name.clone(),
            _ => latest_tag.clone(),
        },
        "published_at": data.get("published_at").cloned().unwrap_or_else(|| json!("")),
        "release_notes": data.get("body").cloned().unwrap_or_else(|| json!("")),
        "html_url": data.get("html_url").cloned().unwrap_or_else(|| json!("")),
        "flavor": flavor,
        "asset": best_asset.map(|a| json!({
            "name": a.get("name").cloned().unwrap_or(Value::Null),
            "size": a.get("size").cloned().unwrap_or_else(|| json!(0)),
            "download_url": a.get("browser_download_url").cloned().unwrap_or(Value::Null),
            "expected_sha": expected_sha.clone().unwrap_or(Value::Null),
        })),
        "sha_url": sha_url.map(Value::String).unwrap_or(Value::Null),
    })
}

/// `updater.check_update(current_version, timeout=2.5)` (`updater.py:386-466`),
/// all three tiers.
///
/// * **tier 1** — the REST list, filtered by `select_update_release`.
/// * **tier 2** — `sniff_latest_tag_redirect`, only reached when
///   `not data or not data.get('tag_name')` (`updater.py:406`).  A falsy tag
///   (`""`, `null`, `0`) on a well-formed document still triggers the fallback.
/// * **tier 3** — `fetch_manifest_assets`, and the synthesised release document
///   replaces `data` only when the asset list is non-empty.
///
/// Both fallbacks swallow their own transport errors; if they come back empty
/// the caller sees `update_network_error`, exactly as Python does.
pub fn check_update(
    current_version: &str,
    ci: bool,
    json_fetch: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    sniff: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    manifest: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
    sha_manifest: &mut dyn FnMut(&str) -> Option<String>,
    sleep: &mut dyn FnMut(Duration),
) -> Value {
    let mut data: Option<Value> = None;
    let mut last_error_code = String::new();
    let mut has_tag = false;

    for url in release_check_urls(current_version) {
        match fetch_release_json(&url, json_fetch, sleep) {
            Ok(payload) => {
                let releases = match payload {
                    Some(Value::Array(list)) => list,
                    Some(value) => vec![value],
                    None => Vec::new(),
                };
                let picked = crate::validators::select_update_release(current_version, &releases);
                has_tag = picked
                    .as_ref()
                    .and_then(|d| d.get("tag_name"))
                    .map(|v| truthy(Some(v)))
                    .unwrap_or(false);
                data = picked;
                if has_tag {
                    break;
                }
            }
            // "Keep provider/network details in logs only. The UI receives a
            // stable code" (`updater.py:398-403`).
            Err(_) => {
                last_error_code = "update_network_error".to_string();
                continue;
            }
        }
    }

    // `updater.py:406` — `if not data or not data.get('tag_name')`.  A falsy tag
    // on an otherwise well-formed document (`""`, `null`, `0`) reaches the
    // fallback exactly as it does in Python.
    if !(data.is_some() && has_tag) {
        if let Some(fallback) = check_update_fallback(ci, sniff, manifest) {
            data = Some(fallback);
        }
    }

    release_check_payload(current_version, data.as_ref(), &last_error_code, sha_manifest)
}

// ------------------------------------------------- readmd.py:check_latest_release

/// `_UPGRADE_CACHE` (`readmd.py:139`).  Python's is a module dict with two keys;
/// `done` is what makes a *negative* answer stick, so a laptop booting offline
/// does not re-probe GitHub on every tick.
#[derive(Debug, Clone, Default)]
pub struct UpgradeCache {
    pub done: bool,
    pub result: Option<Value>,
}

static UPGRADE_CACHE: std::sync::OnceLock<std::sync::Mutex<UpgradeCache>> = std::sync::OnceLock::new();

fn upgrade_cache() -> &'static std::sync::Mutex<UpgradeCache> {
    UPGRADE_CACHE.get_or_init(|| std::sync::Mutex::new(UpgradeCache::default()))
}

/// Test hook, and the twin of a suite that monkeypatches `_UPGRADE_CACHE` in
/// Python.  Production never calls it.
pub fn clear_upgrade_cache() {
    if let Ok(mut cache) = upgrade_cache().lock() {
        cache.done = false;
        cache.result = None;
    }
}

/// The pure body of `readmd.check_latest_release` (`readmd.py:150-179`), with
/// the cache and the socket taken out so the semantics can be pinned.
///
/// Differences from `updater.check_update` that matter and are Python's:
/// * its **own** URL pair and its **own** `User-Agent`, `ReadMD/%VERSION%`
///   (`readmd.py:161`), not `ReadMD-Updater`;
/// * `Accept: application/vnd.github+json` — no `.v3` (`readmd.py:162`);
/// * `timeout=4` and **no retry** at all: the whole body is one `try` whose
///   `except Exception` yields `None` silently (`readmd.py:175-176`), because
///   this runs on the boot path;
/// * the body is read as `resp.read(1024 * 1024)` — **truncated to 1 MiB before**
///   `json.loads`, so a oversized answer is a parse failure, not a success;
/// * it needs `current` to parse as well as the tag: `if latest_release and
///   current and (_compare_versions(tag, VERSION) or 0) > 0`.  The `or 0` turns
///   `compare_versions`'s `None` into `0`, i.e. "no update", never an error;
/// * `url` falls back to `_UPGRADE_RELEASE_URL` when the release carries no
///   usable `html_url`, and the result is only ever two keys, `latest` and `url`.
pub fn query_latest_release(
    version: &str,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
) -> Option<Value> {
    let parsed_current = crate::validators::parse_version(version)?;
    let current_is_prerelease = parsed_current.rank == 0;
    let url = if current_is_prerelease {
        UPGRADE_RELEASES_URL
    } else {
        UPGRADE_RELEASE_URL
    };
    let reply = transport(url).ok()?;
    // `if resp.status == 200` is not written, but `json.loads` on an error body
    // raises and the `except` swallows it into `None` — same outcome, and the
    // explicit test keeps the 1 MiB slice from ever being handed to a parser.
    if reply.status != 200 {
        return None;
    }
    // `resp.read(1024 * 1024).decode('utf-8')` — a *strict* decode, unlike the
    // `errors='replace'` used by `updater._fetch_text`.  Invalid UTF-8 raises and
    // the probe reports "no update".
    let window = if reply.body.len() > 1024 * 1024 {
        &reply.body[..1024 * 1024]
    } else {
        &reply.body[..]
    };
    let text = std::str::from_utf8(window).ok()?;
    let payload: Value = serde_json::from_str(text).ok()?;
    let releases = match payload {
        Value::Array(list) => list,
        value => vec![value],
    };
    let latest_release = crate::validators::select_update_release(version, &releases)?;
    let tag = py_str_or_empty(latest_release.get("tag_name"));
    // `current = _parse_version(VERSION)` then `if latest_release and current
    // and (...) > 0` (`readmd.py:169-170`).  The second parse is Python's, and
    // `current` is a real conjunct: an unparseable running version means "no
    // update" even when a release was selected.
    if crate::validators::parse_version(version).is_none() {
        return None;
    }
    let comparison = crate::validators::compare_versions(&tag, version).unwrap_or(0);
    if comparison <= 0 {
        return None;
    }
    // `str(latest_release.get('html_url') or _UPGRADE_RELEASE_URL)` — an absent
    // key, an explicit `null` and any other falsy value all take the fallback.
    let url = match latest_release.get("html_url") {
        Some(value) if truthy(Some(value)) => py_str_or_empty(Some(value)),
        _ => UPGRADE_RELEASE_URL.to_string(),
    };
    Some(json!({ "latest": tag, "url": url }))
}

/// `readmd.check_latest_release()` (`readmd.py:150-179`) — the cached wrapper.
///
/// A `None` result is cached too, which is the point: the probe runs at most
/// once per process whatever the outcome.
pub fn check_latest_release(
    version: &str,
    transport: &mut dyn FnMut(&str) -> Result<Reply, FetchError>,
) -> Option<Value> {
    if let Ok(cache) = upgrade_cache().lock() {
        if cache.done {
            return cache.result.clone();
        }
    }
    let result = query_latest_release(version, transport);
    if let Ok(mut cache) = upgrade_cache().lock() {
        cache.done = true;
        cache.result = result.clone();
    }
    result
}

// ------------------------------------------------------------- live transport

/// The real, in-process transport for the **sniff**: `ureq` with
/// `redirects(0)`, which is what `_NoRedirectHandler` asks urllib for.
///
/// ureq reports any non-2xx as `Err(Error::Status(code, response))`.  For this
/// caller that is an *answer* — GitHub's 302 body is irrelevant, its `Location`
/// is the whole payload — so both arms are folded into one [`Reply`] and only a
/// `Error::Request` (DNS, TLS, timeout) is a [`FetchError::Transport`].
pub fn sniff_reply_via_ureq(url: &str, timeout: Duration) -> Result<Reply, FetchError> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .user_agent(UPDATE_USER_AGENT)
        .build();
    to_reply(agent.get(url).call())
}

/// The real transport for the **manifest** and the REST fetch: redirects are
/// followed, because `_fetch_manifest_assets` uses plain `urlopen` and GitHub's
/// `/releases/download/…` answers with a 302 to a CDN.
pub fn redirecting_reply_via_ureq(url: &str, timeout: Duration) -> Result<Reply, FetchError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .user_agent(UPDATE_USER_AGENT)
        .build();
    to_reply(agent.get(url).call())
}

fn to_reply(
    result: Result<ureq::Response, ureq::Error>,
) -> Result<Reply, FetchError> {
    let (status, response) = match result {
        Ok(response) => (response.status(), response),
        Err(ureq::Error::Status(code, response)) => (code, response),
        Err(err) => return Err(FetchError::Transport(format!("{err:?}"))),
    };
    use std::io::Read;
    // ureq 2.12.1 exposes `headers_names()` (lower-cased, one entry per received
    // header, duplicates included) and `header(name)`, but no full iterator.
    // `header()` is already case-insensitive and first-match, which is Python's
    // `HTTPMessage.get` semantics, so [`header_lookup`] sees the same answer on
    // top of this shape.  One divergence is recorded rather than hidden: ureq
    // returns `None` for a value that is not valid UTF-8 while still listing its
    // name, so such a `Location` reads as absent here and as an iso-8859-1 string
    // in Python.
    let headers: Vec<(String, String)> = response
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            let value = response.header(&name)?.to_string();
            Some((name, value))
        })
        .collect();
    let mut body: Vec<u8> = Vec::new();
    let mut reader = response.into_reader();
    reader
        .read_to_end(&mut body)
        .map_err(|e| FetchError::Transport(format!("update_network_error: {e}")))?;
    Ok(Reply {
        status,
        headers,
        body,
    })
}

/// A sleep that honours Python's backoff in production.  Split out so tests can
/// assert *that* a backoff was scheduled without paying wall-clock time for it.
pub fn real_sleep(period: Duration) {
    std::thread::sleep(period);
}

/// `updater._fetch_text` against the live transport — the closure shape
/// [`resolve_expected_sha`] takes.
pub fn live_sha_manifest(timeout: Duration) -> impl FnMut(&str) -> Option<String> {
    move |url: &str| {
        let mut sleeper = real_sleep;
        fetch_text(
            url,
            &mut |target| redirecting_reply_via_ureq(target, timeout),
            &mut sleeper,
        )
        .ok()
        .flatten()
    }
}

/// The complete live check: the six closures [`check_update`] needs, built from
/// the `*_via_ureq` adapters and one `timeout` (`updater.check_update`'s own
/// `timeout=2.5`, i.e. [`DEFAULT_CHECK_TIMEOUT`]).  This is what the routed
/// `/api/update/check` handler calls, so all three tiers run on the wire.
pub fn check_update_live(current_version: &str, timeout: Duration) -> Value {
    let ci = is_ci_environment();
    let mut sha_manifest = live_sha_manifest(timeout);
    check_update(
        current_version,
        ci,
        &mut |url| redirecting_reply_via_ureq(url, timeout),
        &mut |url| sniff_reply_via_ureq(url, timeout),
        &mut |url| redirecting_reply_via_ureq(url, timeout),
        &mut sha_manifest,
        &mut real_sleep,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Serialises the two tests that touch the process-wide `_UPGRADE_CACHE`.
    static CACHE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `"ab" * 32` — exactly 64 ASCII hex characters, the shape
    /// `SHA256SUMS.txt` ships.
    const DIGEST: &str = "abababababababababababababababababababababababababababababababab";

    // ------------------------------------------------------------- scaffolding

    #[derive(Clone, Default)]
    struct Recorder(Rc<RefCell<Vec<String>>>);

    impl Recorder {
        fn calls(&self) -> Vec<String> {
            self.0.borrow().clone()
        }
        fn count(&self) -> usize {
            self.0.borrow().len()
        }
        /// A `Reply`-shaped transport: `Some(reply)` answers, `None` is a
        /// transient socket failure (`FetchError::Transport`).  Running out of
        /// scripted answers is also a transport failure, so a test that expects
        /// N calls cannot silently make an N+1-th one succeed.
        fn transport(
            &self,
            answers: Vec<Option<Reply>>,
        ) -> impl FnMut(&str) -> Result<Reply, FetchError> {
            let sink = self.0.clone();
            let mut answers = answers.into_iter();
            move |url: &str| {
                sink.borrow_mut().push(url.to_string());
                match answers.next() {
                    Some(Some(reply)) => Ok(reply),
                    Some(None) => Err(FetchError::Transport("update_network_error".to_string())),
                    None => Err(FetchError::Transport("script exhausted".to_string())),
                }
            }
        }
        /// The `FnMut(&str) -> Option<String>` shape
        /// `resolve_expected_sha` and `release_check_payload` take.
        fn text_transport(
            &self,
            answers: Vec<Option<String>>,
        ) -> impl FnMut(&str) -> Option<String> {
            let sink = self.0.clone();
            let mut answers = answers.into_iter();
            move |url: &str| {
                sink.borrow_mut().push(url.to_string());
                answers.next().flatten()
            }
        }
        /// Same as [`Recorder::transport`] but the scripted answers are already
        /// `Result`s, so a test can produce an `Err(FetchError::Invalid(..))`.
        fn fallible(
            &self,
            answers: Vec<Result<Reply, FetchError>>,
        ) -> impl FnMut(&str) -> Result<Reply, FetchError> {
            let sink = self.0.clone();
            let mut answers = answers.into_iter();
            move |url: &str| {
                sink.borrow_mut().push(url.to_string());
                answers
                    .next()
                    .unwrap_or_else(|| Err(FetchError::Transport("script exhausted".to_string())))
            }
        }
    }

    /// Records every sleep period so the backoff can be asserted without
    /// paying wall-clock time.
    fn sleeper() -> (Rc<RefCell<Vec<Duration>>>, impl FnMut(Duration)) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sink = log.clone();
        (log, move |period: Duration| sink.borrow_mut().push(period))
    }

    fn reply(status: u16, body: &str) -> Reply {
        Reply {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn json_reply(status: u16, body: &str) -> Reply {
        Reply {
            status,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: body.as_bytes().to_vec(),
        }
    }

    /// A 302 that was **not** followed — the `_NoRedirectHandler` outcome.
    fn redirect(location: &str) -> Reply {
        Reply {
            status: 302,
            headers: vec![("Location".to_string(), location.to_string())],
            body: Vec::new(),
        }
    }

    fn asset(name: &str, url: &str, sha: Option<&str>) -> Value {
        let mut map = Map::new();
        map.insert("name".to_string(), json!(name));
        map.insert("browser_download_url".to_string(), json!(url));
        map.insert("size".to_string(), json!(0));
        if let Some(sha) = sha {
            map.insert("expected_sha".to_string(), json!(sha));
        }
        Value::Object(map)
    }

    fn release(tag: &str, extra: &[(&str, Value)]) -> Value {
        let mut map = Map::new();
        map.insert("tag_name".to_string(), json!(tag));
        map.insert("name".to_string(), json!(format!("ReadMD {}", tag)));
        map.insert("published_at".to_string(), json!("2026-01-02T03:04:05Z"));
        map.insert(
            "html_url".to_string(),
            json!(format!("https://github.com/{}/releases/tag/{}", GITHUB_REPO, tag)),
        );
        map.insert("assets".to_string(), Value::Array(Vec::new()));
        for (key, value) in extra {
            map.insert((*key).to_string(), value.clone());
        }
        Value::Object(map)
    }

    /// A realistic two-line `SHA256SUMS.txt`.
    fn manifest_body() -> String {
        format!("{}  ReadMDSetup.exe\n{} ReadMD-portable.zip\n", DIGEST, DIGEST)
    }

    fn names(assets: &[Value]) -> Vec<String> {
        assets
            .iter()
            .map(|a| {
                a.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("<absent>")
                    .to_string()
            })
            .collect()
    }

    // ------------------------------------------------- extract_release_tag
    // Every expectation below is CPython's, from
    // `.qoder-scratch/updater-s1/oracle_tests.out` section
    // `== extract_release_tag ==`.

    #[test]
    fn tag_keeps_the_v_prefix_it_was_given() {
        assert_eq!(
            extract_release_tag("https://github.com/Natsummerance/readMD/releases/tag/v2.4.0"),
            Some("v2.4.0".to_string())
        );
    }

    #[test]
    fn tag_without_a_v_prefix_is_returned_unchanged() {
        assert_eq!(
            extract_release_tag("https://github.com/Natsummerance/readMD/releases/tag/2.4.0"),
            Some("2.4.0".to_string())
        );
    }

    #[test]
    fn tag_stops_at_a_query_string() {
        assert_eq!(
            extract_release_tag("https://github.com/x/y/releases/tag/v2.4.0?foo=1"),
            Some("v2.4.0".to_string())
        );
    }

    #[test]
    fn tag_stops_at_a_fragment() {
        assert_eq!(
            extract_release_tag("https://github.com/x/y/releases/tag/v2.4.0#frag"),
            Some("v2.4.0".to_string())
        );
    }

    #[test]
    fn tag_stops_at_a_tab() {
        assert_eq!(
            extract_release_tag("https://github.com/x/y/releases/tag/v2.4.0\ttrailing"),
            Some("v2.4.0".to_string())
        );
    }

    #[test]
    fn tag_stops_at_a_space() {
        assert_eq!(
            extract_release_tag("https://github.com/x/y/releases/tag/v2.4.0 trailing"),
            Some("v2.4.0".to_string())
        );
    }

    #[test]
    fn whitespace_only_capture_yields_no_tag() {
        assert_eq!(extract_release_tag("/releases/tag/ "), None);
    }

    #[test]
    fn empty_capture_yields_no_tag() {
        assert_eq!(extract_release_tag("/releases/tag/"), None);
    }

    #[test]
    fn relative_location_still_sniffs_a_tag() {
        // `search` never resolves against the request URL, and Python does not
        // either — this is a real behaviour, not an accident.
        assert_eq!(
            extract_release_tag("/Natsummerance/readMD/releases/tag/v9.9.9"),
            Some("v9.9.9".to_string())
        );
    }

    #[test]
    fn location_without_the_marker_yields_no_tag() {
        assert_eq!(extract_release_tag("https://example.invalid/nothing-here"), None);
    }

    #[test]
    fn mirror_prefixed_location_sniffs_through_the_prefix() {
        assert_eq!(
            extract_release_tag(
                "https://ghfast.top/https://github.com/Natsummerance/readMD/releases/tag/v2.3.9"
            ),
            Some("v2.3.9".to_string())
        );
    }

    #[test]
    fn leftmost_of_two_markers_wins() {
        assert_eq!(
            extract_release_tag(
                "https://a/x/releases/tag/v1.0.0 https://b/y/releases/tag/v2.0.0"
            ),
            Some("v1.0.0".to_string())
        );
    }

    #[test]
    fn capture_is_not_percent_decoded() {
        assert_eq!(
            extract_release_tag("https://a/x/releases/tag/%20v2.0.0"),
            Some("%20v2.0.0".to_string())
        );
    }

    #[test]
    fn file_separator_right_after_the_marker_is_a_stop_character() {
        // The `regex` crate's `\s` would *not* stop here and would sniff the
        // bogus tag `x`.  CPython's does, so the answer is `None`.
        assert_eq!(extract_release_tag("/releases/tag/\u{1c}x"), None);
    }

    #[test]
    fn file_separator_inside_the_capture_terminates_it() {
        assert_eq!(
            extract_release_tag("/releases/tag/v2.4\u{1c}junk"),
            Some("v2.4".to_string())
        );
    }

    #[test]
    fn nbsp_terminates_the_capture() {
        assert_eq!(
            extract_release_tag("/releases/tag/v2.4\u{a0}tail"),
            Some("v2.4".to_string())
        );
    }

    #[test]
    fn ideographic_space_terminates_the_capture() {
        assert_eq!(
            extract_release_tag("/releases/tag/v2.4\u{3000}tail"),
            Some("v2.4".to_string())
        );
    }

    #[test]
    fn failed_first_match_retries_at_the_next_marker() {
        assert_eq!(
            extract_release_tag("/releases/tag//releases/tag/v9"),
            Some("v9".to_string())
        );
    }

    #[test]
    fn line_separator_alone_is_not_a_tag() {
        assert_eq!(extract_release_tag("/releases/tag/\u{2028}"), None);
    }

    #[test]
    fn surrounding_whitespace_is_stripped_by_group_one_strip() {
        assert_eq!(extract_release_tag("  /releases/tag/v0.1  "), Some("v0.1".to_string()));
    }

    // ------------------------------------------------------------ headers

    #[test]
    fn header_lookup_is_case_insensitive() {
        let headers = vec![("LoCaTiOn".to_string(), "https://a/b".to_string())];
        assert_eq!(header_lookup(&headers, "location"), Some("https://a/b"));
        assert_eq!(header_lookup(&headers, "LOCATION"), Some("https://a/b"));
    }

    #[test]
    fn repeated_header_yields_the_first_value() {
        // `http.client.HTTPMessage.get` — `get_all` is the only way to see the
        // rest, and Python uses `get`.
        let headers = vec![
            ("Set-Cookie".to_string(), "a=1".to_string()),
            ("set-cookie".to_string(), "b=2".to_string()),
        ];
        assert_eq!(header_lookup(&headers, "Set-Cookie"), Some("a=1"));
    }

    #[test]
    fn absent_header_reads_as_empty_string() {
        let bare = reply(200, "x");
        assert_eq!(header_lookup(&bare.headers, "Location"), None);
        assert_eq!(bare.location(), "");
    }

    #[test]
    fn redirect_reply_exposes_its_location() {
        assert_eq!(
            redirect("https://github.com/x/y/releases/tag/v1.2.3").location(),
            "https://github.com/x/y/releases/tag/v1.2.3".to_string()
        );
    }

    // ---------------------------------------------------- candidate lists

    #[test]
    fn sniff_candidates_is_official_then_two_mirrors() {
        let urls = sniff_candidates(false);
        assert_eq!(
            urls,
            vec![
                "https://github.com/Natsummerance/readMD/releases/latest".to_string(),
                "https://ghfast.top/https://github.com/Natsummerance/readMD/releases/latest"
                    .to_string(),
                "https://ghproxy.net/https://github.com/Natsummerance/readMD/releases/latest"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn ci_sniffs_the_official_endpoint_only() {
        assert_eq!(
            sniff_candidates(true),
            vec!["https://github.com/Natsummerance/readMD/releases/latest".to_string()]
        );
    }

    #[test]
    fn third_mirror_is_never_sniffed() {
        // `MIRROR_PREFIXES[2]` exists (it is used by the download path) but the
        // sniff list at `updater.py:294-300` only ever extends with the first
        // two.  A regression here would put a third-party host on the boot path.
        assert!(sniff_candidates(false)
            .iter()
            .all(|url| !url.contains("mirror.ghproxy.com")));
        assert!(manifest_candidates("v1.0.0", false)
            .iter()
            .all(|url| !url.contains("mirror.ghproxy.com")));
    }

    #[test]
    fn manifest_candidates_interpolate_the_tag_unescaped() {
        let urls = manifest_candidates("v2.4.0", false);
        assert_eq!(urls[0], "https://github.com/Natsummerance/readMD/releases/download/v2.4.0/SHA256SUMS.txt");
        assert_eq!(urls.len(), 3);
        assert_eq!(manifest_candidates("v2.4.0", true).len(), 1);
    }

    #[test]
    fn no_redirect_handler_never_produces_a_follow_request() {
        // `updater.py:285-286` — the class's whole observable behaviour.
        assert_eq!(NoRedirectHandler::redirect_request(), None);
        assert_eq!(redirect_request(), None);
    }

    // ---------------------------------------------- sniff_latest_tag_redirect

    #[test]
    fn sniff_returns_the_first_location_tag() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(redirect(
            "https://github.com/Natsummerance/readMD/releases/tag/v2.4.0",
        ))]);
        assert_eq!(
            sniff_latest_tag_redirect(false, &mut transport),
            Some("v2.4.0".to_string())
        );
        assert_eq!(rec.count(), 1);
        assert_eq!(
            rec.calls()[0],
            "https://github.com/Natsummerance/readMD/releases/latest"
        );
    }

    #[test]
    fn sniff_skips_a_transport_failure_and_uses_the_mirror() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![
            None,
            Some(redirect("https://ghfast.top/https://github.com/a/b/releases/tag/v2.3.9")),
        ]);
        assert_eq!(
            sniff_latest_tag_redirect(false, &mut transport),
            Some("v2.3.9".to_string())
        );
        assert_eq!(rec.count(), 2);
        assert!(rec.calls()[1].starts_with("https://ghfast.top/"));
    }

    #[test]
    fn sniff_skips_a_reply_with_no_location() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![
            Some(reply(200, "<html>no header</html>")),
            Some(redirect("https://github.com/a/b/releases/tag/v3.0.0")),
        ]);
        assert_eq!(
            sniff_latest_tag_redirect(false, &mut transport),
            Some("v3.0.0".to_string())
        );
        assert_eq!(rec.count(), 2);
    }

    #[test]
    fn sniff_skips_a_location_that_carries_no_tag() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![
            Some(redirect("https://github.com/Natsummerance/readMD")),
            Some(redirect("https://github.com/Natsummerance/readMD/releases/tag/v2.0.0")),
        ]);
        assert_eq!(
            sniff_latest_tag_redirect(false, &mut transport),
            Some("v2.0.0".to_string())
        );
        assert_eq!(rec.count(), 2);
    }

    #[test]
    fn sniff_returns_none_after_every_candidate_fails() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![None, None, None]);
        assert_eq!(sniff_latest_tag_redirect(false, &mut transport), None);
        assert_eq!(rec.count(), 3);
    }

    #[test]
    fn ci_sniff_makes_exactly_one_request_even_when_it_fails() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![None]);
        assert_eq!(sniff_latest_tag_redirect(true, &mut transport), None);
        assert_eq!(rec.count(), 1);
    }

    #[test]
    fn ci_detection_matches_pythons_literal_true_test() {
        // `os.environ.get('CI') == 'true' or os.environ.get('GITHUB_ACTIONS')
        // == 'true'` — case-sensitive, and the only two names consulted.
        let expected = std::env::var("CI").as_deref() == Ok("true")
            || std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true");
        assert_eq!(is_ci_environment(), expected);
    }

    // ------------------------------------------------- the acceptance gate
    // CPython: `bool(t and ('ReadMD' in t or len(t) > 20))`, with `len` on a
    // `str` counting **code points**.

    #[test]
    fn acceptance_rejects_empty_text() {
        assert!(!manifest_text_is_acceptable(""));
    }

    #[test]
    fn acceptance_boundary_is_twenty_one_code_points() {
        assert!(!manifest_text_is_acceptable(&"x".repeat(20)));
        assert!(manifest_text_is_acceptable(&"x".repeat(21)));
    }

    #[test]
    fn acceptance_counts_code_points_not_bytes() {
        // 22 characters, 66 UTF-8 bytes.  A byte-length test would pass far too
        // many 404 HTML stubs; `len(manifest_text) > 20` in Python is a
        // character count.
        let cjk = "中文".repeat(11);
        assert_eq!(cjk.len(), 66);
        assert_eq!(cjk.chars().count(), 22);
        assert!(manifest_text_is_acceptable(&cjk));
        // 21 emoji = 84 bytes but 21 code points — still passes on `len`.
        assert!(manifest_text_is_acceptable(&"😀".repeat(21)));
    }

    #[test]
    fn acceptance_rejects_short_whitespace_only_body() {
        assert!(!manifest_text_is_acceptable("   \n  "));
    }

    #[test]
    fn acceptance_accepts_anything_naming_the_product() {
        assert!(manifest_text_is_acceptable("ReadMD"));
        // The real reason for the `'ReadMD' in` clause: a genuine digest list
        // can be shorter than 21 characters.
        assert!(manifest_text_is_acceptable(&format!("{}  a.exe", DIGEST)));
    }

    // -------------------------------------------------- manifest_line_parts
    // One case per measured CPython answer in
    // `.qoder-scratch/updater-s1/oracle_tests.out` section
    // `== manifest_line_parts ==`.  Note that this function returns the digest
    // **as written**; lowercasing is `_fetch_manifest_assets`'s job.

    #[test]
    fn line_two_space_separator() {
        assert_eq!(
            manifest_line_parts(&format!("{}  ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_single_space_separator() {
        assert_eq!(
            manifest_line_parts(&format!("{} ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_digest_case_is_preserved_for_the_caller() {
        let upper = DIGEST.to_uppercase();
        assert_eq!(
            manifest_line_parts(&format!("{}  ReadMDSetup.exe", upper)),
            Some((upper, "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_leading_star_is_not_part_of_the_digest() {
        assert_eq!(
            manifest_line_parts(&format!("*{}  ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_tab_separated_and_padded() {
        assert_eq!(
            manifest_line_parts(&format!("\t{} \t ReadMDSetup.exe \t", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_carriage_return_leading_counts_as_whitespace() {
        assert_eq!(
            manifest_line_parts(&format!("\r{}  ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_all_whitespace_tail_yields_an_empty_filename() {
        // `\s+` gives one character back so `(.+?)` can hold it, and
        // `.strip()` then empties it.  Python really does build a `""` asset.
        assert_eq!(
            manifest_line_parts(&format!("{}   ", DIGEST)),
            Some((DIGEST.to_string(), String::new()))
        );
    }

    #[test]
    fn a_one_character_whitespace_tail_does_not_match_at_all() {
        // F-3, measured with the live `re` from `updater.py:366`:
        //   `digest + " "`   -> NO MATCH  (`\s+` eats it, `(.+?)` starves)
        //   `digest + "\t"`  -> NO MATCH
        //   `digest + "  "`  -> group(2) == " "     -> `.strip()` -> ""
        //   `digest + " \t"` -> group(2) == "\t"    -> `.strip()` -> ""
        // The port used to answer the phantom for the one-character tails too.
        assert_eq!(manifest_line_parts(&format!("{} ", DIGEST)), None);
        assert_eq!(manifest_line_parts(&format!("{}\t", DIGEST)), None);
        assert_eq!(manifest_line_parts(&format!("{}    ", DIGEST)), Some((DIGEST.to_string(), String::new())));
        assert_eq!(manifest_line_parts(&format!("{} \t", DIGEST)), Some((DIGEST.to_string(), String::new())));
    }

    #[test]
    fn a_phantom_empty_asset_is_real_python_and_earns_the_synthetic_entry() {
        // Because the list is non-empty, `updater.py:376-381` also appends
        // `SHA256SUMS.txt`, so a manifest line that is just a digest plus two
        // spaces really does produce two assets, the first named `""`.  A
        // one-space tail produces none.
        assert_eq!(
            names(&parse_manifest_assets(&format!("{}  \n", DIGEST), "v2.4.0")),
            vec!["", "SHA256SUMS.txt"]
        );
        assert!(parse_manifest_assets(&format!("{} \n", DIGEST), "v2.4.0").is_empty());
    }

    #[test]
    fn line_star_prefixed_filename() {
        assert_eq!(
            manifest_line_parts(&format!("{} *ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_lone_star_filename_survives() {
        // `(.+?)` needs one character, so the greedy `\*?` cannot eat it.
        assert_eq!(
            manifest_line_parts(&format!("{} *", DIGEST)),
            Some((DIGEST.to_string(), "*".to_string()))
        );
    }

    #[test]
    fn line_star_then_space_then_name() {
        assert_eq!(
            manifest_line_parts(&format!("{}  * ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_trailing_star_is_not_stripped() {
        assert_eq!(
            manifest_line_parts(&format!("{}  *ReadMDSetup.exe*", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe*".to_string()))
        );
    }

    #[test]
    fn line_without_a_separator_does_not_match() {
        assert_eq!(manifest_line_parts(DIGEST), None);
        assert_eq!(manifest_line_parts(&format!("{}123 ReadMDSetup.exe", DIGEST)), None);
    }

    #[test]
    fn line_of_sixty_three_hex_chars_does_not_match() {
        assert_eq!(
            manifest_line_parts(&format!("{}a  ReadMDSetup.exe", &DIGEST[..62])),
            None
        );
    }

    #[test]
    fn line_of_non_hex_characters_does_not_match() {
        assert_eq!(manifest_line_parts(&format!("{}z  ReadMDSetup.exe", &DIGEST[..63])), None);
    }

    #[test]
    fn empty_line_does_not_match() {
        assert_eq!(manifest_line_parts(""), None);
        assert_eq!(manifest_line_parts("    "), None);
    }

    #[test]
    fn line_name_may_contain_spaces() {
        assert_eq!(
            manifest_line_parts(&format!("{}  ReadMD Setup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMD Setup.exe".to_string()))
        );
    }

    #[test]
    fn line_nbsp_leading_is_consumed_by_the_star_ws() {
        assert_eq!(
            manifest_line_parts(&format!("\u{a0}{}  ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_ideographic_space_trailing_is_consumed_by_the_dollar_ws() {
        assert_eq!(
            manifest_line_parts(&format!("{} ReadMDSetup.exe\u{3000}", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_vertical_tab_before_the_name() {
        assert_eq!(
            manifest_line_parts(&format!("{}  \u{b}ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_file_separator_is_a_valid_column_separator() {
        // U+001C is in CPython's `\s` set and in `py_isspace`, but **not** in
        // the `regex` crate's `\s` — this is the case that forced the
        // hand-rolled parser.
        assert_eq!(
            manifest_line_parts(&format!("{}\u{1c}ReadMDSetup.exe", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    #[test]
    fn line_paragraph_separator_trailing_is_whitespace() {
        assert_eq!(
            manifest_line_parts(&format!("{}  ReadMDSetup.exe\u{2029}", DIGEST)),
            Some((DIGEST.to_string(), "ReadMDSetup.exe".to_string()))
        );
    }

    // -------------------------------------------------- parse_manifest_assets

    #[test]
    fn manifest_yields_assets_in_order_then_the_synthetic_entry() {
        let assets = parse_manifest_assets(&manifest_body(), "v2.4.0");
        assert_eq!(
            names(&assets),
            vec![
                "ReadMDSetup.exe".to_string(),
                "ReadMD-portable.zip".to_string(),
                "SHA256SUMS.txt".to_string(),
            ]
        );
    }

    #[test]
    fn manifest_digest_is_lowercased_into_expected_sha() {
        let body = format!("{}  ReadMDSetup.exe", DIGEST.to_uppercase());
        let assets = parse_manifest_assets(&body, "v2.4.0");
        assert_eq!(assets[0]["expected_sha"], json!(DIGEST));
    }

    #[test]
    fn manifest_assets_carry_size_zero_and_a_direct_download_url() {
        let assets = parse_manifest_assets(&manifest_body(), "v2.4.0");
        assert_eq!(assets[0]["size"], json!(0));
        assert_eq!(
            assets[0]["browser_download_url"],
            json!("https://github.com/Natsummerance/readMD/releases/download/v2.4.0/ReadMDSetup.exe")
        );
        assert_eq!(
            assets[1]["browser_download_url"],
            json!("https://github.com/Natsummerance/readMD/releases/download/v2.4.0/ReadMD-portable.zip")
        );
    }

    #[test]
    fn synthetic_entry_size_is_the_character_count_of_the_manifest() {
        let body = manifest_body();
        let assets = parse_manifest_assets(&body, "v2.4.0");
        let sums = assets.last().unwrap();
        assert_eq!(sums["name"], json!("SHA256SUMS.txt"));
        assert_eq!(sums["size"], json!(body.chars().count()));
        assert_eq!(
            sums["browser_download_url"],
            json!("https://github.com/Natsummerance/readMD/releases/download/v2.4.0/SHA256SUMS.txt")
        );
    }

    #[test]
    fn synthetic_entry_has_no_expected_sha_key_at_all() {
        // Absent, not `null`: `updater.py:437`'s `best_asset.get('expected_sha')`
        // must be able to tell the three states apart.
        let assets = parse_manifest_assets(&manifest_body(), "v2.4.0");
        let sums = assets.last().unwrap().as_object().unwrap();
        assert!(!sums.contains_key("expected_sha"));
        assert!(sums.get("expected_sha").is_none());
    }

    #[test]
    fn self_listing_manifest_produces_two_sha256sums_entries() {
        // Real Python behaviour, preserved deliberately.
        let body = format!("{}  SHA256SUMS.txt\n", DIGEST);
        let assets = parse_manifest_assets(&body, "v9.9.9");
        assert_eq!(names(&assets), vec!["SHA256SUMS.txt", "SHA256SUMS.txt"]);
        assert_eq!(assets[0]["expected_sha"], json!(DIGEST));
        assert_eq!(assets[1]["size"], json!(body.chars().count()));
    }

    #[test]
    fn empty_filename_is_not_filtered_out() {
        let body = format!("{}   \n", DIGEST);
        let assets = parse_manifest_assets(&body, "v2.4.0");
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0]["name"], json!(""));
        assert_eq!(
            assets[0]["browser_download_url"],
            json!("https://github.com/Natsummerance/readMD/releases/download/v2.4.0/")
        );
    }

    #[test]
    fn filename_is_interpolated_without_percent_encoding() {
        let body = format!("{}  ReadMD Setup.exe\n", DIGEST);
        let assets = parse_manifest_assets(&body, "v2.4.0");
        assert_eq!(
            assets[0]["browser_download_url"],
            json!("https://github.com/Natsummerance/readMD/releases/download/v2.4.0/ReadMD Setup.exe")
        );
    }

    #[test]
    fn garbage_manifest_yields_no_assets_and_no_synthetic_entry() {
        let assets = parse_manifest_assets("<html>404 Not Found</html>\n", "v2.4.0");
        assert!(assets.is_empty());
    }

    #[test]
    fn every_splitlines_boundary_starts_a_new_asset_line() {
        // Python's `str.splitlines()` has eleven boundaries where
        // `str::lines()` has two; a manifest pasted through a `R`-only or
        // `U+2028`-only transfer must still parse.
        let line = format!("{}  f.exe", DIGEST);
        let body = ["\n", "\r", "\u{b}", "\u{c}", "\u{1c}", "\u{1d}", "\u{1e}", "\u{85}", "\u{2028}", "\u{2029}"]
            .iter()
            .map(|sep| format!("{}{}", line, sep))
            .collect::<String>();
        let assets = parse_manifest_assets(&body, "v2.4.0");
        assert_eq!(assets.len(), 11, "one asset per splitlines boundary");
    }

    // -------------------------------------------------- fetch_manifest_assets

    #[test]
    fn empty_tag_fetches_nothing_and_is_none() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![]);
        assert_eq!(fetch_manifest_assets("", false, &mut transport), None);
        assert_eq!(rec.count(), 0);
    }

    #[test]
    fn origin_hit_short_circuits_the_mirrors() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(reply(200, &manifest_body()))]);
        let assets = fetch_manifest_assets("v2.4.0", false, &mut transport).unwrap();
        assert_eq!(assets.len(), 3);
        assert_eq!(rec.count(), 1);
    }

    #[test]
    fn non_200_never_writes_a_body() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![
            Some(reply(404, "<html>Not Found</html>")),
            Some(reply(200, &manifest_body())),
        ]);
        let assets = fetch_manifest_assets("v2.4.0", false, &mut transport).unwrap();
        assert_eq!(assets.len(), 3);
        assert_eq!(rec.count(), 2);
        assert!(rec.calls()[1].starts_with("https://ghfast.top/"));
    }

    #[test]
    fn all_candidates_failing_is_none_not_empty_vec() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![None, None, None]);
        assert_eq!(fetch_manifest_assets("v2.4.0", false, &mut transport), None);
        assert_eq!(rec.count(), 3);
    }

    #[test]
    fn a_short_body_is_remembered_across_failing_mirrors() {
        // `updater.py:349-356` — the assignment is not gated by the acceptance
        // test.  A five-byte stub therefore parses to zero assets and
        // `_fetch_manifest_assets` returns `[]`, **not** `None`.  Getting this
        // wrong reports "no manifest" where Python reports "manifest with
        // nothing in it".
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(reply(200, "12345")), None, None]);
        let got = fetch_manifest_assets("v2.4.0", false, &mut transport);
        assert_eq!(got, Some(Vec::new()));
        assert_eq!(rec.count(), 3);
    }

    #[test]
    fn a_rejected_body_does_not_stop_the_loop_and_the_last_200_wins() {
        // The gate only decides whether to `break`.  A five-byte stub and an
        // empty body both fail it, so the third candidate is still consulted and
        // its body is the one that gets parsed.
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![
            Some(reply(200, "short")),
            Some(reply(200, "")),
            Some(reply(200, &manifest_body())),
        ]);
        let got = fetch_manifest_assets("v2.4.0", false, &mut transport);
        assert_eq!(rec.count(), 3);
        let assets = got.unwrap();
        assert_eq!(assets.len(), 3);
    }

    #[test]
    fn empty_200_body_reads_as_no_manifest_at_all() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(reply(200, "")), None, None]);
        assert_eq!(fetch_manifest_assets("v2.4.0", false, &mut transport), None);
    }

    #[test]
    fn manifest_body_is_decoded_laxly() {
        // `data.decode('utf-8', errors='replace')` — invalid bytes become
        // U+FFFD and the line still parses, exactly as in Python.
        let mut body = vec![0xf0_u8, 0x9f, 0xa6]; // truncated emoji
        body.extend_from_slice(b"\n");
        body.extend_from_slice(format!("{}  ReadMDSetup.exe\n", DIGEST).as_bytes());
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(Reply {
            status: 200,
            headers: Vec::new(),
            body,
        })]);
        let assets = fetch_manifest_assets("v2.4.0", false, &mut transport).unwrap();
        assert_eq!(names(&assets), vec!["ReadMDSetup.exe", "SHA256SUMS.txt"]);
    }

    #[test]
    fn ci_consults_only_the_official_manifest() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(reply(404, "nope"))]);
        // Measured against the authority with `CI=true`: `_fetch_manifest_assets`
        // returns **None**, one request.  `urllib.request.urlopen` *raises*
        // `HTTPError` for a 404, so `updater.py:348-356` never assigns
        // `manifest_text` and `updater.py:361-362` answers None — a 404 body is
        // not a "manifest with zero assets".
        assert_eq!(fetch_manifest_assets("v2.4.0", true, &mut transport), None);
        assert_eq!(rec.count(), 1);
    }

    #[test]
    fn a_200_that_parses_into_nothing_is_an_empty_asset_list() {
        // The other side of the same door: only a *read* body that yields no
        // digest line reaches `return assets == []` (`updater.py:383`).
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(reply(200, "no digests here at all"))]);
        assert_eq!(
            fetch_manifest_assets("v2.4.0", true, &mut transport),
            Some(Vec::new())
        );
        assert_eq!(rec.count(), 1);
    }

    // ---------------------------------------------------- check_update_fallback

    #[test]
    fn fallback_synthesises_pythons_exact_release_document() {
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let mut s = sniff.transport(vec![Some(redirect(
            "https://github.com/Natsummerance/readMD/releases/tag/v2.4.1",
        ))]);
        let mut m = manifest.transport(vec![Some(reply(200, &manifest_body()))]);
        let data = check_update_fallback(false, &mut s, &mut m).unwrap();
        // CPython's literal order *is* insertion order — measured as
        // `['tag_name','name','body','published_at','html_url','assets']` — but
        // `preserve_order` is on, so the order above is now observable too.  What
        // this assertion pins is what Python actually relies on, since it only
        // ever does `data.get(key)`: the document carries exactly those six keys,
        // so the key *set* is compared against the constant that records
        // Python's order.
        let keys = data.as_object().unwrap();
        assert_eq!(keys.len(), FALLBACK_DOC_KEYS.len(), "no key added or dropped");
        let mut sorted = keys.keys().cloned().collect::<Vec<_>>();
        sorted.sort();
        let mut want = FALLBACK_DOC_KEYS.iter().map(|k| k.to_string()).collect::<Vec<_>>();
        want.sort();
        assert_eq!(sorted, want);
        assert_eq!(data["tag_name"], json!("v2.4.1"));
        assert_eq!(data["name"], json!("ReadMD v2.4.1"));
        assert_eq!(data["body"], json!("ReadMD v2.4.1"));
        assert_eq!(data["published_at"], json!(""));
        assert_eq!(
            data["html_url"],
            json!("https://github.com/Natsummerance/readMD/releases/tag/v2.4.1")
        );
        assert_eq!(data["assets"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn fallback_without_a_sniffed_tag_makes_no_manifest_request() {
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let mut s = sniff.transport(vec![None, None, None]);
        let mut m = manifest.transport(vec![]);
        assert_eq!(check_update_fallback(false, &mut s, &mut m), None);
        assert_eq!(manifest.count(), 0);
    }

    #[test]
    fn fallback_with_a_tag_but_no_assets_is_none() {
        // `updater.py:411` — `if assets:` is false for `[]`.
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let mut s = sniff.transport(vec![Some(redirect("https://a/b/releases/tag/v2.4.1"))]);
        let mut m = manifest.transport(vec![Some(reply(200, "not a manifest at all, really"))]);
        assert_eq!(check_update_fallback(false, &mut s, &mut m), None);
        assert_eq!(manifest.count(), 1);
    }

    // ---------------------------------------------------------- open_with_retry

    #[test]
    fn first_success_never_sleeps() {
        let rec = Recorder::default();
        let (slept, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(reply(200, "ok"))]);
        let got = open_with_retry("https://x", 2, &mut transport, &mut sleep).unwrap();
        assert_eq!(got.status, 200);
        assert_eq!(rec.count(), 1);
        assert!(slept.borrow().is_empty());
    }

    #[test]
    fn an_http_status_is_never_retried() {
        // `updater.py:240-241` — `except HTTPError: raise`.  A 500 is a
        // definitive answer; retrying it would cost the boot path another 2.5 s.
        let rec = Recorder::default();
        let (slept, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(reply(500, "boom"))]);
        let got = open_with_retry("https://x", 2, &mut transport, &mut sleep).unwrap();
        assert_eq!(got.status, 500);
        assert_eq!(rec.count(), 1);
        assert!(slept.borrow().is_empty());
    }

    #[test]
    fn a_transient_failure_is_retried_once_with_the_declared_backoff() {
        let rec = Recorder::default();
        let (slept, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![None, Some(reply(200, "ok"))]);
        let got = open_with_retry("https://x", TRANSIENT_FETCH_ATTEMPTS, &mut transport, &mut sleep)
            .unwrap();
        assert_eq!(got.status, 200);
        assert_eq!(rec.count(), 2);
        // `TRANSIENT_FETCH_BACKOFF = (0.3,)` — one sleep, 300 ms, and it
        // happens *between* attempts.
        assert_eq!(*slept.borrow(), vec![Duration::from_millis(300)]);
    }

    #[test]
    fn exhausting_attempts_reraises_the_last_transport_error() {
        let rec = Recorder::default();
        let (slept, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![None, None]);
        let err = open_with_retry("https://x", 2, &mut transport, &mut sleep).unwrap_err();
        assert!(matches!(err, FetchError::Transport(_)));
        assert_eq!(rec.count(), 2);
        assert_eq!(slept.borrow().len(), 1);
    }

    #[test]
    fn an_invalid_response_is_not_retried() {
        // `updater.py:240` only special-cases `HTTPError`; a decode failure is
        // raised by `json.loads` *outside* `_open_with_retry`, so the retry
        // helper must not see it.  Here it is delivered as `Invalid` and
        // returned immediately.
        let rec = Recorder::default();
        let (slept, mut sleep) = sleeper();
        let mut transport =
            rec.fallible(vec![Err(FetchError::Invalid("update_response_invalid".into()))]);
        let err = open_with_retry("https://x", 2, &mut transport, &mut sleep).unwrap_err();
        assert!(matches!(err, FetchError::Invalid(_)));
        assert_eq!(rec.count(), 1);
        assert!(slept.borrow().is_empty());
    }

    // ------------------------------------------------------- fetch_release_json

    #[test]
    fn rate_limited_api_reads_as_no_data() {
        // `_fetch_release_json` returns `None` for a non-200 rather than raising,
        // which is what lets `check_update` fall through to tier 2.
        let rec = Recorder::default();
        let (_, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(json_reply(403, "{\"message\":\"rate limit\"}"))]);
        assert_eq!(fetch_release_json("https://x", &mut transport, &mut sleep).unwrap(), None);
    }

    #[test]
    fn malformed_json_is_an_invalid_error() {
        let rec = Recorder::default();
        let (_, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(json_reply(200, "[{"))]);
        let err = fetch_release_json("https://x", &mut transport, &mut sleep).unwrap_err();
        match err {
            FetchError::Invalid(code) => assert!(code.starts_with("update_response_invalid"), "{}", code),
            other => panic!("expected Invalid, got {:?}", other),
        }
    }

    #[test]
    fn json_accepts_an_array_or_a_bare_object() {
        let rec = Recorder::default();
        let (_, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(json_reply(200, "[{\"tag_name\":\"v1\"}]"))]);
        let payload = fetch_release_json("https://x", &mut transport, &mut sleep)
            .unwrap()
            .unwrap();
        assert!(payload.is_array());
        let mut transport = rec.transport(vec![Some(json_reply(200, "{\"tag_name\":\"v1\"}"))]);
        let payload = fetch_release_json("https://x", &mut transport, &mut sleep)
            .unwrap()
            .unwrap();
        assert_eq!(payload["tag_name"], json!("v1"));
    }

    #[test]
    fn json_null_body_is_a_valid_payload() {
        // `json.loads('null')` succeeds, so `_fetch_release_json` hands back
        // `None` *as data*, and `check_update` wraps it in `[None]` before
        // `select_update_release` filters it out.
        let rec = Recorder::default();
        let (_, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(json_reply(200, "null"))]);
        assert_eq!(
            fetch_release_json("https://x", &mut transport, &mut sleep).unwrap(),
            Some(Value::Null)
        );
    }

    #[test]
    fn fetch_text_returns_only_200_bodies() {
        let rec = Recorder::default();
        let (_, mut sleep) = sleeper();
        let mut transport = rec.transport(vec![Some(reply(200, "a  b\n"))]);
        assert_eq!(
            fetch_text("https://x", &mut transport, &mut sleep).unwrap(),
            Some("a  b\n".to_string())
        );
        let mut transport = rec.transport(vec![Some(reply(302, ""))]);
        assert_eq!(fetch_text("https://x", &mut transport, &mut sleep).unwrap(), None);
    }

    // ----------------------------------------------------- URL and versioning

    #[test]
    fn formal_build_checks_the_latest_endpoint() {
        assert_eq!(
            release_check_urls("2.4.0"),
            vec![GITHUB_API_LATEST.to_string()]
        );
    }

    #[test]
    fn prerelease_build_checks_the_release_list() {
        // Only the list endpoint can return the beta's siblings.
        assert_eq!(
            release_check_urls("2.5.0-beta.1"),
            vec![GITHUB_API_RELEASES.to_string()]
        );
        assert_eq!(release_check_urls("v2.5.0-rc.2"), vec![GITHUB_API_RELEASES.to_string()]);
    }

    #[test]
    fn unparseable_version_stays_on_the_formal_channel() {
        // `parse_version('junk')` is `None`, so `is_prerelease` is false.
        assert_eq!(release_check_urls("junk"), vec![GITHUB_API_LATEST.to_string()]);
    }

    #[test]
    fn parse_semver_of_falsy_values_is_the_zero_triple() {
        for value in [Value::Null, json!(0), json!(false), json!("")] {
            assert_eq!(parse_semver(Some(&value)), [0, 0, 0], "{:?}", value);
        }
        assert_eq!(parse_semver(None), [0, 0, 0]);
    }

    #[test]
    fn parse_semver_tolerates_v_prefix_and_build_metadata() {
        assert_eq!(parse_semver(Some(&json!("v2.4.0"))), [2, 4, 0]);
        assert_eq!(parse_semver(Some(&json!("2.4.0+build.7"))), [2, 4, 0]);
        assert_eq!(parse_semver(Some(&json!("2.4"))), [2, 4, 0]);
        assert_eq!(parse_semver(Some(&json!("2"))), [2, 0, 0]);
    }

    #[test]
    fn parse_semver_reads_leading_zeros_as_decimal() {
        assert_eq!(parse_semver(Some(&json!("010.020.030"))), [10, 20, 30]);
    }

    #[test]
    fn parse_semver_of_garbage_is_the_zero_triple() {
        assert_eq!(parse_semver(Some(&json!("junk"))), [0, 0, 0]);
    }

    // ---------------------------------------------------- resolve_expected_sha

    #[test]
    fn missing_sha_url_makes_no_request() {
        let rec = Recorder::default();
        let mut manifest = rec.text_transport(vec![Some("x".to_string())]);
        assert_eq!(resolve_expected_sha(None, Some("a.exe"), &mut manifest), None);
        assert_eq!(resolve_expected_sha(Some(""), Some("a.exe"), &mut manifest), None);
        assert_eq!(resolve_expected_sha(Some("https://x"), None, &mut manifest), None);
        assert_eq!(resolve_expected_sha(Some("https://x"), Some(""), &mut manifest), None);
        assert_eq!(rec.count(), 0);
    }

    #[test]
    fn sha_lookup_returns_a_string_digest_or_nothing() {
        let rec = Recorder::default();
        let body = format!("{}  ReadMDSetup.exe\n", DIGEST.to_uppercase());
        let mut manifest = rec.text_transport(vec![Some(body)]);
        assert_eq!(
            resolve_expected_sha(Some("https://x/SHA256SUMS.txt"), Some("ReadMDSetup.exe"), &mut manifest),
            Some(json!(DIGEST))
        );
        assert_eq!(rec.count(), 1);
        let rec = Recorder::default();
        let mut manifest = rec.text_transport(vec![Some(format!("{}  Other.exe\n", DIGEST))]);
        assert_eq!(
            resolve_expected_sha(Some("https://x/SHA256SUMS.txt"), Some("ReadMDSetup.exe"), &mut manifest),
            None
        );
    }

    // ------------------------------------------------------ asset selection

    #[test]
    fn sha256sums_asset_is_reported_separately_from_the_installer() {
        let assets = vec![
            asset("ReadMDSetup.exe", "https://dl/ReadMDSetup.exe", None),
            asset("SHA256SUMS.txt", "https://dl/SHA256SUMS.txt", None),
        ];
        let (best, sha) = match_release_asset(&assets, "win_installer").unwrap();
        assert_eq!(best.unwrap()["name"], json!("ReadMDSetup.exe"));
        assert_eq!(sha.unwrap()["name"], json!("SHA256SUMS.txt"));
    }

    #[test]
    fn portable_flavor_refuses_the_setup_asset() {
        let assets = vec![asset("ReadMDSetup.exe", "https://dl/ReadMDSetup.exe", None)];
        let (best, _sha) = match_release_asset(&assets, "win_portable").unwrap();
        // The precise portable match fails; only the platform fallback can
        // still pick it up, and that fallback is `cfg!(windows)`-specific.
        if cfg!(windows) {
            assert_eq!(best.unwrap()["name"], json!("ReadMDSetup.exe"));
        } else {
            assert!(best.is_none());
        }
    }

    #[test]
    fn portable_asset_is_preferred_for_a_portable_flavor() {
        let assets = vec![
            asset("ReadMDSetup.exe", "https://dl/ReadMDSetup.exe", None),
            asset("ReadMD-portable.zip", "https://dl/ReadMD-portable.zip", None),
        ];
        let (best, _sha) = match_release_asset(&assets, "win_portable").unwrap();
        assert_eq!(best.unwrap()["name"], json!("ReadMD-portable.zip"));
    }

    #[test]
    fn installer_flavor_ignores_an_empty_asset_list() {
        let (best, sha) = match_release_asset(&[], "win_installer").unwrap();
        assert!(best.is_none());
        assert!(sha.is_none());
    }

    /// `updater.py:136-137` — `name = a.get('name', '')` and then `name.upper()`.
    /// Every row below is the measured CPython 3.11.15 answer
    /// (`scratch/updater_parity_nonstring_name.py`): the `.get` default covers an
    /// **absent** key only, so an explicit `null` raises exactly like a number,
    /// and `check_update`'s `except` (`updater.py:464-466`) answers
    /// `{'ok': False, 'error_code': 'update_response_invalid'}` — no `html_url`,
    /// still HTTP 200.
    #[test]
    fn a_non_string_asset_name_is_pythons_update_response_invalid() {
        let poison: &[Value] = &[
            json!(5),
            json!(0),
            json!(null),
            json!(true),
            json!(2.5),
            json!(["a"]),
            json!({"k": 1}),
        ];
        for name in poison {
            // `None` is the exception channel: pass 1 (`updater.py:135`) walks the
            // whole list, so the row's position never saves the call.
            let alone = vec![json!({ "name": name })];
            assert!(
                match_release_asset(&alone, "win_installer").is_none(),
                "`name: {name}` must raise AttributeError in CPython"
            );
            let second = vec![asset("ReadMDSetup.exe", "https://dl/a.exe", None), json!({"name": name})];
            assert!(
                match_release_asset(&second, "win_installer").is_none(),
                "a poison row in position 2 must still raise: {name}"
            );
            let beside_manifest = vec![
                asset("SHA256SUMS.txt", "https://dl/SHA256SUMS.txt", None),
                json!({ "name": name }),
            ];
            assert!(
                match_release_asset(&beside_manifest, "win_installer").is_none(),
                "the sha scan must not launder a poison row: {name}"
            );

            let payload = release_check_payload(
                "2.4.0",
                Some(&json!({ "tag_name": "2.9.9", "assets": [ { "name": name } ] })),
                "",
                &mut |url| {
                    let _ = url;
                    panic!("the error path must not reach the manifest")
                },
            );
            assert_eq!(
                payload,
                json!({ "ok": false, "error_code": "update_response_invalid" }),
                "`name: {name}`"
            );
        }

        // Legal shapes keep working: the absent key takes Python's `''` default,
        // `''` is a plain empty string, and a `name` that is a string is normal.
        for legal in [
            json!({}),
            json!({ "name": "" }),
            json!({ "name": "ReadMDSetup.exe" }),
        ] {
            let assets = vec![legal];
            assert!(
                match_release_asset(&assets, "win_installer").is_some(),
                "absent or empty `name` does not raise in CPython"
            );
            let payload =
                release_check_payload("2.4.0", Some(&json!({ "tag_name": "2.9.9", "assets": assets })), "", &mut no_manifest);
            assert_eq!(payload["ok"], json!(true), "{:?}", assets);
        }
    }

    #[test]
    fn detected_flavor_is_one_of_the_python_known_set() {
        // Python's docstring set — all **five** values, not the four the host
        // probe happens to produce.
        assert!(
            ["win_portable", "win_installer", "macos", "linux", "source"]
                .contains(&detect_app_flavor().as_str())
        );
    }

    #[test]
    fn the_flavor_decision_reproduces_pythons_five_returns() {
        // F-5.  Every row is a measured `updater.detect_app_flavor()` answer with
        // `sys.platform` / `sys.frozen` / `sys.executable` patched, and the
        // basename column is `ntpath.basename` measured on this Windows box
        // (`'C:\\tmp\\ReadMD-portable.exe' -> 'ReadMD-portable.exe'`, `'' -> ''`,
        // `'/posix-ish/readmd-portable.exe' -> 'readmd-portable.exe'`).
        let table: &[(( &str, bool, &str), &str)] = &[
            (("win32", false, "C:\\Apps\\ReadMD\\ReadMD.exe"), "source"),
            (("win32", false, ""), "source"),
            (("win32", true, "C:\\tmp\\ReadMD-portable.exe"), "win_portable"),
            (("win32", true, "C:\\x\\PORTABLE-ReadMD.exe"), "win_portable"),
            (("win32", true, "/posix-ish/readmd-portable.exe"), "win_portable"),
            (("win32", true, "C:\\tmp\\ReadMDSetup.exe"), "win_installer"),
            (("win32", true, ""), "win_installer"),
            (("win32", true, "C:\\a\\"), "win_installer"),
            (("darwin", false, "/Applications/ReadMD.app/ReadMD"), "macos"),
            (("darwin", true, "C:\\tmp\\ReadMD-portable.exe"), "macos"),
            (("linux", false, "/usr/bin/readmd"), "linux"),
            (("linux", true, "/usr/bin/readmd-portable"), "linux"),
            (("freebsd", false, "whatever"), "linux"),
        ];
        for &((platform, frozen, exe), want) in table {
            assert_eq!(
                detect_app_flavor_from(platform, frozen, exe),
                want,
                "platform={platform} frozen={frozen} exe={exe:?}"
            );
        }
        // `source` is reachable in the decision but not from the host probe: the
        // probe models the packaged app, i.e. Python's `getattr(sys, 'frozen',
        // False)` being true, which is the mapping [`detect_app_flavor_from`] pins.
        // Asserting the pair here keeps that a *stated* choice rather than a
        // branch that quietly went missing.
        assert_ne!(detect_app_flavor(), "source");
        let host_platform = if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(windows) {
            "win32"
        } else {
            "linux"
        };
        let host_exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        assert_eq!(detect_app_flavor(), detect_app_flavor_from(host_platform, true, &host_exe));
        // ... and a *source* checkout on the same box really does answer `source`
        // in the authority, so this is a modelling choice, not Python's own.
        assert_eq!(detect_app_flavor_from(host_platform, false, &host_exe), if host_platform == "win32" { "source" } else { "linux" });
    }

    // --------------------------------------------------- release_check_payload

    fn no_manifest(url: &str) -> Option<String> {
        let _ = url;
        None
    }

    #[test]
    fn a_non_list_assets_value_is_pythons_update_response_invalid() {
        // F-4.  Rows measured against `updater.check_update` with the transport
        // and the sniffer stubbed out (lane report, F-4 table): a non-list
        // `assets` raises inside `match_release_asset` and the `except` at
        // `updater.py:464-466` answers with **exactly two keys**; an *empty*
        // iterable is absorbed because `for a in assets` never runs.
        let raises = vec![
            json!("abc"),
            json!({"k": 1}),
            json!(5),
            json!(null),
            json!(true),
            json!(["x"]),
            json!([5]),
            json!([true]),
        ];
        for assets in raises {
            let data = json!({"tag_name": "v2.5.0", "assets": assets});
            let payload = release_check_payload("2.4.0", Some(&data), "", &mut no_manifest);
            assert_eq!(payload["ok"], json!(false), "{:?} must be rejected", assets);
            assert_eq!(
                payload["error_code"],
                json!("update_response_invalid"),
                "{:?}",
                assets
            );
            assert_eq!(payload.as_object().unwrap().len(), 2, "{:?} keys", assets);
        }
        for assets in vec![json!([]), json!({}), json!("")] {
            let data = json!({"tag_name": "v2.5.0", "assets": assets});
            let payload = release_check_payload("2.4.0", Some(&data), "", &mut no_manifest);
            assert_eq!(payload["ok"], json!(true), "{:?} must be absorbed", assets);
            assert_eq!(payload["asset"], Value::Null, "{:?}", assets);
        }
        // The absent key is `data.get('assets', [])`.
        let payload = release_check_payload(
            "2.4.0",
            Some(&json!({"tag_name": "v2.5.0"})),
            "",
            &mut no_manifest,
        );
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["asset"], Value::Null);
    }

    #[test]
    fn payload_without_data_reports_the_network_error() {
        let payload = release_check_payload("2.4.0", None, "", &mut no_manifest);
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["error_code"], json!("update_network_error"));
        assert_eq!(
            payload["html_url"],
            json!("https://github.com/Natsummerance/readMD/releases")
        );
        assert_eq!(payload.as_object().unwrap().len(), 3);
    }

    #[test]
    fn payload_keeps_the_callers_error_code() {
        let payload = release_check_payload("2.4.0", None, "update_rate_limited", &mut no_manifest);
        assert_eq!(payload["error_code"], json!("update_rate_limited"));
    }

    #[test]
    fn payload_echoes_tag_states_without_collapsing_them() {
        // absent key -> "", explicit null -> null, number -> number.
        let absent = release_check_payload("2.4.0", Some(&json!({})), "", &mut no_manifest);
        assert_eq!(absent["latest_version"], json!(""));
        let null = release_check_payload("2.4.0", Some(&json!({"tag_name": null})), "", &mut no_manifest);
        assert_eq!(null["latest_version"], Value::Null);
        let number = release_check_payload("2.4.0", Some(&json!({"tag_name": 3})), "", &mut no_manifest);
        assert_eq!(number["latest_version"], json!(3));
    }

    #[test]
    fn payload_falls_back_to_the_tag_when_name_is_falsy() {
        let for_tag = json!({"tag_name": "2.5.0", "name": ""});
        let payload = release_check_payload("2.4.0", Some(&for_tag), "", &mut no_manifest);
        assert_eq!(payload["release_name"], json!("2.5.0"));
        let named = json!({"tag_name": "2.5.0", "name": "ReadMD 2.5.0"});
        let payload = release_check_payload("2.4.0", Some(&named), "", &mut no_manifest);
        assert_eq!(payload["release_name"], json!("ReadMD 2.5.0"));
    }

    #[test]
    fn payload_defaults_missing_release_fields_to_empty_strings() {
        let payload = release_check_payload(
            "2.4.0",
            Some(&json!({"tag_name": "2.5.0"})),
            "",
            &mut no_manifest,
        );
        assert_eq!(payload["published_at"], json!(""));
        assert_eq!(payload["release_notes"], json!(""));
        assert_eq!(payload["html_url"], json!(""));
        assert_eq!(payload["sha_url"], Value::Null);
    }

    /// `updater.py:434-435` hands `data.get('assets', [])` to `match_release_asset`,
    /// which **iterates** it (`updater.py:135`) inside the `try` at `updater.py:430`.
    /// Any value that is not a list of mappings therefore raises, and
    /// `updater.py:464-466` answers with exactly two keys, served 200.  Absorbing
    /// those shapes into an empty `Vec` made the kernel report a *successful*
    /// update check for a malformed release document.
    ///
    /// The row-level half of the same exception: a mapping whose `name` is not a
    /// string raises at `name.upper()` (`updater.py:137`) too, and that shape is
    /// pinned by [`a_non_string_asset_name_is_pythons_update_response_invalid`].
    #[test]
    fn check_reports_update_response_invalid_for_uniterable_assets() {
        let tail = |doc: Value| {
            let mut calls = 0usize;
            let payload = release_check_payload("2.4.0", Some(&doc), "", &mut |_| {
                calls += 1;
                None
            });
            assert_eq!(calls, 0, "the error path must not reach the manifest: {doc}");
            payload
        };

        for shape in [
            json!(null),
            json!(5),
            json!(true),
            json!("ab"),
            json!({ "a": 1 }),
            json!([5]),
            json!(["x"]),
            json!([true]),
        ] {
            assert_eq!(
                tail(json!({ "tag_name": "2.5.0", "assets": shape.clone() })),
                json!({ "ok": false, "error_code": "update_response_invalid" }),
                "`assets: {shape}` raises in CPython, so only these two keys may answer",
            );
        }

        // Empty containers iterate to nothing: neither `for` loop runs, so Python
        // stays on the success path with `asset` and `sha_url` null.
        for shape in [json!([]), json!({}), json!("")] {
            let payload = tail(json!({ "tag_name": "2.5.0", "assets": shape.clone() }));
            assert_eq!(payload["ok"], json!(true), "`assets: {shape}` iterates to nothing");
            assert_eq!(payload["asset"], Value::Null);
            assert_eq!(payload["sha_url"], Value::Null);
        }

        // An absent key takes the `[]` default (`data.get('assets', [])`).
        let payload = tail(json!({ "tag_name": "2.5.0" }));
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["latest_version"], json!("2.5.0"));
    }

    /// `updater.py:431` and `:451` — `latest_tag = data.get('tag_name', '')` and
    /// `'release_name': data.get('name') or latest_tag` are *raw* echoes of the
    /// release document.  Python keeps the three shapes apart (absent key → `''`,
    /// explicit `null` → `None`, a number stays a number) while
    /// `is_newer_version` coerces with `str(value or '')` (`versioning.py:14`);
    /// collapsing all of them to `""` invented a version out of a malformed
    /// document and made `"latest_version": null` unreachable.
    #[test]
    fn check_echoes_the_release_document_without_inventing_values() {
        let check = |doc: Value| {
            let mut calls = 0usize;
            let payload = release_check_payload("2.4.0", Some(&doc), "", &mut |_| {
                calls += 1;
                None
            });
            assert_eq!(calls, 0, "no assets, no manifest request: {doc}");
            payload
        };

        // Absent `tag_name` → `''` (`data.get('tag_name', '')`).
        let payload = check(json!({}));
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["latest_version"], json!(""));
        assert_eq!(payload["release_name"], json!(""));
        assert_eq!(payload["has_update"], json!(false));
        assert_eq!(payload["asset"], Value::Null);
        assert_eq!(payload["sha_url"], Value::Null);
        assert_eq!(payload["published_at"], json!(""));
        assert_eq!(payload["release_notes"], json!(""));
        assert_eq!(payload["html_url"], json!(""));

        // Present-but-`null` → `None`, and the `or` fallback hands it straight to
        // `release_name`.
        let payload = check(json!({ "tag_name": null }));
        assert_eq!(payload["latest_version"], Value::Null);
        assert_eq!(payload["release_name"], Value::Null);
        assert_eq!(payload["has_update"], json!(false));

        // An empty `name` is falsy, so the tag is echoed (`updater.py:451`).
        let payload = check(json!({ "tag_name": "2.5.0", "name": "" }));
        assert_eq!(payload["release_name"], json!("2.5.0"));
        let payload = check(json!({ "tag_name": "2.5.0", "name": "ReadMD 2.5.0" }));
        assert_eq!(payload["release_name"], json!("ReadMD 2.5.0"));

        // Non-string tags: `0` is falsy so the comparison sees `''`, `9` is
        // `str(9) == '9'`; both are echoed exactly as the document carried them.
        let payload = check(json!({ "tag_name": 0 }));
        assert_eq!(payload["latest_version"], json!(0));
        assert_eq!(payload["has_update"], json!(false));
        let payload = check(json!({ "tag_name": 9 }));
        assert_eq!(payload["latest_version"], json!(9));
        assert_eq!(payload["release_name"], json!(9));
        assert_eq!(payload["has_update"], json!(true));
        let payload = check(json!({ "tag_name": true }));
        assert_eq!(payload["latest_version"], json!(true));
        assert_eq!(payload["has_update"], json!(false));
    }

    // --------------------------------------------------- query_latest_release
    // `readmd.py:150-179`.  Its expectations are the same `select_update_release`
    // goldens measured above plus its own two-key result shape.

    #[test]
    fn upgrade_probe_returns_only_latest_and_url() {
        let rec = Recorder::default();
        let body = json!([{"tag_name": "2.4.1", "html_url": "https://github.com/x/releases/tag/2.4.1"}])
            .to_string();
        let mut transport = rec.transport(vec![Some(json_reply(200, &body))]);
        let got = query_latest_release("2.4.0", &mut transport).unwrap();
        assert_eq!(
            got.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["latest", "url"]
        );
        assert_eq!(got["latest"], json!("2.4.1"));
        assert_eq!(got["url"], json!("https://github.com/x/releases/tag/2.4.1"));
        // The probe uses the *latest* endpoint for a formal build.
        assert_eq!(rec.calls(), vec![UPGRADE_RELEASE_URL.to_string()]);
    }

    #[test]
    fn upgrade_probe_of_a_prerelease_build_uses_the_release_list() {
        let rec = Recorder::default();
        let body = json!([{"tag_name": "2.5.0-beta.2"}]).to_string();
        let mut transport = rec.transport(vec![Some(json_reply(200, &body))]);
        let got = query_latest_release("2.5.0-beta.1", &mut transport).unwrap();
        assert_eq!(rec.calls(), vec![UPGRADE_RELEASES_URL.to_string()]);
        assert_eq!(got["latest"], json!("2.5.0-beta.2"));
        // No usable `html_url` -> the endpoint itself.
        assert_eq!(got["url"], json!(UPGRADE_RELEASE_URL));
    }

    #[test]
    fn upgrade_probe_url_falls_back_for_absent_null_and_empty_html_url() {
        for release in [
            json!({"tag_name": "2.5.0"}),
            json!({"tag_name": "2.5.0", "html_url": null}),
            json!({"tag_name": "2.5.0", "html_url": ""}),
        ] {
            let rec = Recorder::default();
            let mut transport = rec.transport(vec![Some(json_reply(200, &json!([release]).to_string()))]);
            let got = query_latest_release("2.4.0", &mut transport)
                .unwrap_or_else(|| panic!("{:?} should still be an update", release));
            assert_eq!(got["url"], json!(UPGRADE_RELEASE_URL), "{:?}", release);
        }
    }

    #[test]
    fn upgrade_probe_is_silent_for_equal_older_and_unparseable_tags() {
        for (current, body) in [
            ("2.4.0", json!([{"tag_name": "2.4.0"}])),
            ("2.4.0", json!([{"tag_name": "2.3.9"}])),
            ("2.4.0", json!([{"tag_name": "not-a-version"}])),
            ("junk", json!([{"tag_name": "9.9.9"}])),
        ] {
            let rec = Recorder::default();
            let mut transport = rec.transport(vec![Some(json_reply(200, &body.to_string()))]);
            assert_eq!(query_latest_release(current, &mut transport), None, "{:?}", body);
        }
    }

    #[test]
    fn upgrade_probe_never_promotes_a_beta_onto_a_formal_build() {
        let rec = Recorder::default();
        let body = json!([{"tag_name": "2.5.0-beta.1", "prerelease": true}]).to_string();
        let mut transport = rec.transport(vec![Some(json_reply(200, &body))]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
    }

    #[test]
    fn upgrade_probe_skips_drafts() {
        let rec = Recorder::default();
        let body = json!([{"tag_name": "3.0.0", "draft": true}, {"tag_name": "2.4.1"}]).to_string();
        let mut transport = rec.transport(vec![Some(json_reply(200, &body))]);
        assert_eq!(query_latest_release("2.4.0", &mut transport).unwrap()["latest"], json!("2.4.1"));
    }

    #[test]
    fn upgrade_probe_swallows_every_failure_shape() {
        // transport error, non-200, invalid JSON, invalid UTF-8 — all `None`.
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![None]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
        let mut transport = rec.transport(vec![Some(json_reply(503, "[]"))]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
        let mut transport = rec.transport(vec![Some(json_reply(200, "[{"))]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
        let mut transport = rec.transport(vec![Some(Reply {
            status: 200,
            headers: Vec::new(),
            body: vec![b'[', b'{', 0xff, b'}'],
        })]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
    }

    #[test]
    fn upgrade_probe_does_not_laxly_decode_like_fetch_text() {
        // `resp.read(1024*1024).decode('utf-8')` is a **strict** decode; the
        // updater's `_fetch_text` uses `errors='replace'`.  Same bytes, opposite
        // verdict.
        let mut body = Vec::new();
        body.extend_from_slice(b"[{\"tag_name\":\"2.4.1\",\"note\":\"caf");
        body.extend_from_slice(&[0xc3, 0xa9]); // é, correctly encoded
        body.extend_from_slice(&[0xf0, 0x9f]); // truncated emoji -> invalid
        body.extend_from_slice(b"\"}]");
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(Reply {
            status: 200,
            headers: Vec::new(),
            body: body.clone(),
        })]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
        // and the lax path would have kept going:
        let lax = String::from_utf8_lossy(&body).into_owned();
        assert!(lax.contains("2.4.1"));
    }

    #[test]
    fn upgrade_probe_truncates_the_body_at_one_mebibyte() {
        // `resp.read(1024 * 1024)` happens **before** `json.loads`, so an
        // oversized answer is a parse failure rather than a success.
        let big = json!([{"tag_name": "2.4.1", "pad": "x".repeat(1024 * 1024 + 64)}]);
        let text = big.to_string();
        assert!(text.len() > 1024 * 1024);
        assert!(serde_json::from_str::<Value>(&text).is_ok());
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(json_reply(200, &text))]);
        assert_eq!(query_latest_release("2.4.0", &mut transport), None);
    }

    #[test]
    fn upgrade_probe_of_an_unparseable_running_version_makes_no_request() {
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![Some(json_reply(200, "[]"))]);
        assert_eq!(query_latest_release("\u{751f}\u{65e5}", &mut transport), None);
        assert_eq!(rec.count(), 0);
    }

    // ------------------------------------------------------- _UPGRADE_CACHE

    #[test]
    fn startup_probe_caches_a_positive_answer() {
        let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_upgrade_cache();
        let rec = Recorder::default();
        let body = json!([{"tag_name": "9.9.9"}]).to_string();
        let mut transport = rec.transport(vec![
            Some(json_reply(200, &body)),
            Some(json_reply(200, &body)),
        ]);
        let first = check_latest_release("2.4.0", &mut transport);
        let second = check_latest_release("2.4.0", &mut transport);
        assert_eq!(first, second);
        assert_eq!(first.unwrap()["latest"], json!("9.9.9"));
        assert_eq!(rec.count(), 1, "the probe runs at most once per process");
        clear_upgrade_cache();
    }

    #[test]
    fn startup_probe_caches_a_negative_answer_too() {
        // The whole point of `done`: a laptop booting offline must not re-probe
        // GitHub on every tick.
        let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_upgrade_cache();
        let rec = Recorder::default();
        let mut transport = rec.transport(vec![None, None]);
        assert_eq!(check_latest_release("2.4.0", &mut transport), None);
        assert_eq!(check_latest_release("2.4.0", &mut transport), None);
        assert_eq!(rec.count(), 1);
        clear_upgrade_cache();
    }

    // ------------------------------------------------- check_update, all tiers

    fn api_body(tag: &str, assets: Value) -> String {
        json!([{
            "tag_name": tag,
            "name": format!("ReadMD {}", tag),
            "body": "notes",
            "published_at": "2026-01-02T03:04:05Z",
            "html_url": format!("https://github.com/Natsummerance/readMD/releases/tag/{}", tag),
            "prerelease": false,
            "draft": false,
            "assets": assets,
        }])
        .to_string()
    }

    struct Harness {
        api: Recorder,
        sniff: Recorder,
        manifest: Recorder,
        sha: Recorder,
    }

    impl Harness {
        fn new() -> Self {
            Harness {
                api: Recorder::default(),
                sniff: Recorder::default(),
                manifest: Recorder::default(),
                sha: Recorder::default(),
            }
        }
        fn run(&self, current: &str, answers: Vec<Option<Reply>>, sha_bodies: Vec<Option<String>>) -> Value {
            let (_, mut sleep) = sleeper();
            let mut api = self.api.transport(answers);
            let mut sniff = self.sniff.transport(vec![
                Some(redirect("https://github.com/Natsummerance/readMD/releases/tag/v2.4.1")),
            ]);
            let mut manifest = self.manifest.transport(vec![Some(reply(200, &manifest_body()))]);
            let mut sha = self.sha.text_transport(sha_bodies);
            check_update(
                current,
                true,
                &mut api,
                &mut sniff,
                &mut manifest,
                &mut sha,
                &mut sleep,
            )
        }
    }

    #[test]
    fn tier_one_hit_never_touches_the_redirect_path() {
        let h = Harness::new();
        let payload = h.run("2.4.0", vec![Some(json_reply(200, &api_body("2.4.1", json!([]))))], vec![]);
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["has_update"], json!(true));
        assert_eq!(payload["latest_version"], json!("2.4.1"));
        assert_eq!(h.api.count(), 1);
        assert_eq!(h.sniff.count(), 0);
        assert_eq!(h.manifest.count(), 0);
        assert_eq!(h.sha.count(), 0);
    }

    #[test]
    fn a_rate_limited_api_falls_through_to_sniff_plus_manifest_and_recovers() {
        // The exact situation tiers 2 and 3 exist for: `api.github.com` answers
        // 403, the HTML endpoint still redirects to the newest tag, and the
        // digest list rebuilds the asset set.
        let h = Harness::new();
        let payload = h.run("2.4.0", vec![Some(json_reply(403, "{\"message\":\"rate limit\"}"))], vec![]);
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["has_update"], json!(true));
        assert_eq!(payload["latest_version"], json!("v2.4.1"));
        assert_eq!(payload["release_name"], json!("ReadMD v2.4.1"));
        assert_eq!(payload["published_at"], json!(""));
        assert_eq!(payload["release_notes"], json!("ReadMD v2.4.1"));
        assert_eq!(
            payload["html_url"],
            json!("https://github.com/Natsummerance/readMD/releases/tag/v2.4.1")
        );
        assert_eq!(h.sniff.count(), 1);
        assert_eq!(h.manifest.count(), 1);
        assert_eq!(
            h.manifest.calls()[0],
            "https://github.com/Natsummerance/readMD/releases/download/v2.4.1/SHA256SUMS.txt"
        );
    }

    #[test]
    fn the_sniffed_digest_reaches_the_asset_payload_without_a_second_fetch() {
        let h = Harness::new();
        let payload = h.run("2.4.0", vec![Some(json_reply(403, "{}"))], vec![]);
        if cfg!(windows) {
            assert_eq!(payload["asset"]["expected_sha"], json!(DIGEST));
            assert_eq!(payload["asset"]["name"], json!("ReadMDSetup.exe"));
        }
        // Tier 3 already carries `expected_sha`, so `resolve_expected_sha` must
        // not fetch again.
        assert_eq!(h.sha.count(), 0);
    }

    #[test]
    fn sniffing_a_tag_without_usable_assets_reports_the_network_error() {
        let (_, mut sleep) = sleeper();
        let api = Recorder::default();
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let sha = Recorder::default();
        let mut api_t = api.transport(vec![Some(json_reply(403, "{}"))]);
        let mut sniff_t = sniff.transport(vec![Some(redirect(
            "https://github.com/Natsummerance/readMD/releases/tag/v2.4.1",
        ))]);
        let mut manifest_t = manifest.transport(vec![Some(reply(200, "no digests here at all"))]);
        let mut sha_t = sha.text_transport(vec![]);
        let payload = check_update(
            "2.4.0",
            true,
            &mut api_t,
            &mut sniff_t,
            &mut manifest_t,
            &mut sha_t,
            &mut sleep,
        );
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["error_code"], json!("update_network_error"));
        assert_eq!(
            payload["html_url"],
            json!("https://github.com/Natsummerance/readMD/releases")
        );
    }

    #[test]
    fn every_tier_failing_still_answers_with_the_stable_code() {
        let (slept, mut sleep) = sleeper();
        let api = Recorder::default();
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let sha = Recorder::default();
        let mut api_t = api.transport(vec![None]);
        let mut sniff_t = sniff.transport(vec![None]);
        let mut manifest_t = manifest.transport(vec![None]);
        let mut sha_t = sha.text_transport(vec![]);
        let payload = check_update(
            "2.4.0", true, &mut api_t, &mut sniff_t, &mut manifest_t, &mut sha_t, &mut sleep,
        );
        assert_eq!(payload["ok"], json!(false));
        assert_eq!(payload["error_code"], json!("update_network_error"));
        // Measured against the authority with `CI=true` and every socket raising
        // `URLError`: `calls == ['api', 'api', 'sniff']`, `slept == [0.3]`.  Two
        // API attempts because `_fetch_release_json` goes through
        // `_open_with_retry` (`updater.py:233-246`, `TRANSIENT_FETCH_ATTEMPTS =
        // 2`); one sniff because `_sniff_latest_tag_redirect` calls `opener.open`
        // directly and never retries (`updater.py:309`); **zero** manifest
        // requests because the sniff came back empty and tier 3 is gated on a
        // tag (`updater.py:409-410`).
        assert_eq!(api.count(), 2, "a transient API failure is retried once");
        assert_eq!(sniff.count(), 1, "the sniffer has no retry loop");
        assert_eq!(manifest.count(), 0, "no tag sniffed means no manifest fetch");
        assert_eq!(slept.borrow().len(), 1, "the retry costs one backoff sleep");
    }

    #[test]
    fn a_falsy_tag_name_on_a_good_document_still_reaches_the_fallback() {
        // `updater.py:406` is `if not data or not data.get('tag_name')`.
        for body in [
            json!([{"tag_name": ""}]).to_string(),
            json!([{"tag_name": null}]).to_string(),
            json!([{"no_tag": true}]).to_string(),
        ] {
            let h = Harness::new();
            let payload = h.run("2.4.0", vec![Some(json_reply(200, &body))], vec![]);
            assert_eq!(
                payload["latest_version"],
                json!("v2.4.1"),
                "the sniffed tag must replace the falsy one for {:?}",
                body
            );
            assert_eq!(h.sniff.count(), 1);
        }
    }

    #[test]
    fn a_numeric_tag_name_is_truthy_so_tier_one_still_wins() {
        // `select_update_release` compares `str(3)` == version `3.0.0`, and
        // `updater.py:406`'s `data.get('tag_name')` is *truthy* for `3`.  A
        // string-only `has_tag` test would wrongly fall through to the sniff
        // here; the payload then echoes the raw number, not `""` and not `"3"`.
        let h = Harness::new();
        let body = json!([{"tag_name": 3, "assets": []}]).to_string();
        let payload = h.run("2.4.0", vec![Some(json_reply(200, &body))], vec![]);
        assert_eq!(payload["latest_version"], json!(3));
        assert_eq!(payload["has_update"], json!(true));
        assert_eq!(h.sniff.count(), 0, "a truthy numeric tag must not sniff");
    }

    #[test]
    fn malformed_api_json_falls_through_to_the_sniff() {
        let h = Harness::new();
        let payload = h.run("2.4.0", vec![Some(json_reply(200, "[{"))], vec![]);
        assert_eq!(payload["latest_version"], json!("v2.4.1"));
        assert_eq!(h.sniff.count(), 1);
    }

    #[test]
    fn an_equal_sniffed_tag_reports_no_update_and_no_asset() {
        let (_, mut sleep) = sleeper();
        let api = Recorder::default();
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let sha = Recorder::default();
        let mut api_t = api.transport(vec![Some(json_reply(403, "{}"))]);
        let mut sniff_t = sniff.transport(vec![Some(redirect(
            "https://github.com/Natsummerance/readMD/releases/tag/2.4.0",
        ))]);
        let mut manifest_t = manifest.transport(vec![Some(reply(200, &manifest_body()))]);
        let mut sha_t = sha.text_transport(vec![]);
        let payload = check_update(
            "2.4.0", true, &mut api_t, &mut sniff_t, &mut manifest_t, &mut sha_t, &mut sleep,
        );
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["has_update"], json!(false));
    }

    #[test]
    fn a_prerelease_build_keeps_its_beta_channel_through_the_api() {
        let h = Harness::new();
        let body = json!([
            {"tag_name": "2.5.0-beta.2", "prerelease": true, "assets": []},
            {"tag_name": "2.4.9", "assets": []},
        ])
        .to_string();
        let payload = h.run("2.5.0-beta.1", vec![Some(json_reply(200, &body))], vec![]);
        assert_eq!(payload["latest_version"], json!("2.5.0-beta.2"));
        assert_eq!(payload["has_update"], json!(true));
        assert_eq!(h.api.calls(), vec![GITHUB_API_RELEASES.to_string()]);
    }

    #[test]
    fn tier_one_sha_asset_triggers_exactly_one_digest_fetch() {
        let h = Harness::new();
        let assets = json!([
            {"name": "ReadMDSetup.exe", "size": 1, "browser_download_url": "https://github.com/Natsummerance/readMD/releases/download/2.5.0/ReadMDSetup.exe"},
            {"name": "SHA256SUMS.txt", "size": 2, "browser_download_url": "https://github.com/Natsummerance/readMD/releases/download/2.5.0/SHA256SUMS.txt"},
        ]);
        let body = api_body("2.5.0", assets);
        let sha_body = format!("{}  ReadMDSetup.exe\n", DIGEST);
        let payload = h.run(
            "2.4.0",
            vec![Some(json_reply(200, &body))],
            vec![Some(sha_body)],
        );
        assert_eq!(payload["sha_url"], json!("https://github.com/Natsummerance/readMD/releases/download/2.5.0/SHA256SUMS.txt"));
        assert_eq!(h.sha.count(), 1);
        if cfg!(windows) {
            assert_eq!(payload["asset"]["expected_sha"], json!(DIGEST));
        }
    }

    #[test]
    fn missing_sha_url_means_no_digest_request_at_all() {
        let h = Harness::new();
        let assets = json!([{"name": "ReadMDSetup.exe", "size": 1, "browser_download_url": "https://github.com/Natsummerance/readMD/releases/download/2.5.0/ReadMDSetup.exe"}]);
        let body = api_body("2.5.0", assets);
        let payload = h.run("2.4.0", vec![Some(json_reply(200, &body))], vec![Some("unused".to_string())]);
        assert_eq!(payload["sha_url"], Value::Null);
        assert_eq!(h.sha.count(), 0);
    }

    #[test]
    fn a_digest_fetch_failure_degrades_to_a_null_expected_sha() {
        let h = Harness::new();
        let assets = json!([
            {"name": "ReadMDSetup.exe", "size": 1, "browser_download_url": "https://github.com/Natsummerance/readMD/releases/download/2.5.0/ReadMDSetup.exe"},
            {"name": "SHA256SUMS.txt", "size": 2, "browser_download_url": "https://github.com/Natsummerance/readMD/releases/download/2.5.0/SHA256SUMS.txt"},
        ]);
        let body = api_body("2.5.0", assets);
        let payload = h.run("2.4.0", vec![Some(json_reply(200, &body))], vec![None]);
        assert_eq!(payload["ok"], json!(true));
        if cfg!(windows) {
            assert_eq!(payload["asset"]["expected_sha"], Value::Null);
        }
    }

    #[test]
    fn an_empty_release_array_falls_through_without_inventing_data() {
        let h = Harness::new();
        let payload = h.run("2.4.0", vec![Some(json_reply(200, "[]"))], vec![]);
        // `select_update_release` has no candidate, so tier 2 runs; the harness
        // sniff succeeds, which is exactly Python's behaviour on an empty list.
        assert_eq!(payload["latest_version"], json!("v2.4.1"));
        assert_eq!(h.sniff.count(), 1);
    }

    #[test]
    fn a_null_api_body_is_treated_as_one_non_dict_candidate() {
        // Python: `releases = payload if isinstance(payload, list) else [payload]`
        // with `payload = None` gives `[None]`, which
        // `isinstance(release, dict)` filters out — not a crash.
        let h = Harness::new();
        let payload = h.run("2.4.0", vec![Some(json_reply(200, "null"))], vec![]);
        assert_eq!(payload["latest_version"], json!("v2.4.1"));
    }

    #[test]
    fn ci_mode_never_asks_a_mirror_for_the_manifest() {
        // The harness runs with `ci = true`; a tier-3 recovery must therefore
        // have consulted exactly one URL, the official one.
        let h = Harness::new();
        h.run("2.4.0", vec![Some(json_reply(403, "{}"))], vec![]);
        assert_eq!(h.api.calls(), vec![GITHUB_API_LATEST.to_string()]);
        assert_eq!(h.sniff.calls(), vec!["https://github.com/Natsummerance/readMD/releases/latest".to_string()]);
        assert_eq!(h.manifest.count(), 1);
        assert!(!h.manifest.calls()[0].contains("ghfast"));
    }

    #[test]
    fn non_ci_mode_would_take_the_mirror_when_the_origin_is_unreachable() {
        let (_, mut sleep) = sleeper();
        let api = Recorder::default();
        let sniff = Recorder::default();
        let manifest = Recorder::default();
        let sha = Recorder::default();
        let mut api_t = api.transport(vec![Some(json_reply(403, "{}"))]);
        let mut sniff_t = sniff.transport(vec![
            None,
            Some(redirect("https://ghfast.top/https://github.com/a/b/releases/tag/v2.4.1")),
        ]);
        let mut manifest_t = manifest.transport(vec![
            None,
            Some(reply(200, &manifest_body())),
        ]);
        let mut sha_t = sha.text_transport(vec![]);
        let payload = check_update(
            "2.4.0", false, &mut api_t, &mut sniff_t, &mut manifest_t, &mut sha_t, &mut sleep,
        );
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["latest_version"], json!("v2.4.1"));
        assert_eq!(sniff.count(), 2);
        assert_eq!(manifest.count(), 2);
    }
}
