//! LAN / document-sharing authorization decision logic.
//!
//! Cluster-2 replication lane `plugin-desktop-s1`.  Python authority:
//! `readmd.py`.
//!
//! Ported callables:
//!   * `_is_private`           (readmd.py:3586) — the hand-rolled RFC1918 test.
//!   * `get_lan_ip`            (readmd.py:3597) — its *selection policy* only
//!     (`choose_lan_ip`): the socket probing and `gethostbyname` fallback are
//!     host-environment operations that already live in `server.rs::kernel_lan_ip`
//!     (owned by another lane, not edited here).
//!   * `_lan_route_authorized` (readmd.py:1116) — full pure decision tree,
//!     reusing `crate::validators::paths_within` for the containment check.
//!
//! This module is the pure logic a later serialised lane wires into the request
//! router; nothing here is wired into `server.rs` by this lane.

use crate::validators::paths_within;
use crate::version_spec::nd_value;

// --------------------------------------------------------------- IP privacy

fn is_py_space(ch: char) -> bool {
    // Python's `int()` filler set, MEASURED on CPython 3.11.15 by scanning every
    // code point for which `str.isspace()` is True and trying `int(c + '10')`:
    // exactly 25 code points are accepted —
    //   09-0D, 20, 85, A0, 1680, 2000-200A, 2028, 2029, 202F, 205F, 3000
    // — and exactly four are REJECTED: U+001C, U+001D, U+001E, U+001F
    // (`int('\x1c10')` -> ValueError: invalid literal for int() with base 10).
    // That accepted 25 is precisely Unicode `White_Space`, i.e. Rust's
    // `char::is_whitespace`, so `is_whitespace` is correct here and is NOT a
    // divergence.
    //
    // Do not "unify" this with `crate::native_system::py_strip` /
    // `window_state::SPACE_RANGES`: those model CPython's *other* predicate,
    // `Py_UNICODE_ISSPACE` (29 code points = these 25 plus U+001C..U+001F), used
    // by `str.strip()`/`str.isspace()`/`re` `\s`.  `int()` does not use it.  The
    // two models are deliberately different because the two Python operations
    // are deliberately different; importing the 29-char set here would make
    // `is_private("\u{1c}10.0.1.1")` return true, which CPython says is False.
    ch.is_whitespace()
}

/// Coerce a single dotted-quad component exactly as Python `int()` would, or
/// `None` where Python raises `ValueError`.  Leading/trailing whitespace (the
/// 25-code-point set above) and one optional `+`/`-` sign immediately before a
/// run of Unicode decimal digits are accepted; leading zeros are positional
/// base-10.  A single `_` between two digits is a separator (F-L5): CPython 3.6+
/// accepts `int('1_0') == 10` and `int('1_0_0') == 100`, but rejects a leading,
/// trailing, doubled or sign-adjacent underscore (`'_10'`, `'10_'`, `'1__0'`,
/// `'+_10'` all raise ValueError).
fn py_int(s: &str) -> Option<i128> {
    let t = s.trim_matches(is_py_space as fn(char) -> bool);
    let (neg, rest) = if let Some(r) = t.strip_prefix('-') {
        (true, r)
    } else if let Some(r) = t.strip_prefix('+') {
        (false, r)
    } else {
        (false, t)
    };
    if rest.is_empty() {
        return None;
    }
    let mut digits: Vec<u32> = Vec::new();
    let mut chars = rest.chars().peekable();
    while let Some(ch) = chars.next() {
        match nd_value(ch) {
            Some(v) => digits.push(v),
            None => {
                if ch != '_' || digits.is_empty() {
                    return None;
                }
                // Legal only when the very next code point is another digit; this
                // also rejects `'1__0'` (peek is `'_'`) and a trailing `'10_'`.
                match chars.peek().copied().and_then(nd_value) {
                    Some(_) => {}
                    None => return None,
                }
            }
        }
    }
    if digits.is_empty() {
        return None;
    }
    // Saturating: a component longer than 39 decimal digits cannot exceed the
    // tested ranges (10 / 172 / 192) either way, but must not panic in debug or
    // wrap in release.
    let mut v: i128 = 0;
    for d in digits {
        v = v.saturating_mul(10).saturating_add(d as i128);
    }
    Some(if neg { -v } else { v })
}

/// `_is_private` (readmd.py:3586): split on `'.'`, require exactly four parts,
/// parse only `parts[0]` and `parts[1]` with `int()` (a `ValueError` there means
/// "not private"), then test the three RFC1918 ranges.  `parts[2]`/`parts[3]`
/// are never parsed, so `10.0.abc.def` still counts as private.
pub fn is_private(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    let a = match py_int(parts[0]) {
        Some(v) => v,
        None => return false,
    };
    let b = match py_int(parts[1]) {
        Some(v) => v,
        None => return false,
    };
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

/// The selection half of `get_lan_ip` (readmd.py:3612-3616): return the first
/// RFC1918 candidate, else the first candidate at all, else `None` (the caller
/// falls back to `gethostbyname` / `127.0.0.1`).  Candidates are probed in the
/// order the socket loop collected them.
pub fn choose_lan_ip(candidates: &[String]) -> Option<String> {
    for ip in candidates {
        if is_private(ip) {
            return Some(ip.clone());
        }
    }
    candidates.first().cloned()
}

// ------------------------------------------------------- query parsing (parse_qs)

fn unquote_plus(s: &str) -> String {
    // Python `unquote_plus` = replace '+' with ' ', then percent-decode utf-8
    // with errors='replace'.
    let replaced = s.replace('+', " ");
    percent_encoding::percent_decode_str(&replaced)
        .decode_utf8_lossy()
        .into_owned()
}

fn unquote(s: &str) -> String {
    // Python `unquote` leaves '+' alone; only %XX escapes decode.
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

/// `urllib.parse.parse_qs(query)` with the default `keep_blank_values=False`:
/// split on '&', require an '=', drop pairs whose *raw* value is empty, then
/// `unquote_plus` both sides.  Returns ordered `(key, value)` pairs (Python
/// groups them by key; the caller filters by key which preserves that order).
pub fn parse_qs(query: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for token in query.split('&') {
        if token.is_empty() {
            continue;
        }
        let (raw_key, raw_val) = match token.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if raw_val.is_empty() {
            continue; // keep_blank_values=False
        }
        out.push((unquote_plus(raw_key), unquote_plus(raw_val)));
    }
    out
}

/// The candidate slice from `_lan_route_authorized` (readmd.py:1134): for each
/// of `p`, `path`, `dir` (that precedence order) collect every non-empty parsed
/// value, each in query order.
pub fn lan_candidates(query: &str) -> Vec<String> {
    let pairs = parse_qs(query);
    let mut out = Vec::new();
    for key in ["p", "path", "dir"] {
        for (k, v) in &pairs {
            if k == key && !v.is_empty() {
                out.push(v.clone());
            }
        }
    }
    out
}

const LAN_BLOCKED_PATHS: &[&str] = &[
    "/api/save",
    "/api/upload",
    "/api/image/save",
    "/api/code/run",
    "/api/update/download",
    "/api/update/apply",
    "/api/import/process",
    "/api/control/open",
    "/api/control/next",
    "/api/control/pet-batch",
    "/api/control/pet-menu",
    "/api/pets/import",
    "/api/pets/remove",
    "/api/pets/active",
    "/api/pets/configure",
    "/api/pets/install",
    "/api/pets/runtime/install",
    "/api/pets/uninstall",
    "/api/pets/update_status",
    "/api/pets/check_update",
    "/api/pets/apply_update",
    "/api/plugins/list",
    "/api/plugins/install",
    "/api/plugins/toggle",
    "/api/plugins/uninstall",
    "/api/links/index",
    "/api/transcribe",
];

const LAN_SCOPED_PATHS: &[&str] = &[
    "/api/file",
    "/api/list",
    "/api/ocr",
    "/api/convert",
    "/raw",
    "/api/links/graph",
    "/api/links/backlinks",
    "/api/links/deadlinks",
];

/// `_lan_route_authorized` (readmd.py:1116): restrict a shared LAN client to the
/// document scope.  `shared_root` is `server.shared_root` (an empty string means
/// no root is configured, matching Python's `if not root: return False`).
pub fn lan_route_authorized(path: &str, query: &str, shared_root: &str) -> bool {
    if path.starts_with("/api/skill-imports") {
        return false;
    }
    if path.starts_with("/api/recent/") {
        return false;
    }
    if LAN_BLOCKED_PATHS.contains(&path) {
        return false;
    }
    if !LAN_SCOPED_PATHS.contains(&path) {
        return true;
    }
    if shared_root.is_empty() {
        return false;
    }
    let candidates = lan_candidates(query);
    if candidates.is_empty() {
        return false;
    }
    for cand in candidates {
        // Python does `realpath(unquote(cand))`; `paths_within` already runs
        // `normcase(realpath(..))` on both sides, and realpath is idempotent, so
        // one `unquote` here plus the internal realpath reproduces the contract.
        //
        // F-L3 audit (verified, nothing added): readmd.py:1135-1141 wraps that
        // realpath in `try/except Exception: return False`.  It is not reachable
        // as an ALLOW-instead-of-DENY hole here, for two measured reasons.  (1)
        // `paths_within` is infallible — it returns `bool` and its whole call
        // graph (`validators.rs` realpath/abspath/normcase/split_drive) contains
        // no `Result`, no `?`, no `unwrap()` and no `panic!`, so an error can
        // only ever become a normalised string that the prefix test then denies
        // or accepts; it can never propagate.  (2) Non-strict `ntpath.realpath`
        // was measured NOT to raise for the candidate classes the router can
        // produce — an embedded NUL (`realpath('C:/root/a\x00b')` returns
        // `'C:\\root\\a\x00b'`), a missing path, or a different drive — it raises
        // only with `strict=True`, which readmd.py never passes.  The mixed-drive
        // and empty cases that DO make Python's `os.path.commonpath` raise
        // `(OSError, ValueError)` are handled by the component-wise prefix test in
        // the same deny direction.  A defensive "deny on embedded NUL" guard was
        // considered and rejected: CPython allows that candidate under root
        // `C:/root`, so denying it would manufacture a Rust-DENY/Python-ALLOW
        // divergence in a security gate.
        let target = unquote(&cand);
        if !paths_within(&target, shared_root) {
            return false;
        }
    }
    true
}

// =========================================================== tests (ground truth)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_private_rfc1918_ranges() {
        // Direct `_is_private` snapshots (readmd.py:3586).
        let t = |ip: &str| is_private(ip);
        assert!(t("10.0.0.1"));
        assert!(t("10.255.255.255"));
        assert!(!t("11.0.0.1"));
        assert!(!t("9.255.255.255"));
        assert!(t("172.16.0.1"));
        assert!(t("172.31.255.255"));
        assert!(!t("172.15.0.1"));
        assert!(!t("172.32.0.1"));
        assert!(t("192.168.0.1"));
        assert!(!t("192.169.0.1"));
        assert!(!t("192.167.255.255"));
        // 127.x is loopback but NOT in this hand-rolled RFC1918 set.
        assert!(!t("127.0.0.1"));
        assert!(!t("8.8.8.8"));
        assert!(!t("1.1.1.1"));
        assert!(!t("223.5.5.5"));
        assert!(!t("114.114.114.114"));
        assert!(!t("0.0.0.0"));
        assert!(!t("255.255.255.255"));
    }

    #[test]
    fn is_private_requires_four_dot_parts() {
        for bad in ["", "10", "10.0", "10.0.0", "10.0.0.0.0", "192.168.1", "10.0.0.1.2", "::1", "fe80::1", "2001:db8::1"] {
            assert!(!is_private(bad), "{bad:?}");
        }
    }

    #[test]
    fn is_private_int_coercion_matches_python() {
        // Only parts[0] and parts[1] are int()-parsed (whitespace, sign, leading
        // zeros); parts[2]/[3] are never touched, so garbage there still matches.
        assert!(is_private("10.0.abc.def"));
        assert!(is_private(" 10 . 0 .1.1"));
        assert!(is_private("10. 0.1.1"));
        assert!(is_private("010.0.1.1"));
        assert!(is_private("0010.0.1.1"));
        assert!(is_private("10.016.1.1"));
        assert!(is_private("172.016.0.1"));
        assert!(is_private("172. 16.0.1"));
        assert!(is_private("172.+16.0.1"));
        assert!(is_private("10.-1.1.1")); // a==10 short-circuits past b
        assert!(is_private("+10.0.0.1"));
        assert!(is_private("10.0.1.1 ")); // parts[3] never parsed
        assert!(is_private("10.300.1.1")); // b out of range irrelevant when a==10
        assert!(!is_private("a.b.c.d"));
        assert!(!is_private("10.x.y.z")); // b fails int()
        assert!(!is_private("300.1.1.1"));
        assert!(!is_private("-0.1.2.3"));
        assert!(!is_private("1e1.0.0.1"));
    }

    #[test]
    fn py_int_filler_set_is_pythons_int_not_py_unicode_isspace() {
        // F-L1 REFUTED by measurement: `int()` does NOT accept U+001C..U+001F.
        //   python -c "print(int('\x1c10'))"
        //   ValueError: invalid literal for int() with base 10: '\x1c10'
        // so `readmd.py:3590` answers False for that IP, and so must we.
        assert!(!is_private("\u{1c}10.0.1.1"));
        assert!(!is_private("\u{1d}10.0.1.1"));
        assert!(!is_private("\u{1e}10.0.1.1"));
        assert!(!is_private("\u{1f}10.0.1.1"));
        assert_eq!(py_int(" \u{1c}10"), None);
        for ch in ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}'] {
            assert!(!is_py_space(ch), "{ch:?} must not be int() filler");
        }

        // The 25 code points CPython's int() DOES accept as leading filler,
        // enumerated from the CPython 3.11.15 scan in the fix report.
        const INT_FILLER: [char; 25] = [
            '\t', '\n', '\u{0b}', '\u{0c}', '\r', ' ', '\u{85}', '\u{a0}', '\u{1680}',
            '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}',
            '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}',
            '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
        ];
        use crate::native_system::py_strip;
        for ch in INT_FILLER {
            assert!(is_py_space(ch), "{ch:?} must be int() filler");
            // `_is_private` only int()-parses parts[0] and parts[1].
            assert!(is_private(&format!("{ch}10.0.1.1")), "leading {ch:?}");
            assert!(is_private(&format!("10.{ch}0.1.1")), "leading {ch:?} on b");
            // `_10.0.1.1` is rejected by int(); a *trailing* filler is accepted.
            assert!(is_private(&format!("10{ch}.0.1.1")), "trailing {ch:?}");
        }
        // The two Python predicates differ by exactly these four code points:
        // `str.strip()` (29) removes what `int()` (25) will not.  This assert is
        // the guard against a future "unify the two models" edit in either file.
        for ch in ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}'] {
            assert_eq!(py_strip(&ch.to_string()), "", "{ch:?} is Py_UNICODE_ISSPACE");
        }
        // An over-long component must not panic; CPython yields a huge int that
        // matches none of 10 / 172 / 192, and saturation keeps that verdict.
        assert!(!is_private(&format!("{}.0.1.1", "1".repeat(40))));
    }

    #[test]
    fn py_int_accepts_pythons_underscore_digit_separators() {
        // F-L5, measured on CPython 3.11.15 through the verbatim `_is_private`
        // (readmd.py:3586): int('1_0') == 10, int('1_0_0') == 100, while '_10',
        // '10_', '1__0' and '+_10' all raise ValueError.
        assert_eq!(py_int("1_0"), Some(10));
        assert_eq!(py_int("1_0_0"), Some(100));
        assert_eq!(py_int("-1_0"), Some(-10));
        assert_eq!(py_int("+1_0"), Some(10));
        assert_eq!(py_int("1_٠"), Some(10)); // separator rule is digit-kind agnostic
        assert_eq!(py_int("_10"), None);
        assert_eq!(py_int("10_"), None);
        assert_eq!(py_int("1__0"), None);
        assert_eq!(py_int("+_10"), None);
        assert_eq!(py_int("1_"), None);
        assert_eq!(py_int("_"), None);
        // Observable through the ported authority: 1_0.0.1.1 IS 10.0.1.1 to Python.
        assert!(is_private("1_0.0.1.1"));
        assert!(is_private("10.1_6.1.1")); // 172.16 -> b==16 via '1_6'
        assert!(is_private("1_72.1_6.0.1"));
        assert!(!is_private("_10.0.1.1"));
        assert!(!is_private("10_.0.1.1"));
        assert!(!is_private("1__0.0.1.1"));
        assert!(!is_private("+_10.0.1.1"));
    }

    #[test]
    fn choose_lan_ip_prefers_private_then_first() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
        // First private wins, not the first overall.
        assert_eq!(choose_lan_ip(&s(&["8.8.8.8", "192.168.1.5", "10.0.0.2"])), Some("192.168.1.5".to_string()));
        // No private -> first candidate.
        assert_eq!(choose_lan_ip(&s(&["8.8.8.8", "1.1.1.1"])), Some("8.8.8.8".to_string()));
        // Empty -> None (caller falls back to gethostbyname / 127.0.0.1).
        assert_eq!(choose_lan_ip(&s(&[])), None);
    }

    #[test]
    fn lan_candidates_matches_python_parse_qs() {
        let table: Vec<(&str, Vec<&str>)> = vec![
            ("p=%2Fdocs%2Fa.md", vec!["/docs/a.md"]),
            ("path=/docs/a.md", vec!["/docs/a.md"]),
            ("dir=/docs", vec!["/docs"]),
            ("p=&path=/x", vec!["/x"]),
            ("p=x&path=", vec!["x"]),
            ("a=1&b=2", vec![]),
            ("p=x=y", vec!["x=y"]),
            ("p=a&p=b", vec!["a", "b"]),
            ("dir=/tmp&p=/x&path=/y", vec!["/x", "/y", "/tmp"]), // p,path,dir precedence
            ("p=x+&p=%2B", vec!["x ", "+"]),
            ("p=%", vec!["%"]),
            ("p=%zz", vec!["%zz"]),
            ("", vec![]),
            ("p=%2e%2e%2fx", vec!["../x"]),
            ("noequals", vec![]),
            ("p", vec![]),
            ("p=%C3%A9", vec!["\u{e9}"]),
            ("path=/shared/doc&extra=1", vec!["/shared/doc"]),
            ("p=%41", vec!["A"]),
        ];
        for (q, want) in table {
            assert_eq!(lan_candidates(q), want.iter().map(|x| x.to_string()).collect::<Vec<String>>(), "candidates({q:?})");
        }
    }

    #[test]
    fn parse_qs_skips_blank_and_keyless_tokens() {
        // keep_blank_values=False semantics: a '=' with an empty raw value is gone.
        assert_eq!(parse_qs("p="), Vec::<(String, String)>::new());
        assert_eq!(parse_qs("="), Vec::<(String, String)>::new());
        assert_eq!(parse_qs("p"), Vec::<(String, String)>::new());
        assert_eq!(parse_qs("p=a").len(), 1);
        assert_eq!(parse_qs("p=a&path=b")[0], ("p".to_string(), "a".to_string()));
    }

    #[test]
    fn route_non_scoped_and_blocked_paths() {
        // Blocked / always-denied prefixes never depend on the root or query.
        assert!(!lan_route_authorized("/api/skill-imports", "p=x", "/root"));
        assert!(!lan_route_authorized("/api/skill-imports/preview", "", "/root"));
        assert!(!lan_route_authorized("/api/recent/add", "", "/root"));
        assert!(!lan_route_authorized("/api/save", "p=/root/a.md", "/root"));
        assert!(!lan_route_authorized("/api/code/run", "", "/root"));
        // Anything NOT in the scoped set is allowed outright (reader pages, assets).
        assert!(lan_route_authorized("/api/ping", "", ""));
        assert!(lan_route_authorized("/index.html", "", ""));
    }

    #[test]
    fn scoped_path_needs_root_and_candidates() {
        // Scoped but no shared root configured -> deny.
        assert!(!lan_route_authorized("/api/file", "p=/x", ""));
        // Root set but no p/path/dir candidate -> deny.
        assert!(!lan_route_authorized("/api/file", "other=1", "/tmp/root"));
        assert!(!lan_route_authorized("/api/ocr", "", "/tmp/root"));
    }

    #[test]
    fn scoped_path_containment_against_a_real_temp_root() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("readmd_lg_{nonce}"));
        std::fs::create_dir_all(&root).unwrap();
        let inside = root.join("doc.md");
        std::fs::write(&inside, b"x").unwrap();
        let outside = std::env::temp_dir().join(format!("readmd_lg_outside_{nonce}.md"));
        std::fs::write(&outside, b"x").unwrap();

        let enc = |p: &std::path::Path| -> String {
            percent_encoding::utf8_percent_encode(
                p.to_string_lossy().as_ref(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string()
        };
        let root_s = root.to_string_lossy().to_string();
        let q_in = format!("p={}", enc(&inside));
        let q_out = format!("p={}", enc(&outside));
        let q_escape = "p=..%2F..%2Fetc%2Fpasswd";

        assert!(lan_route_authorized("/api/file", &q_in, &root_s), "inside should be authorized");
        assert!(!lan_route_authorized("/api/file", &q_out, &root_s), "outside must be denied");
        assert!(!lan_route_authorized("/api/file", q_escape, &root_s), "traversal must be denied");

        let _ = std::fs::remove_file(&inside);
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }
}

