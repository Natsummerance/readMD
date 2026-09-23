//! Plugin version / environment decision logic.
//!
//! Cluster-1 replication lane `plugin-desktop-s1`.  Python authority:
//! `src/readmd_modules/plugin_manager.py`.
//!
//! Ported callables (line-by-line decision logic, Python wins on any diff):
//!   * `_version_parts`            (plugin_manager.py:458)
//!   * `_version_satisfies`        (plugin_manager.py:462)
//!   * `_split_requirement`        (plugin_manager.py:445) — needed to feed the
//!     version checker exactly like `_environment_plugin_ready` does.
//!   * `_environment_plugin_ready` (plugin_manager.py:504) — the *pure* decision
//!     tree only; the host facts it consults (`find_spec`,
//!     `_installed_distribution_version`, `_sandbox_metadata_exists`) are
//!     supplied by the caller so this stays offline and side-effect free.
//!   * `_LineEmitter`              (plugin_manager.py:614) — the pip-stream
//!     line splitter, a pure buffering object.
//!
//! Deliberately NOT ported here: `_run_pip_inprocess` (plugin_manager.py:661) —
//! it shells out to `pip`/drives it in-process; the shipped Rust binary must
//! never spawn an external program, so the design replaces the pip pipeline with
//! its own installer.  See the lane fix report.
//!
//! Reuse (read-only) from another module: `crate::validators::normcase` and
//! `crate::native_system::py_strip`.  `py_strip` is the crate's single model for
//! Python's argument-free `str.strip()` (measured 29-code-point
//! `Py_UNICODE_ISSPACE` set); importing it instead of re-implementing keeps this
//! module and `native_system.rs` from carrying two divergent copies.  Note that
//! Python's `int()` uses a *narrower* 25-code-point filler set — see the header
//! of `lan_guard.rs` — so `py_strip` must never be applied to digit parsing.

use std::sync::OnceLock;

use regex::Regex;

use crate::native_system::py_strip;
use crate::validators::normcase;

// -------------------------------------------------------------- Unicode digits
//
// Python `re.findall(r'\d+', raw)` matches runs of Unicode *decimal* digits
// (general category `Nd`), and `int(part)` then evaluates that run.  The Rust
// `regex` crate's `\d` is also `\p{Nd}`, but we scan by hand so the module has
// no hidden behaviour: every `Nd` block is ten contiguous code points starting
// at the block's zero, so a base table plus `ch - base` reproduces `int()`.
// The table below is the exact `Nd` zero set (ascending code points) captured
// from CPython 3.11 in `scratch/rust_parity/plugin_desktop_s1/ground_truth.json`.
// Values are written in decimal to avoid any hex-transcription slip.
const ND_ZEROS: [u32; 66] = [
    48, 1632, 1776, 1984, 2406, 2534, 2662, 2790, 2918, 3046, 3174, 3302, 3430, 3558, 3664, 3792,
    3872, 4160, 4240, 6112, 6160, 6470, 6608, 6784, 6800, 6992, 7088, 7232, 7248, 42528, 43216,
    43264, 43472, 43504, 43600, 44016, 65296, 66720, 68912, 69734, 69872, 69942, 70096, 70384,
    70736, 70864, 71248, 71360, 71472, 71904, 72016, 72784, 73040, 73120, 92768, 92864, 93008,
    120782, 120792, 120802, 120812, 120822, 123200, 123632, 125264, 130032,
];

/// Decimal value of a single `Nd` code point, or `None` when it is not a digit.
pub(crate) fn nd_value(ch: char) -> Option<u32> {
    let cp = ch as u32;
    // Largest base <= cp; the block is the 10 code points [base, base+9].
    let mut best: Option<u32> = None;
    for &base in ND_ZEROS.iter() {
        if base <= cp {
            best = Some(base);
        } else {
            break;
        }
    }
    match best {
        Some(base) if cp - base < 10 => Some(cp - base),
        _ => None,
    }
}

/// `_version_parts` (plugin_manager.py:458): `[int(part) for part in re.findall(r'\d+', raw)[:4]]`.
///
/// Returns at most four component values; each component is one contiguous run
/// of Unicode decimal digits.  Python's `int` is arbitrary precision; the widest
/// `i128` cannot hold it (`i128::MAX` is 39 decimal digits, while
/// `int('1'*40)` = 1111111111111111111111111111111111111111 measured on this
/// box).  A run past that bound therefore **saturates** at `i128::MAX` instead of
/// panicking (debug) or wrapping (release) — the same policy `window_state.rs`'s
/// `py_int` picks for the identical job, so the crate has one answer.  Known,
/// documented cost: two distinct over-long runs compare equal to each other.
pub fn version_parts(raw: &str) -> Vec<i128> {
    let mut runs: Vec<i128> = Vec::new();
    let mut cur: Option<i128> = None;
    for ch in raw.chars() {
        match nd_value(ch) {
            Some(v) => {
                let mut acc = cur.unwrap_or(0);
                acc = acc.saturating_mul(10).saturating_add(v as i128);
                cur = Some(acc);
            }
            None => {
                if let Some(done) = cur.take() {
                    runs.push(done);
                    if runs.len() == 4 {
                        return runs;
                    }
                }
            }
        }
    }
    if let Some(done) = cur {
        runs.push(done);
    }
    runs.truncate(4);
    runs
}

/// Python `re` `\s` for a *str* pattern, spelled out.
///
/// Measured on this box (CPython 3.11.15): the set of code points matched by
/// `\s` is exactly the set where `str.isspace()` is True — 29 code points, i.e.
/// Unicode `White_Space` (25) **plus** `U+001C..U+001F` — while the `regex`
/// crate's `\s` is only `White_Space` (25).  Writing the class out keeps parity
/// and makes a future `\s` slip visible instead of silently narrowing the set.
const PY_WS_CLASS: &str = r"[\x{09}-\x{0D}\x{1C}-\x{20}\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]";

fn version_re_split() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `^(===|==|~=|!=|<=|>=|<|>)\s*(.+)$` (plugin_manager.py:452).
        Regex::new(&format!(
            r"^(===|==|~=|!=|<=|>=|<|>){ws}*(.+)$",
            ws = PY_WS_CLASS
        ))
        .unwrap()
    })
}

fn version_re_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `^([A-Za-z0-9][A-Za-z0-9._-]*)\s*(.*)$` (plugin_manager.py:447).
        Regex::new(&format!(
            r"^([A-Za-z0-9][A-Za-z0-9._-]*){ws}*(.*)$",
            ws = PY_WS_CLASS
        ))
        .unwrap()
    })
}

/// `_split_requirement` (plugin_manager.py:445): turn `'openai-whisper>=1.0,<2'`
/// into `(name, [(operator, version)])`.  An unparseable name yields `("", [])`;
/// clauses without a recognised operator prefix are silently dropped.
///
/// All three whitespace operations are Python `str.strip()` (`requirement.strip()`
/// :447, `clause.strip()` :452, `parsed.group(2).strip()` :454) and therefore go
/// through `py_strip`, **not** `str::trim()`: Rust's `trim` follows Unicode
/// `White_Space` and leaves `U+001C..U+001F`, which CPython strips
/// (`'\x1cp>=1'.strip() == 'p>=1'` measured), turning `('p', [('>=', '1')])` into
/// `('', [])`.
pub fn split_requirement(requirement: &str) -> (String, Vec<(String, String)>) {
    let trimmed = py_strip(requirement);
    let caps = match version_re_name().captures(trimmed) {
        Some(c) => c,
        None => return (String::new(), Vec::new()),
    };
    let name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
    let tail = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    let mut clauses = Vec::new();
    for clause in tail.split(',') {
        if let Some(pc) = version_re_split().captures(py_strip(clause)) {
            let op = pc.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let ver = pc.get(2).map(|m| py_strip(m.as_str()).to_string()).unwrap_or_default();
            clauses.push((op, ver));
        }
    }
    (name, clauses)
}

/// `_version_satisfies` (plugin_manager.py:462): verify the installed version
/// against the parsed clauses.  An unrecognised operator or an empty version
/// component means "not satisfied" (better a re-install than a false "installed").
pub fn version_satisfies(installed: &str, clauses: &[(String, String)]) -> bool {
    let have = version_parts(installed);
    if have.is_empty() {
        return false;
    }
    for (operator, wanted_raw) in clauses {
        if operator != "=="
            && operator != "==="
            && operator != "!="
            && operator != "<="
            && operator != ">="
            && operator != "<"
            && operator != ">"
        {
            return false;
        }
        let want = version_parts(wanted_raw);
        if want.is_empty() {
            return false;
        }
        let width = have.len().max(want.len());
        let mut left = have.clone();
        let mut right = want.clone();
        left.resize(width, 0);
        right.resize(width, 0);
        let eq = left == right;
        let lt = left < right;
        let gt = left > right;
        let pass = match operator.as_str() {
            "==" | "===" => eq,
            "!=" => !eq,
            ">=" => !lt,
            ">" => gt,
            "<=" => !gt,
            "<" => lt,
            _ => false,
        };
        if !pass {
            return false;
        }
    }
    true
}

/// Host facts `_environment_plugin_ready` consults before it reaches its pure
/// decision tree.  The caller (a later serialised lane) resolves them from the
/// sandbox / importlib / filesystem helpers already present in
/// `plugin_manager.rs`; keeping them as inputs makes this function total and
/// side-effect free so it can be unit-tested offline.
pub struct EnvReadyFacts<'a> {
    /// `str(spec.get('import_name') or '')`
    pub import_name: &'a str,
    /// `str(spec.get('package') or import_name)`
    pub package: &'a str,
    /// `importlib.util.find_spec` raised an exception.
    pub find_spec_error: bool,
    /// `find_spec` returned a spec (not `None`).
    pub spec_found: bool,
    /// Raw `found.origin` (normcased internally, matching `os.path.normcase`).
    pub origin: &'a str,
    /// `PLUGINS_SITE_PACKAGES`, normcased internally.
    pub sandbox_root: &'a str,
    /// `_sandbox_metadata_exists(package)`.
    pub sandbox_metadata_exists: bool,
    /// `_installed_distribution_version(package, import_name)`.
    pub installed_version: &'a str,
    /// `_requested_requirement(spec)` — fed through `split_requirement`.
    pub requirement: &'a str,
}

/// `_environment_plugin_ready` (plugin_manager.py:504): the pure decision tree.
/// Returns `(ready, version)`.  Every early-out collapses to `(false, "")` except
/// the version-mismatch tail, which returns `(false, installed_version)`.
pub fn environment_plugin_ready(facts: &EnvReadyFacts) -> (bool, String) {
    if facts.import_name.is_empty() {
        return (false, String::new());
    }
    if facts.find_spec_error || !facts.spec_found {
        return (false, String::new());
    }
    let origin = normcase(facts.origin);
    let sandbox = normcase(facts.sandbox_root);
    // Literal `origin.startswith(os.path.normcase(PLUGINS_SITE_PACKAGES))`.
    if origin.starts_with(sandbox.as_str()) && !facts.sandbox_metadata_exists {
        return (false, String::new());
    }
    let (_name, clauses) = split_requirement(facts.requirement);
    if !clauses.is_empty() && !version_satisfies(facts.installed_version, &clauses) {
        return (false, facts.installed_version.to_string());
    }
    (true, facts.installed_version.to_string())
}

// ---------------------------------------------------------------- _LineEmitter

/// `_LineEmitter` (plugin_manager.py:614): a buffering writer that forwards a
/// pip output stream line-by-line to a callback.  Python splits on the single
/// `'\n'` character and hands over the text before it; `flush` emits a trailing
/// partial line.  `\r` is *not* a boundary here (this is not `splitlines`).
#[derive(Default)]
pub struct LineEmitter {
    buffer: String,
}

impl LineEmitter {
    pub fn new() -> Self {
        LineEmitter { buffer: String::new() }
    }

    /// `write(text)`: append the chunk and drain every complete line.  Returns
    /// the number of code points written (Python's `len(text)`).
    pub fn write<F: FnMut(&str)>(&mut self, text: &str, mut on_line: F) -> usize {
        self.buffer.push_str(text);
        while let Some(nl) = self.buffer.find('\n') {
            let line = self.buffer[..nl].to_string();
            self.buffer.drain(..nl + 1);
            on_line(&line);
        }
        text.chars().count()
    }

    /// `flush()`: emit any buffered partial line and clear the buffer.
    pub fn flush<F: FnMut(&str)>(&mut self, mut on_line: F) {
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            on_line(&line);
        }
    }

    /// `writable()` — always true.
    pub fn writable(&self) -> bool {
        true
    }

    /// `isatty()` — always false.
    pub fn isatty(&self) -> bool {
        false
    }

    /// Number of buffered code points not yet terminated by a newline.
    pub fn buffered_len(&self) -> usize {
        self.buffer.chars().count()
    }
}

// =========================================================== tests (ground truth)
//
// Expected values are snapshots captured from the verbatim Python functions in
// `scratch/rust_parity/plugin_desktop_s1/oracle.py`, not from this Rust code.

#[cfg(test)]
mod tests {
    use super::*;

    fn clauses(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(o, w)| (o.to_string(), w.to_string())).collect()
    }

    #[test]
    fn version_parts_matches_python_matrix() {
        // (raw, expected) straight from `_version_parts` (plugin_manager.py:458).
        let table: Vec<(&str, Vec<i128>)> = vec![
            ("", vec![]),
            (" ", vec![]),
            ("1", vec![1]),
            ("1.0", vec![1, 0]),
            ("1.2.3", vec![1, 2, 3]),
            ("1.2.3.4", vec![1, 2, 3, 4]),
            ("1.2.3.4.5", vec![1, 2, 3, 4]), // [:4] cap
            ("1.2.3.4.5.6.7", vec![1, 2, 3, 4]),
            ("0", vec![0]),
            ("007", vec![7]),
            ("0001", vec![1]),
            ("00", vec![0]),
            ("2.4.0", vec![2, 4, 0]),
            ("v1.2.3", vec![1, 2, 3]),
            ("1.2.3.post1", vec![1, 2, 3, 1]),
            ("1.2.3.dev4", vec![1, 2, 3, 4]),
            ("1.2.3+build.9", vec![1, 2, 3, 9]),
            ("abc", vec![]),
            ("1.x.3", vec![1, 3]),
            ("1..3", vec![1, 3]),
            ("10.0.0-beta", vec![10, 0, 0]),
            ("  4.5  ", vec![4, 5]),
            ("12", vec![12]),
            ("999999999999999999", vec![999999999999999999]),
        ];
        for (raw, want) in table {
            assert_eq!(version_parts(raw), want, "version_parts({raw:?})");
        }
    }

    #[test]
    fn split_requirement_strips_u001c_u001f_like_python_str_strip() {
        // F-V1.  Every expectation is the verbatim CPython result of
        // `_split_requirement` (plugin_manager.py:445) on 3.11.15, captured in
        // scratch/rust_parity/vspec_languard_fix_s12/REPORT.md.  `str::trim()`
        // (Unicode White_Space, 25 code points) fails all of the U+001C..U+001F
        // rows; Python's `str.strip()` (Py_UNICODE_ISSPACE, 29) passes them.
        let table: Vec<(&str, &str, Vec<(&str, &str)>)> = vec![
            ("\u{1c}p>=1", "p", vec![(">=", "1")]),
            ("p\u{1c}>=1", "p", vec![(">=", "1")]),
            ("\u{1c}\u{1d}\u{1e}\u{1f}p >= 1 \u{1c}", "p", vec![(">=", "1")]),
            ("\u{a0}p==2.0", "p", vec![("==", "2.0")]),
            ("p>=1,\u{1c}<2", "p", vec![(">=", "1"), ("<", "2")]),
            ("p>=\u{1c}1", "p", vec![(">=", "1")]),
            ("p\u{1f}>=\u{1e}1.0\u{1d}", "p", vec![(">=", "1.0")]),
            ("\u{2028}p<=9\u{2029}", "p", vec![("<=", "9")]),
            ("p>=1 \u{1c},<2", "p", vec![(">=", "1"), ("<", "2")]),
            ("\u{1c}p", "p", vec![]),
            ("p\u{1c}", "p", vec![]),
            (" \u{1c} ", "", vec![]),
            // Operator alternation is not confused by a separator inside `>=`:
            // Python yields ('>', '=1'), because `\s*` eats U+001C after `>`.
            ("p>\u{1c}=1", "p", vec![(">", "=1")]),
        ];
        for (req, name, cl) in table {
            let (got_name, got_clauses) = split_requirement(req);
            assert_eq!(got_name, name, "split_requirement({req:?}) name");
            assert_eq!(got_clauses, clauses(&cl), "split_requirement({req:?}) clauses");
        }
    }

    #[test]
    fn version_parts_saturates_past_i128_instead_of_overflowing() {
        // F-V2.  CPython: `int('1'*40)` == 1111111111111111111111111111111111111111
        // (40 digits) while `i128::MAX` == 170141183460469231731687303715884105727
        // (39 digits), so the accumulator must saturate rather than panic in debug
        // or wrap in release.
        assert_eq!(version_parts(&"1".repeat(38)), vec![11111111111111111111111111111111111111i128]);
        assert_eq!(version_parts(&"1".repeat(39)), vec![111111111111111111111111111111111111111i128]);
        assert_eq!(version_parts(&"1".repeat(40)), vec![i128::MAX]);
        assert_eq!(version_parts(&"9".repeat(400)), vec![i128::MAX]);
        assert_eq!(version_parts(&format!("{}-{}", "7".repeat(50), "1")), vec![i128::MAX, 1]);
        // No panic, and the comparison path survives a saturated component.
        assert!(!version_satisfies(&"1".repeat(40), &clauses(&[("==", "1")])));
    }

    #[test]
    fn version_parts_handles_unicode_decimal_digits_like_python() {
        // Python `\d` == Unicode Nd; these are NOT ASCII but are matched by findall.
        assert_eq!(version_parts("\u{661}\u{662}\u{663}"), vec![123]); // Arabic-Indic
        assert_eq!(version_parts("\u{ff11}\u{ff12}\u{ff13}"), vec![123]); // fullwidth
        assert_eq!(version_parts("1\u{662}3"), vec![123]); // mixed scripts, one run
    }

    #[test]
    fn version_parts_rejects_non_nd_number_chars() {
        // Superscript-two (No), vulgar-half (No) and Roman-eight (Nl) are NOT `\d`.
        assert_eq!(version_parts("\u{b2}"), Vec::<i128>::new());
        assert_eq!(version_parts("\u{bd}"), Vec::<i128>::new());
        assert_eq!(version_parts("\u{2167}"), Vec::<i128>::new());
    }

    #[test]
    fn satisfies_equality_and_identity() {
        let cases = [
            ("1.2.3", &[("==", "1.2.3")][..], true),
            ("1.2.3", &[("==", "1.2")][..], false),
            ("1.2", &[("==", "1.2.3")][..], false),
            ("1.2.3", &[("===", "1.2.3")][..], true),
            ("007", &[("==", "7")][..], true),
            ("7", &[("==", "007")][..], true),
            ("v1.2.3", &[("==", "1.2.3")][..], true),
            ("1.2.3.4", &[("==", "1.2.3")][..], false),
            ("1.2.3", &[("==", "1.2.3+build.9")][..], false),
        ];
        for (inst, cl, want) in cases {
            assert_eq!(version_satisfies(inst, &clauses(cl)), want, "{inst} == {cl:?}");
        }
    }

    #[test]
    fn satisfies_not_equal() {
        assert!(version_satisfies("1.2.3", &clauses(&[("!=", "1.2")])));
        assert!(!version_satisfies("1.2.3", &clauses(&[("!=", "1.2.3")])));
    }

    #[test]
    fn satisfies_ge_le_length_padded() {
        let cases = [
            ("1.2.3", ">=", "1.2.3", true),
            ("1.2.3", ">=", "1.2.4", false),
            ("1.2.3", ">=", "1.2.2", true),
            ("1.2.3", ">=", "001", true),
            ("1.2.3", ">=", "1.2.3.0", true), // trailing zero pad keeps equality
            ("1.2.3", "<=", "1.2.3", true),
            ("1.2.3", "<=", "1.2.2", false),
            ("1.2.3", "<=", "1.2.4", true),
            ("1.2.3", "<=", "1.2.3.0", true),
        ];
        for (inst, op, want, exp) in cases {
            assert_eq!(version_satisfies(inst, &clauses(&[(op, want)])), exp, "{inst} {op} {want}");
        }
    }

    #[test]
    fn satisfies_gt_lt_strict() {
        let cases = [
            ("1.2.3", ">", "1.2.3", false),
            ("1.2.3", ">", "1.2.2", true),
            ("1.2.3", ">", "1.2.4", false),
            ("1.2.3", ">", "1.2.3.1", false),
            ("0.0", ">", "0", false),
            ("1.2.3", "<", "1.2.3", false),
            ("1.2.3", "<", "1.2.4", true),
            ("1.2.3", "<", "1.2.2", false),
            ("1.2.3", "<", "1.2.3.0", false),
            ("10", ">=", "9", true), // numeric, not lexicographic
        ];
        for (inst, op, want, exp) in cases {
            assert_eq!(version_satisfies(inst, &clauses(&[(op, want)])), exp, "{inst} {op} {want}");
        }
    }

    #[test]
    fn satisfies_unknown_operator_is_false() {
        // "operator not in (...)" -> reject. `~=` parses via split but is rejected here.
        assert!(!version_satisfies("1.2.3", &clauses(&[("~=", "1.2")])));
        assert!(!version_satisfies("1.2.3", &clauses(&[("=<", "1.2.3")])));
        assert!(!version_satisfies("1.2.3", &clauses(&[("", "1.2.3")])));
        // An unknown op short-circuits even behind a satisfied earlier clause.
        assert!(!version_satisfies("1.2.3", &clauses(&[(">=", "1.2.3"), ("~=", "1.2")])));
    }

    #[test]
    fn satisfies_empty_installed_or_bad_wanted_is_false() {
        // `_version_parts(installed)` empty -> immediate False.
        assert!(!version_satisfies("", &clauses(&[("==", "1.2.3")])));
        assert!(!version_satisfies("abc", &clauses(&[("==", "1.2.3")])));
        // want empty -> clause False; but an empty clause list is vacuously True.
        assert!(!version_satisfies("1.2.3", &clauses(&[("==", "abc")])));
        assert!(!version_satisfies("1.2.3", &clauses(&[("==", "")])));
        assert!(version_satisfies("1.2.3", &[]));
        assert!(!version_satisfies("", &[])); // no `have` -> False before the loop
    }

    #[test]
    fn satisfies_prerelease_and_build_components() {
        assert!(!version_satisfies("1.2.3.post1", &clauses(&[("==", "1.2.3")])));
        assert!(!version_satisfies("1.2.3", &clauses(&[("==", "1.2.3.dev4")])));
        // >= treats them numerically by their captured digit runs.
        assert!(version_satisfies("1.2.3.post1", &clauses(&[(">=", "1.2.3")])));
    }

    #[test]
    fn satisfies_range_clauses() {
        assert!(version_satisfies("1.0", &clauses(&[(">=", "1"), ("<", "2")])));
        assert!(!version_satisfies("2.0", &clauses(&[(">=", "1"), ("<", "2")])));
        assert!(!version_satisfies("0.5", &clauses(&[(">=", "1"), ("<", "2")])));
    }

    #[test]
    fn split_requirement_matches_python_matrix() {
        let table: Vec<(&str, &str, Vec<(&str, &str)>)> = vec![
            ("openai-whisper>=1.0,<2", "openai-whisper", vec![(">=", "1.0"), ("<", "2")]),
            ("whisper", "whisper", vec![]),
            ("", "", vec![]),
            ("  ", "", vec![]),
            ("numpy ==1.26.4", "numpy", vec![("==", "1.26.4")]),
            ("a~=2.0", "a", vec![("~=", "2.0")]),
            ("pkg!=1.0", "pkg", vec![("!=", "1.0")]),
            ("p>=1,<=2,!=1.5", "p", vec![(">=", "1"), ("<=", "2"), ("!=", "1.5")]),
            ("p>1", "p", vec![(">", "1")]),
            ("p>=1.0.0,<2.0.0", "p", vec![(">=", "1.0.0"), ("<", "2.0.0")]),
            ("Pkg.Sub-Name_1>=2", "Pkg.Sub-Name_1", vec![(">=", "2")]),
            ("p", "p", vec![]),
            ("p==", "p", vec![]),
            ("p=1", "p", vec![]),
            ("p === 2", "p", vec![("===", "2")]),
            ("p<=1,<", "p", vec![("<=", "1")]),
            ("p~=1.2.3", "p", vec![("~=", "1.2.3")]),
            ("_bad>=1", "", vec![]),
            ("-x>=1", "", vec![]),
            (".x>=1", "", vec![]),
            ("1abc>=2", "1abc", vec![(">=", "2")]),
            ("p>=1 extra", "p", vec![(">=", "1 extra")]),
            ("p >= 1.0 , < 2.0", "p", vec![(">=", "1.0"), ("<", "2.0")]),
        ];
        for (req, name, cl) in table {
            let (got_name, got_clauses) = split_requirement(req);
            assert_eq!(got_name, name, "split_requirement({req:?}) name");
            assert_eq!(got_clauses, clauses(&cl), "split_requirement({req:?}) clauses");
        }
    }

    fn facts<'a>(import_name: &'a str) -> EnvReadyFacts<'a> {
        // A spec found OUTSIDE the sandbox: origin does not start with the
        // sandbox root, so the dist-info gate never fires (the real installed
        // package lives in the interpreter's own site-packages).
        EnvReadyFacts {
            import_name,
            package: "",
            find_spec_error: false,
            spec_found: true,
            origin: "/opt/python/dist/x/__init__.py",
            sandbox_root: "/opt/readmd/plugins/site-packages",
            sandbox_metadata_exists: false,
            installed_version: "",
            requirement: "",
        }
    }

    #[test]
    fn environment_ready_import_name_gate() {
        // Empty import_name -> (False, "").
        let (ready, ver) = environment_plugin_ready(&facts(""));
        assert!(!ready);
        assert_eq!(ver, "");
    }

    #[test]
    fn environment_ready_find_spec_gates() {
        assert_eq!(environment_plugin_ready(&facts("x")).0, true); // found, no clauses
        let mut f = facts("x");
        f.spec_found = false;
        assert_eq!(environment_plugin_ready(&f), (false, String::new()));
        let mut f = facts("x");
        f.find_spec_error = true;
        assert_eq!(environment_plugin_ready(&f), (false, String::new()));
    }

    #[test]
    fn environment_ready_sandbox_origin_needs_metadata() {
        let mut f = facts("x");
        f.origin = "/plugins/site-packages/x/__init__.py";
        f.sandbox_root = "/plugins/site-packages";
        f.sandbox_metadata_exists = false;
        // Origin under the sandbox but no dist-info -> False.
        assert_eq!(environment_plugin_ready(&f), (false, String::new()));
        f.sandbox_metadata_exists = true;
        assert_eq!(environment_plugin_ready(&f), (true, String::new()));
    }

    #[test]
    fn environment_ready_version_mismatch_returns_installed_version() {
        let mut f = facts("whisper");
        f.installed_version = "1.0";
        f.requirement = "openai-whisper>=1.1,<2";
        // installed 1.0 fails >=1.1 -> (False, "1.0").
        assert_eq!(environment_plugin_ready(&f), (false, "1.0".to_string()));
        f.installed_version = "1.5";
        assert_eq!(environment_plugin_ready(&f), (true, "1.5".to_string()));
    }

    #[test]
    fn line_emitter_splits_on_newline_only() {
        let mut em = LineEmitter::new();
        let mut seen: Vec<String> = Vec::new();
        // write returns len(text) in code points.
        assert_eq!(em.write("hello ", |l| seen.push(l.to_string())), 6);
        assert!(seen.is_empty()); // no newline yet
        assert_eq!(em.write("world\nnext", |l| seen.push(l.to_string())), 10);
        assert_eq!(seen, vec!["hello world".to_string()]); // split at the single '\n'
        assert_eq!(em.buffered_len(), 4); // "next"
        em.flush(|l| seen.push(l.to_string()));
        assert_eq!(seen, vec!["hello world".to_string(), "next".to_string()]);
        assert_eq!(em.buffered_len(), 0);
    }

    #[test]
    fn line_emitter_preserves_carriage_returns_and_handles_multiple_lines() {
        let mut em = LineEmitter::new();
        let mut seen: Vec<String> = Vec::new();
        em.write("a\r\nb\nc\n", |l| seen.push(l.to_string()));
        // '\r' is not a boundary; only '\n' splits. Trailing empty after last \n.
        assert_eq!(seen, vec!["a\r".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(em.buffered_len(), 0);
        // Nothing left to flush.
        em.flush(|l| seen.push(l.to_string()));
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn line_emitter_flush_on_empty_is_noop_and_flags() {
        let mut em = LineEmitter::new();
        assert!(em.writable());
        assert!(!em.isatty());
        let mut calls = 0;
        em.flush(|_l| calls += 1);
        assert_eq!(calls, 0); // empty buffer -> no callback (Python `if self._buffer`)
    }
}

